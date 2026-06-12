-- Garlemald-Server #26 (Ul'dah SEQ_005 combat tutorial): BNpc seed rows
-- for the Man0u0 Sapphire Avenue fight, mirroring the Gridania
-- (pools/groups 2-4, spawns 3-7) and Limsa (5-7 / 8-12, seed 054) rows
-- that their SEQ_005 fights use via SpawnBattleNpcById. Until now the
-- Ul'dah fighters existed only as synthetic contentArea:SpawnActor
-- calls in SimpleContent30079.lua — no combat AI, no kill gate, no HP
-- model.
--
-- Actor classes (003_gamedata_actor_class.sql):
--   2203301 GoobbueLesserStandard      (display 3203301, the Escaped
--           Goobbue the tutorial targets)
--   2290004 FighterAllyOpeningAttacker (display 1000010, Thancred —
--           melee ally, ChangeState(2) in-script)
--   2290003 FighterAllyOpeningHealer   (display 1200024, Niellefresne —
--           stays passive like Papalymo / Y'shtola)
-- Positions are the arena coordinates from the upstream
-- SimpleContent30079.lua SpawnActor calls.
--
-- HP follows the 053 pacing rationale: ONE goobbue vs three 250-HP
-- wolves/jellies — 500 HP keeps the single-mob fight in the same
-- ~60-90 s window with the player + Thancred melee + a caster ally.
-- INSERT OR IGNORE-only (idempotent; no schema change).

INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (8, 2203301, 'escaped_goobbue', 6, 0, 1, 4200, 1, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (9, 2290004, 'thancred', 29, 2, 1, 4200, 1, 0, 0, 0, 0, 0);
-- currentJob 23 (CNJ) for the healer ally, same as Y'shtola in 054.
INSERT OR IGNORE INTO "server_battlenpc_pools" ("poolId", "actorClassId", "name", "genusId", "currentJob", "combatSkill", "combatDelay", "combatDmgMult", "aggroType", "immunity", "linkType", "spellListId", "skillListId") VALUES
    (10, 2290003, 'niellefresne', 29, 23, 1, 4200, 1, 0, 0, 0, 0, 0);

INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (8, 8, 'escaped_goobbue', 1, 1, 0, 500, 0, 0, 0, 1, 0, 0, '', 0, 184);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (9, 9, 'thancred', 1, 1, 0, 600, 0, 0, 1, 1, 0, 0, '', 0, 184);
INSERT OR IGNORE INTO "server_battlenpc_groups" ("groupId", "poolId", "scriptName", "minLevel", "maxLevel", "respawnTime", "hp", "mp", "dropListId", "allegiance", "spawnType", "animationId", "actorState", "privateAreaName", "privateAreaLevel", "zoneId") VALUES
    (10, 10, 'niellefresne', 1, 1, 0, 500, 0, 0, 1, 1, 0, 0, '', 0, 184);

INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (13, 'escaped_goobbue', 8, -6.193, 192, 47.658, -2.224);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (14, 'thancred', 9, -26.41, 192, 39.52, 1.2);
INSERT OR IGNORE INTO "server_battlenpc_spawn_locations" ("bnpcId", "customDisplayName", "groupId", "positionX", "positionY", "positionZ", "rotation") VALUES
    (15, 'niellefresne', 10, -11.86, 192, 35.06, -0.8);
