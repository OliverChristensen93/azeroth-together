//! The world: heroes, enemies, missiles in flight and areas on the ground, stepped [`TICK_HZ`]
//! times a second.

mod cast;
mod enemies;
mod missiles;

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::content::{Content, School};
use crate::math::V2;
use crate::rng::Rng;

pub const TICK_HZ: u32 = 30;
pub const DT: f32 = 1.0 / TICK_HZ as f32;
/// How much slower a Chilled enemy moves and winds up.
pub const CHILL_SLOW: f32 = 0.5;
/// A critical strike's damage, as WoW's spells crit.
pub const CRIT_MULT: f32 = 1.5;
/// A hero's size, for what hits them.
pub const HERO_RADIUS: f32 = 0.6;
/// How long control lasts on a tough enemy, against a normal one.
const TOUGH_CONTROL: f32 = 0.5;
/// Seconds between a burn's ticks.
const BURN_TICK: f32 = 1.0;

pub type PlayerId = u8;

/// One player's controls, held until the next input replaces them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    /// The direction to walk, any length; zero stands still.
    pub move_dir: V2,
    /// The ground point under the cursor: where every ability goes.
    pub aim: V2,
    /// The slot whose key is held down; held, it casts again as soon as it can.
    pub cast: Option<u8>,
    /// Roll: a quick dodge toward `move_dir` (or ahead, standing still). A press, kept until a
    /// tick takes it.
    #[serde(default)]
    pub dodge: bool,
    /// Wake at the start again, once every hero has fallen.
    #[serde(default)]
    pub restart: bool,
}

/// What a hero is doing with their hands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Idle,
    /// Casting the ability on `slot`; it goes off when `left` runs out.
    Casting { slot: u8, left: f32, total: f32 },
    /// Channelling the ability on `slot`: it goes off again every `next` seconds until `left`
    /// runs out, the key is let go or the hero moves. `area` is a channelled ground spell's area.
    Channeling {
        slot: u8,
        left: f32,
        total: f32,
        next: f32,
        area: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hero {
    pub player: PlayerId,
    pub class: usize,
    pub pos: V2,
    /// Toward the aim, length 1.
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub alive: bool,
    /// Seconds left on each slot's cooldown.
    pub cooldowns: Vec<f32>,
    pub action: Action,
    /// Damage a Guard still soaks.
    pub absorb: f32,
    /// Until when a Guard holds (world time), and whether it makes the hero untouchable.
    pub guard_until: f32,
    pub immune: bool,
    /// A dodge roll under way: its direction (length 1) and seconds left.
    pub roll: Option<(V2, f32)>,
    /// Seconds before the next roll.
    pub roll_cooldown: f32,
}

impl Hero {
    pub fn is_rolling(&self) -> bool {
        self.roll.is_some()
    }

    /// Untouchable, and unable to act, under an immune Guard (Ice Block).
    pub fn is_immune(&self, now: f32) -> bool {
        self.immune && now < self.guard_until
    }
}

/// What an enemy is doing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EnemyState {
    /// At home, not yet aware of any hero.
    Idle,
    /// Pulled too far from home (or with no hero left standing): running back, untouchable,
    /// healing as it goes.
    Evade,
    /// Walking to (or, for a Ranged one, keeping its distance from) the nearest hero.
    Chase,
    /// Telegraphing a blow at `at` (a point on the ground); it lands when `left` runs out.
    Windup { left: f32, total: f32, at: V2 },
    /// Standing after a blow.
    Recover { left: f32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Burn {
    pub per_tick: f32,
    pub ticks: u32,
    pub next: f32,
    pub by: PlayerId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enemy {
    pub id: u32,
    pub kind: usize,
    pub pos: V2,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub state: EnemyState,
    /// World times until which it is Chilled and Frozen.
    pub chill_until: f32,
    pub frozen_until: f32,
    pub burn: Option<Burn>,
    /// Where it stands when no hero bothers it, and the camp slot it fills, if any.
    pub home: V2,
    pub camp: Option<usize>,
}

impl Enemy {
    pub fn is_frozen(&self, now: f32) -> bool {
        now < self.frozen_until
    }

    pub fn is_chilled(&self, now: f32) -> bool {
        now < self.chill_until
    }
}

/// What a telegraph looks like on the ground, for the client to draw while an enemy winds up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Telegraph {
    /// A wedge from `from` toward `dir`, `radius` long and `angle` radians wide.
    Arc {
        from: V2,
        dir: V2,
        radius: f32,
        angle: f32,
        progress: f32,
    },
    /// A circle at `at`.
    Circle { at: V2, radius: f32, progress: f32 },
    /// A line from `from` to `to` (a missile about to be thrown).
    Line { from: V2, to: V2, progress: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Owner {
    Hero(PlayerId),
    Enemy(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub id: u32,
    pub owner: Owner,
    /// The hero's ability it belongs to; `None` for an enemy's.
    pub ability: Option<usize>,
    /// The Spell.dbc visual the client draws it with.
    pub visual: u32,
    pub pos: V2,
    /// Yards a second.
    pub vel: V2,
    pub radius: f32,
    pub range_left: f32,
    pub pierce_left: u32,
    pub damage: f32,
    pub school: School,
    /// Enemies already hit, so a piercing one hits each once.
    pub hit: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Area {
    pub id: u32,
    pub caster: PlayerId,
    pub ability: usize,
    pub pos: V2,
    pub radius: f32,
    /// Seconds before its first hit lands.
    pub delay: f32,
    /// Seconds it lasts after that.
    pub left: f32,
    /// Seconds to its next tick.
    pub next: f32,
    /// Held up by its caster's channel: it ends when the channel does.
    pub channel: bool,
    /// Its opening hit has landed.
    pub opened: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    Enemy(u32),
    Hero(PlayerId),
}

/// What happened in a step, for the client to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A hero began a cast with a cast time.
    CastStart {
        hero: PlayerId,
        ability: usize,
        time: f32,
    },
    /// A hero's ability went off, aimed at `at`.
    Cast {
        hero: PlayerId,
        ability: usize,
        at: V2,
    },
    /// A cast or channel was stopped before it finished.
    CastStopped { hero: PlayerId },
    MissileLaunched { id: u32 },
    /// A missile is gone: it struck something at `at`, or flew out.
    MissileGone { id: u32, at: V2, struck: bool },
    AreaStarted { id: u32 },
    AreaEnded { id: u32 },
    Damage {
        target: Target,
        amount: f32,
        crit: bool,
        school: School,
    },
    /// A hero's Guard soaked a blow (or Ice Block made it miss).
    Absorbed { hero: PlayerId, amount: f32 },
    Frozen { enemy: u32 },
    Blinked { hero: PlayerId, from: V2, to: V2 },
    /// A hero rolled from `from` toward `to`.
    Dodged { hero: PlayerId, from: V2, to: V2 },
    /// An enemy noticed a hero (or was hit) and comes for them.
    EnemyAggro { id: u32 },
    /// An enemy gave up and runs home.
    EnemyEvade { id: u32 },
    EnemySpawned { id: u32 },
    /// An enemy began a telegraphed blow.
    EnemyWindup { id: u32 },
    /// An enemy's blow landed (on whoever was still in it).
    EnemyStrike { id: u32 },
    EnemyDied { id: u32, by: Option<PlayerId> },
    HeroDied { hero: PlayerId },
    /// The fallen heroes woke at the start; the enemies went home.
    Restarted,
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
            WorldError::UnknownZone(k) => write!(f, "no zone `{k}`"),
            WorldError::UnknownClass(k) => write!(f, "no class `{k}`"),
            WorldError::PlayerTaken(p) => write!(f, "player {p} already has a hero"),
        }
    }
}

impl std::error::Error for WorldError {}

pub struct World {
    content: Content,
    zone: usize,
    time: f32,
    rng: Rng,
    heroes: Vec<Hero>,
    enemies: Vec<Enemy>,
    projectiles: Vec<Projectile>,
    areas: Vec<Area>,
    inputs: HashMap<PlayerId, PlayerInput>,
    events: Vec<Event>,
    next_id: u32,
    /// Every camp's places: who stands there now, and when the next comes if nobody does.
    slots: Vec<CampSlot>,
    /// Whether the camps are filled (tests turn it off and place their own).
    camps_on: bool,
}

/// One place in a camp.
#[derive(Clone, Debug)]
struct CampSlot {
    kind: usize,
    home: V2,
    respawn: f32,
    /// The enemy standing there, if one is.
    alive: Option<u32>,
    /// World time the next one comes, once it is empty.
    back_at: f32,
}

impl World {
    pub fn new(content: Content, zone: &str, seed: u64) -> Result<World, WorldError> {
        let zone = content
            .zone(zone)
            .ok_or_else(|| WorldError::UnknownZone(zone.to_string()))?;
        let mut rng = Rng::new(seed);
        let mut slots = Vec::new();
        for (c, camp) in content.zones[zone].camps.iter().enumerate() {
            let kind = content.zone_camps[zone][c];
            let at = V2::new(camp.at.0, camp.at.1);
            for i in 0..camp.count {
                // The first in the middle, the rest around it.
                let home = if i == 0 {
                    at
                } else {
                    at + V2::from_angle(
                        rng.range(0.0, std::f32::consts::TAU),
                        rng.range(camp.spread * 0.4, camp.spread),
                    )
                };
                slots.push(CampSlot {
                    kind,
                    home,
                    respawn: camp.respawn,
                    alive: None,
                    back_at: 0.0,
                });
            }
        }
        Ok(World {
            content,
            zone,
            time: 0.0,
            rng,
            heroes: Vec::new(),
            enemies: Vec::new(),
            projectiles: Vec::new(),
            areas: Vec::new(),
            inputs: HashMap::new(),
            events: Vec::new(),
            next_id: 1,
            slots,
            camps_on: true,
        })
    }

    /// A hero of `class` for `player`, at the zone's start.
    pub fn add_hero(&mut self, player: PlayerId, class: &str) -> Result<(), WorldError> {
        if self.heroes.iter().any(|h| h.player == player) {
            return Err(WorldError::PlayerTaken(player));
        }
        let class = self
            .content
            .class(class)
            .ok_or_else(|| WorldError::UnknownClass(class.to_string()))?;
        let c = &self.content.classes[class];
        let slots = self.content.slots(class).len();
        let n = self.heroes.len() as f32;
        self.heroes.push(Hero {
            player,
            class,
            pos: self.start() + V2::from_angle(n * 2.1, if n > 0.0 { 2.0 } else { 0.0 }),
            facing: V2::new(1.0, 0.0),
            hp: c.hp,
            max_hp: c.hp,
            mana: c.mana,
            max_mana: c.mana,
            alive: true,
            cooldowns: vec![0.0; slots],
            action: Action::Idle,
            absorb: 0.0,
            guard_until: 0.0,
            immune: false,
            roll: None,
            roll_cooldown: 0.0,
        });
        Ok(())
    }

    /// Whether the zone's camps are filled; off, the world has only those
    /// [`World::spawn_enemy`] puts in.
    pub fn set_camps(&mut self, on: bool) {
        self.camps_on = on;
    }

    /// The player's controls from now on. A dodge press stays until a tick takes it, so one that
    /// comes between ticks is not lost.
    pub fn set_input(&mut self, player: PlayerId, mut input: PlayerInput) {
        if self.inputs.get(&player).is_some_and(|i| i.dodge) {
            input.dodge = true;
        }
        self.inputs.insert(player, input);
    }

    /// One tick.
    pub fn step(&mut self) {
        if self.restart_wanted() {
            self.restart();
            return;
        }
        self.time += DT;
        if self.camps_on {
            self.step_camps();
        }
        for h in 0..self.heroes.len() {
            self.step_hero(h);
            let player = self.heroes[h].player;
            if let Some(i) = self.inputs.get_mut(&player) {
                i.dodge = false;
            }
        }
        self.step_projectiles();
        self.step_areas();
        self.step_enemies();
        self.enemies.retain(|e| e.hp > 0.0);
    }

    /// Takes the events since the last call.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    pub fn zone(&self) -> usize {
        self.zone
    }

    /// Seconds since the run began.
    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn heroes(&self) -> &[Hero] {
        &self.heroes
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    pub fn enemies(&self) -> &[Enemy] {
        &self.enemies
    }

    pub fn projectiles(&self) -> &[Projectile] {
        &self.projectiles
    }

    pub fn areas(&self) -> &[Area] {
        &self.areas
    }

    /// Where the zone's heroes start.
    pub fn start(&self) -> V2 {
        let (x, y, _) = self.content.zones[self.zone].start;
        V2::new(x, y)
    }

    /// Every hero has fallen.
    pub fn is_over(&self) -> bool {
        !self.heroes.is_empty() && self.heroes.iter().all(|h| !h.alive)
    }

    /// The ability on a hero's slot.
    pub fn slot_ability(&self, hero: &Hero, slot: u8) -> Option<usize> {
        self.content.slots(hero.class).get(slot as usize).copied()
    }

    fn id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn restart_wanted(&self) -> bool {
        self.is_over() && self.inputs.values().any(|i| i.restart)
    }

    /// Every hero wakes at the start, whole, as a WoW player released to the graveyard would;
    /// the enemies go home, whole, and forget them. Missiles and areas are gone.
    pub fn restart(&mut self) {
        let start = self.start();
        for (n, h) in self.heroes.iter_mut().enumerate() {
            let c = &self.content.classes[h.class];
            h.pos = start + V2::from_angle(n as f32 * 2.1, if n > 0 { 2.0 } else { 0.0 });
            h.hp = c.hp;
            h.mana = c.mana;
            h.alive = true;
            h.cooldowns.iter_mut().for_each(|c| *c = 0.0);
            h.action = Action::Idle;
            h.absorb = 0.0;
            h.guard_until = 0.0;
            h.immune = false;
            h.roll = None;
            h.roll_cooldown = 0.0;
        }
        for e in &mut self.enemies {
            e.pos = e.home;
            e.hp = e.max_hp;
            e.state = EnemyState::Idle;
            e.burn = None;
            e.chill_until = 0.0;
            e.frozen_until = 0.0;
        }
        self.projectiles.clear();
        self.areas.clear();
        self.events.push(Event::Restarted);
    }

    /// Damage from a hero's ability on enemy `e`: rolls the crit (every hit on a frozen enemy is
    /// one: Shatter), then leaves the ability's effects. `from` is where it came from, for
    /// knockback.
    fn hit_enemy(&mut self, e: usize, by: PlayerId, ability: usize, damage: f32, from: V2) {
        if self.enemies[e].hp <= 0.0 || self.enemies[e].state == EnemyState::Evade {
            return;
        }
        let crit_chance = self
            .heroes
            .iter()
            .find(|h| h.player == by)
            .map_or(0.0, |h| self.content.classes[h.class].crit);
        let frozen = self.enemies[e].is_frozen(self.time);
        let crit = frozen || self.rng.f32() < crit_chance;
        let amount = if crit { damage * CRIT_MULT } else { damage };
        let ab = &self.content.abilities[ability];
        let school = ab.school;
        let fx = ab.effects.clone();
        let tough = self.content.enemies[self.enemies[e].kind].tough;
        let control = if tough { TOUGH_CONTROL } else { 1.0 };
        if amount > 0.0 {
            self.damage_enemy(e, amount, crit, school, Some(by));
        }
        let now = self.time;
        let enemy = &mut self.enemies[e];
        if fx.chill > 0.0 {
            enemy.chill_until = enemy.chill_until.max(now + fx.chill * control);
        }
        if fx.freeze > 0.0 && enemy.hp > 0.0 {
            enemy.frozen_until = enemy.frozen_until.max(now + fx.freeze * control);
            // A frozen enemy's blow never lands.
            enemy.state = EnemyState::Chase;
            let id = enemy.id;
            self.events.push(Event::Frozen { enemy: id });
        }
        let enemy = &mut self.enemies[e];
        if fx.burn > 0.0 && enemy.hp > 0.0 {
            let ticks = (fx.burn_secs / BURN_TICK).ceil().max(1.0) as u32;
            enemy.burn = Some(Burn {
                per_tick: fx.burn / ticks as f32,
                ticks,
                next: BURN_TICK,
                by,
            });
        }
        if fx.knockback > 0.0 && !tough {
            let away = (enemy.pos - from).normalize_or_zero();
            enemy.pos += away * fx.knockback;
        }
    }

    /// Takes `amount` off enemy `e`, and marks it dead at 0 (it leaves at the end of the tick).
    fn damage_enemy(
        &mut self,
        e: usize,
        amount: f32,
        crit: bool,
        school: School,
        by: Option<PlayerId>,
    ) {
        let enemy = &mut self.enemies[e];
        if enemy.hp <= 0.0 || enemy.state == EnemyState::Evade {
            return;
        }
        enemy.hp -= amount;
        let id = enemy.id;
        let died = enemy.hp <= 0.0;
        self.events.push(Event::Damage {
            target: Target::Enemy(id),
            amount,
            crit,
            school,
        });
        if died {
            self.events.push(Event::EnemyDied { id, by });
            if let Some(slot) = self.enemies[e].camp {
                self.slots[slot].alive = None;
                self.slots[slot].back_at = self.time + self.slots[slot].respawn;
            }
        } else if by.is_some() {
            self.aggro(e);
        }
    }

    /// A blow on hero `h`: an immune Guard turns it away, an absorbing one soaks it first.
    fn damage_hero(&mut self, h: usize, amount: f32, school: School) {
        let now = self.time;
        let hero = &mut self.heroes[h];
        if !hero.alive {
            return;
        }
        let player = hero.player;
        if hero.is_immune(now) {
            self.events.push(Event::Absorbed {
                hero: player,
                amount,
            });
            return;
        }
        let mut amount = amount;
        if hero.absorb > 0.0 && now < hero.guard_until {
            let soaked = hero.absorb.min(amount);
            hero.absorb -= soaked;
            amount -= soaked;
            self.events.push(Event::Absorbed {
                hero: player,
                amount: soaked,
            });
        }
        if amount <= 0.0 {
            return;
        }
        let hero = &mut self.heroes[h];
        hero.hp -= amount;
        self.events.push(Event::Damage {
            target: Target::Hero(player),
            amount,
            crit: false,
            school,
        });
        let hero = &mut self.heroes[h];
        if hero.hp <= 0.0 {
            hero.hp = 0.0;
            hero.alive = false;
            hero.action = Action::Idle;
            self.end_channel_area(player);
            self.events.push(Event::HeroDied { hero: player });
        }
    }
}
