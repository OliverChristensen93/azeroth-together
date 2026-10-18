//! Units and effects built the way the net layer builds streamed ones: a [`Guid`], a
//! [`NetEntity`] and an [`ObjectStore`] are all the attach, animation and spell-visual systems
//! need. Copied from `together::units`, which this game does not depend on.

use benilla_protocol::messages::ObjectFields;
use bevy::prelude::*;
use nightfall_sim::UnitRef;

use super::NightfallEntity;
use crate::net::{Guid, NetEntity, ObjectStore};

const FIELD_HEALTH: u16 = 22;
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

/// `FactionTemplate.dbc`: the Human player template every hero wears (a player template is
/// enemy-masked against the whole Monster group, so WoW's own Tab and attack checks find enemies
/// attackable; the all-friendly template 35 made every enemy neutral to them), and the hostile
/// monster template. A side effect of template 1: heroes read as Alliance (Human) in tooltips and
/// `UnitFactionGroup`, and the zone's PvP type is judged against it.
pub(super) const FACTION_PLAYER: u32 = 1;
pub(super) const FACTION_MONSTER: u32 = 14;

const HIGH_CREATURE: u64 = 0xF130 << 48;
/// The high word of a GameObject's guid.
const HIGH_GAMEOBJECT: u64 = 0xF110 << 48;
/// `GAMEOBJECT_DISPLAYID`, `GAMEOBJECT_STATE` and `GAMEOBJECT_TYPE_ID`: their indices in an
/// object's fields.
const FIELD_GO_DISPLAY: u16 = 8;
const FIELD_GO_STATE: u16 = 14;
const FIELD_GO_TYPE: u16 = 21;
/// `GO_STATE_ACTIVE`: open, a chest's lid up.
const GO_STATE_ACTIVE: u32 = 0;
/// `GO_STATE_READY`: shut, a chest's lid down.
const GO_STATE_READY: u32 = 1;
/// `GAMEOBJECT_TYPE_CHEST`: WoW shows its loot cursor over it, and a right-click reaches us.
pub(super) const GO_TYPE_CHEST: u32 = 3;
/// `GAMEOBJECT_TYPE_GENERIC`: a model and nothing to use (an exit's ring).
pub(super) const GO_TYPE_GENERIC: u32 = 5;

/// WoW's `(x, y, z)` (x north, y west, z up) to Bevy's (y up, −z forward).
pub(super) fn wow_to_bevy(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// A point on the simulation's ground plane, at height `y` in Bevy space.
pub(super) fn ground_to_bevy(p: nightfall_sim::V2, y: f32) -> Vec3 {
    Vec3::new(-p.y, y, -p.x)
}

/// The Bevy yaw that faces a model along the simulation direction `d`.
pub(super) fn yaw_of(d: nightfall_sim::V2) -> f32 {
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

/// The hero or enemy a guid names, as [`unit_guid`] made it: a creature's low 24 bits are its
/// enemy's id (its template's entry sits above them, [`super::enemies::enemy_guid_serial`]), a
/// player's guid is its hero's number past [`super::view::HERO_SERIAL_BASE`].
pub(super) fn unit_of(guid: u64) -> Option<UnitRef> {
    if guid & 0xFFFF_0000_0000_0000 == HIGH_CREATURE {
        return Some(UnitRef::Enemy((guid & 0xFF_FFFF) as u32));
    }
    let serial = guid.checked_sub(super::view::HERO_SERIAL_BASE)?;
    (serial < 256).then_some(UnitRef::Hero(serial as u8))
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
            NightfallEntity,
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

/// Write a unit's health, level and, for a player, experience (`xp` toward `next`) into its
/// store when they changed, so WoW's unit frames and XP bar (and their `UNIT_HEALTH` and
/// `PLAYER_XP_UPDATE` events) see them. A hero's power is the fight's ([`super::fight::fields`]).
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

/// A GameObject's guid from its serial.
pub(super) fn object_guid(serial: u64) -> u64 {
    HIGH_GAMEOBJECT | serial
}

/// A GameObject showing `GameObjectDisplayInfo` row `display`, of WoW type `go_type`, drawn shut.
pub(super) fn spawn_object(
    commands: &mut Commands,
    serial: u64,
    display: u32,
    scale: f32,
    pos: Vec3,
    go_type: u32,
) -> Entity {
    let guid = object_guid(serial);
    let pairs = [
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x21),
        (4, scale.to_bits()),
        (FIELD_GO_DISPLAY, display),
        (FIELD_GO_STATE, GO_STATE_READY),
        (FIELD_GO_TYPE, go_type),
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
            NightfallEntity,
        ))
        .id()
}

/// A container drawn open (searched) or shut.
pub(super) fn set_go_open(store: &mut ObjectStore, open: bool) {
    let state = if open {
        GO_STATE_ACTIVE
    } else {
        GO_STATE_READY
    };
    store
        .0
        .merge(ObjectFields::from_pairs(&[(FIELD_GO_STATE, state)]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_names_the_hero_or_enemy_it_was_made_for() {
        // As the enemies' views make it: the template's entry above the enemy's id.
        let serial = super::super::enemies::enemy_guid_serial(3, 41);
        let enemy = unit_guid(UnitKind::Creature { entry: 3 }, serial);
        assert_eq!(unit_of(enemy), Some(UnitRef::Enemy(41)));
        let hero = unit_guid(UnitKind::Player, super::super::view::HERO_SERIAL_BASE + 2);
        assert_eq!(unit_of(hero), Some(UnitRef::Hero(2)));
        assert_eq!(unit_of(3), None);
        assert_eq!(unit_of(0xF110_0000_0000_0007), None);
    }

    #[test]
    fn an_object_carries_its_display_state_and_type_where_wow_reads_them() {
        let mut store = ObjectStore(ObjectFields::from_pairs(&[
            (FIELD_GO_DISPLAY, 259),
            (FIELD_GO_STATE, GO_STATE_READY),
            (FIELD_GO_TYPE, GO_TYPE_CHEST),
        ]));
        assert_eq!(store.0.gameobject_displayid(), Some(259));
        assert_eq!(store.0.gameobject_type_id(), GO_TYPE_CHEST as i32);
        assert_eq!(store.0.gameobject_state(), Some(1), "shut");
        set_go_open(&mut store, true);
        assert_eq!(store.0.gameobject_state(), Some(0), "open");
        set_go_open(&mut store, false);
        assert_eq!(store.0.gameobject_state(), Some(1), "shut again");
        // A container's guid is never taken for a hero or an enemy.
        assert_eq!(unit_of(object_guid(0x20_0007)), None);
    }
}
