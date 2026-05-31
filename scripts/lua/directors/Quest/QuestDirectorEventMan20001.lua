require ("global")
require ("quests/man/man200")

-- Ported verbatim from pmeteor quest_system. Stub director for the
-- man200 event quest — spawns a placeholder SNpc actor at the event
-- location. onEventStarted is intentionally empty (event body lives
-- in man200.lua).

local snpc;

function init()
	return "/Director/Quest/QuestDirectorEventMan20001";
end

function onCreateContentArea(players, director, contentArea, contentGroup)

	snpc = contentArea:SpawnActor(players[1]:GetSNpcSkin() + 107000, "snpc", -203.32, 0, -159.627);
	contentGroup:AddMember(player);
	contentGroup:AddMember(snpc);
	contentGroup:AddMember(director);

end

function onEventStarted(player, director, triggerName)

end

function main()
end
