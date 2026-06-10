# Man0u0 (Ul'dah opener) SEQ_000 softlock — undefined QFLAG_NONE (issue #26)

Issue: https://github.com/swstegall/Garlemald-Server/issues/26

## Research source

- `E:\Project Meteor quest_system\Data\scripts\quest.lua` — defines
  `QFLAG_OFF/OFF_HIDE/TALK/PUSH/REWARD/MAPONLY` and **no `QFLAG_NONE`**.
- `E:\Project Meteor quest_system\Data\scripts` — **64 usages** of
  `QFLAG_NONE` across 15 quest scripts (man0u0 ×9, man0l0 ×5, etc1l8,
  etc3g0, etc3l0, etc3u0, etc5l3, man200, man206, man2l0, pgl200, …),
  zero definitions. The latent bug ships in upstream Meteor too.

## Root cause (found by test, not by the issue's hypotheses)

`man0u0.lua` gates its markers with the idiom

```lua
local asciliaFlag = data:GetFlag(FLAG_SEQ000_MINITUT1) and QFLAG_NONE or QFLAG_TALK;
```

With `QFLAG_NONE` undefined (nil), Lua's `cond and nil or X` trap makes
the expression yield `X` for **both** branches — so a completed tutorial
NPC keeps `QFLAG_TALK` forever. That is exactly the issue's observed
"multiple persistent head markers" symptom, and it makes the tutorial
look stuck.

The issue's runtime hypotheses did **not** reproduce on current
`develop`:

- H1 (SetFlag not persisting): wrong — the integration test proves all
  four `FLAG_SEQ000_MINITUT*` bits persist through the talk → yield →
  resume → drain pipeline (the #28 runtime work fixed that layer).
- H2/H3 (UpdateENPCs / ENPC diff): wrong — with the constant defined,
  markers clear and re-derive correctly through
  `apply_quest_update_enpcs`'s stale-diff + rebroadcast.
- The exit gate (`data:GetFlags() == 0xF and QFLAG_PUSH or QFLAG_NONE`)
  was nil-safe by accident (`QFLAG_PUSH`=3 is truthy; the false branch's
  nil coerces to 0 in the SetENpc binding), so the door arms once the
  flags land.
- H4 (`getJournalMapMarkerList` never invoked) is real but separate —
  untouched here.

## Fix

One line: define `QFLAG_NONE = 0` in `scripts/lua/quest.lua` (0 is
truthy in Lua, and equals `QFLAG_OFF` on the wire, which is what every
usage means). This fixes the marker gating in all 15 ported scripts at
once — including `man0l0` (Limsa opener, issue #25's quest).

Audited all 66 usages in `scripts/lua`: every one is in value position
(`x and QFLAG_NONE or y`, `flag = QFLAG_NONE`, argument lists); none
test `QFLAG_NONE` for truthiness, so nil→0 cannot change control flow
anywhere else.

## Validation performed

- New integration test
  `runtime::integration_tests::man0u0_seq000_tutorial_flags_and_exit_gate`
  drives the REAL `scripts/lua/quests/man/man0u0.lua` through the live
  pipeline (`apply_quest_start_sequence` → `onStateChange`,
  `fire_quest_on_talk_via_command` ×4 with coroutine yield/resume,
  `fire_quest_on_push_via_command` on the exit door) and asserts:
  - start: only Ascilia marked TALK; Farmhand/Mistress/exit suppressed,
  - talk #1: MINITUT0 set, Farmhand+Mistress markers appear,
  - talk #2: MINITUT1 set, Ascilia's marker clears,
  - Farmhand/Mistress talks: MINITUT2/3 set, markers clear,
  - flags == 0xF arms the exit (QFLAG_PUSH), push → SEQ_005.
  Red before the fix (Ascilia stuck at QFLAG_TALK), green after.

## Next test with client

1. Boot, create an Ul'dah character (Man0u0 active, SEQ_000).
2. Only Ascilia shows a marker. Talk her twice (push tutorial + talk
   tutorial) — her marker clears, Farmhand + Mistress light up.
3. Talk both — markers clear as each mini-tutorial completes.
4. The exit gate becomes pushable; pushing it should fire
   `doExitTrigger` → SEQ_005 (combat tutorial content warp — that flow
   has its own known issues, see #28/#35).
5. Limsa (man0l0) should show the same marker-clearing improvement
   (issue #25 retest).
