# GM chat commands — self-targeting + in-game feedback (issue #10)

Issue: https://github.com/swstegall/Garlemald-Server/issues/10

## Research source

- `E:\Project Meteor\Data\scripts\commands\gm\warp.lua` — Meteor's GM
  command contract: `onTrigger(player, ...)` receives the **invoking
  session's player** as the implicit target; the optional trailing
  `<targetname>` only overrides it. Errors and confirmations go back to
  the invoker via `player:SendMessage(MESSAGE_TYPE_SYSTEM_ERROR, "[warp] ", ...)`.
- Meteor's `warp.lua` argc dispatch: exactly 3 numeric args = move
  within the current zone (`DoPlayerMoveInZone`); 4+ args = cross-zone
  `DoZoneChange`. A bare `!warp <zone>` is *not* a supported Meteor form
  (argc==1 only accepts the SWITCH/FLIP town keyword) — it answers with
  a visible usage error.
- `MESSAGE_TYPE_SYSTEM_MESSAGE` / `MESSAGE_TYPE_SYSTEM_ERROR` map to the
  already-ported `CHAT_SYSTEM (0x20)` / `CHAT_SYSTEM_ERROR (0x21)` ids in
  `map-server/src/social/chat.rs`.

## State before this change

The issue's suspected root cause (CommandProcessor handle not threaded
into the packet processor, `cmd: None` at the chat call site) was
already fixed in this fork by commit `6e143f4` ("MSQ groundwork:
auto-add opener quest, chat-GM via FIFO, warp packet"):
`server.rs` constructs `PacketProcessor { cmd: Some(cmd), .. }` and
`main.rs:318` passes the `Arc<CommandProcessor>` through. `cmd: None`
only survives in test fixtures. The upstream repo the issue was filed
against predates that commit.

What was still missing vs the issue's acceptance criteria:

1. `!warp 166`-style invocations did nothing *visible* — every command
   required the full arg list **including the invoker's own character
   name**, and both usage errors and results went only to the server
   log, never back to the player.
2. No command response ever reached the client, so failures were
   indistinguishable from the original "swallowed" bug.

## Behavior implemented

- `CommandProcessor::run_as(line, invoker)` — new entry point carrying
  the chat sender's character name. `Args::rest_joined` falls back to
  the invoker when the trailing `<name>` arg is omitted, so every
  name-taking command (`revive`, `givegil`, `warp`, `setseq`, …)
  self-targets from chat, exactly like Meteor's implicit `player`.
  The stdin/FIFO console path (`run`) passes `invoker = None` and keeps
  `<name>` mandatory — unchanged contract.
- `handle_warp` now also accepts Meteor's 3-arg same-zone form
  `warp <x> <y> <z> [name]` (re-uses the existing same-zone
  `SetActorPosition` emission; cross-zone still requires re-log, as
  before). Note: the existing port emits spawn_type=2 (warp-by-GM) for
  the same-zone move where Meteor uses 0x00; left as-is.
- The chat `!` branch in `processor.rs::handle_chat_message` echoes the
  command response to the sender via
  `SocialEvent::ChatSystemToPlayer` → `build_send_message`:
  `CHAT_SYSTEM` for normal responses, `CHAT_SYSTEM_ERROR` for hard
  errors (and for the "CommandProcessor is not wired" guard, which is
  now also player-visible).

## Server files changed

- `map-server/src/command_processor.rs` — `run_as`, `Args.invoker`
  fallback, `handle_warp` 3-arg form, help text, 6 new tests.
- `map-server/src/processor.rs` — chat `!` branch: invoker threading +
  ChatSystemToPlayer feedback.

## Round 2 — live-client test found two wire-format bugs

First in-game test (2026-06-10, live 1.23b client): commands executed
server-side (`command result response=gave 1000 gil to Prueba Test`)
but nothing rendered in the client and `!warp x y z` didn't move the
actor. Packet logs proved both frames reached the client (world-server
forwarded them verbatim), so the client was *dropping* them:

1. **SendMessage used a wrong opcode + ad-hoc layout.** The port sent
   opcode 0x00CA with `{u64 0, u32 0, u8 type, pad, sender[0x20],
   text, nul}`. Meteor's `SendMessagePacket.cs` is opcode **0x0003**
   (same value both directions — client chat already arrives at
   0x0003), fixed 0x228 body: `sender[0x20]` + `u32 type` + `text[0x200]`.
   Fixed `build_send_message` + `OP_SEND_MESSAGE` accordingly; added a
   layout test (`send_message_matches_meteor_layout`).
2. **Same-zone warp sent the wrong move sequence.** Meteor
   `WorldManager.DoPlayerMoveInZone` = `_0xE2Packet(actor, 0x10)` +
   `CreateSpawnTeleportPacket(spawnType)` where the SetActorPosition
   embeds actor id **0xFFFFFFFF** ("self"), not the player's id, and
   `warp.lua` uses spawn type 0x00. The port sent a single
   SetActorPosition with the player's id + spawn type 2. Fixed
   `handle_warp` to emit the 0xE2(0x10) + SetActorPosition(-1, …, 0)
   pair.

`build_send_message_public` (0x0003, "login greeting") still writes
`u32 type` *before* the sender — opposite of Meteor's layout — and is
likely invisible client-side for the same reason. Not touched here
(separate work unit).

## Unknowns left

- `is_gm` gating: the login snapshot hardcodes `is_gm: false`
  (`processor.rs`, `build_player_snapshot_for_login`) and the `!` path
  does **not** check it — any connected player can run GM commands.
  Acceptable for a dev/test server (and required by the issue's
  acceptance criteria that the testing account is not rejected), but a
  real deployment needs a `characters`/account GM flag checked in the
  chat branch before dispatch.

## Validation performed

- `cargo check -p map-server --all-targets` — clean.
- Unit tests added (run with `cargo test -p map-server command_processor`):
  - `rest_joined_falls_back_to_invoker`
  - `chat_invoker_is_implicit_target` (`givegil 250` self-targets)
  - `explicit_name_overrides_chat_invoker`
  - `warp_same_zone_form_targets_invoker`
  - `warp_same_zone_form_on_console_requires_name`
  - `warp_offline_target_is_reported`

## Next test with client

1. Boot the stack, log in, zone into the world.
2. `!givegil 1000` in chat → expect a `CHAT_SYSTEM` line
   "gave 1000 gil to <you> (total now …)" in the chat log.
3. `!warp 10 0 20` → expect "warped <you> to zone <current> at (10.00, 0.00, 20.00)"
   and the actor visibly moves (same-zone SetActorPosition).
4. `!warp 166` → expect a *visible* usage error line (red,
   CHAT_SYSTEM_ERROR formatting not yet differentiated — both kinds
   render through the same SendMessage packet).
5. Failure signal: no chat line appears → check `gm command from chat`
   / `command result` pairs in the map-server log to localise
   (dispatch vs SendMessage emission).
