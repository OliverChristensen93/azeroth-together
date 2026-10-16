//! What the hero may know, kept to the interface (spec §4.3, §3.4): an enemy our hero does not see
//! is [`Concealed`] (no Tab, no mouseover, no plate) and untrackable (no minimap dot), though it
//! is still drawn where the fog and real walls allow. A sleeper lies down (stand state 3).
//! Stealthed heroes wear WoW's own stealth: `UNIT_FIELD_BYTES_1`'s creep bit (the stealth walk,
//! no plate, no footstep sounds) and the Stealth aura in their first slot (its translucency, and
//! its icon on the buff frame).

use benilla_protocol::field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1};
use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::prelude::*;

use super::enemies::EnemyViews;
use super::run::Sim;
use super::view::Views;
use crate::net::{FieldChanged, Guid, ObjectStore};
use crate::target::{Concealed, Selection};

/// Stealth (Rank 3), a level-30 rogue's: its aura's state kit fades the body.
const STEALTH_AURA: u32 = 1786;
/// `UNIT_FIELD_AURAFLAGS`' nibble for a live, cancelable aura, as the survivors' auras write it.
const AURA_LIVE: u32 = 0x3;
/// `UNIT_FIELD_BYTES_1` byte 3's bits: ghost, creep (stealth), untrackable.
const VIS_GHOST: u8 = 0x1;
const VIS_CREEP: u8 = 0x2;
const VIS_UNTRACKABLE: u8 = 0x4;
/// `UNIT_FIELD_BYTES_1` byte 0's stand states: standing, sleeping.
const STAND: u8 = 0;
const STAND_SLEEP: u8 = 3;

/// `UNIT_FIELD_BYTES_1` as `store` holds it, with the creep and untrackable bits as asked and the
/// stand state replaced when given; the ghost bit, the loyalty and the form stay.
pub(super) fn bytes_1(store: &ObjectFields, stand: Option<u8>, creep: bool, untrackable: bool) -> u32 {
    let mut vis = 0;
    if store.unit_is_ghost_visual() {
        vis |= VIS_GHOST;
    }
    if creep {
        vis |= VIS_CREEP;
    }
    if untrackable {
        vis |= VIS_UNTRACKABLE;
    }
    u32::from(stand.unwrap_or(store.unit_stand_state()))
        | u32::from(store.unit_loyalty_level()) << 8
        | u32::from(store.unit_shapeshift_form()) << 16
        | u32::from(vis) << 24
}

/// The aura fields that put Stealth in the first slot, or take it out. A hero carries no other
/// aura yet, so the slot's flag word is ours whole.
pub(super) fn stealth_aura(on: bool) -> [(u16, u32); 2] {
    if on {
        [(FIELD_UNIT_AURA, STEALTH_AURA), (FIELD_UNIT_AURAFLAGS, AURA_LIVE)]
    } else {
        [(FIELD_UNIT_AURA, 0), (FIELD_UNIT_AURAFLAGS, 0)]
    }
}

/// A hidden enemy can no longer be the hero's target, so the selection is dropped, and the target
/// frame lets go of it. Nightfall has no server to tell and no melee to stop, so it is cleared in
/// place (the last-target pair stays, as in a teardown). Callers write to the resource only for a
/// real clear: a mutable touch marks `Selection` changed, which the target frame's gate reads.
fn clear_selection(selection: &mut Selection) {
    selection.target = None;
    selection.guid = None;
}

/// Every drawn enemy our hero does not see is concealed and untrackable; one they see is neither.
/// A sleeper at its post lies down. An enemy that drops out of sight while selected is deselected.
pub(super) fn enemies(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut selection: ResMut<Selection>,
    mut bodies: Query<(&mut ObjectStore, Has<Concealed>)>,
) {
    for enemy in &sim.world.enemies {
        let Some(&e) = views.drawn.get(&enemy.id) else {
            continue;
        };
        let Ok((mut store, concealed)) = bodies.get_mut(e) else {
            continue;
        };
        let hide = !sim.seen.contains(&enemy.id);
        if hide != concealed {
            if hide {
                commands.entity(e).insert(Concealed);
            } else {
                commands.entity(e).remove::<Concealed>();
            }
        }
        // Read through the shared reference: only a real clear may mark the resource changed.
        if hide && selection.target == Some(e) {
            clear_selection(&mut selection);
        }
        let stand = if enemy.asleep { STAND_SLEEP } else { STAND };
        let now = bytes_1(
            &store.0,
            None,
            store.0.unit_is_stealthed(),
            store.0.unit_is_untrackable(),
        );
        let want = bytes_1(&store.0, Some(stand), false, hide);
        if now != want {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, want)]));
        }
    }
}

/// Every hero's body wears their stealth as the host has it: the creep bit and the Stealth aura,
/// ringing the aura watch as a server's update would, so the aura's kit (the translucency) comes
/// and goes.
pub(super) fn heroes(
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut fields: MessageWriter<FieldChanged>,
) {
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        if store.0.unit_is_stealthed() == hero.stealthed {
            continue;
        }
        let untrackable = store.0.unit_is_untrackable();
        let mut pairs = vec![(
            FIELD_UNIT_BYTES_1,
            bytes_1(&store.0, None, hero.stealthed, untrackable),
        )];
        pairs.extend(stealth_aura(hero.stealthed));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        fields.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Player,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(bytes_1: u32) -> ObjectFields {
        ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, bytes_1)])
    }

    #[test]
    fn the_creep_and_untrackable_bits_come_and_go_and_the_rest_stays() {
        // Sitting (stand state 1) in shapeshift form 5.
        let f = fields(0x0005_0001);
        assert_eq!(bytes_1(&f, None, true, false), 0x0205_0001);
        assert_eq!(bytes_1(&f, None, false, true), 0x0405_0001);
        assert_eq!(bytes_1(&f, Some(STAND_SLEEP), false, false), 0x0005_0003);
        assert_eq!(bytes_1(&fields(0x0200_0000), None, false, false), 0);
    }

    #[test]
    fn a_ghost_stays_a_ghost() {
        assert_eq!(bytes_1(&fields(0x0100_0000), None, true, true), 0x0700_0000);
    }

    #[test]
    fn stealth_puts_its_aura_in_the_first_slot_and_takes_it_out() {
        assert_eq!(
            stealth_aura(true),
            [(FIELD_UNIT_AURA, 1786), (FIELD_UNIT_AURAFLAGS, 0x3)]
        );
        assert_eq!(
            stealth_aura(false),
            [(FIELD_UNIT_AURA, 0), (FIELD_UNIT_AURAFLAGS, 0)]
        );
    }

    const ENEMY: u32 = 5;

    /// A world with one drawn enemy (id [`ENEMY`]) and nothing seen, for [`enemies`] to run on.
    fn world_with_an_enemy() -> (World, Entity) {
        use std::sync::Arc;

        use nightfall_sim::{Content, ContentSources, Enemy, V2};

        let content = Content::from_ron(ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
        })
        .expect("the shipped content parses");
        let mut sim = Sim::for_test(nightfall_sim::World::new(Arc::new(content)));
        sim.world.enemies.push(Enemy {
            id: ENEMY,
            kind: 0,
            pos: V2::new(0.0, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 1.0,
            max_hp: 1.0,
            speed: 0.0,
            asleep: false,
        });
        let mut world = World::new();
        let body = world
            .spawn(ObjectStore(ObjectFields::from_pairs(&[])))
            .id();
        let mut views = EnemyViews::default();
        views.drawn.insert(ENEMY, body);
        world.insert_resource(views);
        world.insert_resource(sim);
        world.init_resource::<Selection>();
        (world, body)
    }

    #[test]
    fn an_unseen_enemy_is_concealed_and_seen_again_is_not() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        world.run_system_once(enemies).unwrap();
        assert!(world.get::<Concealed>(body).is_some(), "unseen: concealed");
        let store = world.get::<ObjectStore>(body).unwrap();
        assert!(!store.0.unit_is_stealthed());
        assert!(store.0.unit_is_untrackable(), "and off the minimap");

        world.resource_mut::<Sim>().seen = vec![ENEMY];
        world.run_system_once(enemies).unwrap();
        assert!(world.get::<Concealed>(body).is_none(), "seen: no stuck marker");
        assert!(!world.get::<ObjectStore>(body).unwrap().0.unit_is_untrackable());
    }

    #[test]
    fn an_enemy_that_drops_out_of_sight_leaves_the_selection() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        *world.resource_mut::<Selection>() = Selection {
            target: Some(body),
            guid: Some(7),
            last: Some(3),
        };
        world.run_system_once(enemies).unwrap();
        let sel = world.resource::<Selection>();
        assert_eq!((sel.target, sel.guid), (None, None));
        assert_eq!(sel.last, Some(3), "the last-target pair stays");
    }

    /// The target frame redraws only when `Selection` changed, so a frame that clears nothing must
    /// not mark it changed (the engine's own click tests pin the same property).
    #[test]
    fn a_frame_that_clears_nothing_leaves_the_selection_unchanged() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, _body) = world_with_an_enemy();
        let other = world.spawn_empty().id();
        *world.resource_mut::<Selection>() = Selection {
            target: Some(other),
            guid: Some(9),
            last: None,
        };
        world.clear_trackers();
        // The enemy is hidden but is not the selection.
        world.run_system_once(enemies).unwrap();
        assert!(!world.is_resource_changed::<Selection>());
        assert_eq!(world.resource::<Selection>().target, Some(other));
    }
}
