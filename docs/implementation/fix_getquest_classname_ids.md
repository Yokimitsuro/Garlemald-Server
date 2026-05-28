# `GetQuest(name)` returned wrong quest ids — SEQ_005 "Now Loading" hang

## Symptom

After the notice-cinematic fix, the Gridania tutorial advanced normally
until the second Yda talk triggered the combat-tutorial transition
(`doContentArea` → `DoZoneChangeContent`). The client then hung forever
at "Now Loading" — the combat cinematic never started. (Same point macOS
softlocked, so it affected both platforms.)

## Root cause

`LuaPlayer::GetQuest(name)` (`map-server/src/lua/userdata.rs`) hardcoded
a class-name → quest-id table that was wrong on all three openers:

```rust
"Man0g0" => 110001,   // wrong
"Man0l0" => 110002,   // wrong
"Man0u0" => 110003,   // wrong
```

The correct ids — the `Id:` headers in
`scripts/lua/quests/man/man0X0.lua`, matching `OpeningDirector.lua`'s
`HasQuest()` gates — are:

```text
man0l0 (Limsa)    = 110001
man0g0 (Gridania) = 110005
man0u0 (Ul'dah)   = 110009
```

`QuestDirectorMan0g001.lua::onEventStarted` does
`player:GetQuest("Man0g0")` then
`callClientFunction(player, "delegateEvent", player, man0g0Quest,
"processTtrBtl001")`. With the broken table that resolved to quest
110001, so the combat-tutorial `RunEventFunction` went out referencing
quest actor `0xA0F1ADB1` (man0l0) instead of `0xA0F1ADB5` (man0g0).

Confirmed on the wire (`packetlogs/map-packets.log`, 2026-05-28):

```text
OUT RunEventFunction owner=0x65300003 (QuestDirectorMan0g001) "noticeEvent"
    delegateEvent(player, 0xA0F1ADB1, "processTtrBtl001")
                          ^^^^^^^^^^ quest 110001, should be 0xA0F1ADB5 (110005)
```

The Gridania player doesn't hold quest 110001, so the client couldn't
resolve the delegated quest, the combat-tutorial cinematic body never
ran, and the content-area warp stayed at "Now Loading".

## Fix

`map-server/src/lua/userdata.rs` — correct the table to
`Man0l0=110001 / Man0g0=110005 / Man0u0=110009`.

## Validation

- `cargo check -p map-server` clean.
- `cargo test -p map-server` — TODO (run locally).
- Live: Gridania tutorial → second Yda talk → expect the combat-tutorial
  cinematic to play (no "Now Loading" hang). Re-capture
  `packetlogs/map-packets.log` and confirm the `processTtrBtl001`
  RunEventFunction now carries quest actor `0xA0F1ADB5`.
