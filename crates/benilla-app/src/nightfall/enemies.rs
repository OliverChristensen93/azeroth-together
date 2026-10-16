//! Nightfall's enemies, drawn: a WoW creature for each enemy near the local hero, gliding between
//! the host's states on the floor the host says it stands on (a house's upper storey included),
//! walking or standing as it moves. It draws what the world holds and decides nothing; which of
//! them the hero sees ([`Sim::seen`]) is what sub-project 3 hides the rest by.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{V2, V3};

use super::content;
use super::run::Sim;
use super::units::{self, ShownVitals, UnitKind, UnitSpec};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::{CreatureRecord, NameCache};
use crate::net::{GuidIndex, ObjectStore};

/// Yards past the view distance an enemy already drawn stays drawn, so one at the edge does not
/// blink in and out.
const KEEP_EXTRA: f32 = 15.0;
/// Within this many yards of the client's own terrain height the host's floor is taken to be the
/// terrain: the client's is exact, the bake's columns are not.
const ON_TERRAIN: f32 = 1.0;

/// Which entity draws which enemy, and the creature entries already named.
#[derive(Resource, Default)]
pub(super) struct EnemyViews {
    pub drawn: HashMap<u32, Entity>,
    named: HashSet<u32>,
}

/// Where an enemy was at the last two states, for gliding between them.
#[derive(Component)]
pub(super) struct EnemyPos {
    prev: V3,
    cur: V3,
    facing: V2,
    /// The host's speed for it, and its kind's walking speed, which pick its gait.
    speed: f32,
    walk: f32,
}

impl EnemyPos {
    fn new(at: V3, facing: V2, speed: f32, walk: f32) -> Self {
        Self {
            prev: at,
            cur: at,
            facing,
            speed,
            walk,
        }
    }

    /// A new state.
    fn advance(&mut self, at: V3, facing: V2, speed: f32) {
        self.prev = self.cur;
        self.cur = at;
        if facing != V2::ZERO {
            self.facing = facing;
        }
        self.speed = speed;
    }

    /// Where it is `alpha` (0..1) of the way from the last state to this one.
    fn at(&self, alpha: f32) -> V3 {
        V3::new(
            self.prev.x + (self.cur.x - self.prev.x) * alpha,
            self.prev.y + (self.cur.y - self.prev.y) * alpha,
            self.prev.z + (self.cur.z - self.prev.z) * alpha,
        )
    }
}

/// Whether an enemy `distance` yards from the local hero is drawn: within the view distance to
/// appear, and [`KEEP_EXTRA`] beyond it to go.
pub(super) fn keep_drawn(distance: f32, drawn: bool, view: f32) -> bool {
    distance <= if drawn { view + KEEP_EXTRA } else { view }
}

/// The height to draw an enemy at: the client's own terrain when the host's floor is that
/// terrain, else the host's floor (an upper storey, a bridge).
pub(super) fn draw_height(sim_z: f32, terrain: Option<f32>) -> f32 {
    match terrain {
        Some(t) if (t - sim_z).abs() <= ON_TERRAIN => t,
        _ => sim_z,
    }
}

/// The movement flags for an enemy moving at `speed` whose kind walks at `walk`: standing,
/// walking (the walk animation) or running.
pub(super) fn gait_flags(speed: f32, walk: f32) -> u32 {
    if speed <= 0.05 {
        0
    } else if speed <= walk + 0.5 {
        move_flags::FORWARD | move_flags::WALK_MODE
    } else {
        move_flags::FORWARD
    }
}

/// The creature entry of an enemy kind: its index plus one, so the name cache files each apart.
fn entry_of(kind: usize) -> u32 {
    kind as u32 + 1
}

fn trace() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_TRACE_SEEN").is_some())
}

/// Every enemy near the local hero gets a creature body, filed in the [`GuidIndex`] and
/// [`NameCache`] as the net layer files a streamed one, so Tab, the target frame and nameplates
/// treat it as WoW's own; one that moves out of range, or out of the world, goes.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<EnemyViews>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut positions: Query<&mut EnemyPos>,
) {
    let world = &sim.world;
    let content = content();
    let view = content.rules.atmosphere.view_distance;
    let me = world.hero(sim.me).map(|h| h.pos);
    for enemy in &world.enemies {
        let drawn = views.drawn.get(&enemy.id).copied();
        let distance = me.map_or(f32::INFINITY, |m| m.distance(enemy.pos));
        if !keep_drawn(distance, drawn.is_some(), view) {
            if let Some(e) = drawn {
                index.0.retain(|_, v| *v != e);
                commands.entity(e).despawn();
                views.drawn.remove(&enemy.id);
            }
            continue;
        }
        let at = V3::at(enemy.pos, enemy.z);
        if let Some(e) = drawn {
            if sim.stepped {
                if let Ok(mut p) = positions.get_mut(e) {
                    p.advance(at, enemy.facing, enemy.speed);
                }
            }
            continue;
        }
        let def = &content.enemies[enemy.kind];
        let entry = entry_of(enemy.kind);
        if views.named.insert(entry) {
            names.insert_creature(
                entry,
                Some(CreatureRecord {
                    name: def.name.clone(),
                    subname: None,
                    creature_type: 0,
                    pet_family: 0,
                    rank: 0,
                    type_flags: 0,
                    civilian: false,
                    racial_leader: false,
                    display_id: def.display,
                }),
            );
        }
        let kind = UnitKind::Creature { entry };
        // A creature guid carries its template entry in bits 24..48, as the server's do: lookups
        // that have only the guid (the live-shot gate, a name query) find the name by it.
        let serial = u64::from(entry) << 24 | u64::from(enemy.id);
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                kind,
                serial,
                display: def.display,
                scale: def.scale,
                level: def.level,
                faction: units::FACTION_MONSTER,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
                extra: &[],
            },
            units::wow_to_bevy([at.x, at.y, at.z]),
            units::yaw_of(enemy.facing),
        );
        commands
            .entity(e)
            .insert(EnemyPos::new(at, enemy.facing, enemy.speed, def.walk_speed));
        index.0.insert(units::unit_guid(kind, serial), e);
        views.drawn.insert(enemy.id, e);
    }
    // Gone from the world (a client's snapshot left it out, a new run): gone from the screen.
    let present: HashSet<u32> = world.enemies.iter().map(|e| e.id).collect();
    views.drawn.retain(|id, e| {
        let keep = present.contains(id);
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// Health and level into every drawn enemy's store, for WoW's target frame.
pub(super) fn vitals(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut bodies: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let content = content();
    for enemy in &sim.world.enemies {
        if let Some((mut store, mut shown)) = views
            .drawn
            .get(&enemy.id)
            .and_then(|&e| bodies.get_mut(e).ok())
        {
            let level = content.enemies[enemy.kind].level;
            units::set_vitals(&mut store, &mut shown, enemy.hp, enemy.max_hp, level, None);
        }
    }
}

/// Every drawn enemy between its last two states, at its height, turned the way it goes,
/// walking when it moves.
pub(super) fn place(
    sim: Res<Sim>,
    world_point: WorldPoint,
    time: Res<Time<Real>>,
    mut bodies: Query<(&EnemyPos, &mut Transform, &mut MovementState)>,
) {
    let alpha = sim.alpha();
    let turn = (time.delta_secs() * 12.0).min(1.0);
    for (p, mut tf, mut mv) in &mut bodies {
        let at = p.at(alpha);
        let mut pos = units::wow_to_bevy([at.x, at.y, at.z]);
        pos.y = draw_height(at.z, world_point.terrain_height_under(pos));
        if tf.translation != pos {
            tf.translation = pos;
        }
        if p.facing != V2::ZERO {
            let target = Quat::from_rotation_y(units::yaw_of(p.facing));
            tf.rotation = tf.rotation.slerp(target, turn);
        }
        let flags = gait_flags(p.speed, p.walk);
        if mv.speed != p.speed || mv.flags != flags {
            mv.speed = p.speed;
            mv.flags = flags;
        }
    }
}

/// `NIGHTFALL_TRACE_SEEN=1`: every 2 seconds, how many of the enemies within the fog our hero
/// sees.
pub(super) fn trace_seen(sim: Res<Sim>, real: Res<Time<Real>>, mut last: Local<f32>) {
    if !trace() || real.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = real.elapsed_secs();
    let Some(me) = sim.world.hero(sim.me) else {
        return;
    };
    let fog = content().rules.atmosphere.fog_end;
    let near = sim
        .world
        .enemies
        .iter()
        .filter(|e| e.pos.distance(me.pos) <= fog)
        .count();
    info!(
        "nightfall: seeing {} of the {near} enemies within {fog} yd",
        sim.seen.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enemy_appears_inside_the_view_and_goes_a_little_past_it() {
        assert!(keep_drawn(100.0, false, 120.0));
        assert!(!keep_drawn(125.0, false, 120.0));
        assert!(keep_drawn(125.0, true, 120.0));
        assert!(!keep_drawn(140.0, true, 120.0));
    }

    #[test]
    fn on_the_ground_the_clients_terrain_wins_and_upstairs_the_hosts_floor() {
        assert_eq!(draw_height(36.4, Some(36.6)), 36.6);
        assert_eq!(draw_height(40.0, Some(36.6)), 40.0);
        assert_eq!(draw_height(40.0, None), 40.0);
    }

    #[test]
    fn standing_walking_and_running_have_their_gaits() {
        assert_eq!(gait_flags(0.0, 2.5), 0);
        assert_eq!(gait_flags(2.5, 2.5), move_flags::FORWARD | move_flags::WALK_MODE);
        assert_eq!(gait_flags(7.0, 2.5), move_flags::FORWARD);
    }

    #[test]
    fn gliding_goes_from_the_last_state_to_this_one() {
        let mut p = EnemyPos::new(V3::new(0.0, 0.0, 1.0), V2::new(1.0, 0.0), 0.0, 2.5);
        p.advance(V3::new(2.0, 4.0, 3.0), V2::ZERO, 2.5);
        assert_eq!(p.at(0.5), V3::new(1.0, 2.0, 2.0));
        assert_eq!(p.facing, V2::new(1.0, 0.0));
        assert_eq!(entry_of(0), 1);
    }
}
