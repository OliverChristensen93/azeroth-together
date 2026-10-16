//! The world, drawn: a WoW player unit per hero, where the simulation says. The local hero is a
//! real WoW player that WoW's character controller moves ([`super::run::feed_pose`] reports where);
//! every other hero glides from where it was at the last state to where it is now ([`SimPos`]).

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{PlayerId, V2};

use super::run::Sim;
use super::units::{self, ShownVitals, UnitKind};
use super::{content, dress};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::NameCache;
use crate::net::{GuidIndex, ObjectStore, SelfGuid};

/// Player guids are this plus the player's number.
const HERO_SERIAL_BASE: u64 = 0x10;

/// Which entity draws which hero, and the run speed last given our controller.
#[derive(Resource, Default)]
pub(super) struct Views {
    pub heroes: HashMap<PlayerId, Entity>,
    speed: f32,
}

/// Where another hero was at the last two states, for gliding between them.
#[derive(Component)]
pub(super) struct SimPos {
    prev: V2,
    cur: V2,
    facing: V2,
    speed: f32,
}

impl SimPos {
    fn new(at: V2, facing: V2) -> Self {
        Self {
            prev: at,
            cur: at,
            facing,
            speed: 0.0,
        }
    }

    /// A new state `span` seconds after the last.
    fn advance(&mut self, at: V2, facing: V2, span: f32) {
        self.speed = self.cur.distance(at) / span.max(1e-3);
        self.prev = self.cur;
        self.cur = at;
        if facing != V2::ZERO {
            self.facing = facing;
        }
    }
}

/// Every hero gets a body, filed in the [`GuidIndex`] and [`NameCache`] as the net layer files a
/// streamed one; ours becomes the [`SelfGuid`], which hands it to WoW's controller.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    world_point: WorldPoint,
    mut self_guid: ResMut<SelfGuid>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut items: ResMut<crate::items::Items>,
    mut positions: Query<&mut SimPos>,
) {
    let world = &sim.world;
    let content = content();
    let fallback_y = content.map.entries[0][2];
    for hero in &world.heroes {
        if let Some(&e) = views.heroes.get(&hero.player) {
            if sim.stepped {
                if let Ok(mut p) = positions.get_mut(e) {
                    p.advance(hero.pos, hero.facing, sim.span());
                }
            }
            continue;
        }
        // The body and outfit tables load after boot.
        let Some(char_create) = char_create.as_deref() else {
            continue;
        };
        let class = &content.classes[hero.class];
        let serial = HERO_SERIAL_BASE + u64::from(hero.player);
        let guid = units::unit_guid(UnitKind::Player, serial);
        let mut at = units::ground_to_bevy(hero.pos, fallback_y);
        if let Some(h) = world_point.terrain_height_under(at) {
            at.y = h;
        }
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            char_create,
            class,
            (hero.race, hero.sex),
            serial,
            content.rules.hero_level,
            at,
            units::yaw_of(hero.facing),
        ) else {
            continue;
        };
        index.0.insert(guid, e);
        names.insert_player(guid, hero.name.clone(), None);
        if hero.player == sim.me {
            self_guid.0 = Some(guid);
        } else {
            commands.entity(e).insert(SimPos::new(hero.pos, hero.facing));
        }
        views.heroes.insert(hero.player, e);
    }
    // A player who left: their hero goes with them.
    views.heroes.retain(|player, e| {
        let keep = world.hero(*player).is_some();
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// Health and level into every hero's store, for WoW's player, party and target frames; and the
/// local hero's run speed into its controller.
pub(super) fn write_vitals(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    mut units: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let world = &sim.world;
    let level = content().rules.hero_level;
    if let (Some(hero), Some(&e)) = (world.hero(sim.me), views.heroes.get(&sim.me)) {
        if hero.speed != views.speed {
            views.speed = hero.speed;
            commands
                .entity(e)
                .insert(crate::net::UnitSpeeds(run_speeds(hero.speed)));
        }
    }
    for hero in &world.heroes {
        if let Some((mut store, mut shown)) = views
            .heroes
            .get(&hero.player)
            .and_then(|&e| units.get_mut(e).ok())
        {
            units::set_vitals(&mut store, &mut shown, hero.hp, hero.max_hp, level, None);
        }
    }
}

/// WoW's speed set for a hero who runs at `run` yards a second, at the stock ratios.
fn run_speeds(run: f32) -> benilla_protocol::MoveSpeeds {
    benilla_protocol::MoveSpeeds {
        walk: run * 2.5 / 7.0,
        run,
        run_back: run * 4.5 / 7.0,
        swim: run * 4.722_222 / 7.0,
        swim_back: run * 2.5 / 7.0,
        turn_rate: std::f32::consts::PI,
    }
}

/// Every other hero between its last two states, on the ground, turned the way it goes, running
/// when it moves.
pub(super) fn place(
    sim: Res<Sim>,
    world_point: WorldPoint,
    time: Res<Time<Real>>,
    mut bodies: Query<(&SimPos, &mut Transform, &mut MovementState)>,
) {
    let alpha = sim.alpha();
    let turn = (time.delta_secs() * 12.0).min(1.0);
    for (p, mut tf, mut mv) in &mut bodies {
        let at = p.prev + (p.cur - p.prev) * alpha;
        let mut pos = units::ground_to_bevy(at, tf.translation.y);
        if let Some(h) = world_point.terrain_height_under(pos) {
            pos.y = h;
        }
        if tf.translation != pos {
            tf.translation = pos;
        }
        if p.facing != V2::ZERO {
            let target = Quat::from_rotation_y(units::yaw_of(p.facing));
            tf.rotation = tf.rotation.slerp(target, turn);
        }
        let flags = if p.speed > 0.1 { move_flags::FORWARD } else { 0 };
        if mv.speed != p.speed || mv.flags != flags {
            mv.speed = p.speed;
            mv.flags = flags;
        }
    }
}
