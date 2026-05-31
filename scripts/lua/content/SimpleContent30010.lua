require ("global")
require ("modifiers")

function onCreate(starterPlayer, contentArea, director)
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
	-- Pmeteor's SimpleContent30010.lua does NOT add NPCs to the player's
	-- party (only to the content director group via director:AddMember
	-- below). Garlemald's variant previously called
	-- `starterPlayer.currentParty:AddMember(papalymo.actorId)` +
	-- `starterPlayer.currentParty:AddMember(yda.actorId)`, which fired
	-- TWO extra OUT 0x017F GroupHeader/Begin/X08/End party trios pre-
	-- kick (visible in the seq005-no-prewarp-spawns capture at lines
	-- 6905..6953 + 6959..7007). Pmeteor's wire trace shows ZERO 0x017F
	-- broadcasts in the SEQ_005 warp window — only the single 0x0183
	-- content trio.
	starterPlayer:SetMod(modifiersGlobal.MinimumHpLock, 1);
	
	
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

function onUpdate(tick, area)
	if area then
		local players = area:GetPlayers()
		local mobs = area:GetMonsters()
		local allies = area:GetAllies()
		local resumeChecks = true
		for player in players do
			if player then
				local exitLoop = false
				
				if allies then
					for i = 0, #allies - 1 do
						if allies[i] then							
							if not allies[i]:IsEngaged() then
								if player:IsEngaged() and player.target then
									
									allies[i].neutral = false
									allies[i].isAutoAttackEnabled = true
									allies[i]:SetMod(modifiersGlobal.Speed, 8)
									allyGlobal.EngageTarget(allies[i], player.target)
									exitLoop = true
									break
								-- todo: support scripted paths
								elseif allies[i]:GetSpeed() > 0 then									
								end
							end
						end
					end
				end
				if exitLoop then
					resumeChecks = false
					break
				end
			end
		end
		if not resumeChecks then
			return
		end
	end
end