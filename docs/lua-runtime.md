# The Lua content runtime

Garlemald's *content* — quests, directors, NPC behaviour, content areas, shops,
guildleves, commands — is not written in Rust. It is the **~1,142 Lua content
scripts** loaded at boot from `scripts/lua/` (which also holds the shared helper
scripts they depend on), ported from upstream Project Meteor, executed by a Lua engine
that lives in `map-server`. This document explains **where the runtime lives**,
**how it executes a script**, and **how a client action becomes a Lua hook call
and then a wire packet**.

Read [`architecture.md`](architecture.md) first if you don't yet know what
`map-server` is.

> **mlua / Lua version.** The engine embeds Lua via
> [`mlua`](https://github.com/mlua-rs/mlua) **0.11 with the `lua54` feature**
> (vendored Lua **5.4**, `send`-safe) — see `Cargo.toml`. The upstream scripts
> were written for an older Lua, so the engine ships a `bit32` shim
> (`band`/`bor`/`lshift`/`rshift`) for scripts that expect it. (Older notes that
> say "Lua 5.1" predate the move to 5.4.)

---

## Where the runtime lives

```
map-server/src/lua/
├── mod.rs          # LuaEngine: VM cache, hook callers, the tick(), fire_signal()/fire_player_event()
├── scheduler.rs    # CoroutineScheduler: parks/resumes yielded coroutines
├── command.rs      # the LuaCommand enum + CommandQueue (the "outbox")
├── userdata.rs     # LuaPlayer / LuaActor / LuaQuestHandle / ... bindings
├── globals.rs      # global Lua functions: GetWorldManager(), GetItemGamedata(), bit32, ...
└── paths.rs        # script-path resolution (quest/director/content/npc/zone)

map-server/src/runtime/
├── quest_apply.rs  # apply_runtime_lua_commands(): drains LuaCommands → wire packets;
│                   #   also fire_quest_hook / fire_quest_on_talk_via_command / ...
└── dispatcher.rs   # event-bridge dispatch helpers

scripts/lua/        # the content itself (see "The script tree")
```

The split is the key idea: **`lua/` runs scripts and collects their intended
side effects; `runtime/` applies those side effects to the live game and emits
packets.** Scripts never touch the network or the database directly.

---

## The script tree (`scripts/lua/`)

Path resolution mirrors the upstream C# `FILEPATH_*` constants and is centralised
in `paths.rs`:

| Kind            | Path pattern                                                      | Resolver anchor                |
|-----------------|------------------------------------------------------------------|--------------------------------|
| Quest           | `quests/<3-char prefix>/<name>.lua`                              | `paths.rs` quest resolver      |
| Director        | `directors/<name>.lua`                                           | `paths.rs` director resolver   |
| Content area    | `content/<name>.lua`                                             | `paths.rs` content resolver    |
| NPC (unique)    | `unique/<zone>/<class>/<id>.lua`, falling back to `base/<class>.lua` | `paths.rs` NPC resolver    |
| Zone            | `unique/<zone>/zone.lua` (or `.../PopulaceStandard/zone.lua`)     | `paths.rs` zone resolver       |

Notable subtrees and files:

- `quests/{man,dft,etc,…}/` — one script per quest, bucketed by the quest id's
  3-char prefix.
- `directors/` — director scripts (quest directors, guildleve directors, …) that
  drive cutscenes and instanced content via a `main` coroutine and
  `onEventStarted`.
- `content/` — content-area `onCreate` / `onUpdate` scripts.
- `commands/` — battle and GM commands (`gm/*.lua`).
- `unique/<zone>/…` and `base/<class>/…` — per-NPC overrides and the base-class
  scripts they fall back to.
- `effects/` — status-effect scripts (`onApply` / `onTick` / `onRemove`; see the
  [gaps](#known-gaps) note).
- Root helpers: `global.lua` (constants + `wait` / `waitForSignal` /
  `callClientFunction` / `attentionMessage` etc.), plus `player.lua`,
  `ally.lua`, `battlenpc.lua`, …

`package.path` is set so `require("global")` resolves against the script root
(`mod.rs`).

---

## How a script executes

### One VM per script, one outbox per call

`LuaEngine::load_script(path)` (`mod.rs`) returns an `(Arc<Lua>, Arc<Mutex<CommandQueue>>)`
pair:

- The **`Arc<Lua>`** is a per-script-path VM, **cached** in a `vm_cache:
  Mutex<HashMap<String, Arc<Lua>>>` and pre-loaded with the global bindings.
  Reusing the VM keeps script-global state across calls the way upstream expects.
- The **`CommandQueue`** is **fresh per call** — it is the "outbox" the script's
  side effects accumulate into, and it is drained after the call returns.

### Bindings: userdata + globals

Scripts manipulate the game through **userdata objects** (`userdata.rs`) and
**global functions** (`globals.rs`). A userdata method does **not** mutate the
world inline — it pushes a `LuaCommand` onto the call's `CommandQueue`. The main
userdata types:

| Userdata             | Represents                         | Examples of methods (push `LuaCommand`s)                                  |
|----------------------|------------------------------------|--------------------------------------------------------------------------|
| `LuaPlayer`          | a player (read-only snapshot)      | `SendMessage`, `SetPos`, `AddItem`, `AddQuest`, `KickEvent`, `RunEventFunction`, `DoZoneChange`, `Warp` |
| `LuaActor`           | any actor                          | `ChangeState`, `PlayAnimation`, `MoveTo`, `SetMod`, `Engage`, `hateContainer` |
| `LuaNpc`             | an NPC (extends actor)             | `GetActorClassId`, `SetQuestGraphic`                                      |
| `LuaQuestHandle`     | the player's row for *this* quest  | `SetQuestFlag`, `StartSequence`, `SetEnpc`, `UpdateEnpcs`, counters       |
| `LuaDirectorHandle`  | a director instance                | `StartDirector`, `EndGuildleve`, `AddMember`, `UpdateUiState`             |
| `LuaContentArea`     | an instanced content area          | `GetPlayers`, `GetAllies`, `GetMonsters`, `SpawnActor`                    |
| `LuaWorldManager`    | the zone/world facade              | `WarpToPosition`, `WarpToPublicArea`, `DoZoneChangeContent`, `SpawnBattleNpcById`, `CreateDirector` |

Global functions (`globals.rs`) are mostly **read-only catalog queries**:
`GetWorldManager()`, `GetItemGamedata(id)`, `GetGuildleveGamedata(id)`,
`GetStaticActor(name)`, the Grand-Company helpers, the `action.TryStatus(...)`
combat entry point, and the `bit32` shim. `print(...)` is routed to
`tracing::debug` (not stdout).

### The hook call → drain cycle

A typical engine entry point (e.g. `mod.rs::call_quest_hook`):

1. Build the userdata arguments for the hook (e.g. a `LuaPlayer` snapshot and a
   `LuaQuestHandle`).
2. Call the named Lua function **inside a coroutine** (`coroutine.resume`) so the
   script may `wait(...)` / `callClientFunction(...)` and yield.
3. If the coroutine **yields**, hand it to the scheduler to park (see below). If
   it **returns**, the call is done.
4. **Drain** the `CommandQueue` and hand the resulting `Vec<LuaCommand>` to the
   apply pipeline.

---

## The coroutine scheduler

Long-running scripts (a director's `main`, a multi-step cutscene) don't block a
thread — they **yield** and the scheduler parks them until a condition is met.
The yield directives (defined in `global.lua`, classified in `scheduler.rs`):

| Lua call                              | Yields                          | Parked on…             | Resumed when…                                  |
|---------------------------------------|---------------------------------|------------------------|------------------------------------------------|
| `wait(seconds)`                       | `("_WAIT_TIME", seconds)`       | a millisecond deadline | the game tick passes that deadline             |
| `waitForSignal(name)`                 | `("_WAIT_SIGNAL", name)`        | a named signal         | something calls `sendSignal(name)` → `fire_signal` |
| `callClientFunction(player, …)` / `kickEventContinue(player, …)` | `("_WAIT_EVENT", player)` | the player's next event | the player's next `EventUpdate` arrives (`fire_player_event`) |

Internally (`scheduler.rs`):

```rust
sleeping_on_time:         Vec<(u64 /*deadline*/, ParkedCoroutine)>
sleeping_on_signal:       HashMap<String, Vec<ParkedCoroutine>>
sleeping_on_player_event: HashMap<u32 /*player_id*/, ParkedCoroutine>
```

- **Time**: drained each game tick; every coroutine whose deadline has passed is
  resumed.
- **Signal**: `fire_signal(name)` resumes **all** coroutines parked on that name.
- **Player event**: an `EventUpdate` packet resumes the **one** coroutine parked
  for that player (last-write-wins — a newer event replaces an older parked
  coroutine).

A resumed coroutine that yields again is simply **re-parked** under its new
directive; when it finally returns, its queued commands are drained and applied.

---

## The `LuaCommand` outbox and the apply pipeline

`command.rs` defines the **`LuaCommand` enum** — the closed set of side effects a
script can request (quest mutations, player/inventory/job changes, actor and
cinematic ops, director/content lifecycle, event RPCs). Representative variants:

- **Quest:** `QuestSetFlag`, `QuestStartSequence`, `QuestSetEnpc`,
  `QuestUpdateEnpcs`, `AddQuest`, `CompleteQuest`, counters.
- **Player/inventory:** `SendMessage`, `SetPos`, `AddItem` / `RemoveItem`,
  `AddExp` / `AddGil`, `SetCurrentJob`, `Die` / `Revive`, Grand-Company ops,
  retainer/chocobo ops.
- **Actor/cinematic:** `PlayAnimation`, `ChangeState`, `SpawnActor`,
  `SpawnBattleNpcById`, `MoveActorToPosition`, `ActorEngage`,
  `HateContainerAddBaseHate`.
- **Director/content:** `CreateDirector`, `StartDirectorMain`, `EndGuildleve`,
  `DirectorAddMember`, `PartyAddMember`, `DoZoneChangeContent`.
- **Event/script:** `RunEventFunction` (cutscene RPC), `KickEvent` (dialog
  trigger), `EndEvent`, `ChangeMusic`, `TryStatus`.

A `CommandQueue` (`command.rs`) is just a `Vec<LuaCommand>` behind a mutex with
`push` / `drain` / `len`. The lifecycle:

```
LuaEngine::load_script ──► Arc<Lua> + fresh CommandQueue
        │
        ├─ globals/userdata closures capture the queue Arc
        │
script runs ─► player:SetQuestFlag(5)  ──► CommandQueue::push(QuestSetFlag{…})
        │
script returns / yields-then-finishes
        │
        └─ CommandQueue::drain() ─► Vec<LuaCommand>
                    │
                    ▼
   runtime/quest_apply.rs::apply_runtime_lua_commands(commands, …)
                    │
                    ├─ mutate actor/quest/session state
                    └─ build + enqueue wire packets (via runtime/dispatcher.rs)
```

`apply_runtime_lua_commands` (`runtime/quest_apply.rs`) is the single funnel that
turns the drained commands into state mutations and outbound packets. Some
commands re-enter the engine — e.g. `QuestStartSequence` flips the quest "dirty"
and triggers an `onStateChange` hook, whose own commands are then drained and
applied in turn.

---

## The hook surface

### Quest hooks

Fired through `runtime/quest_apply.rs::fire_quest_hook` and friends:

| Hook            | Fires when…                                   | Entry point                                   |
|-----------------|-----------------------------------------------|-----------------------------------------------|
| `onStart`       | the player accepts the quest                  | `apply_add_quest`                             |
| `onFinish`      | the player completes the quest                | `apply_complete_quest`                        |
| `onStateChange` | a `quest:StartSequence(seq)` flips the state  | after `apply_quest_start_sequence`            |
| `onTalk`        | the player talks to a quest NPC               | `fire_quest_on_talk_via_command`              |
| `onPush`        | the player "pushes" (proximity-triggers) an NPC | `fire_quest_on_push_via_command`            |

Other hook names appear in scripts/comments but are **not fully wired** yet — see
[Known gaps](#known-gaps).

### Director & content hooks

- **Directors**: a `main` coroutine (resumed via `StartDirectorMain`) plus
  `onEventStarted`, driven by `mod.rs::spawn_director_main` /
  `call_director_on_event_started`.
- **Content areas**: `onCreate` / `onUpdate`, driven by `mod.rs::call_content_hook`
  and the engine `tick`.

---

## End-to-end: a client action becomes a packet

This is the path to trace when you're wiring or debugging content. Example: a
player talking to a quest NPC.

```
1. Client sends a TalkNpc game message
        │
2. map-server/src/processor.rs            (opcode dispatch)
        │   handle_talk_npc(...)
        ▼
3. map-server/src/command_processor.rs    (event → quest bridge)
        ▼
4. runtime/quest_apply.rs::fire_quest_on_talk_via_command(player, quest_id, npc)
        │   resolves the quest script path, builds LuaPlayer + LuaQuestHandle + LuaNpc
        ▼
5. lua/mod.rs::call_quest_hook("onTalk", snapshot, handle, [Npc(spec)])
        │   runs the Lua, which pushes LuaCommands (e.g. SetQuestFlag, StartSequence)
        ▼
6. CommandQueue::drain()  ─► Vec<LuaCommand>
        ▼
7. runtime/quest_apply.rs::apply_runtime_lua_commands(...)
        │   mutates state; StartSequence re-fires onStateChange (back to step 5)
        ▼
8. runtime/dispatcher.rs builds wire packets ─► client
```

Movement, push, and event-update flows follow the same shape: an inbound opcode
in `processor.rs` → a `command_processor.rs` / `quest_apply.rs` bridge → a hook
call → drained `LuaCommand`s → applied → packets. `EventUpdate` packets are
special: they resume a **parked** coroutine (`mod.rs::fire_player_event` →
`scheduler.take_event`) rather than starting a fresh hook.

---

## Known gaps

The content path is real but incomplete. When you pick up content work, check
these first — they are the live edges (and several have tracking issues):

- **`onTalk` / `onPush` for non-quest (populace) NPCs** — the wired path is the
  *quest* one (`fire_quest_on_talk_via_command`); plain populace dialogue needs
  its own dispatcher into the NPC/zone script. (See issue **#26**.)
- **`ActivateCommand`** — selecting a combat action does not yet route to a Lua
  hook. (See issue **#28**.)
- **`onEmote` / `onKillBNpc`** — referenced in scripts/comments but no dispatcher
  is wired.
- **`onNotice`** — implemented (`apply_quest_on_notice`) but currently reached
  only from the post-cinematic director-resume path, not from general event
  traffic.
- **Cross-zone warps** `WarpToPublicArea` / `WarpToPrivateArea` — these commands
  currently log and skip; the loading-screen flow isn't implemented.
- **Status-effect script hooks** (`effects/*.lua` `onApply`/`onTick`/`onRemove`)
  — the Rust apply path currently applies effects inline rather than calling the
  scripts.

Before adding a binding, check whether the engine truly lacks it: the authority
for "is this a real engine binding" is the decompilation cross-reference, not the
upstream C# server's server-side conveniences. When in doubt, grep `lua/` and ask
in the project Discord.

---

## Where to read next

- [`architecture.md`](architecture.md) — how `map-server` fits into the four-binary topology.
- [`dev-environment.md`](dev-environment.md) — run the stack, then watch the Lua
  path with `RUST_LOG=map_server=debug` and packet capture.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — pick up a content issue and open a PR.
