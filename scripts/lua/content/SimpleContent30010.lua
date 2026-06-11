require ("global")
require ("modifiers")
-- onUpdate calls allyGlobal.EngageTarget; without this require the
-- global is nil and every onUpdate tick errored right after the
-- speed SetMod (errors swallowed by the ticker drain). (#28 S0.4)
require ("ally")

function onCreate(starterPlayer, contentArea, director)
	-- Reset the engagement latch: the engine caches ONE VM per script
	-- path for the whole server process, so the top-level
	-- `battleStarted = false` below runs only once — a second run of
	-- this tutorial (new character, or re-entry after a disconnect)
	-- would otherwise inherit battleStarted=true and let the allies
	-- attack before processTtrBtl002's targeting tutorial returns.
	-- (Found during the Limsa #25 port review.)
	battleStarted = false;
	--papalymo = contentArea:SpawnActor(2290005, "papalymo", 365.89, 4.0943, -706.72, -0.718);
	--yda = contentArea:SpawnActor(2290006, "yda", 365.266, 4.122, -700.73, 1.5659);	

	--mob1 = contentArea:SpawnActor(2201407, "mob1", 374.427, 4.4, -698.711, -1.942);
	--mob2 = contentArea:SpawnActor(2201407, "mob2", 375.377, 4.4, -700.247, -1.992);
	--mob3 = contentArea:SpawnActor(2201407, "mob3", 375.125, 4.4, -703.591, -1.54);
    yda = GetWorldManager().SpawnBattleNpcById(6, contentArea);
    papalymo = GetWorldManager().SpawnBattleNpcById(7, contentArea);
	mob1 = GetWorldManager().SpawnBattleNpcById(3, contentArea);
	mob2 = GetWorldManager().SpawnBattleNpcById(4, contentArea);
    mob3 = GetWorldManager().SpawnBattleNpcById(5, contentArea);
    -- Per pmeteor quest_system SimpleContent30010.lua: put Yda + the 3
    -- wolves in MainState 2 (active/engaged) so they stand up and are
    -- rendered as hostile/combat. Without this they spawn in the
    -- default passive state and lie down / aren't visible as enemies.
    -- Papalymo (caster) is intentionally left at the default state.
    yda:ChangeState(2);
    mob1:ChangeState(2);
    mob2:ChangeState(2);
    mob3:ChangeState(2);
	-- Party-add the allies (MeteorReborn SimpleContent30010.lua:17-18)
	-- so the HUD roster renders Yda/Papalymo HP bars — the 1.x party
	-- list reads the real party group (10001), and the content-group
	-- 30006 roster alone never lit the bars in any live run (#28 issue:
	-- "Yda and Papalymo's health should show in my Roster"). The
	-- quest_system_mac variant omits these lines, but its tutorial also
	-- never shows ally bars; MeteorReborn (Decimus's rescript) is the
	-- canonical reference. ContentFinished clears the transient party
	-- roster at teardown, so the party is solo again entering Gridania.
	starterPlayer.currentParty:AddMember(papalymo.actorId);
	starterPlayer.currentParty:AddMember(yda.actorId);
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);
	-- Allies are unkillable for the tutorial too (Modifier::MinimumHpLock
	-- floor-1 clamp, actor/chara.rs): a dead Yda/Papalymo would otherwise
	-- ride the default BNpc respawn weirdness mid-fight. The player's lock
	-- above is cleared at teardown (ContentFinished).
	yda:SetMod(modifiersGlobal.MinimumHpLock, 1);
	papalymo:SetMod(modifiersGlobal.MinimumHpLock, 1);
	
	
	openingStoper = contentArea:SpawnActor(1090384, "openingstoper", 356.09, 3.74, -701.62, -1.41);
	
	director:AddMember(starterPlayer);
	director:AddMember(director);
 	director:AddMember(papalymo);
	director:AddMember(yda);
	director:AddMember(mob1);
	director:AddMember(mob2);
	director:AddMember(mob3);
	
	--director:StartContentGroup();
	
end

function onDestroy()

end

-- #28 S2.5 — everyone fights once the player commits. Script-VM
-- global: persists across onUpdate calls. The VM is process-cached
-- per script path (NOT per content area), so onCreate re-arms it for
-- each fresh run.
battleStarted = false;

function onUpdate(tick, area)
	if not area then return end
	local players = area:GetPlayers()
	local mobs    = area:GetMonsters()   -- live-only (dead filtered by S0.5)
	local allies  = area:GetAllies()

	local engagedPlayer = nil
	for player in players do
		if player and player:IsEngaged() and player.target then
			engagedPlayer = player
			break
		end
	end
	if not battleStarted then
		-- Nothing moves until the player attacks. Latching on the
		-- player's engagement (not the F press) keeps the wolves alive
		-- and targetable through processTtrBtl002's targeting tutorial
		-- (plan R8 — allies killing wolves early could soft-lock
		-- _waitForTargetTutorial client-side).
		if not engagedPlayer then return end
		battleStarted = true
	end

	-- The roster tables are 1-indexed (GetAllies/GetMonsters populate
	-- Lua-standard arrays); iterate 1..#t, not the legacy 0-based form.

	-- Allies: spread across live wolves; re-engage as wolves die (the
	-- corpse-disengage sweep clears their state, S0.5 drops the corpse
	-- from `mobs`).
	local mi = 0
	for i = 1, #allies do
		local ally = allies[i]
		if ally and not ally:IsEngaged() and #mobs > 0 then
			local target = mobs[(mi % #mobs) + 1]
			mi = mi + 1
			ally:SetMod(modifiersGlobal.MovementSpeed, 8)
			allyGlobal.EngageTarget(ally, target)   -- Engage + AddBaseHate (ally.lua)
		end
	end
	-- Wolves: proactive once battle starts (retaliation hate already
	-- covers the struck wolf; this brings the bystanders in).
	for i = 1, #mobs do
		local mob = mobs[i]
		if mob and not mob:IsEngaged() then
			local foe = (i % 2 == 0 and #allies > 0) and allies[1]
						or (engagedPlayer or (#allies > 0 and allies[1]))
			if foe then allyGlobal.EngageTarget(mob, foe) end
		end
	end
end