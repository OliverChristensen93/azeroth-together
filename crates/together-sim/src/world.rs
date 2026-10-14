//! One run: the heroes, the horde, the experience on the ground, stepped [`TICK_HZ`] times a
//! second by [`World::step`].
//!
//! A step reads each player's latest [`PlayerInput`], moves everything, fires abilities, lands
//! hits, and records what happened as [`Event`]s for the client to show. The world itself is the
//! authority: a client only ever draws it, which is how a co-op host will run it for everyone.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::content::{AbilityKind, Content};
use crate::math::V2;
use crate::rng::Rng;

/// Simulation steps per second.
pub const TICK_HZ: u32 = 30;
/// Seconds per step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// Which player a hero belongs to: 0 for the host, then one per player who joins.
pub type PlayerId = u8;

/// The most abilities a hero carries, and the highest level each reaches.
const MAX_ABILITIES: usize = 6;
const MAX_ABILITY_LEVEL: u8 = 8;
/// Cards dealt per level-up.
const HAND: usize = 3;
/// A hero's body radius, which enemies stop at.
const HERO_RADIUS: f32 = 0.7;
/// Within this many yards of a hero an experience gem is collected.
const MAGNET: f32 = 4.5;
const CRIT_CHANCE: f32 = 0.05;
/// Enemies spawn on a ring this far from a hero: outside the view, inside the leash.
const SPAWN_RING: (f32, f32) = (34.0, 42.0);
/// Seconds between spawn batches.
const SPAWN_EVERY: f32 = 0.12;
/// The run's length; enemy health and damage grow toward it.
const RUN_SECS: f32 = 900.0;

/// The experience to go from `level` to the next: cheap early, steeper later.
pub fn xp_to_next(level: u32) -> f32 {
    let l = level as f32;
    if level < 20 {
        5.0 + (l - 1.0) * 10.0
    } else if level < 40 {
        195.0 + (l - 20.0) * 13.0
    } else {
        455.0 + (l - 40.0) * 16.0
    }
}

/// How many enemies the run keeps alive after `elapsed` seconds, for `heroes` heroes.
fn target_alive(elapsed: f32, heroes: usize) -> usize {
    let solo = (14.0 + elapsed / 60.0 * 14.0).min(220.0);
    // Each extra hero brings 60% more of the horde.
    (solo * (1.0 + 0.6 * heroes.saturating_sub(1) as f32)) as usize
}

/// What one player is doing this step. Movement is held until the next input; a pick and a jump
/// are each used once.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    /// The direction to run, any length; zero stands still.
    pub move_dir: V2,
    /// A card from the hero's current [`Offer`], by index.
    pub pick: Option<u8>,
    /// Hop, if on the ground.
    pub jump: bool,
    /// Where the player's own client put the hero, overriding `move_dir` and `jump`. A client
    /// running WoW's character controller moves its hero itself, as WoW clients do, and reports
    /// the pose; the world only keeps it inside the arena.
    pub pose: Option<HeroPose>,
}

/// A hero's position and facing as its player's client reports them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
}

/// A jump's initial upward speed and the gravity that brings the hero back down, in yards and
/// seconds: the 1.12 client's own movement constants.
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Something an ability, a swing or an event can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Hero(PlayerId),
    Enemy(u32),
}

/// What happened during a step, for the client to show.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A hero cast `ability` (its `visual` spell) at `targets`, or at the ground at `dest`.
    Cast {
        caster: PlayerId,
        ability: usize,
        visual: u32,
        targets: Vec<Target>,
        dest: Option<V2>,
    },
    Damage {
        target: Target,
        amount: f32,
        crit: bool,
    },
    /// An enemy hit a hero in melee.
    Swing {
        enemy: u32,
        hero: PlayerId,
        amount: f32,
    },
    EnemyDied {
        id: u32,
    },
    HeroDied {
        player: PlayerId,
    },
    GemCollected {
        id: u32,
        by: PlayerId,
    },
    /// The party reached `level`; every living hero has a new [`Offer`].
    LevelUp {
        level: u32,
    },
}

/// A level-up card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    /// Learn this ability (an index into [`Content::abilities`]).
    Learn(usize),
    /// Raise this ability, already owned, by one level.
    Upgrade(usize),
}

/// The cards a hero is choosing between.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub cards: Vec<Card>,
}

/// An ability a hero has.
#[derive(Clone, Debug)]
pub struct Owned {
    /// Index into [`Content::abilities`].
    pub ability: usize,
    pub level: u8,
    /// Seconds until it fires again.
    pub cd: f32,
}

impl Owned {
    fn damage_mult(&self) -> f32 {
        1.0 + 0.3 * f32::from(self.level - 1)
    }

    fn cooldown(&self, base: f32) -> f32 {
        (base * 0.93f32.powi(i32::from(self.level) - 1)).max(0.2)
    }

    /// One more target at levels 3, 5 and 7.
    fn count(&self, base: u32) -> u32 {
        base + [3u8, 5, 7].iter().filter(|&&l| self.level >= l).count() as u32
    }
}

#[derive(Clone, Debug)]
pub struct Hero {
    pub player: PlayerId,
    /// Index into [`Content::classes`].
    pub class: usize,
    pub pos: V2,
    /// The way the hero last moved; the client turns the model to it.
    pub facing: V2,
    /// Whether the hero is running this step.
    pub moving: bool,
    /// Height above the ground while jumping, 0 on it. Only for show: the game is flat, and a
    /// jump dodges nothing.
    pub air: f32,
    /// Upward speed while jumping, yards per second; 0 on the ground.
    pub vy: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub abilities: Vec<Owned>,
    /// Level-up hands still to choose from, oldest first.
    pub offers: Vec<Offer>,
    pub alive: bool,
    /// Damage this hero has dealt over the run.
    pub damage_done: f64,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    /// Index into [`Content::enemies`].
    pub def: usize,
    pub pos: V2,
    pub facing: V2,
    pub moving: bool,
    pub hp: f32,
    pub max_hp: f32,
    pub damage: f32,
    pub radius: f32,
    attack_cd: f32,
}

/// Experience on the ground, waiting for a hero to come close.
#[derive(Clone, Debug)]
pub struct Gem {
    pub id: u32,
    pub pos: V2,
    pub xp: u32,
}

/// Damage on its way: a missile still flying.
#[derive(Clone, Debug)]
struct PendingHit {
    lands_at: f32,
    caster: PlayerId,
    target: u32,
    damage: f32,
    crit: bool,
    splash: f32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WorldError {
    UnknownZone(String),
    UnknownClass(String),
    PlayerTaken(PlayerId),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::UnknownZone(k) => write!(f, "no zone \"{k}\""),
            WorldError::UnknownClass(k) => write!(f, "no class \"{k}\""),
            WorldError::PlayerTaken(p) => write!(f, "player {p} already has a hero"),
        }
    }
}

impl std::error::Error for WorldError {}

pub struct World {
    content: Arc<Content>,
    zone: usize,
    rng: Rng,
    /// Steps taken while the run was live.
    pub tick: u64,
    /// Seconds of play, not counting time spent choosing cards.
    pub elapsed: f32,
    pub heroes: Vec<Hero>,
    pub enemies: Vec<Enemy>,
    pub gems: Vec<Gem>,
    pending: Vec<PendingHit>,
    inputs: HashMap<PlayerId, PlayerInput>,
    events: Vec<Event>,
    next_id: u32,
    spawn_timer: f32,
    /// The party's shared level and experience toward the next.
    pub level: u32,
    pub xp: f32,
    pub kills: u32,
}

impl World {
    pub fn new(content: Arc<Content>, zone: &str, seed: u64) -> Result<Self, WorldError> {
        let zone = content
            .zone(zone)
            .ok_or_else(|| WorldError::UnknownZone(zone.to_owned()))?;
        Ok(Self {
            content,
            zone,
            rng: Rng::new(seed),
            tick: 0,
            elapsed: 0.0,
            heroes: Vec::new(),
            enemies: Vec::new(),
            gems: Vec::new(),
            pending: Vec::new(),
            inputs: HashMap::new(),
            events: Vec::new(),
            next_id: 1,
            spawn_timer: 0.0,
            level: 1,
            xp: 0.0,
            kills: 0,
        })
    }

    pub fn content(&self) -> &Arc<Content> {
        &self.content
    }

    pub fn zone(&self) -> &crate::content::ZoneDef {
        &self.content.zones[self.zone]
    }

    pub fn center(&self) -> V2 {
        let c = self.zone().center;
        V2::new(c[0], c[1])
    }

    /// Give `player` a hero of `class`, standing near the arena's centre.
    pub fn add_hero(&mut self, player: PlayerId, class: &str) -> Result<(), WorldError> {
        if self.heroes.iter().any(|h| h.player == player) {
            return Err(WorldError::PlayerTaken(player));
        }
        let class_ix = self
            .content
            .class(class)
            .ok_or_else(|| WorldError::UnknownClass(class.to_owned()))?;
        let def = &self.content.classes[class_ix];
        // Side by side, a couple of yards apart.
        let pos = self.center() + V2::new(0.0, 2.5 * f32::from(player));
        self.heroes.push(Hero {
            player,
            class: class_ix,
            pos,
            facing: V2::new(1.0, 0.0),
            moving: false,
            air: 0.0,
            vy: 0.0,
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
            abilities: vec![Owned {
                ability: def.start_ix,
                level: 1,
                cd: 0.5,
            }],
            offers: Vec::new(),
            alive: true,
            damage_done: 0.0,
        });
        Ok(())
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    pub fn enemy(&self, id: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.id == id)
    }

    /// The input `player` holds from now on.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        self.inputs.insert(player, input);
    }

    /// While any living hero has cards to choose, the run waits.
    pub fn paused(&self) -> bool {
        self.heroes.iter().any(|h| h.alive && !h.offers.is_empty())
    }

    /// Every hero has fallen.
    pub fn game_over(&self) -> bool {
        !self.heroes.is_empty() && self.heroes.iter().all(|h| !h.alive)
    }

    /// The events since the last drain, oldest first.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Advance the run by one [`DT`]. Picks are taken even while paused; nothing else moves then.
    pub fn step(&mut self) {
        self.apply_picks();
        if self.paused() || self.game_over() {
            return;
        }
        self.tick += 1;
        self.elapsed += DT;
        self.move_heroes();
        self.spawn_enemies();
        self.move_enemies();
        self.cast_abilities();
        self.land_hits();
        self.reap_enemies();
        self.collect_gems();
        self.level_up();
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn apply_picks(&mut self) {
        for hero in &mut self.heroes {
            let Some(input) = self.inputs.get_mut(&hero.player) else {
                continue;
            };
            let Some(pick) = input.pick.take() else {
                continue;
            };
            let Some(offer) = hero.offers.first() else {
                continue;
            };
            let Some(&card) = offer.cards.get(usize::from(pick)) else {
                continue;
            };
            match card {
                Card::Learn(ability) => hero.abilities.push(Owned {
                    ability,
                    level: 1,
                    cd: 0.3,
                }),
                Card::Upgrade(ability) => {
                    if let Some(owned) = hero.abilities.iter_mut().find(|o| o.ability == ability) {
                        owned.level = (owned.level + 1).min(MAX_ABILITY_LEVEL);
                    }
                }
            }
            hero.offers.remove(0);
        }
    }

    fn move_heroes(&mut self) {
        let center = self.center();
        let leash = self.zone().leash;
        for hero in self.heroes.iter_mut().filter(|h| h.alive) {
            let input = self.inputs.get_mut(&hero.player);
            if let Some(pose) = input.as_ref().and_then(|i| i.pose) {
                let off = pose.pos - center;
                let pos = if off.length() > leash {
                    center + off.normalize_or_zero() * leash
                } else {
                    pose.pos
                };
                hero.moving = pos.distance(hero.pos) > 1e-3;
                hero.pos = pos;
                if pose.facing != V2::ZERO {
                    hero.facing = pose.facing.normalize_or_zero();
                }
                continue;
            }
            let jump = input.as_ref().is_some_and(|i| i.jump);
            let dir = input.map_or(V2::ZERO, |i| {
                i.jump = false;
                i.move_dir
            });
            if jump && hero.air == 0.0 && hero.vy == 0.0 {
                hero.vy = JUMP_SPEED;
            }
            if hero.vy != 0.0 || hero.air > 0.0 {
                hero.vy -= GRAVITY * DT;
                hero.air += hero.vy * DT;
                if hero.air <= 0.0 {
                    hero.air = 0.0;
                    hero.vy = 0.0;
                }
            }
            let dir = dir.normalize_or_zero();
            hero.moving = dir != V2::ZERO;
            if !hero.moving {
                continue;
            }
            hero.facing = dir;
            hero.pos += dir * (hero.speed * DT);
            let off = hero.pos - center;
            if off.length() > leash {
                hero.pos = center + off.normalize_or_zero() * leash;
            }
        }
    }

    fn spawn_enemies(&mut self) {
        self.spawn_timer -= DT;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = SPAWN_EVERY;
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        let target = target_alive(self.elapsed, living.len());
        if self.enemies.len() >= target {
            return;
        }
        let batch = ((target - self.enemies.len()) as f32 / 6.0).ceil().min(4.0) as usize;
        let roster_len = self.zone().enemy_ix.len();
        // A new tier every 45 seconds; the newest three tiers are in play.
        let top = ((self.elapsed / 45.0) as usize).min(roster_len - 1);
        let low = top.saturating_sub(2);
        let progress = (self.elapsed / RUN_SECS).min(1.0);
        let center = self.center();
        let leash = self.zone().leash;
        for _ in 0..batch {
            let near = living[self.rng.index(living.len())];
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let r = self.rng.range(SPAWN_RING.0, SPAWN_RING.1);
            let mut pos = near + V2::from_angle(angle, r);
            // Never outside the arena: pull back onto its edge.
            let off = pos - center;
            if off.length() > leash + 10.0 {
                pos = center + off.normalize_or_zero() * (leash + 10.0);
            }
            let tier = low + self.rng.index(top - low + 1);
            let def_ix = self.zone().enemy_ix[tier];
            let def = &self.content.enemies[def_ix];
            let hp = def.hp * (1.0 + progress * 0.75);
            let damage = def.damage * (1.0 + progress * 0.9);
            let radius = def.radius * def.scale;
            let id = self.id();
            self.enemies.push(Enemy {
                id,
                def: def_ix,
                pos,
                facing: (near - pos).normalize_or_zero(),
                moving: false,
                hp,
                max_hp: hp,
                damage,
                radius,
                attack_cd: 0.5,
            });
        }
    }

    fn move_enemies(&mut self) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        if living.is_empty() {
            for e in &mut self.enemies {
                e.moving = false;
            }
            return;
        }
        // Push apart overlapping bodies, so the horde spreads instead of stacking.
        let n = self.enemies.len();
        let mut shove = vec![V2::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&self.enemies[i], &self.enemies[j]);
                let d = b.pos - a.pos;
                let gap = a.radius + b.radius;
                let dist = d.length();
                if dist < gap && dist > 1e-4 {
                    let push = d * ((gap - dist) / dist * 0.5);
                    shove[i] = shove[i] - push;
                    shove[j] += push;
                }
            }
        }
        let mut swings = Vec::new();
        for (i, e) in self.enemies.iter_mut().enumerate() {
            let def = &self.content.enemies[e.def];
            let (player, target) = living
                .iter()
                .copied()
                .min_by(|a, b| e.pos.distance(a.1).total_cmp(&e.pos.distance(b.1)))
                .expect("living is not empty");
            let to = target - e.pos;
            let dist = to.length();
            let contact = dist <= HERO_RADIUS + e.radius + 0.35;
            e.facing = to.normalize_or_zero();
            e.moving = !contact;
            let step = if contact {
                V2::ZERO
            } else {
                e.facing * (def.speed * DT)
            };
            e.pos += step + shove[i] * (DT * 8.0).min(0.5);
            e.attack_cd -= DT;
            if contact && e.attack_cd <= 0.0 {
                e.attack_cd = 1.3;
                swings.push((e.id, player, e.damage));
            }
        }
        for (enemy, player, amount) in swings {
            self.hurt_hero(player, amount);
            self.events.push(Event::Swing {
                enemy,
                hero: player,
                amount,
            });
        }
    }

    fn hurt_hero(&mut self, player: PlayerId, amount: f32) {
        let Some(hero) = self
            .heroes
            .iter_mut()
            .find(|h| h.player == player && h.alive)
        else {
            return;
        };
        hero.hp -= amount;
        if hero.hp <= 0.0 {
            hero.hp = 0.0;
            hero.alive = false;
            hero.moving = false;
            hero.offers.clear();
            self.events.push(Event::HeroDied { player });
        }
    }

    /// The `count` enemies nearest `from` within `range`, nearest first.
    fn nearest(&self, from: V2, range: f32, count: usize) -> Vec<(u32, f32)> {
        let mut near: Vec<(u32, f32)> = self
            .enemies
            .iter()
            .map(|e| (e.id, from.distance(e.pos)))
            .filter(|&(_, d)| d <= range)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.truncate(count);
        near
    }

    fn roll_damage(&mut self, base: f32) -> (f32, bool) {
        let crit = self.rng.f32() < CRIT_CHANCE;
        (if crit { base * 2.0 } else { base }, crit)
    }

    fn cast_abilities(&mut self) {
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            for a in 0..self.heroes[h].abilities.len() {
                let owned = &mut self.heroes[h].abilities[a];
                owned.cd -= DT;
                if owned.cd > 0.0 {
                    continue;
                }
                let owned = owned.clone();
                let def = self.content.abilities[owned.ability].clone();
                let cast = self.cast(h, &owned, &def);
                // No target in reach: try again next step rather than wasting the cooldown.
                self.heroes[h].abilities[a].cd = if cast {
                    owned.cooldown(def.cooldown)
                } else {
                    0.0
                };
            }
        }
    }

    /// Fire one ability; false when it had nothing to hit.
    fn cast(&mut self, h: usize, owned: &Owned, def: &crate::content::AbilityDef) -> bool {
        let (player, from) = (self.heroes[h].player, self.heroes[h].pos);
        let base = def.damage * owned.damage_mult();
        match def.kind {
            AbilityKind::Bolt | AbilityKind::Strike => {
                let targets = self.nearest(from, def.range, owned.count(def.count) as usize);
                if targets.is_empty() {
                    return false;
                }
                for &(target, dist) in &targets {
                    let (damage, crit) = self.roll_damage(base);
                    let flight = if def.kind == AbilityKind::Bolt && def.missile_speed > 0.0 {
                        dist / def.missile_speed
                    } else {
                        0.0
                    };
                    self.pending.push(PendingHit {
                        lands_at: self.elapsed + flight,
                        caster: player,
                        target,
                        damage,
                        crit,
                        splash: def.radius,
                    });
                }
                self.events.push(Event::Cast {
                    caster: player,
                    ability: owned.ability,
                    visual: def.visual,
                    targets: targets.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
                    dest: None,
                });
                true
            }
            AbilityKind::Nova => {
                let hit: Vec<u32> = self
                    .enemies
                    .iter()
                    .filter(|e| from.distance(e.pos) <= def.radius)
                    .map(|e| e.id)
                    .collect();
                if hit.is_empty() {
                    return false;
                }
                self.events.push(Event::Cast {
                    caster: player,
                    ability: owned.ability,
                    visual: def.visual,
                    targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
                    dest: Some(from),
                });
                for id in hit {
                    let (damage, crit) = self.roll_damage(base);
                    self.damage_enemy(player, id, damage, crit);
                }
                true
            }
        }
    }

    fn land_hits(&mut self) {
        let now = self.elapsed;
        let (due, flying): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| p.lands_at <= now);
        self.pending = flying;
        for hit in due {
            let Some(at) = self.enemy(hit.target).map(|e| e.pos) else {
                // Its target died while the missile flew.
                continue;
            };
            self.damage_enemy(hit.caster, hit.target, hit.damage, hit.crit);
            if hit.splash > 0.0 {
                let around: Vec<u32> = self
                    .enemies
                    .iter()
                    .filter(|e| e.id != hit.target && at.distance(e.pos) <= hit.splash)
                    .map(|e| e.id)
                    .collect();
                for id in around {
                    self.damage_enemy(hit.caster, id, hit.damage * 0.5, false);
                }
            }
        }
    }

    fn damage_enemy(&mut self, by: PlayerId, id: u32, amount: f32, crit: bool) {
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else {
            return;
        };
        if e.hp <= 0.0 {
            return;
        }
        let dealt = amount.min(e.hp);
        e.hp -= amount;
        if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == by) {
            hero.damage_done += f64::from(dealt);
        }
        self.events.push(Event::Damage {
            target: Target::Enemy(id),
            amount,
            crit,
        });
    }

    fn reap_enemies(&mut self) {
        let mut dead = Vec::new();
        self.enemies.retain(|e| {
            if e.hp <= 0.0 {
                dead.push((e.id, e.pos, e.def));
                false
            } else {
                true
            }
        });
        for (id, pos, def) in dead {
            self.kills += 1;
            self.events.push(Event::EnemyDied { id });
            let xp = self.content.enemies[def].xp;
            let gem = self.id();
            self.gems.push(Gem { id: gem, pos, xp });
        }
    }

    fn collect_gems(&mut self) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        let mut gained = 0.0;
        let events = &mut self.events;
        self.gems.retain(
            |g| match living.iter().find(|(_, p)| p.distance(g.pos) <= MAGNET) {
                Some(&(by, _)) => {
                    gained += g.xp as f32;
                    events.push(Event::GemCollected { id: g.id, by });
                    false
                }
                None => true,
            },
        );
        self.xp += gained;
    }

    fn level_up(&mut self) {
        while self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            for h in 0..self.heroes.len() {
                if !self.heroes[h].alive {
                    continue;
                }
                let offer = self.deal(h);
                if !offer.cards.is_empty() {
                    self.heroes[h].offers.push(offer);
                }
            }
            self.events.push(Event::LevelUp { level: self.level });
        }
    }

    /// Up to [`HAND`] distinct cards from what this hero could still learn or raise.
    fn deal(&mut self, h: usize) -> Offer {
        let hero = &self.heroes[h];
        let class = &self.content.classes[hero.class];
        let mut options: Vec<Card> = Vec::new();
        for owned in &hero.abilities {
            if owned.level < MAX_ABILITY_LEVEL {
                options.push(Card::Upgrade(owned.ability));
            }
        }
        if hero.abilities.len() < MAX_ABILITIES {
            for &ability in &class.pool_ix {
                if !hero.abilities.iter().any(|o| o.ability == ability) {
                    options.push(Card::Learn(ability));
                }
            }
        }
        let mut cards = Vec::new();
        while cards.len() < HAND && !options.is_empty() {
            let i = self.rng.index(options.len());
            cards.push(options.swap_remove(i));
        }
        Offer { cards }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    fn world() -> World {
        let mut w = World::new(Arc::new(sample()), "elwynn", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        w
    }

    fn steps(w: &mut World, n: usize) {
        for _ in 0..n {
            w.step();
        }
    }

    #[test]
    fn unknown_zone_and_class_are_errors() {
        let c = Arc::new(sample());
        assert!(matches!(
            World::new(c.clone(), "nowhere", 1),
            Err(WorldError::UnknownZone(_))
        ));
        let mut w = World::new(c, "elwynn", 1).unwrap();
        assert!(matches!(
            w.add_hero(0, "bard"),
            Err(WorldError::UnknownClass(_))
        ));
        w.add_hero(0, "mage").unwrap();
        assert_eq!(w.add_hero(0, "mage"), Err(WorldError::PlayerTaken(0)));
    }

    #[test]
    fn hero_starts_with_class_start_ability() {
        let w = world();
        let hero = w.hero(0).unwrap();
        assert_eq!(hero.abilities.len(), 1);
        assert_eq!(
            w.content().abilities[hero.abilities[0].ability].key,
            "frostbolt"
        );
    }

    #[test]
    fn hero_runs_at_class_speed() {
        let mut w = world();
        let start = w.hero(0).unwrap().pos;
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                pick: None,
                jump: false,
                pose: None,
            },
        );
        steps(&mut w, TICK_HZ as usize);
        let moved = w.hero(0).unwrap().pos.distance(start);
        assert!((moved - 7.5).abs() < 0.01, "moved {moved}");
    }

    #[test]
    fn a_reported_pose_moves_the_hero_inside_the_leash() {
        let mut w = world();
        let center = w.center();
        let pose = |pos: V2| PlayerInput {
            pose: Some(HeroPose {
                pos,
                facing: V2::new(0.0, 2.0),
            }),
            ..Default::default()
        };
        w.set_input(0, pose(center + V2::new(10.0, 5.0)));
        w.step();
        let hero = w.hero(0).unwrap();
        assert_eq!(hero.pos, center + V2::new(10.0, 5.0));
        assert_eq!(hero.facing, V2::new(0.0, 1.0));
        assert!(hero.moving);

        w.set_input(0, pose(center + V2::new(500.0, 0.0)));
        w.step();
        assert!((w.hero(0).unwrap().pos.distance(center) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn a_jump_rises_lands_and_is_used_once() {
        let mut w = world();
        w.set_input(
            0,
            PlayerInput {
                jump: true,
                ..Default::default()
            },
        );
        w.step();
        assert!(w.hero(0).unwrap().air > 0.0);
        let mut peak = 0.0f32;
        for _ in 0..TICK_HZ {
            w.step();
            peak = peak.max(w.hero(0).unwrap().air);
        }
        // v² / 2g: about 1.64 yards.
        assert!((1.4..1.8).contains(&peak), "peak {peak}");
        assert_eq!(
            w.hero(0).unwrap().air,
            0.0,
            "back on the ground after a second"
        );
        steps(&mut w, 10);
        assert_eq!(w.hero(0).unwrap().air, 0.0, "the held input jumped again");
    }

    #[test]
    fn hero_cannot_leave_the_leash() {
        let mut w = world();
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(0.0, 1.0),
                pick: None,
                jump: false,
                pose: None,
            },
        );
        steps(&mut w, TICK_HZ as usize * 60);
        let off = w.hero(0).unwrap().pos.distance(w.center());
        assert!(off <= 150.0 + 1e-3, "off {off}");
    }

    #[test]
    fn enemies_spawn_and_close_in() {
        let mut w = world();
        steps(&mut w, 5);
        assert!(!w.enemies.is_empty());
        let hero = w.hero(0).unwrap().pos;
        let id = w.enemies[0].id;
        let before = w.enemy(id).unwrap().pos.distance(hero);
        steps(&mut w, 10);
        if let Some(e) = w.enemy(id) {
            assert!(e.pos.distance(hero) < before);
        }
    }

    #[test]
    fn frostbolt_kills_give_gems_and_experience() {
        let mut w = world();
        steps(&mut w, TICK_HZ as usize * 20);
        let events = w.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Cast { visual: 116, .. })));
        assert!(w.kills > 0);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::GemCollected { .. }))
                || !w.gems.is_empty()
        );
    }

    #[test]
    fn missiles_land_after_their_flight() {
        let mut w = world();
        let center = w.center();
        w.enemies.push(Enemy {
            id: 999,
            def: 0,
            pos: center + V2::new(14.0, 0.0),
            facing: V2::ZERO,
            moving: false,
            hp: 1000.0,
            max_hp: 1000.0,
            damage: 0.0,
            radius: 0.6,
            attack_cd: 100.0,
        });
        // Frostbolt's first cast comes after 0.5 s, and 14 yards at 28 yd/s takes 0.5 s more.
        steps(&mut w, 16);
        assert_eq!(
            w.enemy(999).unwrap().hp,
            1000.0,
            "landed before the missile arrived"
        );
        steps(&mut w, 20);
        assert!(w.enemy(999).unwrap().hp < 1000.0, "never landed");
    }

    #[test]
    fn level_up_pauses_until_every_hero_picks() {
        let mut w = world();
        w.xp = xp_to_next(1);
        w.step();
        assert_eq!(w.level, 2);
        assert!(w.paused());
        let offer = w.hero(0).unwrap().offers[0].clone();
        assert!(!offer.cards.is_empty());
        let tick = w.tick;
        steps(&mut w, 10);
        assert_eq!(w.tick, tick, "the run moved while cards were up");

        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::ZERO,
                pick: Some(0),
                jump: false,
                pose: None,
            },
        );
        w.step();
        assert!(!w.paused());
        let hero = w.hero(0).unwrap();
        match offer.cards[0] {
            Card::Learn(a) => assert!(hero.abilities.iter().any(|o| o.ability == a)),
            Card::Upgrade(a) => {
                assert_eq!(
                    hero.abilities
                        .iter()
                        .find(|o| o.ability == a)
                        .unwrap()
                        .level,
                    2
                )
            }
        }
    }

    #[test]
    fn a_pick_is_used_once() {
        let mut w = world();
        w.xp = xp_to_next(1) + xp_to_next(2);
        w.step();
        assert_eq!(w.hero(0).unwrap().offers.len(), 2);
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::ZERO,
                pick: Some(0),
                jump: false,
                pose: None,
            },
        );
        w.step();
        w.step();
        assert_eq!(w.hero(0).unwrap().offers.len(), 1);
    }

    #[test]
    fn a_hero_with_no_health_falls_and_the_run_ends() {
        let mut w = world();
        w.hurt_hero(0, 10_000.0);
        assert!(!w.hero(0).unwrap().alive);
        assert!(w.game_over());
        assert!(w.drain_events().contains(&Event::HeroDied { player: 0 }));
        let tick = w.tick;
        w.step();
        assert_eq!(w.tick, tick);
    }

    #[test]
    fn two_heroes_share_experience_and_both_get_cards() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.xp = xp_to_next(1);
        w.step();
        assert!(w.hero(0).unwrap().offers.len() == 1 && w.hero(1).unwrap().offers.len() == 1);
        // One player picking is not enough: the run waits for the other.
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::ZERO,
                pick: Some(0),
                jump: false,
                pose: None,
            },
        );
        w.step();
        assert!(w.paused());
        w.set_input(
            1,
            PlayerInput {
                move_dir: V2::ZERO,
                pick: Some(0),
                jump: false,
                pose: None,
            },
        );
        w.step();
        assert!(!w.paused());
    }

    #[test]
    fn more_heroes_bring_a_bigger_horde() {
        assert!(target_alive(60.0, 2) > target_alive(60.0, 1));
    }

    #[test]
    fn same_seed_same_run() {
        let run = || {
            let mut w = world();
            w.set_input(
                0,
                PlayerInput {
                    move_dir: V2::new(0.3, 1.0),
                    pick: None,
                    jump: false,
                    pose: None,
                },
            );
            steps(&mut w, TICK_HZ as usize * 10);
            (w.kills, w.enemies.len(), w.hero(0).unwrap().hp)
        };
        assert_eq!(run(), run());
    }
}
