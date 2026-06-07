require ("global")
require ("tutorial")
require ("modifiers")
require ("quests/man/man0g0")

--processTtrBtl001: Active Mode Tutorial
--processTtrBtl002: Targetting Tutorial (After active mode done)

function init()
	return "/Director/Quest/QuestDirectorMan0g001";
end

function onEventStarted(player, actor, triggerName)
	-- Ported from pmeteor quest_system QuestDirectorMan0g001.lua. The
	-- prior Garlemald port over-engineered this with `waitForSignal`s
	-- for `playerAttack` / `tpOver1000` / `weaponskillUsed` / 3×`mobkill`
	-- — none of which are reliably wired/triggerable, so the director
	-- stalled after the first cinematic. pmeteor's flow is simple: play
	-- the active-mode cinematic, wait for the player to draw weapon
	-- (`playerActive`), kick + 2nd cinematic, then drive the rest with
	-- `wait()`s and tutorial widgets. Garlemald's prior `SetMod
	-- (MinimumHpLock, 1)` is dropped — pmeteor does not set it and it
	-- was suspected of affecting state during the active-mode step.
	man0g0Quest = player:GetQuest("Man0g0");
	startTutorialMode(player);

	if player:IsDiscipleOfWar() then
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtl001");
		player:EndEvent();
		waitForSignal("playerActive");
		wait(1); -- pmeteor note: "If this isn't here, the scripts bugs out"
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtl002", nil, nil, nil);
		player:EndEvent();

		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9055); -- attacking-enemy success
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_TP);
		wait(3);
		closeTutorialWidget(player);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_WEAPONSKILLS);
		wait(3);
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9065); -- weapon-skill success
	elseif player:IsDiscipleOfMagic() then
		callClientFunction(player, "delegateEvent", player, man0g0Quest, "processTtrBtlMagic001");
		player:EndEvent();
		wait(1);
		kickEventContinue(player, actor, "noticeEvent", "noticeEvent");
		closeTutorialWidget(player);
		showTutorialSuccessWidget(player, 9050);
		wait(1);
		openTutorialWidget(player, CONTROLLER_KEYBOARD, TUTORIAL_DEFEATENEMY);
		wait(3);
		closeTutorialWidget(player);
	end

	wait(3);
	worldMaster = GetWorldMaster();
	player:SendDataPacket("attention", worldMaster, "", 51073, 2);
	wait(7);
	player:ChangeMusic(7);
	player:ChangeState(0);

	callClientFunction(player, "delegateEvent", player, man0g0Quest, "processEvent020_1");
	man0g0Quest:StartSequence(10);

	player:EndEvent();
	player:GetZone():ContentFinished();
	GetWorldManager():DoZoneChange(player, 155, "PrivateAreaMasterPast", 1, 15, 175.38, -1.21, -1156.51, -2.1);
	player:EndEvent();
end

function onUpdate(deltaTime, area)
end

function onTalkEvent(player, npc)

end

function onPushEvent(player, npc)
end

function onCommandEvent(player, command)

end

function onEventUpdate(player, npc)
end

function onCommand(player, command)
end

-- NOTE: this director intentionally defines NO `main` (and no
-- `onCreateContentArea`). pmeteor's QuestDirectorMan0g001 has neither — its
-- `onCreateContentArea` is orphan/dead code never called from C#. The content
-- group's spawning + roster are owned entirely by SimpleContent30010.lua's
-- `onCreate` (director:AddMember ×7), and the content-group wire trio is
-- emitted Rust-side from apply_do_zone_change_content. A `main(director,
-- contentGroup)` here used to crash on a nil `contentGroup` (the engine only
-- passes the director to a director's main), aborting before StartContentGroup
-- with no effect on the wire. (Garlemald-Server #28.)