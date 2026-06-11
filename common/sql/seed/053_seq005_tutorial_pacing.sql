-- Issue #28 (SEQ_005 combat tutorial): pacing. The tutorial NPCs shipped
-- with hp=0, which the spawn path resolves to the level-scaled fallback
-- (minLevel×100 = 100 HP). The 2026-06-10 live run proved 100 HP wolves
-- die in ~18 s — each one-shot by Papalymo's 100-damage cast — before
-- the player can land a single swing, let alone build TP for the
-- weaponskill step the tutorial teaches. Give the wolves enough HP for
-- a ~60-90 s fight (player + Yda melee + a level-scaled Thunder), and
-- the allies a comfortable pool (they additionally carry MinimumHpLock
-- so they cannot die mid-tutorial).
-- UPDATE-only (idempotent; no schema.sql change per the migrations rule).
UPDATE server_battlenpc_groups SET hp = 250 WHERE groupId = 2; -- bloodthirsty_wolf ×3
UPDATE server_battlenpc_groups SET hp = 600 WHERE groupId = 3; -- yda
UPDATE server_battlenpc_groups SET hp = 500 WHERE groupId = 4; -- papalymo
