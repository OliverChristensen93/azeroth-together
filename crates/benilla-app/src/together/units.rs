//! Units and effects built the way the net layer builds streamed ones: a [`Guid`], a
//! [`NetEntity`] and an [`ObjectStore`] are all the attach, animation and spell-visual systems
//! need. Adapted from `survivors::units`, which this mode does not depend on.

use benilla_protocol::messages::ObjectFields;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::TogetherEntity;
use crate::combat_text::CombatTextSpawn;
use crate::creature_anim::{CastEvent, CastEventKind, PlaySeq, SpellGoTargets, SwingMessage};
use crate::net::{Guid, NetEntity, ObjectStore};

const FIELD_HEALTH: u16 = 22;
/// `UNIT_FIELD_POWER1` (mana) and its maximum.
const FIELD_POWER: u16 = 23;
const FIELD_MAX_POWER: u16 = 29;
const FIELD_MAX_HEALTH: u16 = 28;
/// `PLAYER_BYTES_2` (byte 3 the rest state), `PLAYER_XP` and `PLAYER_NEXT_LEVEL_XP`.
const FIELD_PLAYER_BYTES_2: u16 = 194;
const FIELD_PLAYER_XP: u16 = 716;
const FIELD_PLAYER_NEXT_LEVEL_XP: u16 = 717;
/// Rest state 2: normal, not rested.
const REST_STATE_NORMAL: u32 = 2;
const FIELD_LEVEL: u16 = 34;
const FIELD_FACTION: u16 = 35;
const FIELD_BYTES_0: u16 = 36;
const FIELD_VIRTUAL_ITEM_DISPLAY: u16 = 37;
const FIELD_VIRTUAL_ITEM_INFO: u16 = 40;
const FIELD_BYTES_2: u16 = 164;

/// `FactionTemplate.dbc`: friendly to players, and the hostile monster template.
pub(super) const FACTION_FRIENDLY: u32 = 35;
pub(super) const FACTION_MONSTER: u32 = 14;

const HIGH_CREATURE: u64 = 0xF130 << 48;
const HIGH_GAMEOBJECT: u64 = 0xF110 << 48;

const FIELD_GO_DISPLAY: u16 = 8;
const FIELD_GO_STATE: u16 = 14;
const FIELD_GO_TYPE: u16 = 21;
/// `GAMEOBJECT_TYPE_GENERIC`: a model and nothing to use.
const GO_TYPE_GENERIC: u32 = 5;
/// `GO_STATE_READY`.
const GO_STATE_READY: u32 = 1;

/// WoW's `(x, y, z)` (x north, y west, z up) to Bevy's (y up, −z forward).
pub(super) fn wow_to_bevy(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// A point on the simulation's ground plane, at height `y` in Bevy space.
pub(super) fn ground_to_bevy(p: together_sim::V2, y: f32) -> Vec3 {
    Vec3::new(-p.y, y, -p.x)
}

/// The Bevy yaw that faces a model along the simulation direction `d`.
pub(super) fn yaw_of(d: together_sim::V2) -> f32 {
    d.y.atan2(d.x)
}

/// One weapon as the virtual item fields want it: slot, display, class, subclass, material,
/// inventory type, sheath.
pub(super) type Weapon = (u16, u32, u8, u8, u8, u8, u8);

/// What kind of WoW object a unit is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UnitKind {
    /// A creature, named by its template `entry` (see [`crate::names::NameCache`]).
    Creature { entry: u32 },
    /// A player: the local one is driven by WoW's own character controller.
    Player,
}

pub(super) struct UnitSpec<'a> {
    pub kind: UnitKind,
    pub serial: u64,
    pub display: u32,
    pub scale: f32,
    pub level: u32,
    pub faction: u32,
    /// `UNIT_FIELD_BYTES_0`: race, class, gender, power type.
    pub bytes0: u32,
    pub weapons: &'a [Weapon],
    /// `UNIT_FIELD_BYTES_2` byte 0: 0 stowed, 1 melee drawn, 2 ranged drawn.
    pub sheath: u8,
    /// More fields, written after the rest (a player's worn items).
    pub extra: &'a [(u16, u32)],
}

/// The guid a unit of `kind` with `serial` gets: a player's high word is 0.
pub(super) fn unit_guid(kind: UnitKind, serial: u64) -> u64 {
    match kind {
        UnitKind::Creature { .. } => HIGH_CREATURE | serial,
        UnitKind::Player => serial,
    }
}

/// Spawn a unit the attach builds a body for; the caller adds its own components.
pub(super) fn spawn_unit(commands: &mut Commands, spec: &UnitSpec, pos: Vec3, yaw: f32) -> Entity {
    let guid = unit_guid(spec.kind, spec.serial);
    // `OBJECT_FIELD_TYPE`: object | unit, and | player for a player.
    let (typemask, entry, net_kind) = match spec.kind {
        UnitKind::Creature { entry } => (0x9, entry, benilla_protocol::EntityKind::Unit),
        UnitKind::Player => (0x19, 0, benilla_protocol::EntityKind::Player),
    };
    let mut pairs: Vec<(u16, u32)> = vec![
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, typemask),
        (3, entry),
        (4, spec.scale.to_bits()),
        (FIELD_HEALTH, 100),
        (FIELD_MAX_HEALTH, 100),
        (FIELD_LEVEL, spec.level),
        (FIELD_FACTION, spec.faction),
        (FIELD_BYTES_0, spec.bytes0),
        (FIELD_BYTES_2, u32::from(spec.sheath)),
    ];
    if spec.kind == UnitKind::Player {
        // Rest state "normal" and an empty bar: the XP bar's code reads both from the first frame.
        pairs.extend([
            (FIELD_PLAYER_BYTES_2, REST_STATE_NORMAL << 24),
            (FIELD_PLAYER_XP, 0),
            (FIELD_PLAYER_NEXT_LEVEL_XP, 1),
        ]);
    }
    pairs.extend_from_slice(spec.extra);
    for &(slot, display, class, sub, material, inv, sheath) in spec.weapons {
        pairs.push((FIELD_VIRTUAL_ITEM_DISPLAY + slot, display));
        pairs.push((
            FIELD_VIRTUAL_ITEM_INFO + 2 * slot,
            u32::from(class)
                | u32::from(sub) << 8
                | u32::from(material) << 16
                | u32::from(inv) << 24,
        ));
        pairs.push((FIELD_VIRTUAL_ITEM_INFO + 2 * slot + 1, u32::from(sheath)));
    }
    let e = commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: net_kind,
                display_id: Some(spec.display),
                scale: spec.scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            crate::creature_anim::MovementState::default(),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            ShownVitals::default(),
            TogetherEntity,
        ))
        .id();
    // We turn creatures ourselves; the controller turns the player.
    if spec.kind != UnitKind::Player {
        commands.entity(e).insert(crate::net::ScriptedFacing);
    }
    e
}

/// The health, maximum, level and (for a player) experience last written into a unit's store.
#[derive(Component, Default, PartialEq)]
pub(super) struct ShownVitals([u32; 5]);

/// Write a unit's health, power, level and, for a player, experience (`xp` toward `next`) into its
/// store when they changed, so WoW's unit frames and XP bar (and their `UNIT_HEALTH` and
/// `PLAYER_XP_UPDATE` events) see them.
pub(super) fn set_vitals(
    store: &mut ObjectStore,
    shown: &mut ShownVitals,
    hp: f32,
    max_hp: f32,
    level: u32,
    xp: Option<(f32, f32)>,
) {
    let (xp_now, xp_next) = xp.map_or((0, 0), |(x, n)| (x.max(0.0) as u32, n.max(1.0) as u32));
    let now = ShownVitals([
        hp.max(0.0).ceil() as u32,
        max_hp.max(1.0).ceil() as u32,
        level,
        xp_now,
        xp_next,
    ]);
    if *shown == now {
        return;
    }
    let mut pairs = vec![
        (FIELD_HEALTH, now.0[0]),
        (FIELD_MAX_HEALTH, now.0[1]),
        (FIELD_LEVEL, now.0[2]),
        // Mana stays full: abilities cost nothing.
        (FIELD_POWER, 100),
        (FIELD_MAX_POWER, 100),
    ];
    if xp.is_some() {
        pairs.extend([
            (FIELD_PLAYER_XP, now.0[3]),
            (FIELD_PLAYER_NEXT_LEVEL_XP, now.0[4]),
        ]);
    }
    store.0.merge(ObjectFields::from_pairs(&pairs));
    *shown = now;
}

/// Spawn a generic GameObject showing `GameObjectDisplayInfo` row `display`.
pub(super) fn spawn_object(
    commands: &mut Commands,
    serial: u64,
    display: u32,
    scale: f32,
    pos: Vec3,
) -> Entity {
    let guid = HIGH_GAMEOBJECT | serial;
    let pairs = [
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x21),
        (4, scale.to_bits()),
        (FIELD_GO_DISPLAY, display),
        (FIELD_GO_STATE, GO_STATE_READY),
        (FIELD_GO_TYPE, GO_TYPE_GENERIC),
    ];
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::GameObject,
                display_id: Some(display),
                scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            Transform::from_translation(pos),
            Visibility::default(),
            TogetherEntity,
        ))
        .id()
}

/// Zero health: the animation selector plays Death and holds the corpse pose.
pub(super) fn kill_unit(store: &mut ObjectStore) {
    store
        .0
        .merge(ObjectFields::from_pairs(&[(FIELD_HEALTH, 0)]));
}

/// The messages that make a local cast look like a server's.
#[derive(SystemParam)]
pub(super) struct Fx<'w> {
    seq: ResMut<'w, PlaySeq>,
    casts: MessageWriter<'w, CastEvent>,
    gos: MessageWriter<'w, SpellGoTargets>,
    swings: MessageWriter<'w, SwingMessage>,
    text: MessageWriter<'w, CombatTextSpawn>,
}

/// More impacts than this per cast only cost frames.
const MAX_VISUAL_HITS: usize = 12;

impl Fx<'_> {
    /// `SMSG_SPELL_GO`: the cast kit on the caster, and a missile or an impact at each hit.
    pub fn cast(&mut self, caster: Entity, spell: u32, hits: &[Entity], dest: Option<Vec3>) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Go,
            seq,
        });
        self.gos.write(SpellGoTargets {
            caster,
            spell_id: spell,
            hits: hits.iter().copied().take(MAX_VISUAL_HITS).collect(),
            misses: Vec::new(),
            dest,
            ammo_display_id: None,
            seq,
        });
    }

    /// A melee swing: the attack animation, the victim's flinch, blood and the hit sound.
    pub fn swing(&mut self, attacker: Entity, victim: Entity, damage: f32) {
        let seq = self.seq.next();
        self.swings.write(SwingMessage {
            attacker,
            victim: Some(victim),
            hit_info: 0x2,
            victim_state: 1,
            damage: damage.max(0.0) as u32,
            displayed: false,
            seq,
        });
    }

    /// Floating words over `anchor`, 0xAARRGGBB.
    pub fn text(&mut self, anchor: Entity, text: String, color: u32) {
        self.text.write(CombatTextSpawn {
            anchor,
            text,
            category: 0,
            color: Some(color),
        });
    }

    /// A floating number over `anchor`, 0xAARRGGBB.
    pub fn number(&mut self, anchor: Entity, amount: f32, crit: bool, color: u32) {
        let shown = amount.round().max(1.0) as i64;
        self.text.write(CombatTextSpawn {
            anchor,
            text: shown.to_string(),
            category: if crit { 2 } else { 0 },
            color: Some(color),
        });
    }
}

pub(super) const COLOR_SPELL: u32 = 0xFFFF_DE00;
pub(super) const COLOR_HURT: u32 = 0xFFFF_3030;
