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

//! `BattleNpc` — combat-capable NPC. Port of
//! `Actors/Chara/Npc/BattleNpc.cs`.
//!
//! Sits on top of `Npc` with: hate tracking, modifier layers
//! (pool / genus / spawn), mob mods, detection flags, kindred type,
//! and respawn/despawn timers. `die()` / `despawn()` / `force_respawn()`
//! emit `BattleEvent`s instead of calling packet builders directly —
//! matches the rest of our outbox-first dispatch pattern.

#![allow(dead_code)]

use crate::actor::modifier::ModifierMap;
use crate::battle::BattleEvent;
use crate::battle::BattleOutbox;
use crate::battle::HateContainer;

use super::actor_class::ActorClass;
use super::mob_modifier::{MobModifier, MobModifierMap};
use super::npc::Npc;

/// Detection bitfield. Same values as the C# `[Flags] DetectionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DetectionType(pub u32);

impl DetectionType {
    pub const NONE: Self = Self(0x00);
    pub const SIGHT: Self = Self(0x01);
    pub const SCENT: Self = Self(0x02);
    pub const SOUND: Self = Self(0x04);
    pub const LOW_HP: Self = Self(0x08);
    pub const IGNORE_LEVEL_DIFFERENCE: Self = Self(0x10);
    pub const MAGIC: Self = Self(0x20);

    pub const fn bits(self) -> u32 {
        self.0
    }
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl std::ops::BitOr for DetectionType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// `KindredType` — creature family the NPC belongs to. Used for scripted
/// matchups (weakness-to-kindred, etc.).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KindredType {
    #[default]
    Unknown = 0,
    Beast = 1,
    Plantoid = 2,
    Aquan = 3,
    Spoken = 4,
    Reptilian = 5,
    Insect = 6,
    Avian = 7,
    Undead = 8,
    Cursed = 9,
    Voidsent = 10,
}

impl KindredType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Beast,
            2 => Self::Plantoid,
            3 => Self::Aquan,
            4 => Self::Spoken,
            5 => Self::Reptilian,
            6 => Self::Insect,
            7 => Self::Avian,
            8 => Self::Undead,
            9 => Self::Cursed,
            10 => Self::Voidsent,
            _ => Self::Unknown,
        }
    }
}

/// Which modifier layer a stat value came from. Actor mods stack
/// pool → genus → spawn with the spawn row winning on conflicts.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModifierLayer {
    #[default]
    Pool = 0,
    Genus = 1,
    Spawn = 2,
}

// ---------------------------------------------------------------------------
// BattleNpc
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BattleNpc {
    pub npc: Npc,

    // Combat metadata
    pub detection_type: DetectionType,
    pub kindred_type: KindredType,
    pub neutral: bool,

    pub despawn_time_seconds: u32,
    pub respawn_time_seconds: u32,
    pub spawn_distance: u32,

    pub bnpc_id: u32,
    pub spell_list_id: u32,
    pub skill_list_id: u32,
    pub drop_list_id: u32,

    pub pool_id: u32,
    pub genus_id: u32,

    pub pool_mods: ModifierMap,
    pub genus_mods: ModifierMap,
    pub spawn_mods: ModifierMap,

    pub mob_modifiers: MobModifierMap,

    /// Last actor to land a damaging hit; used for EXP/loot attribution.
    /// `0` when nobody has attacked.
    pub last_attacker_actor_id: u32,
}

impl BattleNpc {
    /// `BattleNpc(actorNumber, ActorClass, uniqueId, spawnedArea, …)` —
    /// mirrors the C# ctor; instantiates the underlying `Npc` then layers
    /// combat state on top.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        actor_number: u32,
        actor_class: &ActorClass,
        unique_id: impl Into<String>,
        area_id: u32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        actor_state: u16,
        animation_id: u32,
        custom_display_name: Option<String>,
    ) -> Self {
        let mut npc = Npc::new(
            actor_number,
            actor_class,
            unique_id,
            area_id,
            x,
            y,
            z,
            rotation,
            actor_state,
            animation_id,
            custom_display_name,
        );

        // The C# registers a BattleNpcController on the AIContainer
        // inside the base-class ctor; our AIContainer stays controller-
        // less until spawner::spawn wires a BattleNpcController.
        npc.character.hate = HateContainer::new(npc.actor_id());
        npc.character.chara.spawn_x = x;
        npc.character.chara.spawn_y = y;
        npc.character.chara.spawn_z = z;

        Self {
            npc,
            detection_type: DetectionType::NONE,
            kindred_type: KindredType::Unknown,
            neutral: false,
            despawn_time_seconds: 10,
            respawn_time_seconds: 0,
            spawn_distance: 0,
            bnpc_id: 0,
            spell_list_id: 0,
            skill_list_id: 0,
            drop_list_id: 0,
            pool_id: 0,
            genus_id: 0,
            pool_mods: ModifierMap::default(),
            genus_mods: ModifierMap::default(),
            spawn_mods: ModifierMap::default(),
            mob_modifiers: MobModifierMap::new(),
            last_attacker_actor_id: 0,
        }
    }

    pub fn from_npc(npc: Npc) -> Self {
        let actor_id = npc.actor_id();
        let mut npc = npc;
        npc.character.hate = HateContainer::new(actor_id);
        Self {
            npc,
            detection_type: DetectionType::NONE,
            kindred_type: KindredType::Unknown,
            neutral: false,
            despawn_time_seconds: 10,
            respawn_time_seconds: 0,
            spawn_distance: 0,
            bnpc_id: 0,
            spell_list_id: 0,
            skill_list_id: 0,
            drop_list_id: 0,
            pool_id: 0,
            genus_id: 0,
            pool_mods: ModifierMap::default(),
            genus_mods: ModifierMap::default(),
            spawn_mods: ModifierMap::default(),
            mob_modifiers: MobModifierMap::new(),
            last_attacker_actor_id: 0,
        }
    }

    pub fn actor_id(&self) -> u32 {
        self.npc.actor_id()
    }

    pub fn get_detection_type(&self) -> u32 {
        self.detection_type.bits()
    }

    pub fn set_detection_type(&mut self, v: u32) {
        self.detection_type = DetectionType(v);
    }

    /// Phase C1 — apply combat metadata from the joined
    /// `server_battlenpc_*` DTO and attach the appropriate AI
    /// `Controller` to the AIContainer. Called by
    /// `processor::apply_spawn_battle_npc_by_id` after the DTO is
    /// resolved and the actor_id has been verified.
    ///
    /// Mirrors C# `WorldManager.cs::SpawnBattleNpcById` lines 444–452
    /// (`battleNpc.neutral = …`, `battleNpc.SetDetectionType(…)`,
    /// `battleNpc.respawnTime = …`, drop-list, kindred) and the
    /// `BattleNpcController` / `AllyController` registration that
    /// Meteor handles in the C# base-class ctor.
    ///
    /// Pool / genus / spawn modifier-layer merging (the
    /// `server_battlenpc_pool_mods` / `genus_mods` / `spawn_mods`
    /// rows) is intentionally NOT done here — it's a separate
    /// follow-up (`merge_modifier_layers` already exists; the loader
    /// just isn't wired). Without it, BattleNpcs spawn with baseline
    /// stats; combat math works but harder mobs are under-leveled.
    pub fn apply_spawn_metadata(
        &mut self,
        spawn: &crate::database::BattleNpcSpawn,
        bnpc_id: u32,
        actor_id: u32,
    ) {
        self.bnpc_id = bnpc_id;
        self.pool_id = spawn.pool_id;
        self.genus_id = spawn.genus_id;
        self.drop_list_id = spawn.drop_list_id;
        self.respawn_time_seconds = spawn.respawn_time;
        self.detection_type = DetectionType(spawn.detection);
        // Meteor parity (`WorldManager.cs:449`): aggroType == 0 → neutral.
        // Plus allegiance == 1 ally NPCs (Yda, Papalymo) stay neutral
        // toward the player regardless of how their genus row reads —
        // a non-neutral ally would aggro the player on the very first
        // detection tick.
        self.neutral = spawn.aggro_type == 0 || spawn.allegiance == 1;
        self.kindred_type = KindredType::from_u8(spawn.kindred_id);

        // Attach the AI Controller. Choice of kind comes from allegiance:
        //  - allegiance == 1 → Ally (player-side; AllyController only
        //    aggroes into the active content-group fight).
        //  - any other      → BattleNpc (hostile-to-player default).
        let controller_kind = if spawn.allegiance == 1 {
            crate::battle::controller::ControllerKind::Ally
        } else {
            crate::battle::controller::ControllerKind::BattleNpc
        };
        let mut ctrl = crate::battle::controller::Controller::new(controller_kind, actor_id);
        // The two `DetectionType` definitions (this module + controller)
        // share canonical Meteor C# bit layouts (Sight=0x01, Scent=0x02,
        // Sound=0x04, …) — bridging via the raw u32 round-trips.
        ctrl.battle.detection_type =
            crate::battle::controller::DetectionType(self.detection_type.0);
        ctrl.battle.neutral = self.neutral;
        ctrl.battle.level = self.npc.character.chara.level;
        ctrl.battle.spawn_position =
            common::Vector3::new(spawn.position_x, spawn.position_y, spawn.position_z);
        self.npc.character.ai_container.controller = Some(ctrl);
    }

    pub fn get_mob_mod(&self, m: MobModifier) -> i64 {
        self.mob_modifiers.get(m)
    }

    pub fn set_mob_mod(&mut self, m: MobModifier, v: i64) {
        self.mob_modifiers.set(m, v);
    }

    pub fn get_despawn_time(&self) -> u32 {
        self.despawn_time_seconds
    }

    pub fn set_despawn_time(&mut self, seconds: u32) {
        self.despawn_time_seconds = seconds;
    }

    pub fn get_respawn_time(&self) -> u32 {
        self.respawn_time_seconds
    }

    pub fn set_respawn_time(&mut self, seconds: u32) {
        self.respawn_time_seconds = seconds;
    }

    /// Fold pool → genus → spawn modifier layers into the underlying
    /// `Character.chara.mods`. Later layers overwrite earlier ones on
    /// key conflicts. Called at spawn time by the spawner.
    pub fn merge_modifier_layers(&mut self) {
        let merged = merge_modifier_map(&self.pool_mods, &self.genus_mods, &self.spawn_mods);
        self.npc.character.chara.mods = merged;
    }

    /// Port of `ForceRespawn()`. Restores spawn position + HP, clears
    /// hate, and emits a `Spawn` event for the dispatcher to broadcast.
    pub fn force_respawn(&mut self, outbox: &mut BattleOutbox) {
        // Reset position to spawn coords.
        self.npc.character.base.position_x = self.npc.character.chara.spawn_x;
        self.npc.character.base.position_y = self.npc.character.chara.spawn_y;
        self.npc.character.base.position_z = self.npc.character.chara.spawn_z;
        // Restore HP to max.
        let max_hp = self.npc.character.chara.max_hp;
        self.npc.character.chara.hp = max_hp;
        // Clear hate + last attacker.
        self.npc.character.hate.clear_hate(None);
        self.last_attacker_actor_id = 0;
        // Mark alive + announce.
        outbox.push(BattleEvent::Spawn {
            owner_actor_id: self.actor_id(),
        });
    }

    /// Port of `Die(tick, actionContainer)`. Emits a `Die` event that the
    /// dispatcher fans out. The AI state machine flips into `Death` via
    /// `AIContainer::internal_die` on the following tick.
    pub fn die(&mut self, outbox: &mut BattleOutbox) {
        self.npc.character.chara.hp = 0;
        outbox.push(BattleEvent::Die {
            owner_actor_id: self.actor_id(),
        });
    }

    /// Port of `Despawn(tick)`. Flags the actor for despawn; the zone
    /// cleanup and respawn timer run from `AIContainer::internal_despawn`.
    pub fn despawn(&mut self, outbox: &mut BattleOutbox) {
        outbox.push(BattleEvent::Despawn {
            owner_actor_id: self.actor_id(),
        });
    }

    /// Port of `OnAttack(state)` — invoked from the AI `AttackState` when
    /// a swing connects. Sets the last attacker so loot/EXP attribution
    /// works downstream. Returns `true` if the engagement should continue.
    pub fn on_attack(&mut self, attacker_actor_id: u32) -> bool {
        self.last_attacker_actor_id = attacker_actor_id;
        self.npc.character.hate.update_hate(attacker_actor_id, 1);
        true
    }
}

fn merge_modifier_map(pool: &ModifierMap, genus: &ModifierMap, spawn: &ModifierMap) -> ModifierMap {
    let mut out = pool.clone();
    for (k, v) in genus.iter() {
        out.set_raw(k, v);
    }
    for (k, v) in spawn.iter() {
        out.set_raw(k, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class() -> ActorClass {
        ActorClass::new(1_234_567, "/Chara/Npc/Mob/Dodo", 42, 0, "", 0, 0, 0)
    }

    #[test]
    fn new_battle_npc_seeds_hate_container() {
        let bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 10.0, 0.0, 10.0, 0.0, 0, 0, None);
        assert_eq!(bnpc.npc.character.hate.owner_actor_id, bnpc.actor_id());
    }

    #[test]
    fn force_respawn_restores_position_and_hp() {
        let mut bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 10.0, 0.0, 10.0, 0.0, 0, 0, None);
        bnpc.npc.character.chara.max_hp = 500;
        bnpc.npc.character.chara.hp = 0;
        bnpc.npc.character.base.position_x = 200.0;
        bnpc.npc.character.hate.update_hate(99, 50);

        let mut outbox = BattleOutbox::new();
        bnpc.force_respawn(&mut outbox);

        assert_eq!(bnpc.npc.character.chara.hp, 500);
        assert_eq!(bnpc.npc.character.base.position_x, 10.0);
        assert!(bnpc.npc.character.hate.is_empty());
        assert!(
            outbox
                .events
                .iter()
                .any(|e| matches!(e, BattleEvent::Spawn { .. }))
        );
    }

    #[test]
    fn die_pushes_event_and_zeros_hp() {
        let mut bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 10.0, 0.0, 10.0, 0.0, 0, 0, None);
        bnpc.npc.character.chara.hp = 500;
        let mut ob = BattleOutbox::new();
        bnpc.die(&mut ob);
        assert_eq!(bnpc.npc.character.chara.hp, 0);
        assert!(
            ob.events
                .iter()
                .any(|e| matches!(e, BattleEvent::Die { .. }))
        );
    }

    #[test]
    fn merge_modifier_layers_stacks_with_spawn_winning() {
        use crate::actor::modifier::Modifier;
        let mut bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        bnpc.pool_mods.set(Modifier::Defense, 10.0);
        bnpc.genus_mods.set(Modifier::Defense, 20.0);
        bnpc.spawn_mods.set(Modifier::Defense, 30.0);
        bnpc.pool_mods.set(Modifier::Strength, 5.0);
        bnpc.merge_modifier_layers();
        assert_eq!(bnpc.npc.character.chara.mods.get(Modifier::Defense), 30.0);
        assert_eq!(bnpc.npc.character.chara.mods.get(Modifier::Strength), 5.0);
    }

    #[test]
    fn on_attack_records_last_attacker() {
        let mut bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        assert!(bnpc.on_attack(42));
        assert_eq!(bnpc.last_attacker_actor_id, 42);
        assert!(bnpc.npc.character.hate.has_hate_for(42));
    }

    #[test]
    fn mob_mod_set_and_get() {
        let mut bnpc = BattleNpc::new(7, &class(), "dodo_7", 100, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        bnpc.set_mob_mod(MobModifier::SightRange, 30);
        assert_eq!(bnpc.get_mob_mod(MobModifier::SightRange), 30);
        assert_eq!(bnpc.get_mob_mod(MobModifier::SoundRange), 0);
    }

    fn make_spawn(
        allegiance: u8,
        aggro_type: u8,
        detection: u32,
    ) -> crate::database::BattleNpcSpawn {
        crate::database::BattleNpcSpawn {
            bnpc_id: 3,
            group_id: 2,
            position_x: 365.0,
            position_y: 4.0,
            position_z: -700.0,
            rotation: 1.5,
            pool_id: 2,
            script_name: "bloodthirsty_wolf".to_string(),
            min_level: 4,
            max_level: 4,
            respawn_time: 30,
            hp: 200,
            mp: 0,
            drop_list_id: 17,
            allegiance,
            spawn_type: 0,
            animation_id: 0,
            actor_state: 0,
            private_area_name: String::new(),
            private_area_level: 0,
            zone_id: 166,
            genus_id: 58,
            actor_class_id: 2_201_407,
            current_job: 0,
            combat_skill: 0,
            combat_delay: 0,
            aggro_type,
            speed: 8,
            detection,
            kindred_id: 1,
            kindred_name: "Beast".to_string(),
        }
    }

    #[test]
    fn c1_apply_spawn_metadata_populates_bnpc_fields() {
        let mut bnpc = BattleNpc::new(99, &class(), "wolf_99", 166, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        bnpc.npc.character.chara.level = 4;
        let spawn = make_spawn(
            /*allegiance=*/ 2, /*aggro_type=*/ 1, /*detection=*/ 0x01,
        );
        bnpc.apply_spawn_metadata(&spawn, 3, bnpc.actor_id());

        assert_eq!(bnpc.bnpc_id, 3);
        assert_eq!(bnpc.pool_id, 2);
        assert_eq!(bnpc.genus_id, 58);
        assert_eq!(bnpc.drop_list_id, 17);
        assert_eq!(bnpc.respawn_time_seconds, 30);
        assert_eq!(bnpc.detection_type, DetectionType::SIGHT);
        assert_eq!(bnpc.kindred_type, KindredType::Beast);
        assert!(
            !bnpc.neutral,
            "wolf with aggro_type=1 + hostile allegiance should not be neutral"
        );
    }

    #[test]
    fn c1_apply_spawn_metadata_attaches_battlenpc_controller_for_mob() {
        use crate::battle::controller::ControllerKind;
        let mut bnpc = BattleNpc::new(99, &class(), "wolf_99", 166, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        let actor_id = bnpc.actor_id();
        bnpc.npc.character.chara.level = 4;
        let spawn = make_spawn(
            /*allegiance=*/ 2, /*aggro_type=*/ 1, /*detection=*/ 0x01,
        );
        bnpc.apply_spawn_metadata(&spawn, 3, actor_id);

        let ctrl = bnpc
            .npc
            .character
            .ai_container
            .controller
            .as_ref()
            .expect("C1 should attach a Controller to the AIContainer");
        assert_eq!(ctrl.kind, ControllerKind::BattleNpc);
        assert_eq!(ctrl.owner_actor_id, actor_id);
        assert_eq!(ctrl.battle.detection_type.0, 0x01);
        assert!(!ctrl.battle.neutral);
        assert_eq!(ctrl.battle.level, 4);
    }

    #[test]
    fn c1_apply_spawn_metadata_attaches_ally_controller_for_player_allegiance() {
        use crate::battle::controller::ControllerKind;
        let mut bnpc = BattleNpc::new(99, &class(), "yda_99", 166, 0.0, 0.0, 0.0, 0.0, 0, 0, None);
        let actor_id = bnpc.actor_id();
        // Ally with aggro_type==1 still ends up neutral because
        // allegiance==1 forces neutrality (else Yda aggros the player).
        let spawn = make_spawn(
            /*allegiance=*/ 1, /*aggro_type=*/ 1, /*detection=*/ 0x01,
        );
        bnpc.apply_spawn_metadata(&spawn, 6, actor_id);

        let ctrl = bnpc
            .npc
            .character
            .ai_container
            .controller
            .as_ref()
            .expect("C1 should attach a Controller for ally NPCs too");
        assert_eq!(ctrl.kind, ControllerKind::Ally);
        assert!(
            ctrl.battle.neutral,
            "ally NPCs (allegiance=1) must stay neutral toward the player",
        );
        assert!(
            bnpc.neutral,
            "BattleNpc.neutral must match the controller's neutral flag",
        );
    }

    #[test]
    fn c1_apply_spawn_metadata_zero_aggro_type_forces_neutral_mob() {
        // A hostile-allegiance mob with aggro_type=0 still ends up
        // neutral, matching Meteor `WorldManager.cs:449`.
        let mut bnpc = BattleNpc::new(
            99,
            &class(),
            "passive_99",
            166,
            0.0,
            0.0,
            0.0,
            0.0,
            0,
            0,
            None,
        );
        let spawn = make_spawn(
            /*allegiance=*/ 2, /*aggro_type=*/ 0, /*detection=*/ 0x01,
        );
        bnpc.apply_spawn_metadata(&spawn, 100, bnpc.actor_id());
        assert!(bnpc.neutral);
        assert!(
            bnpc.npc
                .character
                .ai_container
                .controller
                .as_ref()
                .unwrap()
                .battle
                .neutral,
            "Controller.battle.neutral must mirror BattleNpc.neutral",
        );
    }
}
