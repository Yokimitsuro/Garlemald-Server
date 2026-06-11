-- Issue #28 (SEQ_005 combat tutorial): pools 3/4 shipped with the ally
-- actorClassIds crossed. Gamedata says 2290005 = FighterAllyOpeningHealer
-- (displayNameId 1400004, "Papalymo") and 2290006 =
-- FighterAllyOpeningAttacker (displayNameId 2300120, "Yda"); pmeteor's
-- quest_system spawned them that way. With the seed values swapped,
-- SpawnBattleNpcById(6)/"yda" materialised the THM healer class and (7)
-- the melee attacker — wrong nameplates and the wrong combat roles.
-- UPDATE-only (idempotent; no schema.sql change per the migrations rule).
UPDATE server_battlenpc_pools SET actorClassId = 2290006 WHERE poolId = 3; -- 'yda'      → FighterAllyOpeningAttacker (displays "Yda")
UPDATE server_battlenpc_pools SET actorClassId = 2290005 WHERE poolId = 4; -- 'papalymo' → FighterAllyOpeningHealer   (displays "Papalymo")
