-- Ul'dah opening-zone stopper (Man0u0 SEQ_000 Merchant Strip).
-- Ported from quest_system `Data/scripts/base/chara/npc/object/
-- OpeningStoperW0B1.lua`; signature adapted to garlemald's NPC
-- dispatcher convention (player, npc, triggerName) like the already-
-- ported OpeningStoperF0B1. Its actor class (1090373) carries two
-- silent push circles: "caution" (radius 5 — warning message 34109)
-- and "exit" (radius 4 — bounce the player back inside).
require("global");

function init(npc)
	return false, false, 0, 0;
end

function onEventStarted(player, npc, triggerName)
	if (triggerName == "caution") then
		worldMaster = GetWorldMaster();
		player:SendGameMessage(player, worldMaster, 34109, 0x20);
	elseif (triggerName == "exit") then
		GetWorldManager():DoPlayerMoveInZone(player, 5.36433, 196, 133.656, -2.84938);
	end
	player:EndEvent();
end
