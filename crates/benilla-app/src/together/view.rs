//! The world, drawn: a WoW unit per hero and enemy and an object per gem, kept where the
//! simulation says they are, and the step's [`Event`]s played as casts, swings and numbers.
//!
//! The local hero is a real WoW player: WoW's own character controller moves it (the player
//! reports its pose to the simulation, [`super::feed_pose`]), and WoW's unit frames read its health
//! and level. Everything else glides from where it was at the last step to where it is now
//! ([`SimPos`]), since the simulation steps 30 times a second and the screen draws faster.

use std::collections::{HashMap, HashSet};

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use together_sim::{Event, PlayerId, Target, V2};

use super::units::{self, Fx, ShownVitals, UnitKind, UnitSpec, Weapon, COLOR_HURT, COLOR_SPELL};
use super::{actions, dress, Sim, LOCAL_PLAYER};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::{CreatureRecord, NameCache};
use crate::net::{Guid, GuidIndex, ObjectStore, SelfGuid};

/// Which entity draws which part of the world.
#[derive(Resource, Default)]
pub(super) struct Views {
    pub heroes: HashMap<PlayerId, Entity>,
    pub enemies: HashMap<u32, Entity>,
    pub gems: HashMap<u32, Entity>,
    /// Creature entries whose names are in the [`NameCache`].
    named: HashSet<u32>,
}

impl Views {
    fn of(&self, target: Target) -> Option<Entity> {
        match target {
            Target::Hero(p) => self.heroes.get(&p).copied(),
            Target::Enemy(id) => self.enemies.get(&id).copied(),
        }
    }
}

/// Where a body was at the last two steps, and how it was going.
#[derive(Component)]
pub(super) struct SimPos {
    prev: V2,
    cur: V2,
    facing: V2,
    /// Run speed for the animation, 0 standing.
    speed: f32,
    /// Height above the ground at the last two steps (a jump), and the upward speed, whose sign
    /// picks the jump or fall animation.
    prev_air: f32,
    air: f32,
    vy: f32,
}

impl SimPos {
    fn new(at: V2, facing: V2) -> Self {
        Self {
            prev: at,
            cur: at,
            facing,
            speed: 0.0,
            prev_air: 0.0,
            air: 0.0,
            vy: 0.0,
        }
    }

    fn advance(&mut self, at: V2, facing: V2, speed: f32) {
        self.prev = self.cur;
        self.cur = at;
        self.facing = facing;
        self.speed = speed;
    }

    fn advance_air(&mut self, air: f32, vy: f32) {
        self.prev_air = self.air;
        self.air = air;
        self.vy = vy;
    }
}

/// A corpse playing its death before it fades.
#[derive(Component)]
pub(super) struct Dying(f32);

/// Hero guids (player guids, high word 0) are this plus the player id.
pub(super) const HERO_SERIAL_BASE: u64 = 0x10;

/// The name the local hero's frame shows.
fn hero_name() -> String {
    std::env::var("TOGETHER_NAME").unwrap_or_else(|_| "Hero".into())
}

/// The step's events as visuals. Runs before [`sync`], so a dying enemy is still mapped.
pub(super) fn apply_events(
    mut commands: Commands,
    mut sim: ResMut<Sim>,
    mut views: ResMut<Views>,
    mut stores: Query<&mut ObjectStore>,
    mut fx: Fx,
    mut cooldowns: ResMut<crate::spell::Cooldowns>,
) {
    for event in sim.world.drain_events() {
        match event {
            Event::Cast {
                caster,
                visual,
                targets,
                cooldown,
                ..
            } => {
                if caster == LOCAL_PLAYER {
                    actions::start_cooldown(&mut cooldowns, visual, cooldown);
                }
                let Some(me) = views.heroes.get(&caster).copied() else {
                    continue;
                };
                let hits: Vec<Entity> = targets.iter().filter_map(|&t| views.of(t)).collect();
                fx.cast(me, visual, &hits, None);
            }
            Event::Damage {
                target,
                amount,
                crit,
            } => {
                if let Some(e) = views.of(target) {
                    fx.number(e, amount, crit, COLOR_SPELL);
                }
            }
            Event::Swing {
                enemy,
                hero,
                amount,
            } => {
                if let (Some(&from), Some(&to)) =
                    (views.enemies.get(&enemy), views.heroes.get(&hero))
                {
                    fx.swing(from, to, amount);
                    fx.number(to, amount, false, COLOR_HURT);
                }
            }
            Event::EnemyDied { id } => {
                if let Some(e) = views.enemies.remove(&id) {
                    if let Ok(mut store) = stores.get_mut(e) {
                        units::kill_unit(&mut store);
                    }
                    commands.entity(e).remove::<SimPos>().insert(Dying(0.0));
                }
            }
            Event::HeroDied { player } => {
                if let Some(&e) = views.heroes.get(&player) {
                    if let Ok(mut store) = stores.get_mut(e) {
                        units::kill_unit(&mut store);
                    }
                }
            }
            Event::GemCollected { id, .. } => {
                if let Some(e) = views.gems.remove(&id) {
                    commands.entity(e).despawn();
                }
            }
            Event::LevelUp { .. } => {}
        }
    }
}

/// The `GameObjectDisplayInfo` wisp a gem of `xp` wears: bigger tiers, rarer colours.
fn gem_display(xp: u32) -> u32 {
    match xp {
        // `BFD_WispSmallPurple.mdx`.
        20.. => 1307,
        // `BFD_WispSmallGreen.mdx`.
        5.. => 1267,
        // `BFD_WispSmall.mdx`.
        _ => 426,
    }
}

/// Bodies for what is new in the world, none for what has left it, and every body's latest
/// position after a step. Corpses fade once their death has played.
///
/// Every unit also goes into the [`GuidIndex`] and the [`NameCache`], as the net layer files a
/// streamed one, so targeting, tooltips and unit frames find it; the local hero becomes our
/// [`SelfGuid`], which hands it to WoW's character controller.
pub(super) fn sync(
    mut commands: Commands,
    time: Res<Time>,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    creatures: Option<Res<crate::entities::Creatures>>,
    world_point: WorldPoint,
    mut self_guid: ResMut<SelfGuid>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut items: ResMut<crate::items::Items>,
    mut positions: Query<&mut SimPos>,
    mut dying: Query<(Entity, &Guid, &mut Dying)>,
) {
    let world = &sim.world;
    let content = world.content().clone();
    let ground = |p: V2, fallback: f32| {
        let mut at = units::ground_to_bevy(p, fallback);
        if let Some(h) = world_point.terrain_height_under(at) {
            at.y = h;
        }
        at
    };
    let fallback_y = world.zone().center[2];

    for hero in &world.heroes {
        if views.heroes.contains_key(&hero.player) {
            continue;
        }
        // The body's race and sex come from the creature catalog, which loads after boot.
        let Some(creatures) = creatures.as_deref() else {
            continue;
        };
        let class = &content.classes[hero.class];
        // Ours wears the look picked at the menu; another's, its class's first.
        let look = if hero.player == LOCAL_PLAYER {
            Some((sim.look.0, (sim.look.1, sim.look.2)))
        } else {
            class
                .looks
                .iter()
                .find_map(|&d| creatures.display_race_sex(d).map(|rs| (d, rs)))
        };
        let Some((display, (race, sex))) = look else {
            continue;
        };
        let weapons: Vec<Weapon> = class
            .weapons
            .iter()
            .map(|w| {
                (
                    w.slot,
                    w.display,
                    w.class,
                    w.subclass,
                    w.material,
                    w.inventory_type,
                    w.sheath,
                )
            })
            .collect();
        // A player wears items, not display ids: the trainer's outfit and the class's weapons,
        // as stand-in item templates.
        let worn = dress::wear(
            &mut items,
            &dress::pieces(creatures.display_outfit(display), &class.weapons),
        );
        let serial = HERO_SERIAL_BASE + u64::from(hero.player);
        let guid = units::unit_guid(UnitKind::Player, serial);
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                kind: UnitKind::Player,
                serial,
                display,
                scale: 1.0,
                level: world.level,
                faction: units::FACTION_FRIENDLY,
                bytes0: u32::from(race)
                    | u32::from(class.class_id) << 8
                    | u32::from(sex) << 16
                    | u32::from(class.power) << 24,
                weapons: &weapons,
                sheath: class.sheath,
                extra: &worn,
            },
            ground(hero.pos, fallback_y),
            units::yaw_of(hero.facing),
        );
        index.0.insert(guid, e);
        if hero.player == LOCAL_PLAYER {
            // Ours: WoW's controller drives it, from the pose it is spawned in.
            self_guid.0 = Some(guid);
            names.insert_player(guid, hero_name(), None);
        } else {
            names.insert_player(guid, format!("Player {}", hero.player + 1), None);
            commands
                .entity(e)
                .insert(SimPos::new(hero.pos, hero.facing));
        }
        views.heroes.insert(hero.player, e);
    }

    for enemy in &world.enemies {
        if views.enemies.contains_key(&enemy.id) {
            continue;
        }
        let def = &content.enemies[enemy.def];
        // One creature template per enemy kind: its entry names it.
        let entry = enemy.def as u32 + 1;
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
        let serial = u64::from(enemy.id);
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
            ground(enemy.pos, fallback_y),
            units::yaw_of(enemy.facing),
        );
        commands
            .entity(e)
            .insert(SimPos::new(enemy.pos, enemy.facing));
        index.0.insert(units::unit_guid(kind, serial), e);
        views.enemies.insert(enemy.id, e);
    }
    // Gone without a death (a restart): no corpse, just gone.
    views.enemies.retain(|id, e| {
        let keep = world.enemy(*id).is_some();
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });

    for gem in &world.gems {
        if views.gems.contains_key(&gem.id) {
            continue;
        }
        let at = ground(gem.pos, fallback_y) + Vec3::Y * 0.5;
        let e = units::spawn_object(
            &mut commands,
            u64::from(gem.id),
            gem_display(gem.xp),
            1.0,
            at,
        );
        views.gems.insert(gem.id, e);
    }

    if sim.stepped {
        for hero in &world.heroes {
            if let Some(mut p) = views
                .heroes
                .get(&hero.player)
                .and_then(|&e| positions.get_mut(e).ok())
            {
                let speed = if hero.moving && hero.alive {
                    hero.speed
                } else {
                    0.0
                };
                p.advance(hero.pos, hero.facing, speed);
                p.advance_air(hero.air, hero.vy);
            }
        }
        for enemy in &world.enemies {
            if let Some(mut p) = views
                .enemies
                .get(&enemy.id)
                .and_then(|&e| positions.get_mut(e).ok())
            {
                let speed = if enemy.moving {
                    content.enemies[enemy.def].speed
                } else {
                    0.0
                };
                p.advance(enemy.pos, enemy.facing, speed);
            }
        }
    }

    for (e, guid, mut d) in &mut dying {
        d.0 += time.delta_secs();
        if d.0 > 2.5 {
            index.0.remove(&guid.0);
            commands
                .entity(e)
                .remove::<Dying>()
                .insert(benilla_world::model_fade::DespawnFade::default());
        }
    }
}

/// Health, maximum and level into every living unit's store, for WoW's player and target frames.
pub(super) fn write_vitals(
    sim: Res<Sim>,
    views: Res<Views>,
    mut units: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let world = &sim.world;
    for hero in &world.heroes {
        if let Some((mut store, mut shown)) = views
            .heroes
            .get(&hero.player)
            .and_then(|&e| units.get_mut(e).ok())
        {
            let xp = Some((world.xp, together_sim::xp_to_next(world.level)));
            units::set_vitals(
                &mut store,
                &mut shown,
                hero.hp,
                hero.max_hp,
                world.level,
                xp,
            );
        }
    }
    for enemy in &world.enemies {
        if let Some((mut store, mut shown)) = views
            .enemies
            .get(&enemy.id)
            .and_then(|&e| units.get_mut(e).ok())
        {
            let level = world.content().enemies[enemy.def].level;
            units::set_vitals(&mut store, &mut shown, enemy.hp, enemy.max_hp, level, None);
        }
    }
}

/// Every body between its last two steps, on the ground (or above it, jumping), turned the way it
/// goes, with the run and jump animations. The engine's ground clamp re-seats units on the floor
/// at the top of each frame, so the jump height is added again here every frame. The local hero
/// has no [`SimPos`]: the controller places it.
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
        pos.y += p.prev_air + (p.air - p.prev_air) * alpha;
        if tf.translation != pos {
            tf.translation = pos;
        }
        if p.facing != V2::ZERO {
            let target = Quat::from_rotation_y(units::yaw_of(p.facing));
            tf.rotation = tf.rotation.slerp(target, turn);
        }
        let airborne = p.air > 0.0 || p.vy != 0.0;
        let mut flags = if p.speed > 0.0 {
            move_flags::FORWARD
        } else {
            0
        };
        if airborne {
            // The vertical speed's sign and `FALLING` select the jump and fall animations.
            flags |= move_flags::FALLING;
        }
        let speed = p.speed;
        let vy = if airborne { p.vy } else { 0.0 };
        if mv.speed != speed || mv.flags != flags || mv.vertical_speed != vy {
            mv.speed = speed;
            mv.flags = flags;
            mv.vertical_speed = vy;
        }
    }
}
