# Content warp "Now Loading" hang — present the private-area master so the client re-bootstraps the zone-in

> **SUPERSEDED (2026-06-09, issue #28 RCA).** The root-cause theory below
> (client keys the re-bootstrap off the SetMap `zone_actor_id`) was
> refuted by the decompiled client: `[MapElem+0x98]` (where the zone
> actor id lands) is written unconditionally and never compared — it
> cannot trigger a reload. The actual root cause of the same-zone hang:
> the `DeleteAllActors` + `0x00E2` pair was sent with `target_id == 0`
> and silently dropped by the world-server proxy, so the client's
> force-reload latch (`[MapElem+0xbc]`, set by the 0x00E2 receiver) never
> armed and a same-region SetMap took the no-op arm. See
> `captures/issue28-rca/06-synthesis.md` (workspace root) and the
> comments in `apply_do_zone_change_content` (processor.rs) +
> `send_zone_in_bundle` (world_manager.rs). Kept for history.

## Symptom

After the `GetQuest` quest-id fix, the Gridania tutorial advances until
the second Yda talk triggers the combat-tutorial transition
(`man0g0.lua::doContentArea` → `CreateContentArea` + `KickEvent` +
`DoZoneChangeContent`). The client then hangs forever on **"Now
Loading"**: the scene partially loads (the content NPCs' nameplates
appear, but below ground), and the combat cinematic never plays. The
client never echoes `RX 0x0007` (zone-in-complete) after the warp
(captured: 60 echoes after login, 0 after the content warp).

## Root cause (packet-log + code confirmed)

The cross-zone warp (`apply_do_zone_change`) and the content warp
(`apply_do_zone_change_content`) are nearly identical — both do
`DeleteAllActors` → `0x00E2` loading marker → `send_zone_in_bundle`.
The ONE meaningful difference:

```text
apply_do_zone_change       (cross-zone): c.base.zone_id = zone_id        // DIFFERENT zone
apply_do_zone_change_content (content):  c.base.zone_id = parent_zone_id // SAME zone (166)
```

`send_zone_in_bundle` keys the area master + `SetMap` (0x0005) off the
session's zone. It emits:

- `SetMap(region_id, zone_actor_id)` — the client's "which area am I
  in?" signal.
- the `_areaMaster@…` actor spawn with `zone_actor_id` + the zone's
  class path.

For a **cross-zone** change `zone_actor_id` differs from the one the
client already has → the client treats it as a new area, re-runs its
`AreaBase` zone-in bootstrap (per
`E:\meteor-reborn-research\docs\re\lua\finding_areabaseclass_zone_bootstrap_sequence.md`:
"the client creates the AreaBase actor for the zone, local, from zone
id"), and on scene-ready echoes `RX 0x0007`.

For the **content** warp the player stays in zone 166, so
`send_zone_in_bundle` re-emits zone 166's area master with the SAME
`zone_actor_id` (0xA6). The 1.x client compares it against the area it
already has, sees no change, never re-bootstraps, never reaches
scene-ready, and never echoes `RX 0x0007` — the `0x00E2` loading
overlay then hangs forever. The content NPCs spawn at the private-area
coords inside zone 166's still-loaded geometry, which is why their
nameplates appear "below ground".

In 1.x a `SimpleContent` private area reuses the parent zone's map
geometry but is a distinct **area instance** with its own
`PrivateAreaMaster` (a separate `AreaBase`), so the correct fix is to
change the area-master identity (not the map geometry / `region_id`).

## Fix

Present the private area's master on the content warp:

1. `map-server/src/data.rs` — add `content_area_actor_id` to
   `ActiveContentScript`.
2. `map-server/src/lua/userdata.rs` — `CreateContentArea` previously
   derived the content-area actor id as
   `encode_director_actor_id(zone, 0x80000 | 1)`. The encoded local-id
   field is 19 bits (`& 0x7FFFF`), so `0x80000 | 1` collapsed to the
   SAME wire id as the director's local-id 1 (both `0x65300003`). The
   private-area master and the content director are separate actors on
   the same client, so they must differ — changed to local-id `0x40000`
   (a high in-range band that survives the mask): id `0x65340002`.
3. `map-server/src/processor.rs::apply_create_content_area` — store
   `content_area_actor_id` on the session's `ActiveContentScript`.
4. `map-server/src/world_manager.rs::send_zone_in_bundle` — when the
   session has an `active_content_script` AND
   `current_zone_id == parent_zone_id` (the in-zone content warp),
   override `zone_actor_id` / `zone_name` / `zone_class_path` /
   `zone_class_name` with the content area's values (master id
   `0x65340002`, class
   `/Area/PrivateArea/Content/PrivateAreaMasterSimpleContent`). The
   `region_id` (map geometry) is left untouched — same map, new
   instance. The guard scopes the override to the content-entry warp;
   the content-exit `DoZoneChange` (to a different zone) falls through
   to the normal path.

## Unknowns left

- The area-master `ScriptBind` LuaParams are still built from
  `Zone.CreateScriptBindPacket` (15 params). A `PrivateAreaMaster`
  subclass may expect a different constructor schema. It inherits
  `AreaBaseClass`, whose `create(self, ?, isInstanceRaid,
  isEntranceDesion)` args are covered by the zone's 15 params, so this
  is the best evidence-backed guess — but confirm in the post-fix
  capture that the client doesn't report a Client Script ERROR.
- The combat cinematic kickoff (`dispatch_event_start_to_content_
  director`, processor.rs:3192) currently fires server-side DURING the
  warp, before loading completes. With this fix the client should
  complete loading and may then echo the autonomous `IN 0x012D`
  EventStart for the director (as the 1.x reference does ~2.28 s
  post-warp). Whether the cinematic plays cleanly, double-fires, or
  needs the kickoff moved to the client-driven `handle_event_start`
  path is the NEXT thing to determine FROM the post-fix packet log.

## Validation

- `cargo check -p map-server` clean.
- `cargo test -p map-server` — TODO (run locally).
- Live: fresh Gridania character → opening cinematic → camera tutorial
  → journal → second Yda talk → combat-tutorial warp. Expect: the
  "Now Loading" overlay clears (no infinite hang). Re-capture
  `packetlogs/map-packets.log` and confirm:
  - `OUT 0x0005 SetMap` now carries area id `0x65340002` (not `0xA6`).
  - `OUT` area master spawn uses class
    `…/PrivateAreaMasterSimpleContent`.
  - the client now echoes `RX 0x0007` after the warp.
  - then check whether the client echoes `IN 0x012D` EventStart for the
    content director (0x65300003) and whether `processTtrBtl001` plays.
