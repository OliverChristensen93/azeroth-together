//! The companion in a raid (spec §8.2): a small pet that follows its hero, a step behind and to
//! the side, carrying the one stack no death takes. Every client draws every hero's companion from
//! `Carry::pet`; it is a look only: the host's rules know the stack, not the animal.

use std::collections::HashMap;

use bevy::prelude::*;
use nightfall_sim::{InnContent, PlayerId};

use super::enemies::enemy_guid_serial;
use super::inn::glide;
use super::run::Sim;
use super::units::{self, UnitKind, UnitSpec};
use super::view::Views;
use crate::creature_anim::{move_flags, MovementState};
use crate::names::{CreatureRecord, NameCache};
use crate::net::GuidIndex;

/// How far behind and beside its hero a companion walks, yards.
const BEHIND: f32 = 1.6;
const BESIDE: f32 = 0.8;
/// A companion's creature entry is this plus its kind, clear of the enemies' (their kind plus one),
/// so the name cache files each apart.
const PET_ENTRY_BASE: u32 = 0x4000;
/// A companion's guid counter, plus its hero's number: high in the 24 bits, so the guid never reads
/// as the id of an enemy ([`units::unit_of`]).
const PET_COUNTER_BASE: u32 = 0xF0_0000;

/// Which entity draws which hero's companion, and of which kind.
#[derive(Resource, Default)]
pub(super) struct PetViews {
    drawn: HashMap<PlayerId, (u8, Entity)>,
}

#[derive(Component)]
pub(super) struct PetBody;

/// Where a companion walks for a hero at `at` turned `rotation`: behind and a little beside.
pub(super) fn follow_spot(at: Vec3, rotation: Quat) -> Vec3 {
    // Bevy's forward is -Z; the hero's own forward is its rotation of it.
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    at - forward * BEHIND + right * BESIDE
}

/// The companion `kind` names, if the content has one. `Carry::pet` comes from a client over the
/// wire, so an index the content does not know draws nothing; it is never indexed with `[]`.
fn companion_of(inn: &InnContent, kind: u8) -> Option<&nightfall_sim::CompanionDef> {
    inn.companions.get(usize::from(kind))
}

/// Every hero's companion drawn beside them and following; gone with its hero.
#[allow(clippy::type_complexity)]
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    time: Res<Time<Real>>,
    mut pets: ResMut<PetViews>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    heroes: Query<&Transform, Without<PetBody>>,
    mut bodies: Query<(&mut Transform, &mut MovementState), With<PetBody>>,
) {
    let inn = super::inn();
    let dt = time.delta_secs().max(1e-3);
    for hero in &sim.world.heroes {
        let (Some(kind), Some(&he)) = (hero.carry.pet, views.heroes.get(&hero.player)) else {
            continue;
        };
        let Some(def) = companion_of(&inn, kind) else {
            continue;
        };
        let Ok(htf) = heroes.get(he) else {
            continue;
        };
        let spot = follow_spot(htf.translation, htf.rotation);
        match pets.drawn.get(&hero.player) {
            Some(&(k, e)) if k == kind => {
                if let Ok((mut tf, mut mv)) = bodies.get_mut(e) {
                    let before = tf.translation;
                    tf.translation = glide(before, spot, dt);
                    tf.rotation = tf.rotation.slerp(htf.rotation, (dt * 6.0).min(1.0));
                    let speed = before.distance(tf.translation) / dt;
                    let flags = if speed > 0.3 { move_flags::FORWARD } else { 0 };
                    if (mv.speed - speed).abs() > 0.05 || mv.flags != flags {
                        mv.speed = speed;
                        mv.flags = flags;
                    }
                }
            }
            other => {
                if let Some(&(_, old)) = other {
                    index.0.retain(|_, v| *v != old);
                    commands.entity(old).despawn();
                }
                let entry = PET_ENTRY_BASE + u32::from(kind);
                names.insert_creature(
                    entry,
                    Some(CreatureRecord {
                        name: def.name.clone(),
                        subname: None,
                        creature_type: 0,
                        pet_family: 0,
                        rank: 0,
                        type_flags: 0,
                        civilian: true,
                        racial_leader: false,
                        display_id: def.display,
                    }),
                );
                let kind_of = UnitKind::Creature { entry };
                let serial = enemy_guid_serial(entry, PET_COUNTER_BASE + u32::from(hero.player));
                let e = units::spawn_unit(
                    &mut commands,
                    &UnitSpec {
                        kind: kind_of,
                        serial,
                        display: def.display,
                        scale: def.scale,
                        level: 1,
                        faction: units::FACTION_FRIENDLY,
                        bytes0: 0,
                        weapons: &[],
                        sheath: 0,
                        extra: &[],
                    },
                    spot,
                    htf.rotation.to_euler(EulerRot::YXZ).0,
                );
                commands.entity(e).insert(PetBody);
                index.0.insert(units::unit_guid(kind_of, serial), e);
                pets.drawn.insert(hero.player, (kind, e));
            }
        }
    }
    // Gone when the hero has no companion, or no body to follow (a hero whose view went would
    // leave it standing frozen until the run ends).
    pets.drawn.retain(|player, (_, e)| {
        let keep = views.heroes.contains_key(player)
            && sim
                .world
                .hero(*player)
                .is_some_and(|h| h.carry.pet.is_some());
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A run ended: its companions went with its entities (and the name cache keeps its few companion
/// records, a handful for the whole session, which is harmless).
pub(super) fn forget(mut pets: ResMut<PetViews>) {
    *pets = PetViews::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_companion_walks_behind_and_beside() {
        let spot = follow_spot(Vec3::ZERO, Quat::IDENTITY);
        assert!(
            (spot.z - BEHIND).abs() < 1e-5,
            "behind is +Z for a hero facing -Z"
        );
        assert!((spot.x - BESIDE).abs() < 1e-5);
    }

    #[test]
    fn a_companion_is_drawn_once_and_gone_with_its_pet_or_its_hero_body() {
        use bevy::ecs::system::RunSystemOnce;

        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let _ = super::super::INN.set(super::super::shipped_inn());
        let mut sim = nightfall_sim::World::new(super::super::content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        sim.heroes[0].carry.pet = Some(0);
        let mut world = World::new();
        let body = world.spawn(Transform::default()).id();
        let mut views = Views::default();
        views.heroes.insert(0, body);
        world.insert_resource(views);
        world.insert_resource(Sim::for_test(sim));
        world.insert_resource(PetViews::default());
        world.init_resource::<NameCache>();
        world.init_resource::<GuidIndex>();
        world.insert_resource(Time::<Real>::default());
        let drawn = |world: &mut World| {
            (
                world.query::<&PetBody>().iter(world).count(),
                world.resource::<PetViews>().drawn.len(),
                world.resource::<GuidIndex>().0.len(),
            )
        };

        assert_eq!(drawn(&mut world), (0, 0, 0));
        world.run_system_once(sync).unwrap();
        assert_eq!(drawn(&mut world), (1, 1, 1), "one companion for the hero");
        world.run_system_once(sync).unwrap();
        assert_eq!(drawn(&mut world), (1, 1, 1), "not drawn again every frame");
        // The stack is gone from the slot's owner (no companion): the animal goes.
        world.resource_mut::<Sim>().world.heroes[0].carry.pet = None;
        world.run_system_once(sync).unwrap();
        assert_eq!(drawn(&mut world), (0, 0, 0));
        // A hero whose body is gone leaves no companion standing frozen where it was.
        world.resource_mut::<Sim>().world.heroes[0].carry.pet = Some(0);
        world.run_system_once(sync).unwrap();
        assert_eq!(drawn(&mut world), (1, 1, 1));
        world.resource_mut::<Views>().heroes.clear();
        world.run_system_once(sync).unwrap();
        assert_eq!(
            drawn(&mut world),
            (0, 0, 0),
            "despawned with the hero's body"
        );
    }

    #[test]
    fn a_companion_kind_the_content_does_not_know_draws_nothing() {
        let inn = super::super::shipped_inn();
        assert!(companion_of(&inn, 0).is_some(), "the wolf");
        assert!(
            companion_of(&inn, 200).is_none(),
            "a client's index, out of range"
        );
        assert!(companion_of(&inn, u8::MAX).is_none());
    }
}
