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

## Round 2 — player can walk out of the Merchant Strip (SEQ_000)

Live retest: SEQ_000 progressed (QFLAG fix confirmed) but the player
could walk out of the opening area. Packet-log forensics (parsing the
position stream against the trigger coordinates): the player crossed
the `OPENING_STOPER_ULDAH` (1090373) "exit"/"caution" push circles at
0.48 yalms **after** the intro cinematic's EndEvent, and the client
fired no push EventStart at all. Two real gaps vs quest_system:

1. **Missing base script** — quest_system ships
   `base/chara/npc/object/OpeningStoperW0B1.lua` (the handler that
   bounces the player via `DoPlayerMoveInZone` on "exit" and prints
   message 34109 on "caution"); only the Gridania F0B1 had been
   ported. Ported it (dispatcher-convention signature, like F0B1).
2. **ENPC status semantics diverged from pmeteor.**
   `QuestState.AddENpc` sends quest SetEventStatus overrides only for
   NEW/CHANGED registrations and is silent for unchanged ones;
   dropped entries are reset to the actor's own per-condition
   defaults. Our `apply_quest_update_enpcs` force-rebroadcast was
   re-sending the overrides for every active ENPC on every run —
   man0u0 registers the stopper with `isPushEnabled=false`, so each
   UpdateENPCs re-disabled the stopper's own "exit"/"caution" circles
   that the spawn bundle's defaults had armed. The rebroadcast now
   re-emits only the head-marker graphic (which is what the man0g0
   cinematic marker-loss fix actually needed).

Note: the "exit" bounce flows through `DoPlayerMoveInZone` →
`WarpToPosition`, whose delivery is fixed by upstream PR #35
(proxy-dropped warp packets). Testing needs a build with #35 merged.

## Round 3 — WarpToPosition drain arm + stopper SetENpc arg slip

Fourth/fifth retests: the door push fired (dialogue) but the bounce
never moved the player, and the stopper street exits stayed silent.
One missing piece + one script slip:

- `LC::WarpToPosition` was only handled by the login pipeline; the
  quest/NPC drain dropped it silently → every `DoPlayerMoveInZone`
  bounce evaporated after its dialogue. Added the arm +
  `apply_warp_to_position_runtime` (mirror of the login applier).
- man0u0 registered the stopper as `SetENpc(QFLAG_NONE, false, false,
  true)` — an upstream arg slip (push=false / emote=true) that made
  the quest layer disable the stopper's own circles (byte-confirmed:
  spawn arms enable=01 at 14:55:04, quest kills enable=00 at
  14:57:31). Aligned with the sibling pattern (man0g0 BLOCKER1,
  man0u0's own SEQ_010 BLOCKER): `(QFLAG_NONE, false, true)`.

## Round 4 — north/south/east exits: MapObj doors had no body

With the west street bouncing, the player could still leave N/S/E.
Meteor seals those with **closed-door MapObj actors** (class 5900004,
spawn rows 628/629/938 — all present in our seed and spawning) bound
to background layout geometry via the `server_eventnpc_mapobj` table
(id → layoutId, instanceId; door1→421/2825, door2→421/2829,
door3→421/4040). `Npc.GetSpawnPackets` sends
`SetActorBGProperties(instanceId, layoutId)` INSTEAD of an appearance
packet for these — the layout object carries the mesh + collision.
Our pipeline lacked the entire chain, so the doors spawned as
intangible ghosts. Ported: seed `052_server_eventnpc_mapobj.sql`
(79 rows), `SpawnLocation.mapobj_layout_id/instance_id` (loader LEFT
JOIN, Meteor `WorldManager.cs:341`), `BaseActor` carry-through, and
the spawn-bundle branch (BG-properties vs appearance, wire order
instanceId-first per `SetActorBGPropertiesPacket.cs`).

## Round 5 — SEQ_005 battle: port the working Gridania/Limsa twins

Live test of the combat tutorial: F armed (PR #35's work, now in
develop), weapons drew, the `playerActive` signal resumed the director
— and the tutorial froze. Root cause per upstream's own #25 Limsa port
notes: our `QuestDirectorMan0u001.lua` + `SimpleContent30079.lua` were
still the raw upstream import (synthetic `SpawnActor` props, fixed
`wait(4)/wait(6)` chains, no kill gate) — "the exact shape that
stalled Gridania before the #28 rewrites".

Ported the same twins develop now ships for Gridania (#28) and Limsa
(#25):

- `QuestDirectorMan0u001.lua` → Man0l001 shape: DoW branch with the
  milestone-gated tooltip chain (`playerAttack` → `tpOver1000` →
  `weaponskillUsed` → `battleComplete` kill gate, Rust-side
  `fire_content_signal` sources), DoM branch for THM starters, then
  attention 51073/3 → ChangeState(0) → kickEventContinue →
  `processEvent020` arrival → StartSequence(10) → ContentFinished →
  DoZoneChange(175, PrivateAreaMasterPast, 3) into the SEQ_010 strip.
  No `onCreateContentArea` — spawning moved to the content script.
- `SimpleContent30079.lua` → SimpleContent30010/30002 shape: real
  BattleNpcs via `SpawnBattleNpcById` (seed rows 13-15), Thancred +
  goobbue `ChangeState(2)`, Niellefresne passive (the Papalymo role),
  party-added allies for HUD HP bars, `MinimumHpLock` no-die
  guarantees, the engagement-latch `onUpdate` battle driver (latch
  re-armed in `onCreate` — the VM is process-cached), and the
  upstream `1090385` arena stopper prop kept.
- Seed `055_uldah_seq005_tutorial.sql`: pools/groups 8-10 + spawn rows
  13-15 — Escaped Goobbue (2203301, genus 6, 500 HP per the 053 pacing
  rationale scaled to a single mob) and Thancred/Niellefresne
  (allegiance-1 allies → excluded from the battleComplete live count,
  exactly like Yda/Papalymo and Sthalmann/Y'shtola).

Branch rebased onto develop v0.1.5 (PR #35 + the Limsa #25 routing
fixes — resumed-burst login-applier routing, EventUpdate LuaParams
threading — all of which this flow rides).

## Round 6 — instant crash at the SEQ_010 warp + dead-path HUD fixes

Retest of the Round-5 HUD work (f923b28): ally names went blue (the
zone-in roster fix landed) but the party list stayed empty, the goobbue
gauge stayed empty, and the client now closed INSTANTLY at the
post-battle warp (previously ~4 s after arriving in zone 175). Log
forensics (`map.log`, the session ran without packet capture):

1. **Enemy gauge — the claim sat on a dead path.** The whole tutorial
   fight engages via `quest_apply::apply_actor_engage` (the content
   driver's `ActorEngage` LuaCommands + the player's 0x00CD) — zero
   `BattleEvent::Engage` dispatches in the entire session, so the
   hateType=3 + 0x0187 emission added in f923b28 never fired. Extracted
   `dispatcher::emit_hostile_claim_flip` and called it from BOTH engage
   paths. Also fixed the claim's `group_type`: 30012 was the Ifrit
   capture's 64B clone-group type; the tutorial trio registers
   `SimpleContentGroup24B` = 30006, and the claim must carry the type
   the client stored for `monster_group_id` (shared constant
   `GROUP_TYPE_SIMPLE_CONTENT_24B` now used by trio + claim).

2. **Instant crash — party shrink against deleted actors.** New fatal
   burst at 06:50:36: EndEvent → RemoveActor ×3 → DeleteAllActors →
   zone-in bundle whose solo party trio shrinks the roster 3→1. This
   session was the FIRST in which the 3-member tutorial party survived
   to the teardown (the f923b28 roster fix), so the client's group
   member-diff ran against ally actors it had already destroyed —
   uncharted territory for this client (pmeteor never party-adds NPCs)
   and consistent with its known hard-crash on group-packet edge cases
   (the "empty X08" note). `apply_content_finished` now emits the solo
   trio BEFORE the despawns, while the allies still exist client-side,
   making the zone-in trio a 1→1 no-op.

3. **Ghost QuestDirector followed the player to the city.** The
   zone-175 bundle logged `login director spawn packets appended …
   QuestDirectorMan0u001` — `CreateDirector` parks the content director
   in `session.login_director` and ContentFinished never cleared it, so
   every later zone-in re-spawned a director whose content group is
   gone. pmeteor deletes the director with the content area
   (`PrivateAreaContent.CheckDestroy`). Cleared at teardown.

Party-list rows for the allies remain unconfirmed — the 0x0187 claim
(now actually firing) is the best remaining candidate, since the
decompiled `getPlayerParty()` chain is what the client dereferences for
content-party state. Next retest MUST run with packet capture
(`scripts/run-all.sh` exports `GARLEMALD_PACKET_LOG_DIR` by default;
the 06:46 session ran without it).

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
