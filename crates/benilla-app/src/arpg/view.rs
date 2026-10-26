//! The world drawn: a body per hero and enemy, moved between ticks; the sim's events played as
//! WoW's own spell visuals, swings and numbers.
//!
//! A missile is the engine's own: it flies from the caster at an invisible marker standing where
//! the sim's missile will run out of range, at the spell's Spell.dbc speed (the content gives the
//! sim the same speed). When the sim's missile strikes something first, the marker goes and the
//! engine's missile ends with it, and the impact plays on what was struck.

use std::collections::HashMap;

use arpg_sim::{Event, Owner, PlayerId, School, Shape, Target, World, V2};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::units::{self, Fx, Marks, UnitSpec, Weapon};
use super::{Pending, RunEntity, Sim, ME};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

/// The mage's look: class-trainer `CreatureDisplayInfo` ids; the first that resolves to a human
/// is worn, else the first that resolves at all. `ARPG_LOOK=<n>` picks the n-th.
const MAGE_LOOKS: &[u32] = &[
    1294, 1470, 1484, 1592, 1600, 2134, 2135, 2644, 2657, 2810, 3108, 3109, 3292, 3293, 4522,
    4523, 4524, 4526, 4665, 5001, 6058, 6060, 6072, 7669, 10171, 10214, 10215, 10216, 10548,
    10733, 12849,
];
/// A staff in the main hand.
const MAGE_WEAPONS: &[Weapon] = &[(0, 1927, 2, 10, 2, 17, 2)];
/// `ChrClasses` id and power type (mana).
const MAGE_CLASS: u8 = 8;
/// Seconds a corpse lies before it goes.
const CORPSE_SECS: f32 = 4.0;
/// How high above the ground missiles aim (chest height).
const CHEST: f32 = 1.2;
/// Guid serials: heroes, then everything else counting up.
const HERO_SERIAL: u64 = 0x10;
const FIRST_SERIAL: u64 = 0x1000;

/// A hero's body.
#[derive(Component)]
pub(super) struct HeroBody;

/// An enemy's body, by sim id.
#[derive(Component)]
pub(super) struct EnemyBody;

/// The health last written to a body's store.
#[derive(Component, Default)]
pub(super) struct Shown([u32; 2]);

/// Despawns when its seconds run out: a corpse, a missile marker that saw its missile land.
#[derive(Component)]
pub(super) struct Fade(f32);

/// Where every hero, enemy and missile was a tick ago and now, and which body draws each.
#[derive(Resource, Default)]
pub(super) struct Views {
    heroes: HashMap<PlayerId, Entity>,
    enemies: HashMap<u32, Entity>,
    /// A missile's marker and visual, by sim id.
    markers: HashMap<u32, (Entity, u32)>,
    /// The spell a hero is casting or channelling, to reap its precast if it is stopped.
    casting: HashMap<PlayerId, u32>,
    /// An area's dynamic object, by sim id.
    areas: HashMap<u32, Entity>,
    prev_heroes: HashMap<PlayerId, V2>,
    prev_enemies: HashMap<u32, V2>,
    serial: u64,
}

impl Views {
    /// Notes where everything stands before a tick, so the frame can draw between the two.
    pub fn remember(&mut self, world: &World) {
        self.prev_heroes = world.heroes().iter().map(|h| (h.player, h.pos)).collect();
        self.prev_enemies = world.enemies().iter().map(|e| (e.id, e.pos)).collect();
    }

    fn serial(&mut self) -> u64 {
        self.serial = self.serial.max(FIRST_SERIAL) + 1;
        self.serial
    }

    fn body(&self, target: Target) -> Option<Entity> {
        match target {
            Target::Hero(p) => self.heroes.get(&p).copied(),
            Target::Enemy(id) => self.enemies.get(&id).copied(),
        }
    }
}

/// A sim point on the ground plane at height `y`, in Bevy's frame.
pub(super) fn ground(p: V2, y: f32) -> Vec3 {
    units::wow_to_bevy([p.x, p.y, y])
}

/// A Bevy point's place on the sim's ground plane.
pub(super) fn to_ground(p: Vec3) -> V2 {
    V2::new(-p.z, -p.x)
}

/// The Bevy yaw that faces a model along a sim direction.
fn yaw_of(dir: V2) -> f32 {
    units::yaw_toward(ground(dir, 0.0))
}

/// The ground at `p`, or the zone's start height where the terrain is not in yet.
fn floor(world: &WorldPoint, p: V2, fallback: f32) -> Vec3 {
    let mut at = ground(p, fallback);
    if let Some(h) = world.terrain_height_under(at + Vec3::Y * 50.0) {
        at.y = h;
    }
    at
}

fn fallback_y(world: &World) -> f32 {
    world.content().zones[world.zone()].start.2
}

/// Spawns a body for every hero and enemy that has none yet.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    creatures: Option<Res<crate::entities::Creatures>>,
    terrain: WorldPoint,
) {
    let world = &sim.world;
    let fy = fallback_y(world);
    for hero in world.heroes() {
        if views.heroes.contains_key(&hero.player) {
            continue;
        }
        let Some(creatures) = creatures.as_deref() else {
            continue;
        };
        let pick = std::env::var("ARPG_LOOK")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let looks: Vec<(u32, u8, u8)> = MAGE_LOOKS
            .iter()
            .filter_map(|&d| creatures.display_race_sex(d).map(|(r, s)| (d, r, s)))
            .collect();
        let look = match pick {
            Some(i) => looks.get(i % looks.len().max(1)).copied(),
            None => looks
                .iter()
                .find(|l| l.1 == 1)
                .or(looks.first())
                .copied(),
        };
        // The creature data is not in yet.
        let Some((display, race, sex)) = look else {
            continue;
        };
        let bytes0 = u32::from(race) | u32::from(MAGE_CLASS) << 8 | u32::from(sex) << 16;
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: HERO_SERIAL + u64::from(hero.player),
                display,
                scale: 1.0,
                level: 60,
                faction: units::FACTION_FRIENDLY,
                bytes0,
                weapons: MAGE_WEAPONS,
                sheath: 1,
            },
            floor(&terrain, hero.pos, fy),
            yaw_of(hero.facing),
        );
        commands.entity(e).insert((
            HeroBody,
            Shown::default(),
            Marks::buffs(),
        ));
        views.heroes.insert(hero.player, e);
    }
    let fresh: Vec<_> = world
        .enemies()
        .iter()
        .filter(|e| !views.enemies.contains_key(&e.id))
        .map(|e| (e.id, e.kind, e.pos, e.facing))
        .collect();
    for (id, kind, pos, facing) in fresh {
        let def = &world.content().enemies[kind];
        let serial = views.serial();
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial,
                display: def.display,
                scale: def.scale,
                level: u32::from(def.level),
                faction: units::FACTION_MONSTER,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
            },
            floor(&terrain, pos, fy),
            yaw_of(facing),
        );
        commands
            .entity(e)
            .insert((EnemyBody, Shown::default(), Marks::debuffs()));
        views.enemies.insert(id, e);
    }
}

/// The step's events, as WoW would show them.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_events(
    mut commands: Commands,
    sim: Res<Sim>,
    mut pending: ResMut<Pending>,
    mut views: ResMut<Views>,
    mut stores: Query<&mut ObjectStore>,
    terrain: WorldPoint,
    mut fx: Fx,
) {
    let world = &sim.world;
    let fy = fallback_y(world);
    for event in pending.0.drain(..) {
        match event {
            Event::CastStart { hero, ability, .. } => {
                let visual = world.content().abilities[ability].visual;
                if let Some(&e) = views.heroes.get(&hero) {
                    fx.start(e, visual);
                }
                views.casting.insert(hero, visual);
            }
            Event::CastStopped { hero } => {
                let visual = views.casting.remove(&hero);
                if let (Some(&e), Some(visual)) = (views.heroes.get(&hero), visual) {
                    fx.fail(e, visual);
                }
            }
            Event::Cast { hero, ability, at } => {
                let Some(&e) = views.heroes.get(&hero) else {
                    continue;
                };
                let def = &world.content().abilities[ability];
                if def.channel > 0.0 {
                    views.casting.insert(hero, def.visual);
                } else {
                    views.casting.remove(&hero);
                }
                match def.shape {
                    // Its missiles are launched one by one below.
                    Shape::Projectile { .. } => {}
                    Shape::Ground { .. } => {
                        fx.cast(e, def.visual, &[], Some(floor(&terrain, at, fy)));
                    }
                    // Their cast kits are the blast; the ice and chill show as auras.
                    Shape::Nova { .. } | Shape::Cone { .. } => fx.cast(e, def.visual, &[], None),
                    Shape::Guard { .. } | Shape::Blink => fx.cast(e, def.visual, &[e], None),
                }
            }
            Event::MissileLaunched { id } => {
                let Some(m) = world.projectiles().iter().find(|m| m.id == id) else {
                    continue;
                };
                let caster = match m.owner {
                    Owner::Hero(p) => views.heroes.get(&p).copied(),
                    Owner::Enemy(eid) => views.enemies.get(&eid).copied(),
                };
                let Some(caster) = caster else { continue };
                let end = m.pos + m.vel.normalize_or_zero() * m.range_left;
                let at = floor(&terrain, end, fy) + Vec3::Y * CHEST;
                let marker = commands
                    .spawn((
                        Transform::from_translation(at),
                        GlobalTransform::from_translation(at),
                        RunEntity,
                    ))
                    .id();
                views.markers.insert(id, (marker, m.visual));
                fx.cast(caster, m.visual, &[marker], None);
            }
            Event::MissileGone { id, at, struck } => {
                let Some((marker, visual)) = views.markers.remove(&id) else {
                    continue;
                };
                if struck {
                    commands.entity(marker).despawn();
                    // The impact on whatever stands nearest where it struck.
                    if let Some(body) = nearest_body(&views, world, at) {
                        fx.impact(body, visual);
                    }
                } else {
                    // Left for the engine's missile to arrive at, then gone.
                    commands.entity(marker).insert(Fade(1.5));
                }
            }
            Event::AreaStarted { id } => {
                let Some(a) = world.areas().iter().find(|a| a.id == id) else {
                    continue;
                };
                let visual = world.content().abilities[a.ability].visual;
                let serial = views.serial();
                let at = floor(&terrain, a.pos, fy);
                let obj = units::spawn_area_object(&mut commands, serial, visual, a.radius, at);
                views.areas.insert(id, obj);
            }
            Event::AreaEnded { id } => {
                if let Some(obj) = views.areas.remove(&id) {
                    commands.entity(obj).despawn();
                }
            }
            Event::Damage {
                target,
                amount,
                crit,
                school,
            } => {
                let Some(body) = views.body(target) else {
                    continue;
                };
                let color = match target {
                    Target::Hero(_) => units::COLOR_HURT,
                    Target::Enemy(_) => school_color(school),
                };
                fx.number(body, amount, crit, color);
            }
            Event::Absorbed { hero, .. } => {
                if let Some(&e) = views.heroes.get(&hero) {
                    fx.text(e, "Absorb", units::COLOR_SPELL);
                }
            }
            Event::Blinked { hero, .. } => {
                // No sliding across the jump.
                views.prev_heroes.remove(&hero);
            }
            Event::EnemyStrike { id } => {
                let (Some(&e), Some(&me)) = (views.enemies.get(&id), views.heroes.get(&ME)) else {
                    continue;
                };
                let ranged = world.enemies().iter().find(|x| x.id == id).is_some_and(|x| {
                    world.content().enemies[x.kind].behaviour == arpg_sim::Behaviour::Ranged
                });
                // A ranged throw's missile is its own event; a blow swings, landed or not.
                if !ranged {
                    fx.swing(e, me, 0.0);
                }
            }
            Event::EnemyDied { id, .. } => {
                if let Some(e) = views.enemies.get(&id).copied() {
                    if let Some(visual) = caster_visual(world, id) {
                        fx.fail(e, visual);
                    }
                }
                if let Some(e) = views.enemies.remove(&id) {
                    if let Ok(mut store) = stores.get_mut(e) {
                        units::kill_unit(&mut store);
                    }
                    commands
                        .entity(e)
                        .remove::<EnemyBody>()
                        .insert(Fade(CORPSE_SECS));
                }
            }
            Event::HeroDied { hero } => {
                if let Some(&e) = views.heroes.get(&hero) {
                    if let Ok(mut store) = stores.get_mut(e) {
                        units::kill_unit(&mut store);
                    }
                }
            }
            Event::Restarted => {
                // The enemies keep their bodies and stand home again.
                for (_, (e, _)) in views.markers.drain() {
                    commands.entity(e).despawn();
                }
                for (_, e) in views.areas.drain() {
                    commands.entity(e).despawn();
                }
                // Heroes get a fresh body, standing.
                for (_, e) in views.heroes.drain() {
                    commands.entity(e).despawn();
                }
                views.prev_heroes.clear();
                views.prev_enemies.clear();
                views.casting.clear();
            }
            Event::EnemyEvade { id } => {
                if let Some(&e) = views.enemies.get(&id) {
                    fx.text(e, "Evade", units::COLOR_MELEE);
                }
            }
            Event::EnemyWindup { id } => {
                // A caster's windup is its cast: the hands glow until the bolt flies.
                let Some(&e) = views.enemies.get(&id) else {
                    continue;
                };
                let def = world
                    .enemies()
                    .iter()
                    .find(|x| x.id == id)
                    .map(|x| &world.content().enemies[x.kind]);
                if let Some(def) = def.filter(|d| d.behaviour == arpg_sim::Behaviour::Ranged) {
                    fx.start(e, def.missile_visual);
                }
            }
            // A freeze breaks off a caster's windup.
            Event::Frozen { enemy } => {
                if let (Some(&e), Some(visual)) =
                    (views.enemies.get(&enemy), caster_visual(world, enemy))
                {
                    fx.fail(e, visual);
                }
            }
            Event::EnemySpawned { .. }
            | Event::EnemyAggro { .. }
            | Event::Dodged { .. } => {}
        }
    }
}

/// A caster enemy's missile visual, if `id` is a caster still in the world.
fn caster_visual(world: &World, id: u32) -> Option<u32> {
    let enemy = world.enemies().iter().find(|x| x.id == id)?;
    let def = &world.content().enemies[enemy.kind];
    (def.behaviour == arpg_sim::Behaviour::Ranged).then_some(def.missile_visual)
}

fn nearest_body(views: &Views, world: &World, at: V2) -> Option<Entity> {
    let enemy = world
        .enemies()
        .iter()
        .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
        .filter(|e| e.pos.distance(at) < 4.0)
        .and_then(|e| views.enemies.get(&e.id).copied());
    let hero = world
        .heroes()
        .iter()
        .find(|h| h.pos.distance(at) < 2.0)
        .and_then(|h| views.heroes.get(&h.player).copied());
    enemy.or(hero)
}

fn school_color(school: School) -> u32 {
    match school {
        School::Frost => 0xFF80_C8FF,
        School::Fire => 0xFFFF_8020,
        School::Arcane => 0xFFE0_80FF,
        School::Nature => 0xFF60_FF60,
        School::Shadow => 0xFFA0_60E0,
        School::Holy => 0xFFFF_F0A0,
        School::Physical => units::COLOR_MELEE,
    }
}

/// Every body where the sim has it, drawn between the last two ticks, on the ground, walking
/// when it moved.
pub(super) fn place(
    sim: Res<Sim>,
    views: Res<Views>,
    terrain: WorldPoint,
    time: Res<Time<Real>>,
    mut bodies: Query<(&mut Transform, &mut MovementState)>,
) {
    let world = &sim.world;
    let alpha = sim.alpha();
    let fy = fallback_y(world);
    let dt = time.delta_secs();
    let mut put = |e: Entity, prev: Option<V2>, cur: V2, facing: V2, speed: f32, gait: Option<u32>| {
        let Ok((mut tf, mut mv)) = bodies.get_mut(e) else {
            return;
        };
        let p = prev.map_or(cur, |p| p + (cur - p) * alpha);
        let at = floor(&terrain, p, fy);
        tf.translation = at;
        let want = Quat::from_rotation_y(yaw_of(facing));
        tf.rotation = tf.rotation.slerp(want, (dt * 18.0).min(1.0));
        let (s, f) = gait.map_or((0.0, 0), |f| (speed, f));
        if mv.speed != s || mv.flags != f {
            mv.speed = s;
            mv.flags = f;
        }
    };
    for h in world.heroes() {
        let Some(&e) = views.heroes.get(&h.player) else {
            continue;
        };
        let prev = views.prev_heroes.get(&h.player).copied();
        let moved = prev.is_some_and(|p| p.distance(h.pos) > 1e-3);
        let class = &world.content().classes[h.class];
        // A roll runs the legs at the roll's speed.
        let speed = if h.is_rolling() {
            class.dodge.distance / class.dodge.time.max(0.01)
        } else {
            class.speed
        };
        let dir = prev.map_or(h.facing, |p| (h.pos - p).normalize_or_zero());
        // The hero faces the aim, and runs forward, back or sideways under it.
        let walk = (moved && h.alive).then(|| gait(h.facing, dir));
        put(e, prev, h.pos, h.facing, speed, walk);
    }
    for en in world.enemies() {
        let Some(&e) = views.enemies.get(&en.id) else {
            continue;
        };
        let prev = views.prev_enemies.get(&en.id).copied();
        let moved = prev.is_some_and(|p| p.distance(en.pos) > 1e-3);
        let speed = world.content().enemies[en.kind].speed;
        put(e, prev, en.pos, en.facing, speed, moved.then_some(move_flags::FORWARD));
    }
}

/// Forward, backpedal or strafe, by which quarter `dir` falls in from `facing`.
fn gait(facing: V2, dir: V2) -> u32 {
    let ahead = facing.dot(dir);
    // WoW's +y is west: a run to the facing's left has a positive cross product.
    let left = facing.x * dir.y - facing.y * dir.x;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

/// Health into each body's store, and what holds it shown as auras: Chill, Freeze and burns on
/// enemies, Ice Block and Mana Shield on heroes.
pub(super) fn statuses(
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(Entity, &Guid, &mut ObjectStore, &mut Shown, &mut Marks)>,
    mut fields: MessageWriter<FieldChanged>,
) {
    let world = &sim.world;
    let now = world.time();
    let visual = |key: &str| {
        world
            .content()
            .ability(key)
            .map(|a| world.content().abilities[a].visual)
    };
    let (frostbolt, nova, fireball) = (visual("frostbolt"), visual("frost_nova"), visual("fireball"));
    for en in world.enemies() {
        let Some(&e) = views.enemies.get(&en.id) else {
            continue;
        };
        let Ok((entity, guid, mut store, mut shown, mut marks)) = bodies.get_mut(e) else {
            continue;
        };
        units::set_health(&mut store, &mut shown.0, en.hp, en.max_hp);
        let mut on = Vec::new();
        if en.is_frozen(now) {
            on.extend(nova);
        } else if en.is_chilled(now) {
            on.extend(frostbolt);
        }
        if en.burn.is_some() {
            on.extend(fireball);
        }
        marks.set(&on);
        marks.flush(entity, guid.0, &mut store, &mut fields);
    }
    for h in world.heroes() {
        let Some(&e) = views.heroes.get(&h.player) else {
            continue;
        };
        let Ok((entity, guid, mut store, mut shown, mut marks)) = bodies.get_mut(e) else {
            continue;
        };
        units::set_health(&mut store, &mut shown.0, h.hp, h.max_hp);
        let mut on = Vec::new();
        if now < h.guard_until {
            if h.immune {
                on.extend(visual("ice_block"));
            } else if h.absorb > 0.0 {
                on.extend(visual("mana_shield"));
            }
        }
        marks.set(&on);
        marks.flush(entity, guid.0, &mut store, &mut fields);
    }
}

/// Corpses and spent markers go when their time is up.
pub(super) fn reap(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut fading: Query<(Entity, &mut Fade)>,
) {
    let dt = time.delta_secs();
    for (e, mut fade) in &mut fading {
        fade.0 -= dt;
        if fade.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
