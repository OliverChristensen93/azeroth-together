//! The ravens on screen (spec §7.1): each settled flock near our hero drawn as a few birds on the
//! ground, gone with a cry from where they sat when the flock takes off. The birds are a stand-in
//! look until a crow's display is found in the data (measure: rules.ron's night.ravens.display);
//! they are `Concealed` and untrackable, so neither Tab, the mouse, a nameplate nor the
//! minimap's tracking finds them. Which flocks are out and sitting is the sim's
//! ([`nightfall_sim::NightView::flocks`], mirrored on a joined client), so every player sees the
//! same birds, and a take-off's cry is an event every player is sent.

use std::collections::HashMap;

use benilla_protocol::field::FIELD_UNIT_BYTES_1;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{Event, FLOCK_SETTLED, V2};

use super::conceal::UNTRACKABLE_BYTES_1;
use super::content;
use super::enemies::enemy_guid_serial;
use super::night::cue_of;
use super::run::Sim;
use super::units::{self, UnitKind, UnitSpec};
use crate::names::{CreatureRecord, NameCache};
use crate::sound::SoundCue;
use crate::target::Concealed;

/// The creature entry birds are filed under in the name cache: past every enemy kind's.
const RAVEN_ENTRY: u32 = 60_000;
/// How far around a flock's point its birds sit, yards.
const SPREAD: f32 = 1.2;
/// Birds a flock may hold, so each bird's guid counter (`flock * 16 + bird`) stays distinct.
const MAX_BIRDS: usize = 16;
/// Where above the hero's floor a bird's ground is looked for, yards.
const LOOK_FROM: f32 = 50.0;

/// The birds drawn, by flock.
#[derive(Resource, Default)]
pub(super) struct Flocks {
    drawn: HashMap<u8, Vec<Entity>>,
    named: bool,
}

/// Where `n` birds of a flock at `at` sit: on a small circle round it, the same every time.
pub(super) fn bird_spots(at: V2, n: usize) -> Vec<V2> {
    (0..n)
        .map(|i| {
            at + V2::from_angle(
                0.7 + i as f32 * std::f32::consts::TAU / n.max(1) as f32,
                SPREAD,
            )
        })
        .collect()
}

/// The id a bird goes by in its guid's low 24 bits: unique per flock and bird, and never an
/// enemy's (the entry above it is [`RAVEN_ENTRY`], which no enemy kind has).
fn bird_id(flock: u8, bird: usize) -> u32 {
    u32::from(flock) * MAX_BIRDS as u32 + bird as u32
}

/// Each settled flock within the view distance of our hero has its birds; one that took off, or
/// fell out of view, has none. A bird waits for the ground under it (the terrain streams in), so
/// none is drawn in the air.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut flocks: ResMut<Flocks>,
    mut names: ResMut<NameCache>,
    world_point: WorldPoint,
) {
    let content = content();
    let r = &content.rules.night.ravens;
    if !flocks.named {
        names.insert_creature(
            RAVEN_ENTRY,
            Some(CreatureRecord {
                name: "Raven".into(),
                subname: None,
                creature_type: 0,
                pet_family: 0,
                rank: 0,
                type_flags: 0,
                civilian: true,
                racial_leader: false,
                display_id: r.display,
            }),
        );
        flocks.named = true;
    }
    let view = content.rules.atmosphere.view_distance;
    let me = sim.world.hero(sim.me).map(|h| (h.pos, h.z));
    let states = sim.world.night_view().flocks;
    let birds = usize::from(r.birds).min(MAX_BIRDS);
    for (k, &at) in content.map.flocks.iter().enumerate() {
        let key = k as u8;
        let want = me.is_some_and(|(m, _)| m.distance(at) <= view)
            && states.get(k) == Some(&FLOCK_SETTLED);
        let drawn = flocks.drawn.contains_key(&key);
        if want && !drawn {
            let z = me.map_or(0.0, |(_, z)| z);
            let spots = bird_spots(at, birds);
            let heights: Vec<Option<f32>> = spots
                .iter()
                .map(|&p| world_point.terrain_height_under(units::ground_to_bevy(p, z + LOOK_FROM)))
                .collect();
            if heights.iter().any(Option::is_none) {
                continue;
            }
            let bodies: Vec<Entity> = spots
                .iter()
                .zip(heights)
                .enumerate()
                .map(|(b, (&p, y))| {
                    let pos = units::ground_to_bevy(p, y.unwrap_or(z));
                    let e = units::spawn_unit(
                        &mut commands,
                        &UnitSpec {
                            kind: UnitKind::Creature { entry: RAVEN_ENTRY },
                            serial: enemy_guid_serial(RAVEN_ENTRY, bird_id(key, b)),
                            display: r.display,
                            scale: r.scale,
                            level: 1,
                            faction: units::FACTION_FRIENDLY,
                            bytes0: 1 << 8,
                            weapons: &[],
                            sheath: 0,
                            extra: &[(FIELD_UNIT_BYTES_1, UNTRACKABLE_BYTES_1)],
                        },
                        pos,
                        units::yaw_of(p - at),
                    );
                    commands.entity(e).insert(Concealed);
                    e
                })
                .collect();
            flocks.drawn.insert(key, bodies);
        } else if !want && drawn {
            for e in flocks.drawn.remove(&key).unwrap_or_default() {
                commands.entity(e).despawn();
            }
        }
    }
}

/// A flock taking off cries from where it sat.
pub(super) fn cries(sim: Res<Sim>, mut sounds: MessageWriter<SoundCue>) {
    let content = content();
    let Some(cry) = &content.rules.night.ravens.cry else {
        return;
    };
    let z = sim.world.hero(sim.me).map_or(0.0, |h| h.z);
    for e in &sim.events {
        if let Event::RavensFlew { flock } = e {
            if let Some(&at) = content.map.flocks.get(usize::from(*flock)) {
                sounds.write(SoundCue {
                    sound: cue_of(cry),
                    at: Some(units::ground_to_bevy(at, z)),
                });
            }
        }
    }
}

/// A run's end: the birds went with its entities, so the book of them starts over.
pub(super) fn forget(mut flocks: ResMut<Flocks>) {
    flocks.drawn.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birds_sit_round_their_flocks_point() {
        let at = V2::new(10.0, 20.0);
        let spots = bird_spots(at, 3);
        assert_eq!(spots.len(), 3);
        assert!(spots.iter().all(|p| (p.distance(at) - SPREAD).abs() < 1e-4));
        assert_ne!(spots[0], spots[1]);
        assert_eq!(bird_spots(at, 3), spots);
        assert!(bird_spots(at, 0).is_empty());
    }

    #[test]
    fn a_birds_guid_names_the_raven_and_each_bird_is_its_own() {
        let guid = |f, b| enemy_guid_serial(RAVEN_ENTRY, bird_id(f, b));
        // The entry sits in bits 24..48, where the name lookup by guid reads it.
        assert_eq!((guid(2, 1) >> 24) as u32, RAVEN_ENTRY);
        // Every bird of every flock is its own.
        let mut all: Vec<u64> = (0..8u8)
            .flat_map(|f| (0..MAX_BIRDS).map(move |b| guid(f, b)))
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 8 * MAX_BIRDS);
    }

    #[test]
    fn a_ravens_bytes_say_untrackable_and_no_more() {
        // Byte 3 of `UNIT_FIELD_BYTES_1` is the visibility byte; bit 4 is "untrackable".
        assert_eq!(UNTRACKABLE_BYTES_1, 0x0400_0000);
    }
}
