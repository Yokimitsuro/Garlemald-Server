--[[

Special constants used by quest scripts.

]]

-- QUEST FLAGS
QFLAG_OFF		= 0;
QFLAG_OFF_HIDE	= 1;
QFLAG_TALK		= 2;
QFLAG_PUSH		= 3;
QFLAG_REWARD	= 4;
QFLAG_MAPONLY	= 5;
-- 15 ported quest scripts (man0u0, man0l0, etc1l8, ...) gate their
-- markers with `data:GetFlag(...) and QFLAG_NONE or QFLAG_TALK`.
-- Upstream Meteor never defines QFLAG_NONE, so it evaluates to nil and
-- the Lua `and nil or X` trap makes the expression ALWAYS yield X —
-- markers never clear and (in man0u0's case) the SEQ_000 tutorial looks
-- stuck (Garlemald-Server #26). Defining it as 0 (truthy in Lua, equal
-- to QFLAG_OFF on the wire) makes those expressions behave as written.
QFLAG_NONE		= 0;

-- SPECIAL SEQUENCE CONSTANTS
SEQ_ACCEPT		= 65535;
SEQ_COMPLETE	= 65534;

-- NPC LS
NPCLS_GONE		= 0;
NPCLS_INACTIVE	= 1;
NPCLS_ACTIVE	= 2;
NPCLS_ALERT		= 3;