//! One run: the heroes, the horde, the experience on the ground, stepped [`TICK_HZ`] times a
//! second by [`World::step`].
//!
//! A step reads each player's latest [`PlayerInput`], moves everything, fires abilities, lands
//! hits, runs the arena's shrines, chests, pots and power-ups, and records what happened as
//! [`Event`]s for the client to show. The world itself is the authority: a client only ever draws
//! it, which is how a co-op host runs it for everyone.
//!
//! Gold is earned together and spent alone: every coin a hero picks up is credited to every
//! hero's own purse (each with their own [`Stat::GoldGain`]), as experience is shared, so nobody
//! races a friend for coins; but each hero pays for their own chests, so nobody spends a
//! friend's gold.

mod arena;
mod classes;
mod combat;
mod druid;
mod gear;
mod affixes;
mod hazards;
mod horde;
mod pause;
pub(crate) mod ranks;
pub use ranks::CAMP_SCALE;
pub(crate) mod meter;
pub use meter::OTHER_HEALING;
mod hunter;
mod paladin;
mod priest;
mod rogue;
mod shaman;
mod warlock;
mod warrior;

pub use classes::{ActiveBuff, Bank, Boon, HeroFx, StatusFx};
pub use combat::{shield_size, status_bits, HeroSpells, Status};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

use crate::augments::{milestone_rank, Augment, Milestone};
use crate::cards::{
    ability_stats, fallback_gold, AbilityStats, Card, CardKind, Rarity, SpellMods, MAX_RANK,
};
use crate::content::{AbilityKind, Content};
use crate::gear::{GearPiece, HeroGear, Slot};
use crate::interact::{
    chest_price, escort_time, scatter, spot, InteractState, Interactable, InteractableKind,
    ESCORT_DISTANCE, ESCORT_GOAL,
};
use crate::items::{
    ward_cooldown, Item, EMBER_RADIUS, EMBER_SHARE, FANG_HEAL, PURSE_GOLD, SHARD_CHAIN,
    SHARD_REACH, SHARD_SHARE,
};
use crate::math::V2;
use crate::mechanics::{Affix, Mechanic, Shape};
use crate::powerups::{Pickup, PickupKind, PotLoot, PowerUp, BERSERK, BLOODLUST, SWIFTNESS};
use crate::rng::Rng;
use crate::spell::CastSpec;
#[cfg(test)]
use crate::stats::PACT_BOON;
use crate::stats::{Bane, Difficulty, Pact, Stat, Stats};

/// Simulation steps per second.
pub const TICK_HZ: u32 = 30;
/// Seconds per step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// Which player a hero belongs to: 0 for the host, then one per player who joins.
pub type PlayerId = u8;

/// The most abilities a hero carries.
pub const MAX_ABILITIES: usize = 6;
/// Cards dealt per level-up.
const HAND: usize = 3;
/// Free re-deals of a hand, per run.
const REROLLS: u32 = 3;
/// Spells a hero can banish from the card pool in a run.
const BANISHES: u8 = 1;
/// A hero's body radius, which enemies stop at.
const HERO_RADIUS: f32 = 0.7;
/// Within this many yards of a hero an experience gem is collected, before [`Stat::Pickup`].
const MAGNET: f32 = 4.5;
/// The chance of a critical strike, before [`Stat::Crit`].
const CRIT_CHANCE: f32 = crate::spell::BASE_CRIT;
/// The chance an enemy's gem also carries gold.
const GOLD_CHANCE: f32 = 0.3;
/// A gem's gold, per point of the enemy's experience.
const GOLD_PER_XP: f32 = 1.5;
/// Random tomes mixed into the options a level-up deals from.
const LEVEL_UP_TOMES: usize = 2;
/// An elite's health (in tests; the rules' `elite_health` in play), experience and gold, in
/// ordinary enemies of its kind...
#[cfg(test)]
pub(crate) const ELITE_HEALTH: f32 = 15.0;
const ELITE_REWARD: u32 = 5;
/// ...its damage and its size.
const ELITE_DAMAGE: f32 = 1.25;
pub const ELITE_SCALE: f32 = 1.3;
/// A Challenge Idol raises this many rares, and this many more per hero (two alone)...
const CHALLENGE_ELITES: usize = 1;
const CHALLENGE_PER_HERO: usize = 1;
/// ...in a ring this far from it.
const CHALLENGE_RING: (f32, f32) = (9.0, 14.0);
/// The zone's boss comes this many seconds in, and again as many after each one falls.
pub const BOSS_EVERY: f32 = 240.0;
/// A camp's elite chases a hero who comes this near, and otherwise keeps to its camp.
pub const ELITE_AGGRO: f32 = 20.0;
/// From this many seconds in, an ordinary spawn is now and then an elite.
const ELITES_FROM: f32 = 90.0;
const ELITE_CHANCE: f32 = 0.015;
/// Seconds a used interactable stands before it leaves the world.
const SPENT_LINGER: f32 = 6.0;
/// Enemies spawn on a ring this far from a hero: outside the view, inside the leash.
const SPAWN_RING: (f32, f32) = (34.0, 42.0);
/// Seconds between spawn batches.
const SPAWN_EVERY: f32 = 0.12;
/// The share of its health a living hero gets back at each level-up.
const LEVEL_UP_HEAL: f32 = 0.1;
/// How far a camp's elite follows a hero from its camp. Past it for [`LEASH_GRACE`] seconds, it
/// gives up the chase, runs home out of reach (at this times its speed) and heals, as WoW's mobs
/// evade. Clients draw the leash as a ring round the camp.
pub const CAMP_LEASH: f32 = 57.5;
pub const LEASH_GRACE: f32 = 3.0;
const EVADE_SPEED: f32 = 1.6;
/// Seconds between workings-out of the horde's way around obstacles.
const FLOW_EVERY: f32 = 0.4;
/// The run's length; enemy health and damage grow toward it.
const RUN_SECS: f32 = 900.0;
/// A living hero this close to a fallen friend raises them...
pub const REVIVE_RANGE: f32 = 3.5;
/// ...after standing by for this many seconds.
pub const REVIVE_SECS: f32 = 3.0;
/// A raised hero's share of its health.
const REVIVE_HEALTH: f32 = 0.5;
/// Seconds a raised hero cannot be hurt, to get clear of the horde that felled it.
const REVIVE_WARD: f32 = 2.0;

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    /// The direction to run, any length; zero stands still.
    pub move_dir: V2,
    /// A card from the hero's current [`Offer`], by index.
    pub pick: Option<u8>,
    /// Deal the current hand again, if the hero has rerolls left.
    pub reroll: bool,
    /// Pass on the current hand.
    pub skip: bool,
    /// Hop, if on the ground.
    pub jump: bool,
    /// The hero's movement ability was used this input: the pose that comes with it is where it
    /// landed (augments act on where it left and where it landed).
    #[serde(default)]
    pub dash: bool,
    /// Freeze the level-up hand in front, or unfreeze it.
    #[serde(default)]
    pub freeze: bool,
    /// Banish the spell on this card of the hand in front, for the rest of the run.
    #[serde(default)]
    pub banish: Option<u8>,
    /// Where the player's own client put the hero, overriding `move_dir` and `jump`. A client
    /// running WoW's character controller moves its hero itself, as WoW clients do, and reports
    /// the pose; the world only keeps it inside the arena.
    pub pose: Option<HeroPose>,
    /// Switch on the hero's dormant gear piece in this slot (its index in [`Slot::ALL`]),
    /// paying its gold.
    #[serde(default)]
    pub gear_on: Option<u8>,
    /// The player has the game menu open: the run waits, as for a hand of cards. Held, like
    /// movement, until an input says otherwise.
    #[serde(default)]
    pub menu: bool,
    /// Ready to go on after a pause ([`World::tend_pause`]). Used once.
    #[serde(default)]
    pub ready: bool,
    /// The player gives up: their hero falls for good. Used once.
    #[serde(default)]
    pub give_up: bool,
}

/// A hero's position and facing as its player's client reports them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
}

/// A jump's initial upward speed and the gravity that brings the hero back down, in yards and
/// seconds: the 1.12 client's own movement constants.
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Something an ability, a swing or an event can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Hero(PlayerId),
    Enemy(u32),
}

/// What happened during a step, for the client to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A hero cast `ability` (its `visual` spell) at `targets`, or at the ground at `dest`; it is
    /// ready again in `cooldown` seconds.
    Cast {
        caster: PlayerId,
        ability: usize,
        visual: u32,
        targets: Vec<Target>,
        dest: Option<V2>,
        cooldown: f32,
    },
    Damage {
        target: Target,
        amount: f32,
        crit: bool,
    },
    /// An elite or boss marked the ground (`shape` laid at `pos`) for `kind`, going off in
    /// `secs`.
    Telegraph {
        id: u32,
        kind: Mechanic,
        pos: V2,
        shape: Shape,
        secs: f32,
    },
    /// A marking went off, or was called off: it goes.
    TelegraphEnd { id: u32 },
    /// An elite or boss cast `visual` at `targets`, or at the ground at `dest`.
    EnemyCast {
        enemy: u32,
        visual: u32,
        targets: Vec<Target>,
        dest: Option<V2>,
    },
    /// A mechanic threw or pulled hero `hero` to `to`; its own client moves it there.
    Shoved { hero: PlayerId, to: V2 },
    /// An affix joined the run.
    AffixGained { affix: Affix },
    /// For `secs` the horde comes from every side, not only from behind.
    Surrounded { secs: f32 },
    /// An enemy hit a hero in melee.
    Swing {
        enemy: u32,
        hero: PlayerId,
        amount: f32,
    },
    EnemyDied {
        id: u32,
    },
    /// An enemy left experience (and perhaps gold) on the ground where it died.
    GemDropped {
        id: u32,
        pos: V2,
        xp: u32,
        gold: u32,
    },
    HeroDied {
        player: PlayerId,
    },
    /// A fallen hero was raised by a friend standing by.
    HeroRevived {
        player: PlayerId,
        by: PlayerId,
    },
    GemCollected {
        id: u32,
        by: PlayerId,
    },
    /// The party reached `level`; every living hero has a new [`Offer`].
    LevelUp {
        level: u32,
    },
    /// Gold into a hero's purse.
    GoldGained {
        player: PlayerId,
        amount: u32,
    },
    /// The heroes in `by` filled an interactable's circle, and it did its work.
    Interacted {
        id: u32,
        kind: InteractableKind,
        by: Vec<PlayerId>,
    },
    /// Every elite a Challenge Idol raised is dead; a cache stands where it was.
    ChallengeWon {
        id: u32,
    },
    /// The zone's boss came.
    BossCame {
        def: usize,
    },
    /// A boss died; a cache stands where it fell.
    BossDied {
        id: u32,
    },
    /// A banner's carry ended: `won`, it reached its spot in time and a cache stands there.
    EscortDone {
        id: u32,
        won: bool,
    },
    /// `by` broke pot `id`, and this is what it held.
    PotBroken {
        id: u32,
        by: PlayerId,
        loot: PotLoot,
    },
    /// `by` took something from the ground; a power-up works for the whole party.
    PickupTaken {
        id: u32,
        kind: PickupKind,
        by: PlayerId,
    },
    /// A boss or an elite dropped a piece of equipment for `player`, for the stash.
    GearFound {
        player: PlayerId,
        piece: GearPiece,
    },
    /// `player` paid to switch on the piece it wears in `slot`.
    GearOn {
        player: PlayerId,
        slot: Slot,
    },
}

/// What dealt a hand: a level-up, or an interactable of some kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    LevelUp,
    Interactable(InteractableKind),
    /// A spell reached an augment milestone: an index into the content's abilities.
    Augment(usize),
}

/// The cards a hero is choosing between.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub cards: Vec<Card>,
    /// What dealt it, which also decides what a reroll deals.
    pub source: Source,
}

/// An ability a hero has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Owned {
    /// Index into [`Content::abilities`].
    pub ability: usize,
    /// Its rank, 1 up to [`MAX_RANK`].
    pub level: u8,
    /// Extra damage from the rarity of the cards that taught and raised it (0.25 is +25%).
    pub bonus: f32,
    /// Seconds until it fires again.
    pub cd: f32,
    /// Augments taken, in order: a general one at rank 5, a capstone at 10, a general one at 15.
    #[serde(default)]
    pub augments: Vec<Augment>,
    /// Casts so far, for the augments that act on every few casts.
    #[serde(default)]
    pub casts: u32,
    /// Casts made in the current burst of its charges.
    #[serde(default)]
    pub burst: u32,
    /// What its upgrade cards have added up to.
    #[serde(default)]
    pub mods: SpellMods,
}

impl Owned {
    /// A newly learned spell at rank 1 with a damage `bonus`.
    pub fn new(ability: usize, bonus: f32) -> Owned {
        Owned {
            ability,
            level: 1,
            bonus,
            cd: 0.3,
            augments: Vec::new(),
            casts: 0,
            burst: 0,
            mods: SpellMods::default(),
        }
    }

    /// The augment it waits for, if it has reached a milestone (rank 5, 10 or 15) without taking
    /// it; it ranks no higher until it does. A spell `with_capstones` takes one at rank 10, any
    /// other a general augment.
    pub fn awaiting(&self, with_capstones: bool) -> Option<Milestone> {
        let n = self.augments.len();
        if n >= 3 || self.level < milestone_rank(n) {
            return None;
        }
        Some(if n == 1 && with_capstones {
            Milestone::Capstone
        } else {
            Milestone::General
        })
    }

    pub fn has(&self, augment: Augment) -> bool {
        self.augments.contains(&augment)
    }

    /// Its numbers now, from its rank and bonus alone; [`AbilityStats::with`] adds the hero's
    /// stats.
    pub fn stats(&self, def: &crate::content::AbilityDef) -> AbilityStats {
        ability_stats(def, self.bonus)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    pub player: PlayerId,
    /// What the player called the hero, for the frames over and around it.
    pub name: String,
    /// The body: `ChrRaces` id and sex (0 male, 1 female). Only for show.
    pub race: u8,
    pub sex: u8,
    /// Index into [`Content::classes`].
    pub class: usize,
    pub pos: V2,
    /// The way the hero last moved; the client turns the model to it.
    pub facing: V2,
    /// Whether the hero is running this step.
    pub moving: bool,
    /// The way it has been running lately, smoothed (zero until it first moves): the horde
    /// comes from behind it ([`World::spawn_angle`]).
    #[serde(default)]
    pub heading: V2,
    /// Height above the ground while jumping, 0 on it. Only for show: the game is flat, and a
    /// jump dodges nothing.
    pub air: f32,
    /// Upward speed while jumping, yards per second; 0 on the ground.
    pub vy: f32,
    pub hp: f32,
    /// The class's health with [`Stat::Health`] on top.
    pub max_hp: f32,
    /// Yards per second: the class's speed with [`Stat::Speed`] on top.
    pub speed: f32,
    /// Bonuses from tomes, pacts and items.
    pub stats: Stats,
    /// The tomes taken, each with its rank: at most the rules' `tome_slots` of them, each up to
    /// `tome_max_rank`.
    pub tomes: Vec<(Stat, u8)>,
    /// Gold in this hero's own purse.
    pub gold: u32,
    /// Chests this hero has paid for, which sets the next one's price.
    pub chests: u32,
    /// Items held, each with its number of copies.
    pub items: Vec<(Item, u8)>,
    /// Seconds until a Frost Ward blocks a blow again; 0 ready.
    pub barrier: f32,
    pub abilities: Vec<Owned>,
    /// Level-up hands still to choose from, oldest first.
    pub offers: Vec<Offer>,
    /// Re-deals of a hand left this run.
    pub rerolls: u32,
    /// Its level-up hand is frozen: what is left of it once a card is taken comes back at the
    /// next level-up, instead of a new hand.
    #[serde(default)]
    pub frozen: bool,
    /// A frozen hand's cards, waiting for the next level-up.
    #[serde(default)]
    pub frozen_hand: Vec<Card>,
    /// Banishes left this run, and the spells banished: they never come up again.
    #[serde(default)]
    pub banishes: u8,
    #[serde(default)]
    pub banished: Vec<usize>,
    pub alive: bool,
    /// Fallen: how far a friend standing by has raised it, 0 to 1.
    pub revive: f32,
    /// Seconds left of a raised hero's grace, when nothing can hurt it.
    pub ward: f32,
    /// Damage this hero has dealt over the run, items' included.
    pub damage_done: f64,
    /// The same, by ability (an index into the content's abilities): the damage meter.
    pub damage_by: Vec<(usize, f64)>,
    /// Of that, what went into bosses, by ability too: the meter's boss view.
    #[serde(default)]
    pub boss_damage: f64,
    #[serde(default)]
    pub boss_by: Vec<(usize, f64)>,
    /// Health this hero's healing restored (its own and, through its spells, its friends'), and
    /// what its shields and wards soaked up: the meter's healing view.
    #[serde(default)]
    pub healed: f64,
    #[serde(default)]
    pub absorbed: f64,
    /// The same healing by ability; [`meter::OTHER_HEALING`] for what came from no spell.
    #[serde(default)]
    pub healed_by: Vec<(usize, f64)>,
    /// This step's bookkeeping for [`World::tally_healing`]: health at its start, and what blows
    /// took since.
    #[serde(skip)]
    pub(crate) hp_mark: Option<f32>,
    #[serde(skip)]
    pub(crate) taken: f32,
    /// What its spells' augments keep track of: a shield, streaks, charges, timers.
    #[serde(default)]
    pub spell: HeroSpells,
    /// What its talents from the web between runs give ([`World::apply_talents`]).
    #[serde(default)]
    pub talents: crate::talents::HeroTalents,
    /// The equipment it brought into the run, dormant until switched on, and what it found.
    #[serde(default)]
    pub gear: HeroGear,
}

impl Hero {
    /// The rank of the tome of `stat`, 0 if not taken.
    pub fn tome_rank(&self, stat: Stat) -> u8 {
        self.tomes
            .iter()
            .find(|(s, _)| *s == stat)
            .map_or(0, |&(_, n)| n)
    }

    /// Copies of `item` held.
    pub fn stacks(&self, item: Item) -> u8 {
        self.items
            .iter()
            .find(|(i, _)| *i == item)
            .map_or(0, |&(_, n)| n)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Yards per second.
    pub speed: f32,
    pub radius: f32,
    /// A tougher, bigger one, raised by a Challenge Idol, worth more when it dies.
    pub elite: bool,
    /// The zone's boss, who leaves a cache when it dies.
    pub boss: bool,
    attack_cd: f32,
    /// The hero who hit it last: the one its death counts for.
    #[serde(skip)]
    last_hit: Option<PlayerId>,
    /// A camp's elite: the camp it guards ([`ELITE_AGGRO`]).
    #[serde(skip)]
    home: Option<V2>,
    /// Chilled, Frozen, Stunned, Burning and the rest.
    #[serde(skip)]
    pub status: Status,
    /// An elite's or boss's mechanics: their cooldowns, a wind-up, a charge under way.
    #[serde(skip)]
    act: hazards::Act,
}

/// A spot an elite guards. While its elite lives, `elite` is its id; once it dies, a new one
/// comes after `respawn` seconds (the rules' `elite_respawn`).
#[derive(Clone, Debug, PartialEq)]
pub struct Camp {
    pub pos: V2,
    pub elite: Option<u32>,
    pub respawn: f32,
}

/// Experience on the ground, waiting for a hero to come close; some carry gold too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gem {
    pub id: u32,
    pub pos: V2,
    pub xp: u32,
    pub gold: u32,
}

/// What a co-op client needs to draw the world, sent by the host after each step
/// ([`World::snapshot`], [`World::apply_snapshot`]). Gems are not in it: they never move, so they
/// travel as the [`Event::GemDropped`] and [`Event::GemCollected`] that make and take them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub elapsed: f32,
    pub level: u32,
    pub xp: f32,
    pub kills: u32,
    pub difficulty: Difficulty,
    pub next_boss: f32,
    #[serde(default)]
    pub menu_pause: bool,
    #[serde(default)]
    pub pause_ready: Vec<PlayerId>,
    #[serde(default)]
    pub surround: f32,
    #[serde(default)]
    pub next_surround: f32,
    /// Where the elite camps are, for their leash rings.
    #[serde(default)]
    pub camps: Vec<V2>,
    pub buffs: Vec<(PowerUp, f32)>,
    pub heroes: Vec<Hero>,
    pub enemies: Vec<EnemyState>,
    pub interactables: Vec<Interactable>,
    pub pickups: Vec<Pickup>,
    pub affixes: Vec<Affix>,
}

/// An enemy as a snapshot carries it: only what a client draws, packed small, since a late run
/// has hundreds of them in every snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyState {
    pub id: u32,
    pub def: u16,
    /// From the zone's centre, in sixteenths of a yard.
    pub pos: (i16, i16),
    /// The way it faces, in 256ths of a turn.
    pub facing: u8,
    pub moving: bool,
    pub elite: bool,
    pub boss: bool,
    /// A camp's elite ([`World::guards`]); `elite` without it is a rare.
    #[serde(default)]
    pub guard: bool,
    /// Health and its maximum, rounded up to whole points.
    pub hp: u32,
    pub max_hp: u32,
    /// Frozen, Chilled, Burning, Stunned ([`status_bits`]).
    pub status: u16,
    /// The abilities whose damage over time is on it, each once ([`Status::dots`]).
    #[serde(default)]
    pub dots: Vec<u16>,
}

/// Yards to sixteenths, held to `i16`.
fn pack_yards(yards: f32) -> i16 {
    (yards * 16.0)
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

/// A direction to 256ths of a turn.
fn pack_facing(dir: V2) -> u8 {
    let turns = dir.y.atan2(dir.x) / std::f32::consts::TAU;
    ((turns.rem_euclid(1.0) * 256.0).round() as u32 % 256) as u8
}

fn unpack_facing(f: u8) -> V2 {
    V2::from_angle(f32::from(f) / 256.0 * std::f32::consts::TAU, 1.0)
}

/// Damage on its way: a missile still flying, with the cast it belongs to.
#[derive(Clone, Debug)]
struct PendingHit {
    lands_at: f32,
    caster: PlayerId,
    spec: CastSpec,
    target: u32,
    /// Where it was fired from: the line it pierces along.
    from: V2,
    /// The missile itself, not a returning one: only it pierces.
    primary: bool,
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
    /// The arena's shrines, altars, chests, banners and pots.
    pub interactables: Vec<Interactable>,
    /// Where elites keep camp: each holds one, and a new one some minutes after it dies.
    pub camps: Vec<Camp>,
    /// Health shards and power-ups on the ground.
    pub pickups: Vec<Pickup>,
    /// The party's power-ups at work, with the seconds each has left.
    pub buffs: Vec<(PowerUp, f32)>,
    /// Seconds until the arena next gets a chance at a new shrine, or a new pot.
    shrine_timer: f32,
    pot_timer: f32,
    pending: Vec<PendingHit>,
    /// Blizzards, burning ground and Frozen Orbs.
    grounds: Vec<combat::Ground>,
    /// Ground elites and bosses have marked, about to go off.
    hazards: Vec<hazards::Hazard>,
    /// The affixes in play, how many of the run's stages have added one, and seconds to
    /// Volcanic's next eruption.
    pub affixes: Vec<Affix>,
    affix_stage: usize,
    volcano_timer: f32,
    /// Spells augments asked for, run after the cast that set them off; and echoes, by the time
    /// they are due.
    triggers: Vec<combat::Trigger>,
    delayed: Vec<(f32, combat::Trigger)>,
    /// The first target of the last missile cast, for an augment's second missile elsewhere.
    last_target: Option<u32>,
    /// The spell whose healing context is open ([`meter`]), and this step's heals by spell:
    /// target, healer, ability, amount.
    heal_source: Option<(PlayerId, usize)>,
    heal_credit: Vec<(PlayerId, PlayerId, usize, f32)>,
    inputs: HashMap<PlayerId, PlayerInput>,
    events: Vec<Event>,
    next_id: u32,
    spawn_timer: f32,
    /// The party's shared level and experience toward the next.
    pub level: u32,
    pub xp: f32,
    pub kills: u32,
    /// How much harder pacts have made the horde.
    pub difficulty: Difficulty,
    /// Seconds into the run the boss comes next.
    pub next_boss: f32,
    /// A player opened the game menu, and the run waits until every living hero's player has
    /// said Ready: those who have, so far.
    pub menu_pause: bool,
    pub pause_ready: Vec<PlayerId>,
    /// Players who gave up: their heroes cannot be raised.
    pub quit: Vec<PlayerId>,
    /// Elites running home to their camp, out of reach until they get there ([`CAMP_LEASH`]);
    /// and those past their leash, with the seconds they have been.
    pub evading: Vec<u32>,
    leash_out: Vec<(u32, f32)>,
    /// What stands in the horde's way, once the host's client has filled it in; and seconds
    /// until its flow field is worked out again.
    pub obstacles: Option<crate::obstacles::Obstacles>,
    flow_timer: f32,
    /// The camps' elites alive, by enemy id: the elite rank ([`ranks`]); and how many bosses have
    /// come.
    pub guards: Vec<u32>,
    pub bosses_come: u32,
    /// Seconds left of a swarm, when the horde comes from every side; seconds into the run the
    /// next one starts (infinite once this boss's has come, or while a boss is up); and the
    /// boss time it came for.
    pub surround: f32,
    pub next_surround: f32,
    swarm_for: Option<f32>,
    /// Experience earned per experience picked up: 1 in a normal run.
    pub xp_rate: f32,
    /// The rank a newly learned spell starts at: 1 in a normal run.
    pub learn_rank: u8,
}

impl World {
    /// A run in `zone`, its interactables scattered by `seed`.
    pub fn new(content: Arc<Content>, zone: &str, seed: u64) -> Result<Self, WorldError> {
        let zone = content
            .zone(zone)
            .ok_or_else(|| WorldError::UnknownZone(zone.to_owned()))?;
        let shrine_every = content.rules.shrine_every;
        let mut world = Self {
            content,
            zone,
            rng: Rng::new(seed),
            tick: 0,
            elapsed: 0.0,
            heroes: Vec::new(),
            enemies: Vec::new(),
            gems: Vec::new(),
            interactables: Vec::new(),
            camps: Vec::new(),
            pickups: Vec::new(),
            buffs: Vec::new(),
            shrine_timer: shrine_every,
            pot_timer: arena::POT_EVERY,
            pending: Vec::new(),
            grounds: Vec::new(),
            hazards: Vec::new(),
            affixes: Vec::new(),
            affix_stage: 0,
            volcano_timer: 3.0,
            triggers: Vec::new(),
            delayed: Vec::new(),
            last_target: None,
            heal_source: None,
            heal_credit: Vec::new(),
            inputs: HashMap::new(),
            events: Vec::new(),
            next_id: 1,
            spawn_timer: 0.0,
            level: 1,
            xp: 0.0,
            kills: 0,
            difficulty: Difficulty::default(),
            next_boss: BOSS_EVERY,
            menu_pause: false,
            pause_ready: Vec::new(),
            quit: Vec::new(),
            evading: Vec::new(),
            leash_out: Vec::new(),
            obstacles: None,
            flow_timer: 0.0,
            guards: Vec::new(),
            bosses_come: 0,
            surround: 0.0,
            next_surround: f32::INFINITY,
            swarm_for: None,
            xp_rate: 1.0,
            learn_rank: 1,
        };
        world.next_surround = BOSS_EVERY * 0.5;
        let content = world.content.clone();
        for (kind, pos) in scatter(&content.zones[zone], &mut world.rng) {
            world.place(kind, pos);
        }
        world.place_camps();
        world.place_pots();
        Ok(world)
    }

    /// A new, ready interactable of `kind` at `pos`.
    pub fn place(&mut self, kind: InteractableKind, pos: V2) -> u32 {
        let id = self.id();
        self.interactables.push(Interactable {
            id,
            kind,
            pos,
            charge: 0.0,
            state: InteractState::Ready,
        });
        id
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

    /// Give `player` a hero of `class`, standing near the arena's centre: a male human named
    /// "Hero" (see [`World::add_hero_as`]).
    pub fn add_hero(&mut self, player: PlayerId, class: &str) -> Result<(), WorldError> {
        self.add_hero_as(player, class, "Hero", 1, 0)
    }

    /// Give `player` a hero of `class` with a name and body, standing near the arena's centre.
    pub fn add_hero_as(
        &mut self,
        player: PlayerId,
        class: &str,
        name: &str,
        race: u8,
        sex: u8,
    ) -> Result<(), WorldError> {
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
            name: name.to_owned(),
            race,
            sex,
            class: class_ix,
            pos,
            facing: V2::new(1.0, 0.0),
            moving: false,
            heading: V2::ZERO,
            air: 0.0,
            vy: 0.0,
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
            stats: Stats::default(),
            tomes: Vec::new(),
            gold: 0,
            chests: 0,
            items: Vec::new(),
            barrier: 0.0,
            abilities: vec![Owned {
                cd: 0.5,
                ..Owned::new(def.start_ix, 0.0)
            }],
            offers: Vec::new(),
            rerolls: REROLLS,
            frozen: false,
            frozen_hand: Vec::new(),
            banishes: BANISHES,
            banished: Vec::new(),
            alive: true,
            revive: 0.0,
            ward: 0.0,
            damage_done: 0.0,
            damage_by: Vec::new(),
            boss_damage: 0.0,
            boss_by: Vec::new(),
            healed: 0.0,
            absorbed: 0.0,
            healed_by: Vec::new(),
            hp_mark: None,
            taken: 0.0,
            spell: HeroSpells::default(),
            talents: Default::default(),
            gear: HeroGear::default(),
        });
        self.apply_race(self.heroes.len() - 1);
        Ok(())
    }

    /// Hero `h`'s racial passives ([`crate::content::RaceDef`]) join its stats.
    fn apply_race(&mut self, h: usize) {
        let content = self.content.clone();
        let Some(race) = content.race(self.heroes[h].race) else {
            return;
        };
        for passive in &race.passives {
            for &(stat, amount) in &passive.stats {
                self.heroes[h].stats.add(stat, amount);
            }
        }
        self.refresh(h);
        let hero = &mut self.heroes[h];
        hero.hp = hero.max_hp;
    }

    /// Give `player`'s hero the spell `key` at `rank` with `augments`, in place of the same spell
    /// or, if it has a free slot, as a new one: for test loadouts and showcases. False if the
    /// spell does not exist or there is no room.
    pub fn give_spell(
        &mut self,
        player: PlayerId,
        key: &str,
        rank: u8,
        augments: &[Augment],
    ) -> bool {
        let Some(ability) = self.content.ability(key) else {
            return false;
        };
        let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) else {
            return false;
        };
        let owned = Owned {
            level: rank.clamp(1, MAX_RANK),
            augments: augments.to_vec(),
            ..Owned::new(ability, 0.0)
        };
        if let Some(o) = hero.abilities.iter_mut().find(|o| o.ability == ability) {
            *o = owned;
        } else if hero.abilities.len() < MAX_ABILITIES {
            hero.abilities.push(owned);
        } else {
            return false;
        }
        true
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    pub fn enemy(&self, id: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.id == id)
    }

    pub fn interactable(&self, id: u32) -> Option<&Interactable> {
        self.interactables.iter().find(|i| i.id == id)
    }

    /// Move elite camp `c` to `pos`, kept inside the arena, for the same reason as
    /// [`World::move_interactable`]: its elite makes for the new spot.
    pub fn move_camp(&mut self, c: usize, pos: V2) {
        let pos = self.inside(pos);
        let Some(camp) = self.camps.get_mut(c) else {
            return;
        };
        camp.pos = pos;
        if let Some(e) = camp
            .elite
            .and_then(|id| self.enemies.iter_mut().find(|e| e.id == id))
        {
            e.home = Some(pos);
        }
    }

    /// `pos`, pulled back onto the leash's edge if it lies past it.
    fn inside(&self, pos: V2) -> V2 {
        let center = self.center();
        let off = pos - center;
        if off.length() > self.zone().leash {
            center + off.normalize_or_zero() * self.zone().leash
        } else {
            pos
        }
    }

    /// Move an interactable to `pos`, kept inside the arena: the host's client moves one off a
    /// spot that only the drawn world knows is no good (under water, inside a building).
    pub fn move_interactable(&mut self, id: u32, pos: V2) {
        let pos = self.inside(pos);
        if let Some(it) = self.interactables.iter_mut().find(|i| i.id == id) {
            it.pos = pos;
        }
    }

    /// The input `player` holds from now on.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        self.inputs.insert(player, input);
    }

    /// Fold an input into what the next step takes, for inputs that arrive faster than steps (a
    /// co-op client's, over the network): movement and pose are the newest; a pick, reroll, skip
    /// or jump waits until a step uses it.
    pub fn queue_input(&mut self, player: PlayerId, input: PlayerInput) {
        let held = self.inputs.entry(player).or_default();
        held.move_dir = input.move_dir;
        if input.pose.is_some() {
            held.pose = input.pose;
        }
        held.pick = input.pick.or(held.pick);
        held.reroll |= input.reroll;
        held.skip |= input.skip;
        held.jump |= input.jump;
        held.dash |= input.dash;
        held.freeze |= input.freeze;
        held.banish = input.banish.or(held.banish);
        held.gear_on = input.gear_on.or(held.gear_on);
        held.menu = input.menu;
        held.ready |= input.ready;
        held.give_up |= input.give_up;
    }

    /// `player` left: their hero goes, and the run no longer waits on them.
    pub fn remove_hero(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.inputs.remove(&player);
    }

    /// The part of the world a co-op client draws: everything it shows, nothing it would need to
    /// step the world itself.
    pub fn snapshot(&self) -> Snapshot {
        let center = self.center();
        Snapshot {
            tick: self.tick,
            elapsed: self.elapsed,
            level: self.level,
            xp: self.xp,
            kills: self.kills,
            difficulty: self.difficulty,
            next_boss: self.next_boss,
            menu_pause: self.menu_pause,
            pause_ready: self.pause_ready.clone(),
            surround: self.surround,
            next_surround: self.next_surround,
            camps: self.camps.iter().map(|c| c.pos).collect(),
            buffs: self.buffs.clone(),
            heroes: self.heroes.clone(),
            enemies: self
                .enemies
                .iter()
                .map(|e| EnemyState {
                    id: e.id,
                    def: e.def as u16,
                    pos: (
                        pack_yards(e.pos.x - center.x),
                        pack_yards(e.pos.y - center.y),
                    ),
                    facing: pack_facing(e.facing),
                    moving: e.moving,
                    elite: e.elite,
                    boss: e.boss,
                    guard: self.guards.contains(&e.id),
                    hp: e.hp.max(0.0).ceil() as u32,
                    max_hp: e.max_hp.ceil() as u32,
                    status: e.status.bits(),
                    dots: e.status.dots(),
                })
                .collect(),
            interactables: self.interactables.clone(),
            pickups: self.pickups.clone(),
            affixes: self.affixes.clone(),
        }
    }

    /// Take a host's [`Snapshot`] as this world's state, and its step's events as this world's,
    /// for [`World::drain_events`]. A client's world is never stepped: it mirrors the host's.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot, events: Vec<Event>) {
        self.tick = snapshot.tick;
        self.elapsed = snapshot.elapsed;
        self.level = snapshot.level;
        self.xp = snapshot.xp;
        self.kills = snapshot.kills;
        self.difficulty = snapshot.difficulty;
        self.next_boss = snapshot.next_boss;
        self.menu_pause = snapshot.menu_pause;
        self.pause_ready = snapshot.pause_ready;
        self.surround = snapshot.surround;
        self.next_surround = snapshot.next_surround;
        // A mirror knows where the camps are, never steps them.
        self.camps = snapshot
            .camps
            .into_iter()
            .map(|pos| Camp {
                pos,
                elite: None,
                respawn: f32::INFINITY,
            })
            .collect();
        self.buffs = snapshot.buffs;
        self.heroes = snapshot.heroes;
        self.interactables = snapshot.interactables;
        self.pickups = snapshot.pickups;
        self.affixes = snapshot.affixes;
        let center = self.center();
        let defs = &self.content.enemies;
        self.guards = snapshot
            .enemies
            .iter()
            .filter(|s| s.guard)
            .map(|s| s.id)
            .collect();
        self.enemies = snapshot
            .enemies
            .into_iter()
            .filter_map(|s| {
                let def = usize::from(s.def);
                let d = defs.get(def)?;
                Some(Enemy {
                    id: s.id,
                    def,
                    pos: center + V2::new(f32::from(s.pos.0) / 16.0, f32::from(s.pos.1) / 16.0),
                    facing: unpack_facing(s.facing),
                    moving: s.moving,
                    hp: s.hp as f32,
                    max_hp: s.max_hp as f32,
                    damage: 0.0,
                    speed: d.speed,
                    radius: d.radius * d.scale * if s.elite { ELITE_SCALE } else { 1.0 },
                    elite: s.elite,
                    boss: s.boss,
                    attack_cd: 0.0,
                    last_hit: None,
                    home: None,
                    status: Status::mirror(s.status, &s.dots),
                    act: Default::default(),
                })
            })
            .collect();
        for event in &events {
            match *event {
                Event::GemDropped { id, pos, xp, gold } => {
                    self.gems.push(Gem { id, pos, xp, gold })
                }
                Event::GemCollected { id, .. } => self.gems.retain(|g| g.id != id),
                _ => {}
            }
        }
        self.events.extend(events);
    }

    /// While any living hero has cards to choose, the run waits.
    pub fn paused(&self) -> bool {
        self.heroes.iter().any(|h| h.alive && !h.offers.is_empty())
    }

    /// The run waits: for cards ([`World::paused`]) or for a player in the game menu.
    pub fn held(&self) -> bool {
        self.paused() || self.menu_pause
    }

    /// Every hero has fallen.
    pub fn game_over(&self) -> bool {
        !self.heroes.is_empty() && self.heroes.iter().all(|h| !h.alive)
    }

    /// The events not yet drained, oldest first: a host sends each step's slice of them.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The events since the last drain, oldest first.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Advance the run by one [`DT`]. Picks are taken even while paused; nothing else moves then.
    pub fn step(&mut self) {
        self.apply_picks();
        self.switch_gear();
        self.tend_pause();
        if self.held() || self.game_over() {
            return;
        }
        self.tick += 1;
        self.elapsed += DT;
        self.mark_healing();
        self.move_heroes();
        self.revive_heroes();
        self.regenerate();
        self.tend_spells();
        self.tend_surround();
        self.spawn_enemies();
        self.spawn_bosses();
        self.move_enemies();
        self.tend_mechanics();
        self.tend_affixes();
        self.cast_abilities();
        self.land_hits();
        self.tend_grounds();
        self.tend_statuses();
        self.reap_enemies();
        self.collect_gems();
        self.collect_pickups();
        self.use_interactables();
        self.tend_arena();
        self.tally_healing();
        self.level_up();
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    /// Each hero's card choice: a pick, a skip or a reroll of the hand in front of them.
    fn apply_picks(&mut self) {
        for h in 0..self.heroes.len() {
            let player = self.heroes[h].player;
            let Some(input) = self.inputs.get_mut(&player) else {
                continue;
            };
            let (pick, reroll, skip) = (
                input.pick.take(),
                std::mem::take(&mut input.reroll),
                std::mem::take(&mut input.skip),
            );
            let (freeze, banish) = (std::mem::take(&mut input.freeze), input.banish.take());
            if self.heroes[h].offers.is_empty() {
                continue;
            }
            let levelling = self.heroes[h].offers[0].source == Source::LevelUp;
            if freeze && levelling {
                let hero = &mut self.heroes[h];
                hero.frozen = !hero.frozen;
                hero.frozen_hand.clear();
                continue;
            }
            if let Some(i) = banish {
                self.banish(h, usize::from(i));
                continue;
            }
            let capstones = matches!(
                self.heroes[h].offers[0].cards.first().map(|c| c.kind),
                Some(CardKind::Augment(_, a)) if a.capstone_of().is_some()
            );
            if reroll && self.heroes[h].rerolls > 0 && !capstones {
                self.heroes[h].rerolls -= 1;
                // A fresh hand is no longer the frozen one.
                if levelling {
                    self.heroes[h].frozen = false;
                }
                let source = self.heroes[h].offers[0].source;
                let hand = self.deal(h, source);
                self.heroes[h].offers[0] = hand;
                continue;
            }
            if skip {
                let hand = self.heroes[h].offers.remove(0);
                if levelling && self.heroes[h].frozen {
                    self.carry_frozen(h, hand.cards);
                }
                self.redeal_stale(h);
                continue;
            }
            let hand = &self.heroes[h].offers[0];
            let Some(&card) = pick.and_then(|p| hand.cards.get(usize::from(p))) else {
                continue;
            };
            let hand = self.heroes[h].offers.remove(0);
            if levelling && self.heroes[h].frozen {
                // The card taken is not replaced: the next hand is one card smaller.
                let mut rest = hand.cards;
                if let Some(i) = pick.map(usize::from).filter(|&i| i < rest.len()) {
                    rest.remove(i);
                }
                self.carry_frozen(h, rest);
            }
            self.take_card(h, card);
            self.redeal_stale(h);
        }
    }

    /// A frozen hand's `rest` held for hero `h`'s next level-up: the next level-up hand already
    /// waiting takes it, else it waits for one. Nothing left unfreezes.
    fn carry_frozen(&mut self, h: usize, rest: Vec<Card>) {
        let hero = &mut self.heroes[h];
        if rest.is_empty() {
            hero.frozen = false;
            hero.frozen_hand.clear();
            return;
        }
        match hero
            .offers
            .iter_mut()
            .find(|o| o.source == Source::LevelUp)
        {
            Some(next) => next.cards = rest,
            None => hero.frozen_hand = rest,
        }
    }

    /// Hero `h` banishes the spell on card `i` of the hand in front, if it has a banish left
    /// and the card offers a spell to learn: it never comes up again, and the card is replaced
    /// (a frozen hand just loses it).
    fn banish(&mut self, h: usize, i: usize) {
        let hero = &self.heroes[h];
        let Some(CardKind::Learn(ability)) = hero.offers[0].cards.get(i).map(|c| c.kind) else {
            return;
        };
        if hero.banishes == 0 {
            return;
        }
        let source = hero.offers[0].source;
        let hero = &mut self.heroes[h];
        hero.banishes -= 1;
        hero.banished.push(ability);
        if hero.frozen && source == Source::LevelUp {
            hero.offers[0].cards.remove(i);
            if !hero.offers[0].cards.is_empty() {
                return;
            }
            hero.frozen = false;
        }
        let hand = self.deal(h, source);
        self.heroes[h].offers[0] = hand;
    }

    /// Hero `h`'s waiting level-up hands dealt anew where a card no longer makes sense: a spell
    /// just learned, a full spell bar, a spell waiting at its augment, a tome at its last rank.
    /// Several level-ups deal their hands at once, so an earlier pick can spoil a later hand.
    fn redeal_stale(&mut self, h: usize) {
        let mut i = 0;
        while i < self.heroes[h].offers.len() {
            let source = self.heroes[h].offers[i].source;
            if source != Source::LevelUp {
                i += 1;
                continue;
            }
            let valid: Vec<bool> = self.heroes[h].offers[i]
                .cards
                .iter()
                .map(|c| self.still_valid(h, c.kind))
                .collect();
            if valid.iter().all(|&v| v) {
                i += 1;
                continue;
            }
            // A frozen hand keeps what still makes sense.
            if self.heroes[h].frozen {
                let mut keep = valid.into_iter();
                let offer = &mut self.heroes[h].offers[i];
                offer.cards.retain(|_| keep.next().unwrap_or(true));
                if !offer.cards.is_empty() {
                    i += 1;
                    continue;
                }
                self.heroes[h].frozen = false;
            }
            let hand = self.deal(h, source);
            if hand.cards.is_empty() {
                self.heroes[h].offers.remove(i);
            } else {
                self.heroes[h].offers[i] = hand;
                i += 1;
            }
        }
    }

    /// Whether a card dealt earlier still makes sense for hero `h`: a spell it can still learn
    /// or raise, a tome it can still take.
    fn still_valid(&self, h: usize, kind: CardKind) -> bool {
        match kind {
            CardKind::Learn(_) => self.spells(h).contains(&kind),
            CardKind::Upgrade(a, _) => self.spells(h).contains(&CardKind::Upgrade(a, [None; 3])),
            CardKind::Tome(_) => self.tomes(h, true).contains(&kind),
            _ => true,
        }
    }

    /// What a picked card does for hero `h`.
    fn take_card(&mut self, h: usize, card: Card) {
        let hero = &mut self.heroes[h];
        match card.kind {
            CardKind::Learn(ability) => {
                hero.abilities.push(Owned {
                    level: self.learn_rank.clamp(1, MAX_RANK),
                    ..Owned::new(ability, card.rarity.learn_bonus())
                });
                // A spell learned at a milestone's rank (a test run's) shows its screen next.
                self.queue_augments(h, true);
            }
            CardKind::Upgrade(ability, boosts) => {
                if let Some(owned) = hero.abilities.iter_mut().find(|o| o.ability == ability) {
                    owned.level = (owned.level + 1).min(MAX_RANK);
                    for boost in boosts.into_iter().flatten() {
                        owned.mods.add(boost, boost.amount(card.rarity));
                    }
                }
                // A milestone reached: its augment screen comes next.
                self.queue_augments(h, true);
            }
            CardKind::Augment(ability, augment) => {
                if let Some(owned) = hero.abilities.iter_mut().find(|o| o.ability == ability) {
                    if !owned.augments.contains(&augment) {
                        owned.augments.push(augment);
                    }
                }
            }
            CardKind::Tome(stat) => {
                hero.stats.add(stat, stat.tome_amount(card.rarity));
                match hero.tomes.iter_mut().find(|(s, _)| *s == stat) {
                    Some((_, n)) => *n += 1,
                    None => hero.tomes.push((stat, 1)),
                }
            }
            CardKind::Gold => {
                let gold = fallback_gold(self.level) as f32;
                self.give_gold(h, gold);
            }
            CardKind::Heal => hero.hp = hero.max_hp,
            CardKind::Reroll => hero.rerolls += 1,
            CardKind::Pact(pact) => {
                hero.stats.add(pact.boon, pact.boon_amount(card.rarity));
                pact.bane.apply(&mut hero.stats, &mut self.difficulty);
            }
            CardKind::Item(item) => {
                match hero.items.iter_mut().find(|(i, _)| *i == item) {
                    Some((_, n)) => *n = n.saturating_add(1),
                    None => hero.items.push((item, 1)),
                }
                if let Some((stat, each)) = item.stat() {
                    hero.stats.add(stat, each);
                }
            }
        }
        self.refresh(h);
    }

    /// Hero `h`'s health and speed after its stats changed: a bigger maximum heals the difference,
    /// a smaller one cuts health to fit.
    fn refresh(&mut self, h: usize) {
        let hero = &mut self.heroes[h];
        let class = &self.content.classes[hero.class];
        let max = class.base_hp * (1.0 + hero.stats.get(Stat::Health));
        if max > hero.max_hp && hero.alive {
            hero.hp += max - hero.max_hp;
            // More health to the maximum is no heal.
            if let Some(mark) = &mut hero.hp_mark {
                *mark += max - hero.max_hp;
            }
        }
        hero.max_hp = max;
        hero.hp = hero.hp.min(max);
        let swift = if self.buffs.iter().any(|&(p, _)| p == PowerUp::Swiftness) {
            SWIFTNESS
        } else {
            0.0
        };
        hero.speed = class.speed
            * (1.0 + hero.stats.get(Stat::Speed))
            * (1.0 + swift)
            * (1.0 + hero.spell.haste + hero.spell.fx.haste());
    }

    fn move_heroes(&mut self) {
        let center = self.center();
        let leash = self.zone().leash;
        let mut dashes = Vec::new();
        for (h, hero) in self.heroes.iter_mut().enumerate().filter(|(_, h)| h.alive) {
            let input = self.inputs.get_mut(&hero.player);
            if let Some(pose) = input.as_ref().and_then(|i| i.pose) {
                let off = pose.pos - center;
                let pos = if off.length() > leash {
                    center + off.normalize_or_zero() * leash
                } else {
                    pose.pos
                };
                if let Some(input) = input {
                    if std::mem::take(&mut input.dash) {
                        dashes.push((h, hero.pos, pos, true));
                    }
                }
                hero.moving = pos.distance(hero.pos) > 1e-3;
                if hero.moving {
                    horde::steer(&mut hero.heading, pos - hero.pos);
                }
                hero.pos = pos;
                if pose.facing != V2::ZERO {
                    hero.facing = pose.facing.normalize_or_zero();
                }
                continue;
            }
            let jump = input.as_ref().is_some_and(|i| i.jump);
            let dir = input.map_or(V2::ZERO, |i| {
                i.jump = false;
                if std::mem::take(&mut i.dash) {
                    dashes.push((h, hero.pos, hero.pos, false));
                }
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
            horde::steer(&mut hero.heading, dir);
            hero.pos += dir * (hero.speed * DT);
            let off = hero.pos - center;
            if off.length() > leash {
                hero.pos = center + off.normalize_or_zero() * leash;
            }
        }
        for (h, left, landed, client_moved) in dashes {
            self.defend(h, left, landed, client_moved);
        }
        if !self.triggers.is_empty() {
            self.run_triggers();
        }
    }

    /// A fallen hero with a living friend standing by comes back after [`REVIVE_SECS`] of it, with
    /// half its health and a moment's grace; left alone, the progress ebbs away.
    fn revive_heroes(&mut self) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        for hero in &mut self.heroes {
            if hero.alive {
                hero.ward = (hero.ward - DT).max(0.0);
                continue;
            }
            if self.quit.contains(&hero.player) {
                continue;
            }
            let by = living
                .iter()
                .find(|(p, at)| *p != hero.player && at.distance(hero.pos) <= REVIVE_RANGE)
                .map(|&(p, _)| p);
            let Some(by) = by else {
                hero.revive = (hero.revive - DT / REVIVE_SECS).max(0.0);
                continue;
            };
            hero.revive += DT / REVIVE_SECS;
            if hero.revive >= 1.0 {
                hero.alive = true;
                hero.revive = 0.0;
                hero.hp = hero.max_hp * REVIVE_HEALTH;
                hero.ward = REVIVE_WARD;
                self.events.push(Event::HeroRevived {
                    player: hero.player,
                    by,
                });
            }
        }
    }

    /// Living heroes heal by their class's base regeneration and their [`Stat::Regen`], and their
    /// Frost Wards recover.
    fn regenerate(&mut self) {
        let content = self.content.clone();
        for hero in self.heroes.iter_mut().filter(|h| h.alive) {
            let regen = content.classes[hero.class].base_regen + hero.stats.get(Stat::Regen);
            hero.hp = (hero.hp + regen * DT).min(hero.max_hp);
            hero.barrier = (hero.barrier - DT).max(0.0);
        }
    }

    fn spawn_enemies(&mut self) {
        self.spawn_timer -= DT;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = SPAWN_EVERY;
        let living: Vec<(V2, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.pos, h.heading))
            .collect();
        if living.is_empty() {
            return;
        }
        let horde = 1.0 + self.difficulty.horde + self.surround_horde();
        let target = (target_alive(self.elapsed, living.len()) as f32 * horde) as usize;
        // Elites and bosses come on top of the horde, not out of it.
        let alive = self.enemies.iter().filter(|e| !e.elite && !e.boss).count();
        if alive >= target {
            return;
        }
        let batch = ((target - alive) as f32 / 6.0).ceil().min(4.0) as usize;
        let (low, top) = self.tiers();
        let center = self.center();
        let leash = self.zone().leash;
        for _ in 0..batch {
            let (near, heading) = living[self.rng.index(living.len())];
            let r = self.rng.range(SPAWN_RING.0, SPAWN_RING.1);
            // Never outside the arena: pulled back onto its edge.
            let spot = |angle: f32| {
                let pos = near + V2::from_angle(angle, r);
                let off = pos - center;
                if off.length() > leash + 10.0 {
                    center + off.normalize_or_zero() * (leash + 10.0)
                } else {
                    pos
                }
            };
            let mut pos = spot(self.spawn_angle(heading));
            // Not inside a tree or a house, nor somewhere walled off from the heroes: a few more
            // tries round the ring.
            for _ in 0..6 {
                if self.obstacles.as_ref().is_none_or(|o| o.reachable(pos)) {
                    break;
                }
                pos = spot(self.spawn_angle(heading));
            }
            let tier = low + self.rng.index(top - low + 1);
            let elite = self.elapsed >= ELITES_FROM && self.rng.f32() < ELITE_CHANCE;
            self.spawn_enemy(self.zone().enemy_ix[tier], pos, near, elite);
        }
    }

    /// The zone's boss, when its time comes: near a living hero, and as much tougher as there are
    /// heroes to fight it.
    fn spawn_bosses(&mut self) {
        let Some(def) = self.zone().boss_ix else {
            return;
        };
        // One boss at a time: while one stands the clock waits, so the next comes a full
        // [`BOSS_EVERY`] after the last one falls, however long the fight took.
        if self.enemies.iter().any(|e| e.boss) {
            self.next_boss = self.next_boss.max(self.elapsed + BOSS_EVERY);
            return;
        }
        if self.elapsed < self.next_boss {
            return;
        }
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        self.next_boss = self.elapsed + BOSS_EVERY;
        let party = 1.0 + 0.6 * (living.len() - 1) as f32;
        let near = living[self.rng.index(living.len())];
        let angle = self.rng.range(0.0, std::f32::consts::TAU);
        let pos = near + V2::from_angle(angle, SPAWN_RING.0);
        self.spawn_enemy(def, pos, near, false);
        // As tough as the rules' `boss_health` of the horde's strongest now (the run's last boss,
        // `final_boss_health`), and more with more heroes: a boss keeps pace with the run however
        // late it comes. At least twice an ordinary enemy's size.
        self.bosses_come += 1;
        let rules = &self.content.rules;
        let times = if self.bosses_come >= rules.final_boss {
            rules.final_boss_health
        } else {
            rules.boss_health
        };
        let hp = times * self.horde_health() * party;
        let kind = &self.content.enemies[def];
        let radius = kind.radius * kind.scale.max(ranks::BOSS_MIN_SCALE);
        let boss = self.enemies.last_mut().expect("just spawned");
        boss.boss = true;
        boss.hp = hp;
        boss.max_hp = hp;
        boss.radius = radius;
        self.events.push(Event::BossCame { def });
    }

    /// The zone's roster tiers in play now, weakest and strongest: a new tier every 45 seconds,
    /// the newest three at once.
    fn tiers(&self) -> (usize, usize) {
        let top = ((self.elapsed / 45.0) as usize).min(self.zone().enemy_ix.len() - 1);
        (top.saturating_sub(2), top)
    }

    /// An enemy of kind `def_ix` at `pos`, facing `toward`, grown for the run's progress and the
    /// party's [`Difficulty`]; an elite is tougher still.
    fn spawn_enemy(&mut self, def_ix: usize, pos: V2, toward: V2, elite: bool) -> u32 {
        let def = &self.content.enemies[def_ix];
        let progress = (self.elapsed / RUN_SECS).min(1.0);
        let d = self.difficulty;
        let (health, hits, size) = if elite {
            (self.content.rules.elite_health, ELITE_DAMAGE, ELITE_SCALE)
        } else {
            (1.0, 1.0, 1.0)
        };
        let hp = def.hp * (1.0 + progress * 0.75) * (1.0 + d.enemy_health) * health;
        let damage = def.damage * (1.0 + progress * 0.9) * (1.0 + d.enemy_damage) * hits;
        let rules = &self.content.rules;
        let base = (def.speed * rules.enemy_speed).min(rules.enemy_speed_cap.max(def.speed));
        let speed = base * (1.0 + d.enemy_speed);
        let radius = def.radius * def.scale * size;
        let id = self.id();
        self.enemies.push(Enemy {
            id,
            def: def_ix,
            pos,
            facing: (toward - pos).normalize_or_zero(),
            moving: false,
            hp,
            max_hp: hp,
            damage,
            speed,
            radius,
            elite,
            boss: false,
            attack_cd: 0.5,
            last_hit: None,
            home: None,
            status: Status::default(),
            act: Default::default(),
        });
        id
    }

    fn move_enemies(&mut self) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && h.spell.fx.hidden <= 0.0)
            .map(|h| (h.player, h.pos))
            .collect();
        if living.is_empty() {
            for e in &mut self.enemies {
                e.moving = false;
            }
            return;
        }
        self.flow_timer -= DT;
        if self.flow_timer <= 0.0 {
            self.flow_timer = FLOW_EVERY;
            if let Some(o) = self.obstacles.as_mut().filter(|o| o.any()) {
                let at: Vec<V2> = living.iter().map(|l| l.1).collect();
                o.flow(&at);
            }
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
        let content = self.content.clone();
        let obstacles = self.obstacles.as_ref();
        for (i, e) in self.enemies.iter_mut().enumerate() {
            let (player, target) = living
                .iter()
                .copied()
                .min_by(|a, b| e.pos.distance(a.1).total_cmp(&e.pos.distance(b.1)))
                .expect("living is not empty");
            let to = target - e.pos;
            let dist = to.length();
            // A camp's elite guards its camp. It fights a hero who comes near or
            // hurts it, but only so far from home: past its leash it runs back out of reach and
            // heals, as WoW's mobs evade. With no fight on, it waits at home.
            if let Some(home) = e.home {
                let evading = self.evading.contains(&e.id);
                let out = self.leash_out.iter().position(|&(id, _)| id == e.id);
                if !evading && e.pos.distance(home) > CAMP_LEASH {
                    // Past the leash: a few seconds' grace, then home.
                    let k = out.unwrap_or_else(|| {
                        self.leash_out.push((e.id, 0.0));
                        self.leash_out.len() - 1
                    });
                    self.leash_out[k].1 += DT;
                    if self.leash_out[k].1 >= LEASH_GRACE {
                        self.leash_out.remove(k);
                        self.evading.push(e.id);
                    }
                } else if let Some(k) = out {
                    self.leash_out.remove(k);
                }
                let evading = self.evading.contains(&e.id);
                let fighting = dist <= ELITE_AGGRO || e.hp < e.max_hp;
                if evading || !fighting {
                    let back = home - e.pos;
                    e.moving = back.length() > 1.0;
                    if e.moving {
                        let pace = if evading { EVADE_SPEED } else { 1.0 };
                        e.facing = back.normalize_or_zero();
                        e.pos += e.facing * (e.speed * pace * DT).min(back.length());
                    } else if evading {
                        // Home: whole again, and ready for the next fight.
                        self.evading.retain(|&id| id != e.id);
                        e.hp = e.max_hp;
                        e.status = Status::default();
                        e.act = hazards::Act::default();
                    }
                    if !evading {
                        e.pos += shove[i] * (DT * 8.0).min(0.5);
                    }
                    continue;
                }
            }
            let contact = dist <= HERO_RADIUS + e.radius + 0.35;
            if e.status.held() {
                e.moving = false;
                e.pos += shove[i] * (DT * 8.0).min(0.5);
                continue;
            }
            // A charge or leap under way, or a wind-up while its telegraph is up.
            if World::dash_step(e) {
                continue;
            }
            if e.act.windup > 0.0 {
                e.act.windup -= DT;
                e.moving = false;
                continue;
            }
            if e.status.fx.fear > 0.0 || e.status.fx.lost > 0.0 {
                // Feared, it runs from whoever feared it; lost (Smoke Bomb), it mills about.
                let away = (e.pos - e.status.fx.fear_from).normalize_or_zero();
                let fleeing = e.status.fx.fear > 0.0 && away != V2::ZERO;
                e.moving = fleeing;
                if fleeing {
                    e.facing = away;
                    let tough = e.elite || e.boss;
                    e.pos += away * (combat::chilled(e.speed, &e.status, tough) * DT);
                }
                e.pos += shove[i] * (DT * 8.0).min(0.5);
                continue;
            }
            let mechanics = &content.enemies[e.def].mechanics;
            let acts = e.elite || e.boss;
            let frenzied = acts && e.hp < e.max_hp * 0.5 && mechanics.contains(&Mechanic::Frenzy);
            // A caster keeps its distance and only casts.
            let keep = if acts && mechanics.contains(&Mechanic::Caster) {
                hazards::CASTER_KEEP
            } else {
                0.0
            };
            // A frenzy hurries it, but never past the horde's speed cap: a hero must still be
            // able to run from a frenzied elite.
            let speed = if frenzied {
                (e.speed * hazards::FRENZY_SPEED).min(content.rules.enemy_speed_cap.max(e.speed))
            } else {
                e.speed
            };
            // Around a tree or a wall in the way, rather than through it.
            e.facing = obstacles
                .filter(|_| !contact)
                .and_then(|o| o.steer(e.pos, target))
                .unwrap_or_else(|| to.normalize_or_zero());
            let rooted = e.status.fx.root > 0.0;
            e.moving = !contact && !rooted && dist > keep;
            let step = if e.moving {
                e.facing * (combat::chilled(speed, &e.status, e.elite || e.boss) * DT)
            } else {
                V2::ZERO
            };
            let step = step + shove[i] * (DT * 8.0).min(0.5);
            e.pos = match obstacles {
                Some(o) => o.slide(e.pos, step),
                None => e.pos + step,
            };
            e.attack_cd -= DT;
            if contact && e.attack_cd <= 0.0 {
                e.attack_cd = if frenzied { hazards::FRENZY_SWING } else { 1.3 };
                swings.push((e.id, player, e.damage * World::weakness(e)));
            }
        }
        for (enemy, player, amount) in swings {
            let amount = self.hurt_hero_from(player, amount, Some(enemy));
            self.events.push(Event::Swing {
                enemy,
                hero: player,
                amount,
            });
        }
    }

    /// A blow of `amount` at a hero, less its [`Stat::Armor`]; what it took.
    #[cfg(test)]
    fn hurt_hero(&mut self, player: PlayerId, amount: f32) -> f32 {
        self.hurt_hero_from(player, amount, None)
    }

    /// A blow of `amount` at a hero from enemy `from`: less its armor and Frozen Core's guard,
    /// then against its shield; what its health took.
    fn hurt_hero_from(&mut self, player: PlayerId, amount: f32, from: Option<u32>) -> f32 {
        let Some(h) = self
            .heroes
            .iter()
            .position(|h| h.player == player && h.alive)
        else {
            return 0.0;
        };
        let hero = &mut self.heroes[h];
        if hero.ward > 0.0 || self.buffs.iter().any(|&(p, _)| p == PowerUp::DivineShield) {
            return 0.0;
        }
        let wards = hero.stacks(Item::FrostWard);
        if wards > 0 && hero.barrier <= 0.0 {
            hero.absorbed += f64::from(amount * (1.0 - hero.stats.get(Stat::Armor)));
            hero.barrier = ward_cooldown(wards);
            self.proc_cast(player, Item::FrostWard, vec![Target::Hero(player)]);
            return 0.0;
        }
        let guard = if hero.spell.guard > 0.0 { 0.5 } else { 1.0 };
        let amount = amount * (1.0 - hero.stats.get(Stat::Armor)) * guard;
        let blow = amount;
        let amount = self.ward_blow(h, amount, from);
        let amount = self.shield_blow(h, amount, from);
        let hero = &mut self.heroes[h];
        hero.absorbed += f64::from((blow - amount).max(0.0));
        hero.taken += amount;
        hero.hp -= amount;
        if hero.hp <= 0.0 && self.cheat_death(h) {
            return amount;
        }
        let hero = &mut self.heroes[h];
        if hero.hp <= 0.0 {
            // Its cards wait: a fallen hero is not waited on, and chooses once raised.
            hero.hp = 0.0;
            hero.alive = false;
            hero.moving = false;
            hero.revive = 0.0;
            self.events.push(Event::HeroDied { player });
        } else {
            self.after_blow(h, amount, from);
            self.frozen_core(h);
            if !self.triggers.is_empty() {
                self.run_triggers();
            }
        }
        amount
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

    /// An ability's numbers with the party's power-ups on top: Berserk's damage, Bloodlust's
    /// cooldowns.
    fn sharpen(&self, mut stats: AbilityStats) -> AbilityStats {
        for &(p, _) in &self.buffs {
            match p {
                PowerUp::Berserk => stats.damage *= 1.0 + BERSERK,
                PowerUp::Bloodlust => stats.cooldown *= BLOODLUST,
                _ => {}
            }
        }
        stats
    }

    fn damage_enemy(&mut self, by: PlayerId, ability: usize, id: u32, amount: f32, crit: bool) {
        // Running home from a broken leash, it cannot be hurt.
        if self.evading.contains(&id) {
            return;
        }
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else {
            return;
        };
        if e.hp <= 0.0 {
            return;
        }
        let dealt = amount.min(e.hp);
        e.hp -= amount;
        e.last_hit = Some(by);
        let boss = e.boss;
        if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == by) {
            hero.damage_done += f64::from(dealt);
            meter::add_to(&mut hero.damage_by, ability, dealt);
            if boss {
                hero.boss_damage += f64::from(dealt);
                meter::add_to(&mut hero.boss_by, ability, dealt);
            }
        }
        self.events.push(Event::Damage {
            target: Target::Enemy(id),
            amount,
            crit,
        });
    }

    /// An item of `by`'s acting: a cast of its proc ability at `targets`, for the client to show;
    /// the ability, to file its damage under. `None` if the content lacks the ability.
    fn proc_cast(&mut self, by: PlayerId, item: Item, targets: Vec<Target>) -> Option<usize> {
        let ability = self.content.ability(item.proc()?)?;
        let visual = self.content.abilities[ability].visual;
        self.events.push(Event::Cast {
            caster: by,
            ability,
            visual,
            targets,
            dest: None,
            cooldown: 0.0,
        });
        Some(ability)
    }

    /// A spell of `by`'s hit enemy `target` for `damage`: a Static Shard may arc from it to the
    /// enemies around it.
    fn on_hit(&mut self, by: PlayerId, target: u32, damage: f32) {
        let Some(stacks) = self.hero(by).map(|h| h.stacks(Item::StaticShard)) else {
            return;
        };
        if stacks == 0 || self.rng.f32() >= Item::StaticShard.chance(stacks) {
            return;
        }
        let Some(at) = self.enemy(target).map(|e| e.pos) else {
            return;
        };
        let arcs: Vec<u32> = self
            .nearest(at, SHARD_REACH, SHARD_CHAIN + 1)
            .into_iter()
            .map(|(id, _)| id)
            .filter(|&id| id != target)
            .take(SHARD_CHAIN)
            .collect();
        if arcs.is_empty() {
            return;
        }
        let targets = arcs.iter().map(|&id| Target::Enemy(id)).collect();
        let Some(ability) = self.proc_cast(by, Item::StaticShard, targets) else {
            return;
        };
        for id in arcs {
            self.damage_enemy(by, ability, id, damage * SHARD_SHARE, false);
        }
    }

    /// `by` killed an enemy of `max_hp` and `worth` experience at `pos`: a Vampiric Fang heals,
    /// an Ember Core may burst over the corpse, a Goblin Purse may spill gold.
    fn on_kill(&mut self, by: PlayerId, pos: V2, max_hp: f32, worth: u32) {
        let Some(h) = self.heroes.iter().position(|x| x.player == by) else {
            return;
        };
        self.talent_on_kill(h);
        let hero = &mut self.heroes[h];
        if hero.alive {
            let heal = FANG_HEAL * f32::from(hero.stacks(Item::VampiricFang));
            hero.hp = (hero.hp + heal).min(hero.max_hp);
        }
        let embers = hero.stacks(Item::EmberCore);
        if embers > 0 && self.rng.f32() < Item::EmberCore.chance(embers) {
            let burnt: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= EMBER_RADIUS)
                .map(|e| e.id)
                .collect();
            let targets = burnt.iter().map(|&id| Target::Enemy(id)).collect();
            if let Some(ability) = self.proc_cast(by, Item::EmberCore, targets) {
                for id in burnt {
                    self.damage_enemy(by, ability, id, max_hp * EMBER_SHARE, false);
                }
            }
        }
        let purses = self.heroes[h].stacks(Item::GoblinPurse);
        if purses > 0 && self.rng.f32() < Item::GoblinPurse.chance(purses) {
            self.give_gold(h, worth as f32 * PURSE_GOLD);
        }
    }

    fn reap_enemies(&mut self) {
        let mut dead = Vec::new();
        let mut deaths = Vec::new();
        let mut for_affixes = Vec::new();
        self.enemies.retain(|e| {
            if e.hp <= 0.0 {
                // A ghost (Spiteful) leaves nothing behind for the affixes.
                if e.act.expires.is_none() {
                    for_affixes.push((e.pos, e.def, e.max_hp));
                }
                dead.push((
                    e.id,
                    e.pos,
                    e.def,
                    e.elite || e.boss,
                    e.boss,
                    e.max_hp,
                    e.last_hit,
                ));
                deaths.push(combat::Death {
                    pos: e.pos,
                    max_hp: e.max_hp,
                    killer: e.last_hit,
                    status: e.status.clone(),
                });
                false
            } else {
                true
            }
        });
        for (id, pos, def, elite, boss, max_hp, killer) in dead {
            self.kills += 1;
            self.events.push(Event::EnemyDied { id });
            if boss {
                self.place(InteractableKind::Cache, pos);
                self.events.push(Event::BossDied { id });
            }
            let guard = self.guards.contains(&id);
            self.guards.retain(|&g| g != id);
            if elite {
                self.drop_gear(boss);
            }
            // An elite is worth several, and always carries gold; so is a boss.
            let reward = if guard {
                ranks::CAMP_REWARD
            } else if elite {
                ELITE_REWARD
            } else {
                1
            };
            let worth = self.content.enemies[def].xp;
            let gold = if elite || self.rng.f32() < GOLD_CHANCE {
                ((worth as f32 * GOLD_PER_XP).ceil() as u32).max(1) * reward
            } else {
                0
            };
            self.drop_gem(pos, worth * reward, gold);
            if let Some(by) = killer {
                self.on_kill(by, pos, max_hp, worth);
            }
        }
        if !deaths.is_empty() {
            self.spell_deaths(deaths);
        }
        self.affix_deaths(&for_affixes);
    }

    /// Gems within a living hero's reach ([`MAGNET`] and its [`Stat::Pickup`]) are theirs.
    fn collect_gems(&mut self) {
        let reach: Vec<(PlayerId, V2, f32)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos, MAGNET * (1.0 + h.stats.get(Stat::Pickup))))
            .collect();
        let mut taken = Vec::new();
        self.gems.retain(
            |g| match reach.iter().find(|&&(_, p, r)| p.distance(g.pos) <= r) {
                Some(&(by, ..)) => {
                    taken.push((g.clone(), by));
                    false
                }
                None => true,
            },
        );
        for (gem, by) in taken {
            self.take_gem(gem, by);
        }
    }

    /// `by` picked up `gem`: its experience for the party, grown by `by`'s [`Stat::XpGain`]; its
    /// gold into every hero's purse.
    fn take_gem(&mut self, gem: Gem, by: PlayerId) {
        let gain = self.hero(by).map_or(0.0, |h| h.stats.get(Stat::XpGain));
        self.xp += gem.xp as f32 * (1.0 + gain) * self.xp_rate;
        self.events.push(Event::GemCollected { id: gem.id, by });
        if gem.gold > 0 {
            for h in 0..self.heroes.len() {
                self.give_gold(h, gem.gold as f32);
            }
        }
    }

    /// `base` gold into hero `h`'s purse, grown by its [`Stat::GoldGain`].
    fn give_gold(&mut self, h: usize, base: f32) {
        let hero = &mut self.heroes[h];
        let fortune = if self.buffs.iter().any(|&(p, _)| p == PowerUp::Fortune) {
            2.0
        } else {
            1.0
        };
        let amount = (base * (1.0 + hero.stats.get(Stat::GoldGain)) * fortune).round() as u32;
        if amount == 0 {
            return;
        }
        hero.gold += amount;
        self.events.push(Event::GoldGained {
            player: hero.player,
            amount,
        });
    }

    fn level_up(&mut self) {
        while self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            // Every living hero catches its breath.
            for hero in self.heroes.iter_mut().filter(|h| h.alive) {
                hero.hp = (hero.hp + hero.max_hp * LEVEL_UP_HEAL).min(hero.max_hp);
            }
            // The fallen too: their hands wait until they are raised.
            for h in 0..self.heroes.len() {
                self.offer(h, Source::LevelUp);
                self.queue_augments(h, false);
            }
            self.events.push(Event::LevelUp { level: self.level });
        }
    }

    /// A new hand from `source` for hero `h`, if there is anything to deal.
    fn offer(&mut self, h: usize, source: Source) {
        let hero = &mut self.heroes[h];
        if source == Source::LevelUp && hero.frozen && !hero.frozen_hand.is_empty() {
            let cards = std::mem::take(&mut hero.frozen_hand);
            hero.offers.push(Offer { cards, source });
            // Anything it held that no longer makes sense goes.
            self.redeal_stale(h);
            return;
        }
        let offer = self.deal(h, source);
        if !offer.cards.is_empty() {
            self.heroes[h].offers.push(offer);
        }
    }

    /// Up to [`HAND`] distinct cards for hero `h` from what `source` deals, each of a
    /// [`Rarity`] rolled with the hero's [`Stat::Luck`]: always legendary from a golden shrine,
    /// never below rare from an altar. Treasure (from a chest or a cache) is an item, of the
    /// item's own rarity, the rarer ones the less often ([`World::lucky_item`]).
    fn deal(&mut self, h: usize, source: Source) -> Offer {
        if let Source::Augment(ability) = source {
            return self.augment_offer(h, ability).unwrap_or(Offer {
                cards: Vec::new(),
                source,
            });
        }
        let luck = self.heroes[h].stats.get(Stat::Luck);
        let mut options = self.options(h, source);
        let treasure = matches!(
            source,
            Source::Interactable(InteractableKind::Chest | InteractableKind::Cache)
        );
        let mut cards = Vec::new();
        while cards.len() < HAND && !options.is_empty() {
            let i = if treasure {
                self.lucky_item(&options, luck)
            } else {
                self.offer_index(h, &options)
            };
            let mut kind = options.swap_remove(i);
            let rolled = Rarity::roll(&mut self.rng, luck);
            let rarity = match (kind, source) {
                (CardKind::Item(item), _) => item.rarity(),
                (_, Source::Interactable(InteractableKind::GoldenShrine)) => Rarity::Legendary,
                (_, Source::Interactable(InteractableKind::CursedAltar)) => {
                    rolled.max(Rarity::Rare)
                }
                _ => rolled,
            };
            if let CardKind::Upgrade(ability, _) = kind {
                kind = CardKind::Upgrade(ability, self.roll_boosts(h, ability, rarity));
            }
            cards.push(Card { kind, rarity });
        }
        // Nothing left worth dealing: something useful rather than a short hand.
        for kind in CardKind::FALLBACKS {
            if cards.len() >= HAND {
                break;
            }
            cards.push(Card {
                kind,
                rarity: Rarity::Common,
            });
        }
        Offer { cards, source }
    }

    /// An index into a treasure hand's `options`: a rarity rolled with `luck`, then an item of
    /// it, or failing that of the rarest tier below it, or failing that any.
    fn lucky_item(&mut self, options: &[CardKind], luck: f32) -> usize {
        let want = Rarity::roll(&mut self.rng, luck);
        let rarity = |k: &CardKind| match k {
            CardKind::Item(item) => item.rarity(),
            _ => Rarity::Common,
        };
        let best = options.iter().map(rarity).filter(|&r| r <= want).max();
        let pool: Vec<usize> = (0..options.len())
            .filter(|&i| Some(rarity(&options[i])) == best)
            .collect();
        if pool.is_empty() {
            return self.rng.index(options.len());
        }
        pool[self.rng.index(pool.len())]
    }

    /// Every card `source` could deal hero `h`: a level-up's spells and a couple of tomes, a
    /// shrine's tomes, an altar's pacts, a chest's items (those whose proc ability the content
    /// has).
    fn options(&mut self, h: usize, source: Source) -> Vec<CardKind> {
        let Source::Interactable(kind) = source else {
            let mut options = self.spells(h);
            let mut extra = self.tomes(h, false);
            for _ in 0..LEVEL_UP_TOMES.min(extra.len()) {
                let i = self.rng.index(extra.len());
                options.push(extra.swap_remove(i));
            }
            return options;
        };
        match kind {
            InteractableKind::TitanShrine => self.tomes(h, false),
            InteractableKind::GoldenShrine => self.tomes(h, true),
            InteractableKind::CursedAltar => {
                let mut boons = Stat::TOMES.to_vec();
                boons.push(Stat::Projectiles);
                boons
                    .into_iter()
                    .map(|boon| {
                        // Never a bane that takes back the very stat it gives.
                        let banes: Vec<Bane> = Bane::ALL
                            .into_iter()
                            .filter(|&b| b != Bane::Weaker(boon))
                            .collect();
                        let bane = banes[self.rng.index(banes.len())];
                        CardKind::Pact(Pact { boon, bane })
                    })
                    .collect()
            }
            InteractableKind::Chest | InteractableKind::Cache => {
                let hero = &self.heroes[h];
                let limit = self.content.rules.item_max_copies;
                Item::ALL
                    .into_iter()
                    .filter(|i| {
                        i.proc()
                            .is_none_or(|key| self.content.ability(key).is_some())
                    })
                    .filter(|&i| hero.stacks(i) < i.max_copies(limit))
                    .map(CardKind::Item)
                    .collect()
            }
            InteractableKind::ChallengeIdol
            | InteractableKind::WarBanner
            | InteractableKind::Pot => Vec::new(),
        }
    }

    /// The tomes hero `h` could take (with `projectiles`, the Tome of Multitudes among them): those
    /// it has below their top rank, and while it has a free slot, those it has not.
    fn tomes(&self, h: usize, projectiles: bool) -> Vec<CardKind> {
        let hero = &self.heroes[h];
        let rules = &self.content.rules;
        let free = (hero.tomes.len() as u32) < rules.tome_slots;
        Stat::ALL
            .into_iter()
            .filter(|&s| s != Stat::Projectiles || projectiles)
            .filter(|&s| match hero.tome_rank(s) {
                0 => free,
                rank => rank < rules.tome_max_rank,
            })
            .map(CardKind::Tome)
            .collect()
    }

    /// The spells hero `h` could still raise, and learn while it has a free slot.
    fn spells(&self, h: usize) -> Vec<CardKind> {
        let hero = &self.heroes[h];
        let class = &self.content.classes[hero.class];
        let mut options: Vec<CardKind> = hero
            .abilities
            .iter()
            .filter(|o| o.level < MAX_RANK)
            .filter(|o| {
                let key = &self.content.abilities[o.ability].key;
                o.awaiting(!Augment::capstones_of(key).is_empty()).is_none()
            })
            .map(|o| CardKind::Upgrade(o.ability, [None; 3]))
            .collect();
        if hero.abilities.len() < MAX_ABILITIES {
            for &ability in &class.pool_ix {
                if !hero.abilities.iter().any(|o| o.ability == ability)
                    && !hero.banished.contains(&ability)
                {
                    options.push(CardKind::Learn(ability));
                }
            }
        }
        options
    }

    /// Every interactable's circle: it fills while a hero who can use it stands in it, drains
    /// while nobody does, and does its work when full (a pot's at a touch). A challenge under way
    /// watches for its last elite to fall; a banner on its way, for its carrier to arrive.
    fn use_interactables(&mut self) {
        for i in 0..self.interactables.len() {
            let it = &mut self.interactables[i];
            let (id, kind, pos) = (it.id, it.kind, it.pos);
            match &mut it.state {
                InteractState::Spent(_) => continue,
                InteractState::Challenge(left) => {
                    let enemies = &self.enemies;
                    left.retain(|id| enemies.iter().any(|e| e.id == *id));
                    if left.is_empty() {
                        it.state = InteractState::Spent(SPENT_LINGER);
                        self.place(InteractableKind::Cache, pos);
                        self.events.push(Event::ChallengeWon { id });
                    }
                    continue;
                }
                InteractState::Escort { carrier, to, left } => {
                    *left -= DT;
                    let (carrier, to, left) = (*carrier, *to, *left);
                    let at = self
                        .heroes
                        .iter()
                        .find(|h| h.player == carrier && h.alive)
                        .map(|h| h.pos);
                    let won = at.is_some_and(|p| p.distance(to) <= ESCORT_GOAL);
                    if won || left <= 0.0 || at.is_none() {
                        self.interactables[i].state = InteractState::Spent(SPENT_LINGER);
                        if won {
                            self.place(InteractableKind::Cache, to);
                        }
                        self.events.push(Event::EscortDone { id, won });
                    }
                    continue;
                }
                InteractState::Ready => {}
            }
            let who = self.standing_in(kind, pos);
            let it = &mut self.interactables[i];
            let rate = DT / kind.charge_secs().max(DT);
            if who.is_empty() {
                it.charge = (it.charge - rate * 0.5).max(0.0);
                continue;
            }
            it.charge += rate;
            if it.charge < 1.0 {
                continue;
            }
            it.charge = 0.0;
            // A pot is gone at once; the rest stand a moment, used, before they go.
            let linger = if kind == InteractableKind::Pot {
                0.0
            } else {
                SPENT_LINGER
            };
            it.state = InteractState::Spent(linger);
            let by = self.activate(i, who);
            self.events.push(Event::Interacted { id, kind, by });
        }
    }

    /// The living heroes standing in a `kind` circle at `pos` who could use it, nearest first:
    /// for a chest, only those who can pay for their next one.
    fn standing_in(&self, kind: InteractableKind, pos: V2) -> Vec<PlayerId> {
        let mut inside: Vec<(PlayerId, f32)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && h.pos.distance(pos) <= kind.radius())
            .filter(|h| kind != InteractableKind::Chest || h.gold >= chest_price(h.chests))
            .map(|h| (h.player, h.pos.distance(pos)))
            .collect();
        inside.sort_by(|a, b| a.1.total_cmp(&b.1));
        inside.into_iter().map(|(p, _)| p).collect()
    }

    /// Interactable `i`'s circle filled with `who` in it (nearest first): do its work, and say
    /// whom it served.
    fn activate(&mut self, i: usize, who: Vec<PlayerId>) -> Vec<PlayerId> {
        let (kind, pos) = (self.interactables[i].kind, self.interactables[i].pos);
        let index = |w: &World, p: PlayerId| w.heroes.iter().position(|h| h.player == p);
        match kind {
            InteractableKind::ChallengeIdol => {
                let living = self.heroes.iter().filter(|h| h.alive).count();
                let n = CHALLENGE_ELITES + CHALLENGE_PER_HERO * living;
                let (_, top) = self.tiers();
                let def = self.zone().enemy_ix[top];
                let mut elites = Vec::with_capacity(n);
                for k in 0..n {
                    let angle = k as f32 / n as f32 * std::f32::consts::TAU;
                    let r = self.rng.range(CHALLENGE_RING.0, CHALLENGE_RING.1);
                    elites.push(self.spawn_enemy(def, pos + V2::from_angle(angle, r), pos, true));
                }
                self.interactables[i].state = InteractState::Challenge(elites);
                who
            }
            InteractableKind::WarBanner => {
                // Its taker carries it, to a spot well away, against the clock.
                let by = who[0];
                let speed = self.hero(by).map_or(7.0, |h| h.speed);
                let taken: Vec<V2> = self.interactables.iter().map(|i| i.pos).collect();
                let content = self.content.clone();
                let zone = &content.zones[self.zone];
                let to = spot(zone, pos, ESCORT_DISTANCE, &taken, &mut self.rng);
                let left = escort_time(to.distance(pos), speed);
                self.interactables[i].state = InteractState::Escort {
                    carrier: by,
                    to,
                    left,
                };
                vec![by]
            }
            InteractableKind::Pot => {
                let (id, by) = (self.interactables[i].id, who[0]);
                self.break_pot(id, pos, by);
                vec![by]
            }
            InteractableKind::Chest => {
                // Only the opener pays, and only the opener chooses.
                let by = who[0];
                if let Some(h) = index(self, by) {
                    let hero = &mut self.heroes[h];
                    let price = chest_price(hero.chests);
                    hero.gold -= price;
                    hero.chests += 1;
                    let keys = hero.stacks(Item::SkeletonKey);
                    if keys > 0 && self.rng.f32() < Item::SkeletonKey.chance(keys) {
                        self.heroes[h].gold += price;
                        self.events.push(Event::GoldGained {
                            player: by,
                            amount: price,
                        });
                    }
                    self.offer(h, Source::Interactable(kind));
                }
                vec![by]
            }
            InteractableKind::TitanShrine
            | InteractableKind::GoldenShrine
            | InteractableKind::CursedAltar
            | InteractableKind::Cache => {
                for &p in &who {
                    if let Some(h) = index(self, p) {
                        self.offer(h, Source::Interactable(kind));
                    }
                }
                who
            }
        }
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

    /// A training dummy: an enemy with `hp` at `pos` that never moves or swings.
    fn dummy(w: &mut World, id: u32, pos: V2, hp: f32) {
        w.enemies.push(Enemy {
            id,
            def: 0,
            pos,
            facing: V2::ZERO,
            moving: false,
            hp,
            max_hp: hp,
            damage: 0.0,
            speed: 0.0,
            radius: 0.6,
            elite: false,
            boss: false,
            attack_cd: 100.0,
            last_hit: None,
            home: None,
            status: Status::default(),
            act: Default::default(),
        });
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
                ..Default::default()
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
                ..Default::default()
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
        dummy(&mut w, 999, center + V2::new(14.0, 0.0), 1000.0);
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
    fn a_level_up_gives_back_a_tenth_of_health() {
        let mut w = world();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.5;
        w.xp = xp_to_next(1);
        w.level_up();
        let h = &w.heroes[0];
        assert!((h.hp - h.max_hp * (0.5 + LEVEL_UP_HEAL)).abs() < 1e-3, "{}", h.hp);
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
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
        let hero = w.hero(0).unwrap();
        let card = offer.cards[0];
        let (want_level, want_bonus) = match card.kind {
            CardKind::Learn(_) => (1, card.rarity.learn_bonus()),
            CardKind::Upgrade(..) => (2, 0.0),
            CardKind::Tome(stat) => {
                assert!((hero.stats.get(stat) - stat.tome_amount(card.rarity)).abs() < 1e-6);
                return;
            }
            other => panic!("a fresh hero's level-up dealt {other:?}"),
        };
        let owned = hero
            .abilities
            .iter()
            .find(|o| Some(o.ability) == card.ability())
            .expect("the picked ability is owned");
        assert_eq!(owned.level, want_level);
        assert!((owned.bonus - want_bonus).abs() < 1e-6);
    }

    #[test]
    fn a_mirror_fed_snapshots_shows_what_the_host_sees() {
        let mut host = world();
        host.add_hero_as(1, "mage", "Friend", 4, 1).unwrap();
        let mut mirror = World::new(Arc::new(sample()), "elwynn", 99).unwrap();
        for _ in 0..TICK_HZ * 5 {
            host.step();
            let events = host.drain_events();
            mirror.apply_snapshot(host.snapshot(), events.clone());
            assert_eq!(mirror.drain_events(), events);
        }
        assert_eq!(mirror.tick, host.tick);
        assert_eq!(mirror.enemies.len(), host.enemies.len());
        assert_eq!(mirror.gems, host.gems, "gems arrive as events");
        for (m, h) in mirror.enemies.iter().zip(&host.enemies) {
            assert_eq!(m.id, h.id);
            assert!(m.pos.distance(h.pos) < 0.05, "packed position drifted");
            assert!(h.facing == V2::ZERO || m.facing.distance(h.facing.normalize_or_zero()) < 0.05);
            assert!((m.hp - h.hp).abs() < 1.0);
        }
        assert_eq!(mirror.hero(1).unwrap().name, "Friend");
        assert_eq!(mirror.hero(1).unwrap().race, 4);
        assert_eq!(mirror.paused(), host.paused());
    }

    /// A level-up hand of tomes for hero 0, so taking one spoils nothing.
    fn tome_hand() -> Offer {
        Offer {
            cards: [Stat::Damage, Stat::Area, Stat::Luck]
                .into_iter()
                .map(|s| Card {
                    kind: CardKind::Tome(s),
                    rarity: Rarity::Common,
                })
                .collect(),
            source: Source::LevelUp,
        }
    }

    fn press(w: &mut World, input: PlayerInput) {
        w.queue_input(0, input);
        w.step();
    }

    #[test]
    fn a_frozen_hand_comes_back_one_card_smaller() {
        let mut w = world();
        w.heroes[0].offers = vec![tome_hand()];
        press(&mut w, PlayerInput { freeze: true, ..Default::default() });
        assert!(w.heroes[0].frozen);
        press(&mut w, PlayerInput { pick: Some(0), ..Default::default() });
        assert!(w.heroes[0].offers.is_empty());
        w.offer(0, Source::LevelUp);
        let kinds: Vec<CardKind> = w.heroes[0].offers[0].cards.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [CardKind::Tome(Stat::Area), CardKind::Tome(Stat::Luck)]
        );
    }

    #[test]
    fn unfreezing_deals_a_fresh_hand_next_time() {
        let mut w = world();
        w.heroes[0].offers = vec![tome_hand()];
        press(&mut w, PlayerInput { freeze: true, ..Default::default() });
        press(&mut w, PlayerInput { freeze: true, ..Default::default() });
        assert!(!w.heroes[0].frozen);
        press(&mut w, PlayerInput { pick: Some(0), ..Default::default() });
        assert!(w.heroes[0].frozen_hand.is_empty());
    }

    #[test]
    fn a_banished_spell_never_comes_up_again() {
        let mut w = world();
        let learn = w
            .spells(0)
            .into_iter()
            .find(|k| matches!(k, CardKind::Learn(_)))
            .expect("a spell to learn");
        let CardKind::Learn(ability) = learn else {
            unreachable!()
        };
        w.heroes[0].offers = vec![Offer {
            cards: vec![Card {
                kind: learn,
                rarity: Rarity::Common,
            }],
            source: Source::LevelUp,
        }];
        press(&mut w, PlayerInput { banish: Some(0), ..Default::default() });
        assert_eq!(w.heroes[0].banishes, 0);
        assert!(!w.spells(0).contains(&learn));
        assert!(w.heroes[0]
            .offers
            .iter()
            .all(|o| o.cards.iter().all(|c| c.kind != CardKind::Learn(ability))));
        // None left: a second banish does nothing.
        let before = w.heroes[0].offers.clone();
        press(&mut w, PlayerInput { banish: Some(0), ..Default::default() });
        assert_eq!(w.heroes[0].offers, before);
    }

    #[test]
    fn a_waiting_hand_is_dealt_anew_once_a_pick_spoils_it() {
        let mut w = world();
        let learn = w
            .spells(0)
            .into_iter()
            .find(|k| matches!(k, CardKind::Learn(_)))
            .expect("a spell to learn");
        let hand = Offer {
            cards: vec![Card {
                kind: learn,
                rarity: Rarity::Common,
            }],
            source: Source::LevelUp,
        };
        w.heroes[0].offers = vec![hand.clone(), hand];
        w.queue_input(
            0,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.step();
        assert!(
            w.heroes[0]
                .offers
                .iter()
                .all(|o| o.cards.iter().all(|c| c.kind != learn)),
            "the second hand still offers the spell just learned"
        );
    }

    #[test]
    fn queued_one_shots_survive_a_newer_movement_input() {
        let mut w = world();
        w.xp = xp_to_next(1);
        w.step();
        assert!(w.paused());
        // A pick, then a newer pose-only input before the step: the pick must still land.
        w.queue_input(
            0,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.queue_input(
            0,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: w.center(),
                    facing: V2::new(1.0, 0.0),
                }),
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
    }

    #[test]
    fn a_player_who_leaves_is_not_waited_on() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.xp = xp_to_next(1);
        w.step();
        w.set_input(
            0,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.step();
        assert!(w.paused(), "player 1 still has cards");
        w.remove_hero(1);
        assert!(!w.paused());
        assert!(w.hero(1).is_none());
    }

    #[test]
    fn damage_by_ability_adds_up_to_the_total() {
        let mut w = world();
        steps(&mut w, TICK_HZ as usize * 20);
        let hero = w.hero(0).unwrap();
        assert!(hero.damage_done > 0.0);
        let sum: f64 = hero.damage_by.iter().map(|(_, d)| d).sum();
        assert!(
            (sum - hero.damage_done).abs() < 1e-6,
            "{sum} vs {}",
            hero.damage_done
        );
        let frostbolt = hero.abilities[0].ability;
        assert!(hero
            .damage_by
            .iter()
            .any(|&(a, d)| a == frostbolt && d > 0.0));
    }

    #[test]
    fn a_reroll_deals_again_and_is_spent() {
        let mut w = world();
        w.xp = xp_to_next(1);
        w.step();
        let before = w.hero(0).unwrap().rerolls;
        w.set_input(
            0,
            PlayerInput {
                reroll: true,
                ..Default::default()
            },
        );
        w.step();
        let hero = w.hero(0).unwrap();
        assert_eq!(hero.rerolls, before - 1);
        assert_eq!(
            hero.offers.len(),
            1,
            "a reroll replaces the hand, it does not add one"
        );
    }

    #[test]
    fn a_skip_passes_on_the_hand() {
        let mut w = world();
        w.xp = xp_to_next(1);
        w.step();
        let abilities = w.hero(0).unwrap().abilities.len();
        w.set_input(
            0,
            PlayerInput {
                skip: true,
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
        assert_eq!(w.hero(0).unwrap().abilities.len(), abilities);
        assert_eq!(w.hero(0).unwrap().abilities[0].level, 1);
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
                ..Default::default()
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

    /// Steps, taking the first card whenever one is up, so the run never stalls on a hand.
    fn play(w: &mut World, n: usize) {
        for _ in 0..n {
            for p in w.heroes.iter().map(|h| h.player).collect::<Vec<_>>() {
                w.set_input(
                    p,
                    PlayerInput {
                        pick: Some(0),
                        ..Default::default()
                    },
                );
            }
            w.step();
        }
    }

    #[test]
    fn a_friend_standing_by_raises_a_fallen_hero() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[0].max_hp = 1e9;
        w.heroes[0].hp = 1e9;
        w.hurt_hero(1, 10_000.0);
        assert!(!w.hero(1).unwrap().alive);
        assert!(!w.game_over(), "one hero still stands");
        // The heroes start 2.5 yards apart: inside reach.
        play(&mut w, (REVIVE_SECS * TICK_HZ as f32) as usize + 2);
        let hero = w.hero(1).unwrap();
        assert!(hero.alive, "not raised");
        assert!((hero.hp - hero.max_hp * REVIVE_HEALTH).abs() < 1e-3);
        assert!(w
            .drain_events()
            .contains(&Event::HeroRevived { player: 1, by: 0 }));
        // The grace: a blow right after rising does nothing.
        w.hurt_hero(1, 10_000.0);
        assert!(w.hero(1).unwrap().alive);
    }

    #[test]
    fn a_fallen_hero_alone_stays_down() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[0].max_hp = 1e9;
        w.heroes[0].hp = 1e9;
        w.heroes[1].pos = w.heroes[0].pos + V2::new(REVIVE_RANGE * 3.0, 0.0);
        w.hurt_hero(1, 10_000.0);
        play(&mut w, (REVIVE_SECS * TICK_HZ as f32) as usize * 2);
        assert!(!w.hero(1).unwrap().alive);
        assert_eq!(w.hero(1).unwrap().revive, 0.0);
    }

    #[test]
    fn a_fallen_hero_keeps_its_cards_and_is_not_waited_on() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(REVIVE_RANGE * 3.0, 0.0);
        w.hurt_hero(1, 10_000.0);
        w.xp = xp_to_next(1);
        w.step();
        assert_eq!(
            w.hero(1).unwrap().offers.len(),
            1,
            "a hand for the fallen too"
        );
        w.set_input(
            0,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused(), "the run waited on a fallen hero");
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
                ..Default::default()
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
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
    }

    /// The world with nothing in the arena but one `kind` of interactable, under hero 0's feet.
    fn only(w: &mut World, kind: InteractableKind) -> u32 {
        w.interactables.clear();
        let at = w.hero(0).unwrap().pos;
        w.place(kind, at)
    }

    /// Steps until the circle under hero 0 has had time to fill.
    fn fill(w: &mut World, kind: InteractableKind) {
        steps(w, (kind.charge_secs() * TICK_HZ as f32) as usize + 2);
    }

    #[test]
    fn a_coin_lands_in_every_purse_with_its_own_gold_gain() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].stats.add(Stat::GoldGain, 0.5);
        let at = w.hero(0).unwrap().pos;
        w.gems.push(Gem {
            id: 77,
            pos: at,
            xp: 1,
            gold: 10,
        });
        w.step();
        assert_eq!(w.hero(0).unwrap().gold, 10);
        assert_eq!(w.hero(1).unwrap().gold, 15, "player 1 has more gold gain");
        assert!(w.drain_events().contains(&Event::GoldGained {
            player: 1,
            amount: 15
        }));
    }

    #[test]
    fn elites_always_drop_gold_and_more_experience() {
        let mut w = world();
        w.interactables.clear();
        let far = w.center() + V2::new(100.0, 0.0);
        dummy(&mut w, 500, far, 0.0);
        w.enemies[0].elite = true;
        w.step();
        let gem = w
            .gems
            .iter()
            .find(|g| g.pos == far)
            .expect("a gem where it fell");
        assert_eq!(gem.xp, w.content().enemies[0].xp * ELITE_REWARD);
        assert!(gem.gold > 0);
    }

    #[test]
    fn a_tome_card_raises_its_stat_and_health_follows() {
        let mut w = world();
        let before = w.hero(0).unwrap().max_hp;
        w.heroes[0].hp = before / 2.0;
        w.take_card(
            0,
            Card {
                kind: CardKind::Tome(Stat::Health),
                rarity: Rarity::Common,
            },
        );
        let hero = w.hero(0).unwrap();
        assert!((hero.max_hp - before * 1.1).abs() < 1e-3);
        assert!(
            (hero.hp - (before / 2.0 + before * 0.1)).abs() < 1e-3,
            "the new health is healed"
        );
        w.take_card(
            0,
            Card {
                kind: CardKind::Tome(Stat::Speed),
                rarity: Rarity::Legendary,
            },
        );
        let class_speed = w.content().classes[0].speed;
        assert!(w.hero(0).unwrap().speed > class_speed * 1.15);
    }

    #[test]
    fn armor_softens_blows_and_regen_heals() {
        let mut w = world();
        w.heroes[0].stats.add(Stat::Armor, 0.5);
        let took = w.hurt_hero(0, 10.0);
        assert!((took - 5.0).abs() < 1e-4);
        w.interactables.clear();
        w.heroes[0].stats.add(Stat::Regen, 3.0);
        let hp = w.hero(0).unwrap().hp;
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(0.0, 1.0),
                ..Default::default()
            },
        );
        steps(&mut w, TICK_HZ as usize);
        assert!(w.hero(0).unwrap().hp > hp + 2.0, "regen did not heal");
    }

    #[test]
    fn pickup_range_reaches_further_and_wisdom_gives_more_experience() {
        let mut w = world();
        w.interactables.clear();
        let at = w.hero(0).unwrap().pos + V2::new(MAGNET + 2.0, 0.0);
        w.gems.push(Gem {
            id: 77,
            pos: at,
            xp: 2,
            gold: 0,
        });
        w.step();
        assert_eq!(w.gems.len(), 1, "out of plain reach");
        w.heroes[0].stats.add(Stat::Pickup, 1.0);
        w.heroes[0].stats.add(Stat::XpGain, 0.5);
        w.step();
        assert!(w.gems.is_empty());
        assert!((w.xp - 3.0).abs() < 1e-4, "xp {}", w.xp);
    }

    #[test]
    fn a_titan_shrine_fills_while_stood_in_and_deals_tomes() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::TitanShrine);
        steps(&mut w, TICK_HZ as usize);
        let charge = w.interactable(id).unwrap().charge;
        assert!(
            charge > 0.3 && charge < 0.4,
            "a third full after a second: {charge}"
        );
        fill(&mut w, InteractableKind::TitanShrine);
        let offer = &w.hero(0).unwrap().offers[0];
        assert_eq!(
            offer.source,
            Source::Interactable(InteractableKind::TitanShrine)
        );
        assert_eq!(offer.cards.len(), 3);
        assert!(offer
            .cards
            .iter()
            .all(|c| matches!(c.kind, CardKind::Tome(_))));
        assert!(w.paused(), "a shrine's choice waits like a level-up");
        assert!(matches!(
            w.interactable(id).unwrap().state,
            InteractState::Spent(_)
        ));
        assert!(w.drain_events().iter().any(|e| matches!(
            e,
            Event::Interacted { id: i, by, .. } if *i == id && by == &vec![0]
        )));
    }

    #[test]
    fn a_circle_drains_when_left_and_a_used_one_goes() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::TitanShrine);
        steps(&mut w, TICK_HZ as usize);
        w.heroes[0].pos += V2::new(20.0, 0.0);
        w.set_input(0, PlayerInput::default());
        steps(&mut w, TICK_HZ as usize * 3);
        assert_eq!(w.interactable(id).unwrap().charge, 0.0, "drained away");
        w.interactables[0].state = InteractState::Spent(0.5);
        steps(&mut w, TICK_HZ as usize);
        assert!(
            w.interactable(id).is_none(),
            "a used shrine leaves the world"
        );
    }

    #[test]
    fn friends_in_the_circle_each_choose_and_one_outside_does_not() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        w.add_hero(2, "mage").unwrap();
        w.heroes[2].pos += V2::new(30.0, 0.0);
        only(&mut w, InteractableKind::TitanShrine);
        // Hero 1 starts 2.5 yards from hero 0: inside the 3-yard circle.
        fill(&mut w, InteractableKind::TitanShrine);
        assert_eq!(w.hero(0).unwrap().offers.len(), 1);
        assert_eq!(w.hero(1).unwrap().offers.len(), 1);
        assert!(w.hero(2).unwrap().offers.is_empty());
    }

    #[test]
    fn a_golden_shrine_deals_only_legendary_tomes() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::GoldenShrine);
        fill(&mut w, InteractableKind::GoldenShrine);
        let offer = &w.hero(0).unwrap().offers[0];
        assert!(offer
            .cards
            .iter()
            .all(|c| c.rarity == Rarity::Legendary && matches!(c.kind, CardKind::Tome(_))));
        assert!(matches!(
            w.interactable(id).unwrap().state,
            InteractState::Spent(_)
        ));
    }

    #[test]
    fn a_reroll_of_a_shrine_hand_deals_tomes_again() {
        let mut w = world();
        only(&mut w, InteractableKind::TitanShrine);
        fill(&mut w, InteractableKind::TitanShrine);
        w.set_input(
            0,
            PlayerInput {
                reroll: true,
                ..Default::default()
            },
        );
        w.step();
        let offer = &w.hero(0).unwrap().offers[0];
        assert_eq!(
            offer.source,
            Source::Interactable(InteractableKind::TitanShrine)
        );
        assert!(offer
            .cards
            .iter()
            .all(|c| matches!(c.kind, CardKind::Tome(_))));
    }

    #[test]
    fn a_pact_gives_its_boon_and_takes_its_bane() {
        let mut w = world();
        only(&mut w, InteractableKind::CursedAltar);
        fill(&mut w, InteractableKind::CursedAltar);
        let offer = w.hero(0).unwrap().offers[0].clone();
        for card in &offer.cards {
            let CardKind::Pact(p) = card.kind else {
                panic!("an altar deals pacts: {card:?}");
            };
            assert!(card.rarity >= Rarity::Rare);
            assert_ne!(p.bane, Bane::Weaker(p.boon), "a pact that undoes itself");
        }
        let personal = Card {
            kind: CardKind::Pact(Pact {
                boon: Stat::Damage,
                bane: Bane::Weaker(Stat::Health),
            }),
            rarity: Rarity::Rare,
        };
        let before = w.hero(0).unwrap().max_hp;
        w.take_card(0, personal);
        let hero = w.hero(0).unwrap();
        let boon = Stat::Damage.tome_amount(Rarity::Rare) * PACT_BOON;
        assert!((hero.stats.get(Stat::Damage) - boon).abs() < 1e-5);
        assert!(hero.max_hp < before);
        assert_eq!(w.difficulty, Difficulty::default(), "a personal bane");
        let shared = Card {
            kind: CardKind::Pact(Pact {
                boon: Stat::Projectiles,
                bane: Bane::EnemySpeed,
            }),
            rarity: Rarity::Rare,
        };
        w.take_card(0, shared);
        assert_eq!(w.hero(0).unwrap().stats.get(Stat::Projectiles), 1.0);
        assert!(w.difficulty.enemy_speed > 0.0);
    }

    #[test]
    fn a_chest_waits_for_gold_and_only_its_opener_pays_and_chooses() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        let id = only(&mut w, InteractableKind::Chest);
        fill(&mut w, InteractableKind::Chest);
        assert_eq!(w.interactable(id).unwrap().charge, 0.0, "nobody can pay");
        w.heroes[1].gold = chest_price(0) + 5;
        fill(&mut w, InteractableKind::Chest);
        let (opener, friend) = (w.hero(1).unwrap(), w.hero(0).unwrap());
        assert_eq!(opener.gold, 5);
        assert_eq!(opener.chests, 1);
        assert!(
            chest_price(opener.chests) > chest_price(0),
            "the next costs more"
        );
        assert_eq!(opener.offers.len(), 1);
        assert!(opener.offers[0]
            .cards
            .iter()
            .all(|c| matches!(c.kind, CardKind::Item(i) if i.rarity() == c.rarity)));
        assert!(friend.offers.is_empty(), "a friend's chest is not ours");
        assert!(matches!(
            w.interactable(id).unwrap().state,
            InteractState::Spent(_)
        ));
    }

    #[test]
    fn a_won_challenge_leaves_a_free_cache() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::ChallengeIdol);
        fill(&mut w, InteractableKind::ChallengeIdol);
        let InteractState::Challenge(elites) = w.interactable(id).unwrap().state.clone() else {
            panic!("no challenge under way");
        };
        assert_eq!(elites.len(), CHALLENGE_ELITES + CHALLENGE_PER_HERO);
        let tough = w.enemy(elites[0]).unwrap();
        assert!(tough.elite && tough.max_hp >= w.content().enemies[tough.def].hp * ELITE_HEALTH);
        for e in &mut w.enemies {
            if elites.contains(&e.id) {
                e.hp = 0.0;
            }
        }
        steps(&mut w, 2);
        assert!(matches!(
            w.interactable(id).unwrap().state,
            InteractState::Spent(_)
        ));
        let cache = w
            .interactables
            .iter()
            .find(|i| i.kind == InteractableKind::Cache)
            .expect("a cache where the idol stood");
        assert!(cache.ready());
        let (cache_id, cache_pos) = (cache.id, cache.pos);
        assert!(w.drain_events().contains(&Event::ChallengeWon { id }));
        // Free: no gold needed.
        w.heroes[0].pos = cache_pos;
        fill(&mut w, InteractableKind::Cache);
        assert_eq!(w.hero(0).unwrap().offers.len(), 1);
        assert!(matches!(
            w.interactable(cache_id).unwrap().state,
            InteractState::Spent(_)
        ));
    }

    #[test]
    fn an_interactable_moves_but_stays_in_the_arena() {
        let mut w = world();
        let id = w.interactables[0].id;
        let to = w.center() + V2::new(10.0, -4.0);
        w.move_interactable(id, to);
        assert_eq!(w.interactable(id).unwrap().pos, to);
        w.move_interactable(id, w.center() + V2::new(0.0, 900.0));
        assert!((w.interactable(id).unwrap().pos.distance(w.center()) - 150.0).abs() < 1e-3);
    }

    /// Give hero 0 `n` copies of `item`, as picking its card `n` times would.
    fn hold(w: &mut World, item: Item, n: u8) {
        for _ in 0..n {
            w.take_card(
                0,
                Card {
                    kind: CardKind::Item(item),
                    rarity: item.rarity(),
                },
            );
        }
    }

    #[test]
    fn an_item_card_stacks_and_a_stat_item_raises_its_stat() {
        let mut w = world();
        hold(&mut w, Item::RabbitsFoot, 2);
        let hero = w.hero(0).unwrap();
        assert_eq!(hero.stacks(Item::RabbitsFoot), 2);
        assert!((hero.stats.get(Stat::Luck) - 0.4).abs() < 1e-5);
    }

    #[test]
    fn a_static_shard_arcs_to_the_enemies_around_a_hit() {
        let mut w = world();
        w.interactables.clear();
        hold(&mut w, Item::StaticShard, 10);
        let at = w.center() + V2::new(60.0, 0.0);
        for k in 0..4 {
            dummy(&mut w, 900 + k, at + V2::new(k as f32, 0.0), 1000.0);
        }
        // Five tries at the capped 50% chance: an arc all but surely comes.
        for _ in 0..5 {
            w.on_hit(0, 900, 100.0);
        }
        let shard = w.content().ability("static_shard").unwrap();
        let hero = w.hero(0).unwrap();
        assert!(hero.damage_by.iter().any(|&(a, d)| a == shard && d > 0.0));
        assert_eq!(
            w.enemy(900).unwrap().hp,
            1000.0,
            "the arc skips its own target"
        );
        assert!(w.enemy(901).unwrap().hp < 1000.0);
    }

    #[test]
    fn a_kill_heals_with_fangs_and_may_burst_and_spill_gold() {
        let mut w = world();
        w.interactables.clear();
        hold(&mut w, Item::VampiricFang, 4);
        hold(&mut w, Item::EmberCore, 10);
        hold(&mut w, Item::GoblinPurse, 10);
        w.heroes[0].hp = 10.0;
        let at = w.center() + V2::new(60.0, 0.0);
        dummy(&mut w, 950, at + V2::new(1.0, 0.0), 1000.0);
        for _ in 0..10 {
            w.on_kill(0, at, 500.0, 4);
        }
        let hero = w.hero(0).unwrap();
        assert!((hero.hp - (10.0 + 10.0 * 4.0 * FANG_HEAL)).abs() < 1e-3);
        assert!(hero.gold > 0, "ten kills at 40% never spilled gold");
        assert!(w.enemy(950).unwrap().hp < 1000.0, "no ember ever burst");
    }

    #[test]
    fn a_frost_ward_blocks_a_blow_then_recovers() {
        let mut w = world();
        hold(&mut w, Item::FrostWard, 1);
        let hp = w.hero(0).unwrap().hp;
        assert_eq!(w.hurt_hero(0, 30.0), 0.0, "the ward takes the first blow");
        assert_eq!(w.hero(0).unwrap().hp, hp);
        assert!(w.hurt_hero(0, 30.0) > 0.0, "and not the next");
        w.heroes[0].barrier = DT;
        w.regenerate();
        assert_eq!(w.hurt_hero(0, 30.0), 0.0, "ready again");
    }

    #[test]
    fn a_skeleton_key_can_open_a_chest_for_free() {
        let mut w = world();
        hold(&mut w, Item::SkeletonKey, 5);
        only(&mut w, InteractableKind::Chest);
        w.heroes[0].gold = chest_price(0);
        fill(&mut w, InteractableKind::Chest);
        assert_eq!(
            w.hero(0).unwrap().gold,
            chest_price(0),
            "five keys always open free"
        );
        assert_eq!(w.hero(0).unwrap().chests, 1);
    }

    #[test]
    fn luck_finds_rarer_treasure() {
        let mut w = world();
        let options: Vec<CardKind> = Item::ALL.map(CardKind::Item).to_vec();
        let epic = |w: &mut World, luck: f32| {
            (0..2000)
                .filter(|_| {
                    let i = w.lucky_item(&options, luck);
                    options[i] == CardKind::Item(Item::FrostWard)
                })
                .count()
        };
        let plain = epic(&mut w, 0.0);
        let lucky = epic(&mut w, 2.0);
        assert!(lucky > plain * 3, "{lucky} vs {plain}");
    }

    #[test]
    fn the_boss_comes_on_time_and_leaves_a_cache() {
        let mut w = world();
        w.interactables.clear();
        w.elapsed = BOSS_EVERY - DT * 0.5;
        w.step();
        let boss = w.enemies.iter().find(|e| e.boss).expect("the boss came");
        let (id, def) = (boss.id, boss.def);
        assert_eq!(w.content().enemies[def].name, "Hogger");
        assert!((w.next_boss - BOSS_EVERY * 2.0).abs() < 0.1, "{}", w.next_boss);
        assert!(w.drain_events().contains(&Event::BossCame { def }));
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        assert!(w
            .interactables
            .iter()
            .any(|i| i.kind == InteractableKind::Cache && i.ready()));
        assert!(w.drain_events().contains(&Event::BossDied { id }));
    }

    #[test]
    fn a_slow_boss_fight_does_not_bring_the_next_boss_at_once() {
        let mut w = world();
        w.interactables.clear();
        w.elapsed = BOSS_EVERY - DT * 0.5;
        w.step();
        let id = w.enemies.iter().find(|e| e.boss).expect("the boss came").id;
        // The fight drags on past when the next boss was once due.
        w.elapsed = BOSS_EVERY * 2.0 + 20.0;
        w.step();
        assert_eq!(w.enemies.iter().filter(|e| e.boss).count(), 1);
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        w.step();
        assert!(!w.enemies.iter().any(|e| e.boss), "no fresh boss straight after the kill");
        assert!(w.next_boss > w.elapsed + BOSS_EVERY - 1.0, "{} vs {}", w.next_boss, w.elapsed);
    }

    /// Hero 0 too tough to fall, for tests that run a while: through its stats, so a card's
    /// refresh keeps it.
    fn sturdy(w: &mut World) {
        w.heroes[0].stats.add(Stat::Health, 1e6);
        w.refresh(0);
        w.heroes[0].hp = w.heroes[0].max_hp;
    }

    /// The shrines standing, not counting pots and caches.
    fn shrines(w: &World) -> Vec<&Interactable> {
        w.interactables
            .iter()
            .filter(|i| {
                i.live() && !matches!(i.kind, InteractableKind::Pot | InteractableKind::Cache)
            })
            .collect()
    }

    #[test]
    fn a_used_shrine_makes_room_for_a_new_one_out_of_sight() {
        let mut w = world();
        sturdy(&mut w);
        let cap = w.zone().shrines_at_once as usize;
        assert_eq!(shrines(&w).len(), cap, "a full arena to start");
        let first = shrines(&w)[0].id;
        w.interactables.retain(|i| i.id != first);
        let every = (w.content().rules.shrine_every * TICK_HZ as f32) as usize;
        play(&mut w, every + 2);
        assert_eq!(shrines(&w).len(), cap, "a new one came");
        let hero = w.hero(0).unwrap().pos;
        let new = shrines(&w).into_iter().max_by_key(|i| i.id).unwrap();
        assert!(
            new.pos.distance(hero) >= 40.0,
            "it came in sight: {:?}",
            new.pos
        );
        play(&mut w, every * 3);
        assert_eq!(shrines(&w).len(), cap, "never more than the zone allows");
    }

    /// The pots standing.
    fn pots(w: &World) -> Vec<V2> {
        w.interactables
            .iter()
            .filter(|i| i.kind == InteractableKind::Pot && i.ready())
            .map(|i| i.pos)
            .collect()
    }

    #[test]
    fn pots_start_scattered_over_the_arena_and_break_at_a_touch() {
        let mut w = world();
        let all = pots(&w);
        assert_eq!(all.len() as u32, w.zone().pots, "a full arena to start");
        let center = w.center();
        let spread = all
            .iter()
            .map(|p| p.distance(center))
            .fold(0.0f32, f32::max);
        assert!(spread > 60.0, "bunched near the start: {spread}");
        let pot = all[0];
        w.heroes[0].pos = pot + V2::new(0.8, 0.0);
        w.step();
        let events = w.drain_events();
        let loot = events.iter().find_map(|e| match e {
            Event::PotBroken { by: 0, loot, .. } => Some(*loot),
            _ => None,
        });
        match loot.expect("no word of what it held") {
            PotLoot::Gold(g) => assert!(g > 0 && w.hero(0).unwrap().gold > 0),
            PotLoot::Xp(x) => assert!(x > 0),
            PotLoot::Pickup(kind) => {
                let lying = w.pickups.iter().find(|p| p.kind == kind).expect("dropped");
                let off = lying.pos.distance(w.hero(0).unwrap().pos);
                assert!(off > 3.0 && off < 6.0, "{off} yards off");
            }
        }
        assert!(!pots(&w).contains(&pot), "the pot is gone");
    }

    #[test]
    fn a_power_up_from_a_pot_waits_a_few_steps_off() {
        let mut w = world();
        w.interactables.clear();
        let at = w.hero(0).unwrap().pos + V2::new(1.0, 0.0);
        // Break pots until one holds a power-up.
        for _ in 0..200 {
            let id = w.place(InteractableKind::Pot, at);
            w.break_pot(id, at, 0);
            if w.pickups
                .iter()
                .any(|p| matches!(p.kind, PickupKind::PowerUp(_)))
            {
                break;
            }
        }
        let up = w
            .pickups
            .iter()
            .find(|p| matches!(p.kind, PickupKind::PowerUp(_)))
            .expect("no power-up in 200 pots")
            .clone();
        w.step();
        assert!(
            w.pickups.iter().any(|p| p.id == up.id),
            "taken without a step to it"
        );
        assert!(w.buffs.is_empty() || up.kind == PickupKind::PowerUp(PowerUp::Magnet));
        w.heroes[0].pos = up.pos;
        w.step();
        assert!(
            w.pickups.iter().all(|p| p.id != up.id),
            "not taken when stepped on"
        );
    }

    #[test]
    fn a_broken_pot_is_made_good_out_of_sight() {
        let mut w = world();
        sturdy(&mut w);
        let pot = pots(&w)[0];
        w.interactables.retain(|i| i.pos != pot);
        play(&mut w, (arena::POT_EVERY * TICK_HZ as f32) as usize + 2);
        assert_eq!(pots(&w).len() as u32, w.zone().pots, "no new pot came");
        let hero = w.hero(0).unwrap().pos;
        let newest = w
            .interactables
            .iter()
            .filter(|i| i.kind == InteractableKind::Pot)
            .max_by_key(|i| i.id)
            .unwrap();
        assert!(
            newest.pos.distance(hero) >= arena::POT_UNSEEN,
            "it came in sight"
        );
    }

    #[test]
    fn a_carried_banner_reaching_its_spot_in_time_leaves_a_cache() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::WarBanner);
        let start = w.interactable(id).unwrap().pos;
        fill(&mut w, InteractableKind::WarBanner);
        let InteractState::Escort { carrier, to, left } = w.interactable(id).unwrap().state else {
            panic!("no carry under way");
        };
        assert_eq!(carrier, 0);
        let far = to.distance(start);
        assert!(far >= 30.0, "a carry of only {far} yards");
        assert!(left > far / w.hero(0).unwrap().speed);
        w.heroes[0].pos = to;
        w.step();
        assert!(w
            .drain_events()
            .contains(&Event::EscortDone { id, won: true }));
        assert!(w
            .interactables
            .iter()
            .any(|i| i.kind == InteractableKind::Cache && i.pos == to));
    }

    #[test]
    fn a_banner_carried_too_slowly_is_lost() {
        let mut w = world();
        let id = only(&mut w, InteractableKind::WarBanner);
        fill(&mut w, InteractableKind::WarBanner);
        let InteractState::Escort { left, .. } = w.interactable(id).unwrap().state else {
            panic!("no carry under way");
        };
        w.heroes[0].max_hp = 1e9;
        w.heroes[0].hp = 1e9;
        play(&mut w, (left * TICK_HZ as f32) as usize + 2);
        assert!(w
            .drain_events()
            .contains(&Event::EscortDone { id, won: false }));
        assert!(!w
            .interactables
            .iter()
            .any(|i| i.kind == InteractableKind::Cache));
    }

    /// Lay `kind` at hero 0's feet and step once, so it is taken.
    fn grab(w: &mut World, kind: PickupKind) {
        let at = w.hero(0).unwrap().pos;
        w.drop_pickup(at, kind);
        w.step();
    }

    #[test]
    fn a_health_shard_heals_a_quarter() {
        let mut w = world();
        w.heroes[0].hp = 10.0;
        grab(&mut w, PickupKind::Health);
        let hero = w.hero(0).unwrap();
        assert!(hero.hp >= 10.0 + hero.max_hp * 0.25 - 1e-3);
        assert!(w.pickups.is_empty());
    }

    #[test]
    fn a_power_up_works_for_the_whole_party_then_runs_out() {
        let mut w = world();
        sturdy(&mut w);
        w.add_hero(1, "mage").unwrap();
        let speed = w.hero(1).unwrap().speed;
        grab(&mut w, PickupKind::PowerUp(PowerUp::Swiftness));
        assert!((w.hero(1).unwrap().speed - speed * (1.0 + SWIFTNESS)).abs() < 1e-3);
        let secs = PowerUp::Swiftness.duration().unwrap();
        play(&mut w, (secs * TICK_HZ as f32) as usize + 2);
        assert!(w.buffs.is_empty());
        assert!(
            (w.hero(1).unwrap().speed - speed).abs() < 1e-3,
            "back to normal"
        );
    }

    #[test]
    fn berserk_bloodlust_shield_and_fortune_do_their_work() {
        let mut w = world();
        let def = w.content().abilities[0].clone();
        let plain = w.sharpen(ability_stats(&def, 0.0));
        grab(&mut w, PickupKind::PowerUp(PowerUp::Berserk));
        grab(&mut w, PickupKind::PowerUp(PowerUp::Bloodlust));
        let sharp = w.sharpen(ability_stats(&def, 0.0));
        assert!((sharp.damage - plain.damage * 2.0).abs() < 1e-3);
        assert!((sharp.cooldown - plain.cooldown * BLOODLUST).abs() < 1e-3);
        grab(&mut w, PickupKind::PowerUp(PowerUp::DivineShield));
        assert_eq!(w.hurt_hero(0, 50.0), 0.0);
        grab(&mut w, PickupKind::PowerUp(PowerUp::Fortune));
        let gold = w.hero(0).unwrap().gold;
        w.give_gold(0, 10.0);
        assert_eq!(w.hero(0).unwrap().gold, gold + 20);
    }

    #[test]
    fn a_detonation_slays_the_horde_near_and_wounds_a_boss() {
        let mut w = world();
        w.interactables.clear();
        let at = w.hero(0).unwrap().pos;
        dummy(&mut w, 700, at + V2::new(10.0, 0.0), 500.0);
        dummy(&mut w, 701, at + V2::new(10.0, 2.0), 10_000.0);
        w.enemies[1].boss = true;
        dummy(&mut w, 702, at + V2::new(60.0, 0.0), 500.0);
        w.power_up(PowerUp::Detonation, 0);
        assert!(w.enemy(700).unwrap().hp <= 0.0);
        let boss = w.enemy(701).unwrap();
        assert!((boss.hp - 10_000.0 * 0.85).abs() < 1.0);
        assert_eq!(w.enemy(702).unwrap().hp, 500.0, "out of reach");
    }

    #[test]
    fn a_magnet_draws_in_every_gem() {
        let mut w = world();
        for k in 0..5u32 {
            let far = w.center() + V2::new(60.0 + k as f32, 40.0);
            w.drop_gem(far, 1, 2);
        }
        grab(&mut w, PickupKind::PowerUp(PowerUp::Magnet));
        assert!(w.gems.is_empty());
        assert!(w.hero(0).unwrap().gold >= 10);
    }

    #[test]
    fn what_nobody_takes_fades() {
        let mut w = world();
        sturdy(&mut w);
        let far = w.center() + V2::new(70.0, 0.0);
        w.drop_pickup(far, PickupKind::Health);
        // Level-ups hold the clock a step each: play by the clock, not by steps.
        for _ in 0..TICK_HZ * 60 {
            if w.elapsed > arena::PICKUP_LIFE + 0.1 {
                break;
            }
            play(&mut w, 1);
        }
        assert!(w.pickups.iter().all(|p| p.pos != far));
    }

    #[test]
    fn a_mirror_sees_the_hosts_shrines_pickups_and_power_ups() {
        let mut host = world();
        let mut mirror = World::new(Arc::new(sample()), "elwynn", 99).unwrap();
        grab(&mut host, PickupKind::PowerUp(PowerUp::Fortune));
        let far = host.center() + V2::new(70.0, 0.0);
        host.drop_pickup(far, PickupKind::Health);
        for _ in 0..TICK_HZ * 3 {
            host.step();
            let events = host.drain_events();
            mirror.apply_snapshot(host.snapshot(), events);
        }
        assert_eq!(mirror.interactables, host.interactables);
        assert_eq!(mirror.pickups, host.pickups);
        assert_eq!(mirror.buffs, host.buffs);
        assert!(!mirror.buffs.is_empty());
    }

    /// Take a tome of `stat` `n` times, as picking its card would.
    fn read(w: &mut World, stat: Stat, n: u8) {
        for _ in 0..n {
            w.take_card(
                0,
                Card {
                    kind: CardKind::Tome(stat),
                    rarity: Rarity::Common,
                },
            );
        }
    }

    /// Every tome in hands dealt by `source` to hero 0, over many deals.
    fn tomes_dealt(w: &mut World, source: Source) -> Vec<Stat> {
        (0..200)
            .flat_map(|_| w.deal(0, source).cards)
            .filter_map(|c| match c.kind {
                CardKind::Tome(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tomes_rank_up_and_full_slots_offer_no_new_tome() {
        let mut w = world();
        let slots = w.content().rules.tome_slots as usize;
        let held = [Stat::Damage, Stat::Speed, Stat::Luck, Stat::Armor];
        assert_eq!(held.len(), slots);
        read(&mut w, Stat::Damage, 2);
        assert_eq!(w.hero(0).unwrap().tome_rank(Stat::Damage), 2);
        for &s in &held[1..] {
            read(&mut w, s, 1);
        }
        for source in [
            Source::LevelUp,
            Source::Interactable(InteractableKind::TitanShrine),
        ] {
            let dealt = tomes_dealt(&mut w, source);
            assert!(!dealt.is_empty(), "{source:?} dealt no tomes at all");
            assert!(
                dealt.iter().all(|s| held.contains(s)),
                "{source:?} offered a new tome with the slots full: {dealt:?}"
            );
        }
    }

    #[test]
    fn a_maxed_tome_is_never_offered() {
        let mut w = world();
        let max = w.content().rules.tome_max_rank;
        read(&mut w, Stat::Damage, max);
        assert_eq!(w.hero(0).unwrap().tome_rank(Stat::Damage), max);
        let dealt = tomes_dealt(&mut w, Source::Interactable(InteractableKind::TitanShrine));
        assert!(!dealt.contains(&Stat::Damage));
    }

    #[test]
    fn a_maxed_item_is_never_offered() {
        let mut w = world();
        let limit = w.content().rules.item_max_copies;
        hold(
            &mut w,
            Item::StaticShard,
            Item::StaticShard.max_copies(limit),
        );
        let chest = Source::Interactable(InteractableKind::Chest);
        for _ in 0..200 {
            let hand = w.deal(0, chest);
            assert!(!hand
                .cards
                .iter()
                .any(|c| c.kind == CardKind::Item(Item::StaticShard)));
        }
    }

    #[test]
    fn when_everything_is_maxed_a_hand_is_gold_a_heal_and_a_reroll() {
        let mut w = world();
        let rules = w.content().rules.clone();
        let limit = rules.item_max_copies;
        for item in Item::ALL {
            hold(&mut w, item, item.max_copies(limit));
        }
        let chest = w.deal(0, Source::Interactable(InteractableKind::Chest));
        let kinds: Vec<CardKind> = chest.cards.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, CardKind::FALLBACKS.to_vec());
        for s in [Stat::Damage, Stat::Speed, Stat::Luck, Stat::Armor] {
            read(&mut w, s, rules.tome_max_rank);
        }
        let shrine = w.deal(0, Source::Interactable(InteractableKind::TitanShrine));
        assert_eq!(shrine.cards.len(), HAND, "a full hand, never a short one");
        assert!(shrine
            .cards
            .iter()
            .all(|c| CardKind::FALLBACKS.contains(&c.kind)));
        // Each does what it says.
        w.heroes[0].hp = 1.0;
        let gold = w.hero(0).unwrap().gold;
        let rerolls = w.hero(0).unwrap().rerolls;
        for kind in CardKind::FALLBACKS {
            w.take_card(
                0,
                Card {
                    kind,
                    rarity: Rarity::Common,
                },
            );
        }
        let hero = w.hero(0).unwrap();
        assert_eq!(hero.hp, hero.max_hp);
        assert_eq!(hero.gold, gold + fallback_gold(w.level));
        assert_eq!(hero.rerolls, rerolls + 1);
    }

    #[test]
    fn each_camp_gets_an_elite_who_guards_it() {
        let mut w = world();
        assert_eq!(w.camps.len() as u32, w.zone().elite_camps);
        w.step();
        for camp in &w.camps {
            let id = camp.elite.expect("an elite at every camp");
            let elite = w.enemy(id).unwrap();
            assert!(elite.elite && elite.home == Some(camp.pos));
        }
        // With no hero near, it stays home; a hero close by draws it out.
        let camp = w.camps[0].pos;
        let id = w.camps[0].elite.unwrap();
        w.heroes[0].pos = camp + V2::new(ELITE_AGGRO * 2.0, 0.0);
        play(&mut w, TICK_HZ as usize * 2);
        assert!(
            w.enemy(id).unwrap().pos.distance(camp) < 2.0,
            "it wandered off"
        );
        w.heroes[0].pos = camp + V2::new(ELITE_AGGRO * 0.5, 0.0);
        sturdy(&mut w);
        play(&mut w, TICK_HZ as usize);
        assert!(
            w.enemy(id).unwrap().pos.distance(camp) > 2.0,
            "it did not give chase"
        );
    }

    #[test]
    fn a_moved_camp_takes_its_elite_with_it() {
        let mut w = world();
        w.step();
        let id = w.camps[0].elite.unwrap();
        let to = w.center() + V2::new(-30.0, 30.0);
        w.move_camp(0, to);
        assert_eq!(w.camps[0].pos, to);
        assert_eq!(w.enemy(id).unwrap().home, Some(to));
    }

    #[test]
    fn a_slain_camp_elite_comes_back_after_the_respawn_time() {
        let mut w = world();
        w.interactables.clear();
        sturdy(&mut w);
        w.step();
        let id = w.camps[0].elite.unwrap();
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        assert!(w.enemy(id).is_none() && w.camps[0].elite.is_none());
        let wait = w.content().rules.elite_respawn;
        let start = w.elapsed;
        for _ in 0..TICK_HZ as usize * (wait as usize + 30) {
            if w.camps[0].elite.is_some() {
                break;
            }
            play(&mut w, 1);
        }
        let back = w.camps[0].elite.expect("no new elite came");
        assert_ne!(back, id);
        let waited = w.elapsed - start;
        assert!(
            (waited - wait).abs() < 0.5,
            "came back after {waited} s, not {wait}"
        );
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
                    ..Default::default()
                },
            );
            steps(&mut w, TICK_HZ as usize * 10);
            (w.kills, w.enemies.len(), w.hero(0).unwrap().hp)
        };
        assert_eq!(run(), run());
    }
}
