// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Battle packets. The client recognizes two containers for combat results:
//! `CommandResultX10` (up to 16 results) and `CommandResultX18` (up to 24).
//! `BattleAction` is the server's version of the same container, used for
//! server-initiated actions (cutscenes, traps, etc.). Each entry serializes
//! a `CommandResult` / `BattleAction` struct with target id, amount, type,
//! flags, animation id.
//!
//! The full wire format has ~40 flags per action; Phase 7 ports the
//! container + per-entry header. Deeper fields (proc flags, miss/parry/
//! block results, reflected damage) pass through as zero-padded bytes
//! until the battle math lands.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::body;

// ---------------------------------------------------------------------------
// 0x0195 SetEnmityIndicator — per-mob hate UI cue.
//
// Wire format (per wiki + retail bytes from
// `ffxiv_traces/combat_skills.pcapng` 0x0195 records):
//
//   subpacket source_id = bnpc emitting hate (REQUIRED — the wiki's
//                          "Source actor ID must set" requirement;
//                          the client routes the indicator to that
//                          actor's nameplate)
//   body size = 0x08 bytes
//   0x00  u32 target_actor_id  — the actor the bnpc is focused on
//                                 (player); use NO_ENMITY_TARGET
//                                 sentinel when hate is established
//                                 but no specific lock-on yet
//   0x04  u16 hate_amount      — percentage 0..100 (0=invisible,
//                                 1-49=green, 50-79=yellow, 80+=red);
//                                 special value HATE_LOCKED (0xFFFF)
//                                 means "permanently locked"
//   0x06  u16 zero/padding
//
// Lifecycle observed in the captures (combat_skills.pcapng):
//   1. (target=0xE0000000, hate=100)  — hate established, no target
//   2. (target=player,     hate=100)  — locked onto player, red gem
//   3. (target=player,     hate=0xFFFF) — permanently locked
//   4. (target=0xE0000000, hate=0xFFFF) — disengage from target,
//                                          retain max hate
//
// Project Meteor never emits this opcode (their `HateContainer.cs` is
// stubbed with a "todo: actually implement enmity properly" comment);
// the C# fork's combat targeting comes through entirely via 0x00DB
// `SetActorTarget`, which the client renders as a target indicator
// rather than the colored enmity gem.

/// Sentinel `target_actor_id` for "hate established but not locked
/// onto a specific actor". Same `0xE0000000` constant the 1.x client
/// uses for `SetTargetPacket.attackTarget` when no target.
pub const NO_ENMITY_TARGET: u32 = 0xE000_0000;

/// `hate_amount` sentinel for "permanent lock". Captured retail value;
/// out-of-range vs. the 0..100 percentage but the client interprets it
/// as the strongest possible hate.
pub const HATE_AMOUNT_LOCKED: u16 = 0xFFFF;

/// 0x0195 SetEnmityIndicator. `bnpc_actor_id` MUST be the bnpc emitting
/// hate — the client routes the gem update to that actor's nameplate
/// via the SubPacket source_id.
pub fn build_set_enmity_indicator(
    bnpc_actor_id: u32,
    target_actor_id: u32,
    hate_amount: u16,
) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(target_actor_id).unwrap();
    c.write_u16::<LittleEndian>(hate_amount).unwrap();
    // 2 bytes trailing padding stay zero.
    SubPacket::new(OP_SET_ENMITY_INDICATOR, bnpc_actor_id, data)
}

/// One unit of battle action serialized into the X10/X18 containers.
#[derive(Debug, Clone, Default)]
pub struct BattleAction {
    pub target_id: u32,
    pub amount: i32,
    pub amount_mitigated: i32,
    pub effect_id: u32,
    pub param: u32,
    pub animation: u16,
    pub flag: u8,
}

/// One unit of command result (player-initiated action). Only
/// `target_id` / `amount` / `worldmaster_text_id` / `hit_effect`
/// (pmeteor's `effectId` impact flags) / `param` / `hit_num` reach the
/// wire — the rest mirror `battle::command::CommandResult` for callers
/// that carry the full row through conversion.
#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    pub target_id: u32,
    pub hit_num: u8,
    pub sub_command: u8,
    pub hit_effect: u32,
    pub mitigated_amount: u32,
    pub amount: u32,
    pub command_type: u8,
    pub animation_id: u32,
    pub worldmaster_text_id: u16,
    pub param: u32,
    pub action_property: u8,
}

// Retail/pmeteor CommandResult container header, shared by X00/X01/X10/
// X18 (pmeteor `CommandResultX01Packet.cs:48-57` + retail bytes from
// `ffxiv_traces/combat_skills.pcapng` 0x0139 records):
//   +0x00 u32 sourceActorId
//   +0x04 u32 animationId
//   +0x08..0x20 zero (retail puts an unknown f32 at +0x1C on some
//                auto-attack rows; pmeteor never writes it)
//   +0x20 u32 numActions
//   +0x24 u16 commandId
//   +0x26 u16 marker — 0x0810; the X00 writer ships DECIMAL 810
//          (0x032A, pmeteor quirk byte-confirmed in the capture's
//          state-flush 0x013C records)
fn write_result_header(
    c: &mut Cursor<&mut [u8]>,
    source_actor_id: u32,
    animation_id: u32,
    num_actions: u32,
    command_id: u16,
    marker: u16,
) {
    c.write_u32::<LittleEndian>(source_actor_id).unwrap();
    c.write_u32::<LittleEndian>(animation_id).unwrap();
    c.set_position(0x20);
    c.write_u32::<LittleEndian>(num_actions).unwrap();
    c.write_u16::<LittleEndian>(command_id).unwrap();
    c.write_u16::<LittleEndian>(marker).unwrap();
}

fn encode_battle_action(c: &mut Cursor<&mut [u8]>, entry: &BattleAction) {
    c.write_u32::<LittleEndian>(entry.target_id).unwrap();
    c.write_i32::<LittleEndian>(entry.amount).unwrap();
    c.write_i32::<LittleEndian>(entry.amount_mitigated).unwrap();
    c.write_u32::<LittleEndian>(entry.effect_id).unwrap();
    c.write_u32::<LittleEndian>(entry.param).unwrap();
    c.write_u16::<LittleEndian>(entry.animation).unwrap();
    c.write_u8(entry.flag).unwrap();
    c.write_u8(0).unwrap();
}

/// 0x013C CommandResultX00 — "no hits" confirmation (pmeteor
/// `CommandResultX00Packet.cs`; byte-pinned to the capture's
/// state-flush X00, map-packets.log:38304). numActions is 0 and the
/// +0x26 marker is pmeteor's decimal-810 quirk.
pub fn build_command_result_x00(actor_id: u32, animation_id: u32, command_id: u16) -> SubPacket {
    let mut data = body(0x48);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_result_header(&mut c, actor_id, animation_id, 0, command_id, 810);
    }
    SubPacket::new(OP_COMMAND_RESULT_X00, actor_id, data)
}

/// 0x0139 CommandResultX01 — one hit (pmeteor
/// `CommandResultX01Packet.cs:40-69`; byte-pinned to retail
/// `combat_skills.pcapng` records). Row at +0x28:
/// `u32 targetId | u16 amount | u16 worldMasterTextId | u32 effectId |
///  u8 param | u8 hitNum`.
pub fn build_command_result_x01(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    action: &CommandResult,
) -> SubPacket {
    let mut data = body(0x58);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_result_header(&mut c, actor_id, animation_id, 1, command_id, 0x810);
        c.write_u32::<LittleEndian>(action.target_id).unwrap();
        c.write_u16::<LittleEndian>(action.amount as u16).unwrap();
        c.write_u16::<LittleEndian>(action.worldmaster_text_id)
            .unwrap();
        c.write_u32::<LittleEndian>(action.hit_effect).unwrap();
        c.write_u8(action.param as u8).unwrap();
        c.write_u8(action.hit_num).unwrap();
    }
    SubPacket::new(OP_COMMAND_RESULT_X01, actor_id, data)
}

/// 0x013A CommandResultX10 — up to 10 hits in one packet (pmeteor
/// `CommandResultX10Packet.cs` — the name's "X10" is the packet-size
/// label, not the capacity).
pub fn build_command_result_x10(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
) -> SubPacket {
    build_command_result_container(
        actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        10,
        OP_COMMAND_RESULT_X10,
        0xD8,
        // Column starts: targetId / amount / worldMasterTextId /
        // effectId / param / hitNum (CommandResultX10Packet.cs:54-81).
        [0x28, 0x50, 0x64, 0x78, 0xA0, 0xAA],
    )
}

/// 0x013B CommandResultX18 — up to 18 hits (pmeteor
/// `CommandResultX18Packet.cs`).
pub fn build_command_result_x18(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
) -> SubPacket {
    build_command_result_container(
        actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        18,
        OP_COMMAND_RESULT_X18,
        0x148,
        // Column starts per CommandResultX18Packet.cs:59-81.
        [0x28, 0x70, 0x94, 0xB8, 0x100, 0x112],
    )
}

/// Columnar multi-hit container — unlike the X01 row layout, the
/// pmeteor/retail multi-result packets store each field as its own
/// array: all targetIds, then all amounts, then all text ids, …
#[allow(clippy::too_many_arguments)]
fn build_command_result_container(
    actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[CommandResult],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
    columns: [u64; 6],
) -> SubPacket {
    let mut data = body(packet_size);
    let max = actions.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        write_result_header(
            &mut c,
            actor_id,
            animation_id,
            max as u32,
            command_id,
            0x810,
        );
        let rows = &actions[*list_offset..*list_offset + max];
        c.set_position(columns[0]);
        for a in rows {
            c.write_u32::<LittleEndian>(a.target_id).unwrap();
        }
        c.set_position(columns[1]);
        for a in rows {
            c.write_u16::<LittleEndian>(a.amount as u16).unwrap();
        }
        c.set_position(columns[2]);
        for a in rows {
            c.write_u16::<LittleEndian>(a.worldmaster_text_id).unwrap();
        }
        c.set_position(columns[3]);
        for a in rows {
            c.write_u32::<LittleEndian>(a.hit_effect).unwrap();
        }
        c.set_position(columns[4]);
        for a in rows {
            c.write_u8(a.param as u8).unwrap();
        }
        c.set_position(columns[5]);
        for a in rows {
            c.write_u8(a.hit_num).unwrap();
        }
    }
    *list_offset += max;
    SubPacket::new(opcode, actor_id, data)
}

/// 0x013A BattleActionX10 — server-initiated variant using `BattleAction`
/// entries. Opcode collides with `CommandResultX10`; distinguished by the
/// source actor and the payload layout.
pub fn build_battle_action_x10(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
) -> SubPacket {
    build_battle_action_container(
        player_actor_id,
        source_actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        16,
        OP_BATTLE_ACTION_X10,
        0xD8,
    )
}

pub fn build_battle_action_x18(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
) -> SubPacket {
    build_battle_action_container(
        player_actor_id,
        source_actor_id,
        animation_id,
        command_id,
        actions,
        list_offset,
        24,
        OP_BATTLE_ACTION_X18,
        0x148,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_battle_action_container(
    player_actor_id: u32,
    source_actor_id: u32,
    animation_id: u32,
    command_id: u16,
    actions: &[BattleAction],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = actions.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u32::<LittleEndian>(source_actor_id).unwrap();
        c.write_u32::<LittleEndian>(animation_id).unwrap();
        c.write_u16::<LittleEndian>(command_id).unwrap();
        c.write_u16::<LittleEndian>(max as u16).unwrap();
        for i in 0..max {
            encode_battle_action(&mut c, &actions[*list_offset + i]);
        }
    }
    *list_offset += max;
    SubPacket::new(opcode, player_actor_id, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the four 0x0195 packets captured from
    /// `ffxiv_traces/combat_skills.pcapng` — full hate lifecycle from
    /// "established, no target" → "locked on player at 100%" →
    /// "permanently locked" → "released target, retain max hate".
    /// Mob actor id 0x44D035D5; player 0x029B2941.
    #[test]
    fn enmity_indicator_lifecycle_matches_retail_capture() {
        // #1: hate established, no target lock yet (red gem, 100%).
        let p1 = build_set_enmity_indicator(0x44D0_35D5, NO_ENMITY_TARGET, 100);
        assert_eq!(p1.data, [0x00, 0x00, 0x00, 0xE0, 0x64, 0x00, 0x00, 0x00]);
        assert_eq!(p1.header.source_id, 0x44D0_35D5);
        assert_eq!(p1.game_message.opcode, OP_SET_ENMITY_INDICATOR);

        // #2: locked onto player, hate=100 (red gem).
        let p2 = build_set_enmity_indicator(0x44D0_35D5, 0x029B_2941, 100);
        assert_eq!(p2.data, [0x41, 0x29, 0x9B, 0x02, 0x64, 0x00, 0x00, 0x00]);

        // #3: permanently locked (HATE_AMOUNT_LOCKED sentinel).
        let p3 = build_set_enmity_indicator(0x44D0_35D5, 0x029B_2941, HATE_AMOUNT_LOCKED);
        assert_eq!(p3.data, [0x41, 0x29, 0x9B, 0x02, 0xFF, 0xFF, 0x00, 0x00]);

        // #4: target released, max hate retained.
        let p4 = build_set_enmity_indicator(0x44D0_35D5, NO_ENMITY_TARGET, HATE_AMOUNT_LOCKED);
        assert_eq!(p4.data, [0x00, 0x00, 0x00, 0xE0, 0xFF, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn enmity_indicator_body_is_eight_bytes() {
        let p = build_set_enmity_indicator(0x1, 0x2, 50);
        assert_eq!(p.data.len(), 8);
        // Wiki's gem-color thresholds: 0=invisible, 1-49=green,
        // 50-79=yellow, 80+=red. We pass 50 — boundary into yellow.
        let hate = u16::from_le_bytes([p.data[4], p.data[5]]);
        assert_eq!(hate, 50);
    }

    /// Reproduce retail 0x0139 record #2 from
    /// `ffxiv_traces/combat_skills.pcapng` — invigorate (cmd 27260)
    /// pressed by player 0x029B2941, self-targeted. animationId
    /// 0x0E00223F == `server_battle_commands.battleAnimation` for
    /// 27260, textId 30328, effectId 0x20036776, hitNum 1. Pins the
    /// full retail body byte-for-byte.
    #[test]
    fn command_result_x01_matches_retail_invigorate() {
        let pkt = build_command_result_x01(
            0x029B_2941,
            0x0E00_223F,
            27260,
            &CommandResult {
                target_id: 0x029B_2941,
                amount: 0,
                worldmaster_text_id: 30328,
                hit_effect: 0x2003_6776,
                param: 0,
                hit_num: 1,
                ..Default::default()
            },
        );
        assert_eq!(pkt.game_message.opcode, OP_COMMAND_RESULT_X01);
        assert_eq!(pkt.header.source_id, 0x029B_2941);
        let mut expected = [0u8; 0x38];
        expected[0x00..0x04].copy_from_slice(&0x029B_2941u32.to_le_bytes());
        expected[0x04..0x08].copy_from_slice(&0x0E00_223Fu32.to_le_bytes());
        expected[0x20..0x24].copy_from_slice(&1u32.to_le_bytes());
        expected[0x24..0x26].copy_from_slice(&27260u16.to_le_bytes());
        expected[0x26..0x28].copy_from_slice(&0x0810u16.to_le_bytes());
        expected[0x28..0x2C].copy_from_slice(&0x029B_2941u32.to_le_bytes());
        expected[0x2E..0x30].copy_from_slice(&30328u16.to_le_bytes());
        expected[0x30..0x34].copy_from_slice(&0x2003_6776u32.to_le_bytes());
        expected[0x35] = 1; // hitNum
        assert_eq!(pkt.data, expected);
    }

    /// Reproduce retail 0x0139 record #3 — the auto-attack row:
    /// anim 0x19001000, cmd 22104 (`AttackCommand`), target the mob,
    /// amount 0x51=81, textId 30301, effectId 0x08000604, param 1.
    /// The capture carries an unknown f32 1.0 at +0x1C that neither
    /// pmeteor nor we write ("Missing... last value is float" in
    /// `CommandResultX01Packet.cs`) — excluded from the comparison.
    #[test]
    fn command_result_x01_matches_retail_auto_attack() {
        let pkt = build_command_result_x01(
            0x029B_2941,
            0x1900_1000,
            22104,
            &CommandResult {
                target_id: 0x44D0_35D5,
                amount: 81,
                worldmaster_text_id: 30301,
                hit_effect: 0x0800_0604,
                param: 1,
                hit_num: 1,
                ..Default::default()
            },
        );
        let mut expected = [0u8; 0x38];
        expected[0x00..0x04].copy_from_slice(&0x029B_2941u32.to_le_bytes());
        expected[0x04..0x08].copy_from_slice(&0x1900_1000u32.to_le_bytes());
        expected[0x20..0x24].copy_from_slice(&1u32.to_le_bytes());
        expected[0x24..0x26].copy_from_slice(&22104u16.to_le_bytes());
        expected[0x26..0x28].copy_from_slice(&0x0810u16.to_le_bytes());
        expected[0x28..0x2C].copy_from_slice(&0x44D0_35D5u32.to_le_bytes());
        expected[0x2C..0x2E].copy_from_slice(&81u16.to_le_bytes());
        expected[0x2E..0x30].copy_from_slice(&30301u16.to_le_bytes());
        expected[0x30..0x34].copy_from_slice(&0x0800_0604u32.to_le_bytes());
        expected[0x34] = 1; // param
        expected[0x35] = 1; // hitNum
        assert_eq!(pkt.data[..0x1C], expected[..0x1C]);
        assert_eq!(pkt.data[0x20..], expected[0x20..]);
    }

    /// Reproduce the state-flush 0x013C from the pmeteor capture
    /// (`captures/pmeteor-quest/20260426-160210-gridania-manual3/
    /// map-packets.log:38304`) — src player 1, anim 0x72000062,
    /// numActions 0, cmd 0, and the decimal-810 (0x032A) marker quirk
    /// at +0x26.
    #[test]
    fn command_result_x00_matches_pmeteor_state_flush() {
        let pkt = build_command_result_x00(1, 0x7200_0062, 0);
        assert_eq!(pkt.game_message.opcode, OP_COMMAND_RESULT_X00);
        let mut expected = [0u8; 0x28];
        expected[0x00..0x04].copy_from_slice(&1u32.to_le_bytes());
        expected[0x04..0x08].copy_from_slice(&0x7200_0062u32.to_le_bytes());
        expected[0x26..0x28].copy_from_slice(&810u16.to_le_bytes());
        assert_eq!(pkt.data, expected);
    }

    /// X10 columnar layout — two rows land in the per-field arrays at
    /// pmeteor's column offsets (targets +0x28, amounts +0x50, textIds
    /// +0x64, effectIds +0x78, params +0xA0, hitNums +0xAA), and the
    /// capacity is 10 per packet.
    #[test]
    fn command_result_x10_packs_columns() {
        let rows: Vec<CommandResult> = (0..2u32)
            .map(|i| CommandResult {
                target_id: 0x1000 + i,
                amount: 10 + i,
                worldmaster_text_id: 30301,
                hit_effect: 0x0800_0604,
                param: 1,
                hit_num: (i + 1) as u8,
                ..Default::default()
            })
            .collect();
        let mut offset = 0usize;
        let pkt = build_command_result_x10(7, 0x1900_1000, 22104, &rows, &mut offset);
        assert_eq!(offset, 2);
        assert_eq!(pkt.data.len(), 0xD8 - 0x20);
        assert_eq!(&pkt.data[0x20..0x24], &2u32.to_le_bytes());
        assert_eq!(&pkt.data[0x24..0x26], &22104u16.to_le_bytes());
        assert_eq!(&pkt.data[0x28..0x2C], &0x1000u32.to_le_bytes());
        assert_eq!(&pkt.data[0x2C..0x30], &0x1001u32.to_le_bytes());
        assert_eq!(&pkt.data[0x50..0x52], &10u16.to_le_bytes());
        assert_eq!(&pkt.data[0x52..0x54], &11u16.to_le_bytes());
        assert_eq!(&pkt.data[0x64..0x66], &30301u16.to_le_bytes());
        assert_eq!(&pkt.data[0x78..0x7C], &0x0800_0604u32.to_le_bytes());
        assert_eq!(pkt.data[0xA0], 1);
        assert_eq!(pkt.data[0xAA], 1);
        assert_eq!(pkt.data[0xAB], 2);

        // Capacity is 10 — a 12-row burst leaves 2 for the next packet.
        let many: Vec<CommandResult> = (0..12).map(|_| CommandResult::default()).collect();
        let mut off = 0usize;
        let _ = build_command_result_x10(7, 0, 0, &many, &mut off);
        assert_eq!(off, 10);
    }
}
