require ("global")
require ("quests/man/man0l1")

-- Ported verbatim from pmeteor quest_system. Director for the Limsa
-- phase-2 quest man0l1: spawns Sisipu in the content area and
-- registers her in the content group. onEventStarted just closes the
-- event for now (script body lives in man0l1.lua).

function init()
	return "/Director/Quest/QuestDirectorMan0l101";
end

function onCreateContentArea(players, director, contentArea, contentGroup)

	sisipu = contentArea:SpawnActor(2290007, "sisipu", -49, 36.43, 162, 2.2);

	for _, player in pairs(players) do
        contentGroup:AddMember(player);
    end;

	contentGroup:AddMember(director);
	contentGroup:AddMember(sisipu);

end

function onEventStarted(player, director, triggerName)

	player:EndEvent();


end

function main()
end
