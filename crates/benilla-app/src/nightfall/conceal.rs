//! What the hero may know, kept to the interface (spec §4.3, §3.4): an enemy our hero does not see
//! is [`Concealed`] (no Tab, no mouseover, no plate) and untrackable (no minimap dot), though it
//! is still drawn where the fog and real walls allow. A sleeper lies down (stand state 3).
//! A hero's stealth look (the creep bit, the Stealth aura and form) is `auras::heroes`.

use benilla_protocol::field::FIELD_UNIT_BYTES_1;
use benilla_protocol::messages::ObjectFields;
use bevy::prelude::*;

use super::enemies::EnemyViews;
use super::run::Sim;
use crate::net::ObjectStore;
use crate::target::{Concealed, Selection};

/// `UNIT_FIELD_BYTES_1` byte 3's bits: ghost, creep (stealth), untrackable.
const VIS_GHOST: u8 = 0x1;
const VIS_CREEP: u8 = 0x2;
const VIS_UNTRACKABLE: u8 = 0x4;
/// `UNIT_FIELD_BYTES_1` of a creature that stands, is no ghost and cannot be tracked: for a unit
/// that is untrackable from the moment it is spawned (the ravens), as [`enemies`] makes a hidden
/// enemy.
pub(super) const UNTRACKABLE_BYTES_1: u32 = (VIS_UNTRACKABLE as u32) << 24;
/// `UNIT_FIELD_BYTES_1` byte 0's stand states: standing, sleeping.
const STAND: u8 = 0;
const STAND_SLEEP: u8 = 3;

/// `UNIT_FIELD_BYTES_1` as `store` holds it, with the creep and untrackable bits as asked and the
/// stand state replaced when given; the ghost bit, the loyalty and the form stay.
pub(super) fn bytes_1(
    store: &ObjectFields,
    stand: Option<u8>,
    creep: bool,
    untrackable: bool,
) -> u32 {
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

/// A hidden enemy can no longer be the hero's target, so the selection is dropped, and the target
/// frame lets go of it. Nightfall has no server to tell and no melee to stop, so it is cleared in
/// place (the last-target pair stays, as in a teardown). Callers write to the resource only for a
/// real clear: a mutable touch marks `Selection` changed, which the target frame's gate reads.
fn clear_selection(selection: &mut Selection) {
    selection.target = None;
    selection.guid = None;
}

/// How long a selected enemy may stay out of sight before the selection lets go of it, in real
/// seconds. A fight's target steps behind a tree or a pillar all the time (and on a joined client
/// the seen set only changes at snapshot rate), and dropping it on the first hidden frame would
/// cost the hero their target every time; but a target that stays hidden is no longer one.
pub(super) const TARGET_GRACE: f32 = 3.0;

/// The selected enemy's grace. A target only earns one by being seen while it is selected: a
/// hidden enemy selected from memory (the last-target keys) is dropped at once.
#[derive(Resource, Default)]
pub(super) struct TargetGrace {
    /// The selected enemy last seen while it was selected.
    seen: Option<Entity>,
    /// The selected enemy found out of sight, once its grace has started (since `since`). Reset
    /// when it is seen again, or when the selection moves to something else.
    entity: Option<Entity>,
    since: f32,
}

/// Every drawn enemy our hero does not see is concealed and untrackable; one they see is neither.
/// A sleeper at its post lies down. A selected enemy that stays out of sight for [`TARGET_GRACE`]
/// is deselected.
pub(super) fn enemies(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    time: Res<Time<Real>>,
    mut grace: ResMut<TargetGrace>,
    mut selection: ResMut<Selection>,
    mut bodies: Query<(&mut ObjectStore, Has<Concealed>)>,
) {
    let now = time.elapsed_secs();
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
        follow_target(&mut grace, &mut selection, e, hide, now);
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
    settle_grace(&mut grace, &selection);
}

/// The selected body `e`, seen or not this frame (`hide`): a seen one earns its grace, one out of
/// sight starts the clock, and the selection lets go of it once the grace is spent. Shared by the
/// enemies and the rivals, each with a [`TargetGrace`] of its own.
pub(super) fn follow_target<S: std::ops::DerefMut<Target = Selection>>(
    grace: &mut TargetGrace,
    selection: &mut S,
    e: Entity,
    hide: bool,
    now: f32,
) {
    // Read through the shared reference: only a real clear may mark the resource changed.
    if selection.target != Some(e) {
        return;
    }
    if !hide {
        grace.seen = Some(e);
        grace.entity = None;
    } else if grace.seen != Some(e) {
        // Never seen while selected: there is no sight to lose.
        clear_selection(&mut *selection);
    } else if grace.entity != Some(e) {
        grace.entity = Some(e);
        grace.since = now;
    } else if now - grace.since >= TARGET_GRACE {
        clear_selection(&mut *selection);
    }
}

/// The selection moved on (or was cleared elsewhere): the old target's clock is not the new one's.
pub(super) fn settle_grace(grace: &mut TargetGrace, selection: &Selection) {
    if grace.entity.is_some() && grace.entity != selection.target {
        grace.entity = None;
    }
    if grace.seen.is_some() && grace.seen != selection.target {
        grace.seen = None;
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
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
            rivals: "",
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
            fight: Default::default(),
        });
        let mut world = World::new();
        let body = world.spawn(ObjectStore(ObjectFields::from_pairs(&[]))).id();
        let mut views = EnemyViews::default();
        views.drawn.insert(ENEMY, body);
        world.insert_resource(views);
        world.insert_resource(sim);
        world.init_resource::<Selection>();
        world.init_resource::<TargetGrace>();
        // A clock's first update only starts it; after that, `pass` moves it.
        let mut time = Time::<Real>::default();
        time.update_with_duration(std::time::Duration::ZERO);
        world.insert_resource(time);
        (world, body)
    }

    /// Lets `secs` of real time pass for the next run of [`enemies`].
    fn pass(world: &mut World, secs: f32) {
        world
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_secs_f32(secs));
    }

    fn select(world: &mut World, body: Entity) {
        *world.resource_mut::<Selection>() = Selection {
            target: Some(body),
            guid: Some(7),
            last: Some(3),
        };
    }

    /// `body` selected and seen while selected, then out of sight from here on: the state a fight's
    /// target is in when it steps behind a pillar.
    fn select_and_see_then_hide(world: &mut World, body: Entity) {
        use bevy::ecs::system::RunSystemOnce;

        select(world, body);
        world.resource_mut::<Sim>().seen = vec![ENEMY];
        world.run_system_once(enemies).unwrap();
        world.resource_mut::<Sim>().seen = vec![];
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
        assert!(
            world.get::<Concealed>(body).is_none(),
            "seen: no stuck marker"
        );
        assert!(!world
            .get::<ObjectStore>(body)
            .unwrap()
            .0
            .unit_is_untrackable());
    }

    #[test]
    fn a_target_out_of_sight_for_a_moment_stays_selected() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        select_and_see_then_hide(&mut world, body);
        world.run_system_once(enemies).unwrap();
        pass(&mut world, 1.0);
        world.run_system_once(enemies).unwrap();
        assert!(world.get::<Concealed>(body).is_some(), "it is concealed");
        assert_eq!(world.resource::<Selection>().target, Some(body));
    }

    #[test]
    fn a_target_out_of_sight_past_the_grace_leaves_the_selection() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        select_and_see_then_hide(&mut world, body);
        world.run_system_once(enemies).unwrap();
        pass(&mut world, 3.5);
        world.run_system_once(enemies).unwrap();
        let sel = world.resource::<Selection>();
        assert_eq!((sel.target, sel.guid), (None, None));
        assert_eq!(sel.last, Some(3), "the last-target pair stays");
    }

    /// An enemy picked out of memory (the last-target keys) while hidden was never seen as a
    /// target, so it earns no grace; else pressing the key every 3 s would hold an unseen enemy.
    #[test]
    fn a_target_selected_while_out_of_sight_is_dropped_at_once() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        select(&mut world, body);
        world.run_system_once(enemies).unwrap();
        let sel = world.resource::<Selection>();
        assert_eq!((sel.target, sel.guid), (None, None));
        assert_eq!(sel.last, Some(3), "the last-target pair stays");
    }

    #[test]
    fn seeing_the_target_again_restarts_the_grace() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, body) = world_with_an_enemy();
        select_and_see_then_hide(&mut world, body);
        world.run_system_once(enemies).unwrap();
        pass(&mut world, 2.0);
        world.run_system_once(enemies).unwrap();
        // Seen again, then hidden again: 4 s since the first loss, but only 2 s since the second.
        world.resource_mut::<Sim>().seen = vec![ENEMY];
        world.run_system_once(enemies).unwrap();
        world.resource_mut::<Sim>().seen = vec![];
        world.run_system_once(enemies).unwrap();
        pass(&mut world, 2.0);
        world.run_system_once(enemies).unwrap();
        assert_eq!(world.resource::<Selection>().target, Some(body));
        // And the restarted grace still runs out.
        pass(&mut world, 1.5);
        world.run_system_once(enemies).unwrap();
        assert_eq!(world.resource::<Selection>().target, None);
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
