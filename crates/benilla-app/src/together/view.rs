//! The world, drawn: a WoW unit per hero and enemy, an object per gem and coin, a model with a
//! ground ring per shrine and chest, all kept where the simulation says they are; and the step's
//! [`Event`]s played as casts, swings and numbers.
//!
//! The local hero is a real WoW player: WoW's own character controller moves it (the player
//! reports its pose to the simulation, [`super::feed_pose`]), and WoW's unit frames read its health
//! and level. Everything else glides from where it was at the last step to where it is now
//! ([`SimPos`]), since the simulation steps 30 times a second and the screen draws faster.

use std::collections::{HashMap, HashSet};

use benilla_world::world_point::{Subject, WorldPoint};
use bevy::prelude::*;
use together_sim::{Event, InteractState, InteractableKind, PlayerId, Target, ELITE_SCALE, V2};

use super::units::{self, Fx, ShownVitals, UnitKind, UnitSpec, COLOR_HURT, COLOR_SPELL};
use super::{actions, dress, Sim};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::{CreatureRecord, NameCache};
use crate::net::{Guid, GuidIndex, ObjectStore, SelfGuid};

/// Which entity draws which part of the world.
#[derive(Resource, Default)]
pub(super) struct Views {
    pub heroes: HashMap<PlayerId, Entity>,
    pub enemies: HashMap<u32, Entity>,
    pub gems: HashMap<u32, Entity>,
    pub interactables: HashMap<u32, ShrineView>,
    /// Interactables whose spot has been checked for water and walls.
    settled: HashSet<u32>,
    /// Creature entries whose names are in the [`NameCache`].
    named: HashSet<u32>,
    /// The run speed last given the local hero's controller.
    speed: f32,
}

/// The bodies that draw one interactable: its model, the ring on the ground marking its circle,
/// and a beam of light that marks it from afar while it can be used. How full the circle is shows
/// in the HUD's prompt ([`super::hud::prompt`]).
pub(super) struct ShrineView {
    /// Where it was drawn: it is drawn anew if the host moves it.
    pos: V2,
    model: Entity,
    /// The ring and the beam, while it can be used.
    marks: Option<[Entity; 2]>,
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

/// The priest's Resurrection (rank 1), whose visual raises a fallen hero.
const RESURRECTION: u32 = 2006;

/// Hero guids (player guids, high word 0) are this plus the player id.
pub(super) const HERO_SERIAL_BASE: u64 = 0x10;

/// Object guids for an interactable's model, ring and beam: this plus its id times four, plus
/// the part. Gem ids share the world's id counter, so these sit far above them.
const SHRINE_SERIAL_BASE: u64 = 0x4000_0000;

/// Creature entries of elites: this plus their kind's own entry, so the name cache files them
/// apart, ranked elite, which gives their target frame its gold dragon.
const ELITE_ENTRY_BASE: u32 = 0x8000;

/// `CreatureTemplate.rank` of an elite.
const RANK_ELITE: u32 = 1;

/// The colour of gold in combat text, 0xAARRGGBB.
const COLOR_GOLD: u32 = 0xFFFF_D100;

/// The `GameObjectDisplayInfo` row and scale an interactable's model wears.
fn shrine_look(kind: InteractableKind) -> (u32, f32) {
    match kind {
        // `GlyphedCrystal.mdx`, Silithus's titan glyph crystal.
        InteractableKind::TitanShrine => (6431, 2.0),
        // `G_EpicBrazierYellow.mdx`.
        InteractableKind::GoldenShrine => (6660, 1.2),
        // `WarlockShrine.mdx`.
        InteractableKind::CursedAltar => (328, 1.0),
        // `GoldPileSmall01.mdx`, heaped up.
        InteractableKind::GreedShrine => (2614, 3.0),
        // `TreasureChest01.mdx`.
        InteractableKind::Chest => (259, 1.2),
        // `G_ChestTitan.mdx`.
        InteractableKind::Cache => (1387, 1.2),
        // `TrollShrine.mdx`, Stranglethorn's troll idol.
        InteractableKind::ChallengeIdol => (652, 1.0),
        // `ElvenFountain.mdx`.
        InteractableKind::Moonwell => (439, 0.8),
    }
}

/// The glowing ground rune that marks an interactable's circle (blue for the titans' gifts,
/// purple for the altar and greed, green for the rest), and the rune's own radius at scale 1 in
/// yards, to size it to the circle.
fn ring_look(kind: InteractableKind) -> (u32, f32) {
    match kind {
        // `G_RuneGroundPurple01.mdx`.
        InteractableKind::CursedAltar | InteractableKind::GreedShrine => (465, 2.8),
        // `G_RuneGroundGreen01.mdx`.
        InteractableKind::ChallengeIdol | InteractableKind::Moonwell => (674, 2.8),
        // `G_RuneGroundBlue01.mdx`.
        _ => (343, 1.0),
    }
}

/// `UtherShrineLightBeam.mdx`: a shaft of light over a ready interactable, seen across the arena,
/// and its scale: narrow enough not to swallow a hero standing in it.
const BEAM: u32 = 3272;
const BEAM_SCALE: f32 = 0.2;


/// The spell whose visual plays on a hero an interactable served.
fn shrine_visual(kind: InteractableKind) -> Option<u32> {
    match kind {
        // Holy Nova.
        InteractableKind::TitanShrine
        | InteractableKind::GoldenShrine
        | InteractableKind::Moonwell => Some(15237),
        // Corruption.
        InteractableKind::CursedAltar => Some(172),
        // Judgement.
        InteractableKind::GreedShrine => Some(20271),
        // Demoralizing Shout.
        InteractableKind::ChallengeIdol => Some(1160),
        InteractableKind::Chest | InteractableKind::Cache => None,
    }
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
                if caster == sim.me {
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
            Event::HeroRevived { player, by } => {
                // The raiser casts Resurrection on the fallen; its health returns with the
                // next vitals, which stands it up.
                if let (Some(&from), Some(&to)) = (views.heroes.get(&by), views.heroes.get(&player))
                {
                    fx.cast(from, RESURRECTION, &[to], None);
                }
            }
            // Gems are drawn from the world's list, which the drop already filled.
            Event::GemDropped { .. } => {}
            Event::GemCollected { id, .. } => {
                if let Some(e) = views.gems.remove(&id) {
                    commands.entity(e).despawn();
                }
            }
            Event::LevelUp { .. } | Event::ChallengeWon { .. } => {}
            Event::GoldGained { player, amount } => {
                // Everyone's purse fills with every coin; only our own is shown.
                if player == sim.me {
                    if let Some(&e) = views.heroes.get(&player) {
                        fx.text(e, format!("+{amount} gold"), COLOR_GOLD);
                    }
                }
            }
            Event::Interacted { kind, by, .. } => {
                let Some(visual) = shrine_visual(kind) else {
                    continue;
                };
                for p in by {
                    if let Some(&e) = views.heroes.get(&p) {
                        fx.cast(e, visual, &[e], None);
                    }
                }
            }
        }
    }
}

/// The `GameObjectDisplayInfo` row and scale a gem wears: a heap of coins when it carries gold,
/// else a wisp, bigger tiers in rarer colours.
fn gem_look(xp: u32, gold: u32) -> (u32, f32) {
    if gold > 0 {
        // `GoldPileSmall01.mdx`.
        return (2614, 1.0);
    }
    let wisp = match xp {
        // `BFD_WispSmallPurple.mdx`.
        20.. => 1307,
        // `BFD_WispSmallGreen.mdx`.
        5.. => 1267,
        // `BFD_WispSmall.mdx`.
        _ => 426,
    };
    (wisp, 1.0)
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
    char_create: Option<Res<crate::entities::CharCreate>>,
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
        // The body and outfit tables load after boot.
        let Some(char_create) = char_create.as_deref() else {
            continue;
        };
        let class = &content.classes[hero.class];
        // Each hero wears the race and sex its player picked at their menu.
        let look = (hero.race, hero.sex);
        let serial = HERO_SERIAL_BASE + u64::from(hero.player);
        let guid = units::unit_guid(UnitKind::Player, serial);
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            char_create,
            class,
            look,
            serial,
            world.level,
            ground(hero.pos, fallback_y),
            units::yaw_of(hero.facing),
        ) else {
            continue;
        };
        index.0.insert(guid, e);
        names.insert_player(guid, hero.name.clone(), None);
        if hero.player == sim.me {
            // Ours: WoW's controller drives it, from the pose it is spawned in.
            self_guid.0 = Some(guid);
        } else {
            // Another player's: it goes where the world says, as an enemy does.
            commands
                .entity(e)
                .insert(SimPos::new(hero.pos, hero.facing));
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

    for enemy in &world.enemies {
        if views.enemies.contains_key(&enemy.id) {
            continue;
        }
        let def = &content.enemies[enemy.def];
        // One creature template per enemy kind, and one more for its elites: the entry names it.
        let (entry, rank, scale) = if enemy.elite {
            let entry = ELITE_ENTRY_BASE + enemy.def as u32 + 1;
            (entry, RANK_ELITE, def.scale * ELITE_SCALE)
        } else {
            (enemy.def as u32 + 1, 0, def.scale)
        };
        if views.named.insert(entry) {
            names.insert_creature(
                entry,
                Some(CreatureRecord {
                    name: def.name.clone(),
                    subname: None,
                    creature_type: 0,
                    pet_family: 0,
                    rank,
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
                scale,
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
        let lift = if gem.gold > 0 { 0.0 } else { 0.5 };
        let at = ground(gem.pos, fallback_y) + Vec3::Y * lift;
        let (display, scale) = gem_look(gem.xp, gem.gold);
        let e = units::spawn_object(&mut commands, u64::from(gem.id), display, scale, at);
        views.gems.insert(gem.id, e);
    }

    sync_shrines(&mut commands, world, &mut views, &ground, fallback_y);

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

/// A model for every interactable, kept for the run, and while it can be used, a ring on the
/// ground around it and a beam of light over it. One the host moved is drawn anew where it went;
/// bodies of an interactable that left the world (a mirror's first snapshot) go with it.
fn sync_shrines(
    commands: &mut Commands,
    world: &together_sim::World,
    views: &mut Views,
    ground: &impl Fn(V2, f32) -> Vec3,
    fallback_y: f32,
) {
    for it in &world.interactables {
        let serial = SHRINE_SERIAL_BASE + u64::from(it.id) * 4;
        let at = ground(it.pos, fallback_y);
        if let Some(moved) = views.interactables.remove(&it.id) {
            if moved.pos == it.pos {
                views.interactables.insert(it.id, moved);
            } else {
                commands.entity(moved.model).despawn();
                for e in moved.marks.into_iter().flatten() {
                    commands.entity(e).despawn();
                }
            }
        }
        let view = views.interactables.entry(it.id).or_insert_with(|| {
            let (display, scale) = shrine_look(it.kind);
            ShrineView {
                pos: it.pos,
                model: units::spawn_object(commands, serial, display, scale, at),
                marks: None,
            }
        });
        // A challenge under way keeps its ring: the fight is there.
        let marked = it.ready() || matches!(it.state, InteractState::Challenge(_));
        match (marked, view.marks) {
            (true, None) => {
                let (rune, rune_radius) = ring_look(it.kind);
                let size = it.kind.radius() / rune_radius;
                view.marks = Some([
                    units::spawn_object(commands, serial + 1, rune, size, at + Vec3::Y * 0.05),
                    units::spawn_object(commands, serial + 2, BEAM, BEAM_SCALE, at),
                ]);
            }
            (false, Some(marks)) => {
                for e in marks {
                    commands.entity(e).despawn();
                }
                view.marks = None;
            }
            _ => {}
        }
    }
    views.interactables.retain(|id, view| {
        let keep = world.interactable(*id).is_some();
        if !keep {
            commands.entity(view.model).despawn();
            for e in view.marks.into_iter().flatten() {
                commands.entity(e).despawn();
            }
        }
        keep
    });
}

/// Whether a spot is dry ground in the open: `None` until the terrain under it has loaded.
fn usable(world_point: &WorldPoint, at: V2) -> Option<bool> {
    let probe = units::ground_to_bevy(at, 0.0);
    let ground = world_point.terrain_height_under(probe)?;
    let wet = world_point
        .water_surface_at(Subject::Player, [at.x, at.y, ground])
        .is_some_and(|surface| surface > ground + 0.3);
    let indoors = world_point.indoors_at(Vec3::new(probe.x, ground, probe.z));
    Some(!wet && !indoors)
}

/// Alone or hosting: each interactable's spot is checked once the terrain under it has loaded,
/// and one under water or inside a building moves to the nearest dry, open spot in the arena,
/// searched in widening rings. Joined, the host has done it, and its snapshots say where.
pub(super) fn settle_shrines(
    mut sim: ResMut<Sim>,
    net: Res<super::session::Net>,
    world_point: WorldPoint,
    mut views: ResMut<Views>,
) {
    /// Rings searched, this many yards apart, and spots on each.
    const RINGS: u32 = 12;
    const RING_STEP: f32 = 4.0;
    const SPOTS: u32 = 12;
    if net.is_client() {
        return;
    }
    let center = sim.world.center();
    let leash = sim.world.zone().leash;
    let unsettled: Vec<(u32, V2)> = sim
        .world
        .interactables
        .iter()
        .filter(|i| !views.settled.contains(&i.id))
        .map(|i| (i.id, i.pos))
        .collect();
    for (id, pos) in unsettled {
        let Some(fine) = usable(&world_point, pos) else {
            continue;
        };
        views.settled.insert(id);
        if fine {
            continue;
        }
        let spot = (1..=RINGS)
            .flat_map(|ring| {
                (0..SPOTS).map(move |k| {
                    let angle = k as f32 / SPOTS as f32 * std::f32::consts::TAU;
                    pos + V2::from_angle(angle, ring as f32 * RING_STEP)
                })
            })
            .filter(|p| p.distance(center) <= leash - 3.0)
            .find(|&p| usable(&world_point, p) == Some(true));
        if let Some(to) = spot {
            sim.world.move_interactable(id, to);
        }
    }
}

/// Health, maximum and level into every living unit's store, for WoW's player and target frames;
/// and the local hero's run speed into its controller.
pub(super) fn write_vitals(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    mut units: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let world = &sim.world;
    if let (Some(hero), Some(&e)) = (world.hero(sim.me), views.heroes.get(&sim.me)) {
        if hero.speed != views.speed {
            views.speed = hero.speed;
            commands.entity(e).insert(crate::net::UnitSpeeds(run_speeds(hero.speed)));
        }
    }
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

/// WoW's speed set for a hero who runs at `run` yards a second: walking, backing and swimming
/// at the stock 2.5 / 4.5 / 4.72 to 7.0 ratios of it, and the stock turn rate.
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
