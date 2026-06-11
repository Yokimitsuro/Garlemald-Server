-- Garlemald-Server #25 follow-up (Limsa SEQ_005 deck-battle tutorial):
-- BNpc seed rows for the Man0l0 ship fight, mirroring the Gridania
-- tutorial rows (pools/groups 2-4, spawn rows 3-7) that SEQ_005
-- Gridania uses via SpawnBattleNpcById. Until now the Limsa fighters
-- existed only as synthetic contentArea:SpawnActor calls in
-- SimpleContent30002.lua — no combat AI, no kill gate, no HP model.
--
-- Actor classes (003_gamedata_actor_class.sql):
--   2205403 JellyfishScenarioLimsaLv00 (display 3205403, the tutorial
--           "town mob" the client's processTtrBtl002 targets)
--   2290001 FighterAllyOpeningHealer   (display 1900006, Y'shtola —
--           speaker of processTtrBtl001's say)
--   2290002 FighterAllyOpeningAttacker (display 1600179, Sthalmann —
--           speaker of processTtrBtl002/003's says)
-- Positions are the deck coordinates from the upstream
-- SimpleContent30002.lua SpawnActor calls.
--
-- HP follows the 053 pacing rationale: jellies sized for a ~60-90 s
-- three-mob fight; allies comfortable + MinimumHpLock'd in-script.
--
-- The zone-193 ROOT populace rows 530-532 (031_server_spawn_locations:
-- opening_jelly/yshtola/stahlmann, same classes + coordinates) are
-- deliberately KEPT: they are pmeteor-parity deck dressing in the base
-- zone, and the content-instance fan-out filter (world_manager.rs,
-- CONTENT_ACTOR_BAND_BIT) drops non-content-band actors from the
-- instance view, so they never appear alongside these BattleNpcs —
-- the same mechanism that hides the zone-166 populace Yda/Papalymo
-- from the Gridania tutorial fight.
-- INSERT OR IGNORE-only (idempotent; no schema change).

INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (5, 2205403, 'frenzied_aurelia', 20, 0, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (6, 2290002, 'sthalmann', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
-- currentJob 23 (CNJ): Y'shtola is canonically a conjurer. The caster
-- spawn path currently treats jobs 22|23 identically (same default
-- loop spell), so this only matters if CNJ ever gets a distinct kit.
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (7, 2290001, 'yshtola', 29, 23, 1, 4200, 1, 0, 0, 0, 0, 0);

INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (5, 5, 'frenzied_aurelia', 1, 1, 0, 250, 0, 0, 0, 1, 0, 0, '', 0, 193);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (6, 6, 'sthalmann', 1, 1, 0, 600, 0, 0, 1, 1, 0, 0, '', 0, 193);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (7, 7, 'yshtola', 1, 1, 0, 500, 0, 0, 1, 1, 0, 0, '', 0, 193);

INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (8, 'frenzied_aurelia', 5, -0.02, 17.35, 14.24, -2.81);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (9, 'frenzied_aurelia', 5, -3.02, 17.35, 14.24, -2.81);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (10, 'frenzied_aurelia', 5, -6.02, 17.35, 14.24, -2.81);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (11, 'sthalmann', 6, 0, 16.35, 22, 3);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (12, 'yshtola', 7, -8, 16.35, 6, 0.5);
