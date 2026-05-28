# Notice cinematic — stop sending the premature `EndEvent` mid-cutscene

## Symptom

The 1.x client crashes during the Gridania opening tutorial on native
**Windows** (`ffxivgame.exe` access violation `c0000005` at
`ffxivgame+0x492550`, reading `[NULL+4]`). On **macOS/Wine** the same
build does NOT crash — it plays the cinematic through and only
softlocks later at the SEQ_005 combat-tutorial transition.

## Root cause (packet-log confirmed)

`map-server`'s `onNotice` hook for the opener quests
(`man0l0` / `man0g0` / `man0u0`) is:

```lua
callClientFunction(player, "delegateEvent", player, quest, "processTtrNomal001withHQ")  -- RunEventFunction (0x0130)
player:EndEvent()                                                                        -- EndEvent (0x0131)
quest:UpdateENPCs()
```

`callClientFunction` yields the coroutine; `apply_quest_on_notice`
auto-resumes it in the same drain pass (the 1.x client never sends a
`0x012E EventUpdate` to mark notice-cutscene completion, so the resume
is needed to run `UpdateENPCs`). The side effect: `player:EndEvent()`
was translated into the outbox and sent **in the same millisecond as
the RunEventFunction**, while the cinematic is still playing.

Captured in `packetlogs/map-packets.log` (Gridania repro 2026-05-28):

```text
07:13:03.874 OUT RunEventFunction  delegateEvent processTtrNomal001withHQ  owner=0x65300002 (OpeningDirector)
07:13:03.874 OUT EndEvent          "noticeEvent"                            owner=0x00000000
```

Two problems with that EndEvent:
1. It fires ~0 ms into the cinematic (pmeteor's reference capture keeps
   `noticeEvent` open and only sends its EndEvent ~2 min later, after
   the combat tutorial).
2. `owner = 0x00000000` — the `EventSession.current_event_owner` had
   already been cleared by the time the resumed command was translated,
   so the fallback produced a zero owner instead of the director.

The client processes that premature, zero-owner EndEvent and tears down
its event-session state mid-cutscene. When the cinematic then marshals
its own outbound `EventUpdate` (opcode 0x012E — the EXE's
`Lua_send6argRpc_via_opcode_0x12e` at `ffxivgame+0x494090`), the event
owner resolves to NULL and the `StackOperator` getter dereferences it
(`mov ecx,[ecx+4]` with `ecx=NULL`). Native Windows faults; Wine
tolerates the low-address read, which is exactly why macOS survives the
cinematic and Windows does not.

WinDbg stack (native Windows):

```text
ffxivgame+0x492550   mov ecx,[ecx+4]   ; ecx = NULL  -> c0000005
ffxivgame+0x494b62   ; 6-arg RPC arg builder
ffxivgame+0x496166   ; WhichStackOperator type dispatch
ffxivgame+0x8cd...   ; Lua engine
```

## Cross-platform packet-log diff (confirms the diagnosis)

`packetlogs/map-packets_mac.log` is a **working** macOS/Wine capture of
the same server build. Comparing it against the Windows
`map-packets.log`:

- The premature `EndEvent "noticeEvent"` with `owner=0x00000000` is
  emitted **identically on both** (Mac L1759, same +0 ms after the
  RunEventFunction; Mac player id is 2 vs Windows 4 — just session
  numbering). So the server output is identical; the divergence is
  purely client robustness.
- The Mac client **tolerates it and continues**: ~64 s into the
  cinematic it sends the outbound `EventUpdate (0x012E)` (Mac L4712,
  `07:45:33`, `trigger=player, serverCodes=0x30400000` + LuaParams).
- That `EventUpdate` is exactly the packet the Windows client faults
  while building (`Lua_send6argRpc_via_opcode_0x12e`) — because the
  premature EndEvent closed the client's event session, so the event
  owner resolves to NULL. Wine reads `[NULL+4]` as garbage and
  proceeds; native Windows faults.

So keeping `noticeEvent` open (this fix) gives the EventUpdate a valid
owner on both platforms — Windows stops faulting, Mac is unaffected
(it already tolerated the bad state).

## Fix (final)

`map-server/src/runtime/quest_apply.rs::apply_quest_on_notice` — **remove
the force-resume of the parked `_WAIT_EVENT` coroutine**. The
RunEventFunction (cinematic kickoff) is already dispatched from the first
drain pass; the hook's trailing `player:EndEvent()` + `quest:UpdateENPCs()`
stay parked behind the `callClientFunction` yield.

The client sends a `0x012E EventUpdate` when the cinematic reaches its
server-sync point. `handle_event_update` → `EventSession::update_event`
→ `dispatch_event_updated_drain` resumes the coroutine and emits the
`EndEvent` + `UpdateENPCs` **then** — after the cinematic, so:
- the client's EventUpdate was built against an OPEN event (valid owner,
  no NULL deref → no Windows crash), and
- the deferred EndEvent closes the event afterward, taking the player out
  of event-mode so WASD movement works again.

### Why the two earlier attempts were wrong

1. **Original (force-resume, emit EndEvent):** fired EndEvent ~0 ms after
   RunEventFunction with owner=0 → premature teardown → Windows crash on
   the later EventUpdate (Wine tolerated → macOS only softlocked at
   SEQ_005).
2. **First fix (force-resume, suppress EndEvent from outbox):** stopped
   the crash, but the force-resume **consumed the coroutine**, so the
   client's later EventUpdate had nothing to resume → the EndEvent never
   fired → the event stayed open → player could not move with WASD
   (user-confirmed 2026-05-28).
3. **Final fix (don't resume at all):** the EndEvent fires naturally on
   the client's EventUpdate — correct timing, no crash, movement restored.

The "client never sends an EventUpdate for the notice cinematic, so we
must force-resume" assumption (the reason the force-resume existed) is
disproven by the working macOS capture `packetlogs/map-packets_mac.log`,
which shows the client's first EventUpdate ~64 s after the kickoff.

## Scope / risk

Applies to every quest's `onNotice` notice cinematic. All relied on the
client sending an EventUpdate to advance; the force-resume was a global
workaround that is no longer needed. Test a normal (non-tutorial) quest
notice cutscene too to confirm none depended on the immediate
server-driven EndEvent (if one does, that quest's player would stay
event-locked until it sends an EventUpdate — re-introduce a *scoped*
resume for that case rather than the global one).

## Validation

- `cargo check -p map-server` clean.
- `cargo test -p map-server` — TODO (run locally).
- Live: fresh Gridania character → intro cinematic → camera tutorial →
  move to corner → journal prompt. Expect: no client crash on Windows;
  the player can open the journal, select the quest, and reach the
  second Yda talk. Re-capture `packetlogs/map-packets.log` and confirm
  the `EndEvent "noticeEvent"` at +0 ms is gone.
