//! The game's data: classes, abilities, enemies and zones, read from RON files (`content/` at the
//! repository root) so they can be added to and tuned without recompiling.
//!
//! Everything refers to everything else by `key`; [`Content::from_ron`] resolves those keys to
//! indices once and fails with a [`ContentError`] naming the first one that does not exist.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::interact::Placement;
use crate::spell::{Tag, Tags, Targeting};

/// How an ability picks its targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AbilityKind {
    /// A missile at the nearest `count` enemies in `range`; lands after `missile_speed` travel, and
    /// splashes everything within `radius` of the target when `radius` is above 0.
    Bolt,
    /// An instant hit on the nearest `count` enemies in `range`.
    Strike,
    /// Everything within `radius` of where it lands (the caster, unless its targeting says
    /// otherwise), at once.
    Nova,
    /// A lasting area of `radius` where it lands, hitting what stands in it every second for
    /// `duration` seconds.
    Ground,
    /// A shield on the caster worth `shield` of its maximum health, until broken or `duration`
    /// seconds pass.
    Shield,
    /// Banks a charge of its `ward` kind on each cooldown, up to `charges`; with the bank full it
    /// strikes the nearest enemy in `range` instead (overflow).
    Ward,
    /// An effect on the caster (and, tagged Aura, on allies near it) for `duration` seconds: more
    /// damage, faster cooldowns, guard, haste, thorns, hiding.
    Buff,
}

/// What a [`AbilityKind::Ward`] spell's charges do when they spend themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WardKind {
    /// Heals a hero who drops below `heal_below` of their health by `heal` of it.
    Heal,
    /// Blocks a blow completely, and strikes its attacker back for the spell's damage.
    Block,
    /// Strikes an enemy that hits the hero for the spell's damage; the blow still lands.
    Thorns,
}

fn one() -> u32 {
    1
}
fn one_f() -> f32 {
    1.0
}
fn default_cooldown() -> f32 {
    2.0
}
fn default_range() -> f32 {
    20.0
}
fn default_enemy_radius() -> f32 {
    0.6
}
fn three() -> u32 {
    3
}
fn five() -> u32 {
    5
}
fn six() -> u32 {
    6
}

#[derive(Clone, Debug, Deserialize)]
pub struct AbilityDef {
    pub key: String,
    pub name: String,
    /// The `Spell.dbc` id whose visuals and sounds the client plays for this ability.
    pub visual: u32,
    pub kind: AbilityKind,
    pub damage: f32,
    #[serde(default = "default_cooldown")]
    pub cooldown: f32,
    #[serde(default = "default_range")]
    pub range: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default = "one")]
    pub count: u32,
    /// Yards per second for a [`AbilityKind::Bolt`]; 0 lands at once.
    #[serde(default)]
    pub missile_speed: f32,
    /// One line for the menu and tooltips; empty for none.
    #[serde(default)]
    pub description: String,
    /// What it is: school, delivery, effects. Augments, talents and tomes go by these.
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// How it picks where it lands; by its kind when left out (missiles and strikes the nearest
    /// enemy, novas around the caster, ground areas the thickest crowd).
    #[serde(default)]
    pub target: Option<Targeting>,
    /// Hits on each target (Arcane Missiles' missiles); 1 when left out.
    #[serde(default = "one")]
    pub volley: u32,
    /// Seconds a ground area or a shield lasts.
    #[serde(default)]
    pub duration: f32,
    /// Seconds of burning ground a nova leaves where it lands (Flamestrike); 0 for none.
    #[serde(default)]
    pub leaves_ground: f32,
    /// Share of each hit dealt again as Burning over four seconds (Pyroblast).
    #[serde(default)]
    pub burn: f32,
    /// Seconds a Freeze lasts, for a spell tagged Freeze.
    #[serde(default = "two_f")]
    pub freeze: f32,
    /// Seconds a Stun lasts, for a spell tagged Stun.
    #[serde(default = "one_f")]
    pub stun: f32,
    /// Yards a Knockback pushes, for a spell tagged Knockback.
    #[serde(default = "knockback_f")]
    pub knockback: f32,
    /// Max health a [`AbilityKind::Shield`] is worth (0.25 is a quarter).
    #[serde(default)]
    pub shield: f32,
    /// [`AbilityDef::tags`] as a set.
    #[serde(skip)]
    pub tag_set: Tags,
    // --- What the other classes' spells add (see `spell::SpellFx`). ---
    /// Share of its damage that heals the caster.
    #[serde(default)]
    pub leech: f32,
    /// Seconds of Fear (for a spell tagged Fear), Root (tagged Root) and Weakened on a hit.
    #[serde(default = "fear_f")]
    pub fear: f32,
    #[serde(default = "root_f")]
    pub root: f32,
    #[serde(default)]
    pub weaken: f32,
    /// Extra damage its target takes from every source (0.15 is +15%), and for how long.
    #[serde(default)]
    pub mark: f32,
    #[serde(default = "six_f")]
    pub mark_secs: f32,
    /// Sunder stacks each hit adds.
    #[serde(default)]
    pub sunder: u8,
    /// A heal's size as a share of max health; for a heal ward, the share of health below which
    /// its charges spend themselves.
    #[serde(default)]
    pub heal: f32,
    #[serde(default = "half_f")]
    pub heal_below: f32,
    /// What a [`AbilityKind::Ward`] banks, and how many charges at most.
    #[serde(default)]
    pub ward: Option<WardKind>,
    #[serde(default = "three_u8")]
    pub charges: u8,
    /// Jumps of a Chain spell.
    #[serde(default)]
    pub jumps: u32,
    /// Extra damage (1 is +100%) to enemies below `execute_at` of their health.
    #[serde(default)]
    pub execute: f32,
    #[serde(default = "execute_at_f")]
    pub execute_at: f32,
    /// Combo points a builder's cast gives.
    #[serde(default)]
    pub combo: u8,
    /// Times its damage over time stacks on one enemy, and its length in seconds.
    #[serde(default = "one_u8")]
    pub stacks: u8,
    #[serde(default = "burn_secs_f")]
    pub dot_time: f32,
    /// A summon or totem ([`AbilityKind::Ground`]): enemies it strikes each beat; it `follows`
    /// its caster (a pet) or stands where it was put (a totem).
    #[serde(default)]
    pub strikes: u32,
    #[serde(default)]
    pub follows: bool,
    /// A trap: a ground area that waits, armed, until an enemy steps in.
    #[serde(default)]
    pub trap: bool,
    /// Extra damage to enemies held by a Stun, Freeze or Fear (1 is +100%).
    #[serde(default)]
    pub vs_held: f32,
    /// Yards the caster moves the way it faces (a defensive button).
    #[serde(default)]
    pub leap: f32,
    /// A [`AbilityKind::Buff`]'s effects: share of blows taken away, extra move speed, how much
    /// faster cooldowns run, extra damage, damage back to attackers (a share of `damage`: 1 is all
    /// of it), seconds hidden from the horde, and the health share a killing blow leaves.
    #[serde(default)]
    pub guard: f32,
    #[serde(default)]
    pub haste: f32,
    #[serde(default)]
    pub haste_cd: f32,
    #[serde(default)]
    pub power: f32,
    #[serde(default)]
    pub thorns: f32,
    #[serde(default)]
    pub hide: f32,
    #[serde(default)]
    pub cheat: f32,
}

fn fear_f() -> f32 {
    crate::spell::FEAR_SECS
}
fn root_f() -> f32 {
    crate::spell::ROOT_SECS
}
fn six_f() -> f32 {
    6.0
}
fn half_f() -> f32 {
    0.5
}
fn three_u8() -> u8 {
    3
}
fn execute_at_f() -> f32 {
    crate::spell::EXECUTE_AT
}
fn burn_secs_f() -> f32 {
    crate::spell::BURN_SECS
}

fn two_f() -> f32 {
    2.0
}

fn knockback_f() -> f32 {
    crate::spell::KNOCKBACK
}

impl AbilityDef {
    /// How it picks where it lands: its own `target`, or its kind's way.
    pub fn targeting(&self) -> Targeting {
        self.target.unwrap_or(match self.kind {
            AbilityKind::Bolt | AbilityKind::Strike => Targeting::Nearest,
            AbilityKind::Nova => Targeting::Around,
            AbilityKind::Ground => Targeting::Densest,
            AbilityKind::Shield | AbilityKind::Ward | AbilityKind::Buff => Targeting::Caster,
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDef {
    pub key: String,
    pub name: String,
    /// `CreatureDisplayInfo.dbc` id: the model the client shows.
    pub display: u32,
    #[serde(default = "one_f")]
    pub scale: f32,
    pub level: u32,
    pub hp: f32,
    /// Yards per second.
    pub speed: f32,
    /// Per swing.
    pub damage: f32,
    pub xp: u32,
    /// Body radius in yards, before `scale`.
    #[serde(default = "default_enemy_radius")]
    pub radius: f32,
    /// What its elites and bosses do beyond walking up and swinging; ordinary ones never do.
    #[serde(default)]
    pub mechanics: Vec<crate::mechanics::Mechanic>,
    /// A boss's sentence for the boss bar; without one, its mechanics' hints are used.
    #[serde(default)]
    pub hint: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneDef {
    pub key: String,
    pub name: String,
    /// The continent the menu names under the zone, e.g. "Eastern Kingdoms".
    #[serde(default)]
    pub continent: String,
    /// A line or two for the menu; empty for none.
    #[serde(default)]
    pub description: String,
    /// `Map.dbc` id: 0 Eastern Kingdoms, 1 Kalimdor.
    pub map: u32,
    /// The arena's centre in WoW coordinates, written `(x, y, z)`; `z` only seeds the camera.
    pub center: [f32; 3],
    /// How far from the centre heroes may go, in yards.
    pub leash: f32,
    /// Enemy keys, weakest first: the run brings in stronger ones as it goes on.
    pub enemies: Vec<String>,
    /// The kinds of shrine, altar, chest and banner that stand about the arena, each with how
    /// many may stand at once; none when left out.
    #[serde(default)]
    pub interactables: Vec<Placement>,
    /// How many of them stand at once (5 when left out): new ones come as old ones are used.
    #[serde(default = "five")]
    pub shrines_at_once: u32,
    /// How many pots stand scattered over the arena (6 when left out); a broken one is made good
    /// out of sight.
    #[serde(default = "six")]
    pub pots: u32,
    /// How many camps an elite guards (3 when left out); each gets a new elite the rules'
    /// `elite_respawn` seconds after its last one dies.
    #[serde(default = "three")]
    pub elite_camps: u32,
    /// The enemy key of the zone's boss, who comes every few minutes; none when left out.
    #[serde(default)]
    pub boss: String,
    /// The item level of the equipment its bosses and elites drop (10 when left out).
    #[serde(default = "ten_levels")]
    pub item_level: u16,
    /// [`ZoneDef::boss`] resolved to an index into [`Content::enemies`].
    #[serde(skip)]
    pub boss_ix: Option<usize>,
    /// [`ZoneDef::enemies`] resolved to indices into [`Content::enemies`].
    #[serde(skip)]
    pub enemy_ix: Vec<usize>,
    /// The rares that guard the zone's camps, camp by camp (enemy keys); none, and a camp's
    /// guard is a rare of the horde's own kind.
    #[serde(default)]
    pub rares: Vec<String>,
    #[serde(skip)]
    pub rare_ix: Vec<usize>,
}

fn ten_levels() -> u16 {
    10
}

/// One weapon a hero holds, as the client's virtual item fields want it.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct WeaponDef {
    /// 0 main hand, 1 off hand, 2 ranged.
    pub slot: u16,
    /// `ItemDisplayInfo.dbc` id.
    pub display: u32,
    pub class: u8,
    pub subclass: u8,
    pub material: u8,
    pub inventory_type: u8,
    pub sheath: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub key: String,
    pub name: String,
    /// A line or two for the menu; empty for none.
    #[serde(default)]
    pub description: String,
    /// The class colour (`RAID_CLASS_COLORS`), `0xRRGGBB`.
    #[serde(default = "white")]
    pub color: u32,
    /// `ChrClasses.dbc` id.
    pub class_id: u8,
    /// `UNIT_FIELD_BYTES_0` byte 3: 0 mana, 1 rage, 3 energy.
    pub power: u8,
    /// `CreatureDisplayInfo.dbc` ids of this class's trainers: the bodies a hero can wear.
    pub looks: Vec<u32>,
    #[serde(default)]
    pub weapons: Vec<WeaponDef>,
    /// The sheath state the hero fights in: 1 melee, 2 ranged.
    #[serde(default = "one_u8")]
    pub sheath: u8,
    pub base_hp: f32,
    /// Health back every second before any [`crate::Stat::Regen`]: a little for cloth, more for
    /// the classes who fight in the thick of it.
    #[serde(default = "half")]
    pub base_regen: f32,
    /// Yards per second.
    pub speed: f32,
    /// The ability every run starts with.
    pub start: String,
    /// The abilities level-up cards offer.
    pub pool: Vec<String>,
    /// The ability on the defensive button (F); none when left out.
    #[serde(default)]
    pub defensive: String,
    #[serde(skip)]
    pub start_ix: usize,
    #[serde(skip)]
    pub pool_ix: Vec<usize>,
    /// [`ClassDef::defensive`] resolved to an index into [`Content::abilities`].
    #[serde(skip)]
    pub defensive_ix: Option<usize>,
}

fn one_u8() -> u8 {
    1
}

fn half() -> f32 {
    0.5
}

fn white() -> u32 {
    0xFF_FF_FF
}

fn four() -> u32 {
    4
}
fn five_u8() -> u8 {
    5
}
fn ninety() -> f32 {
    90.0
}
fn one_eighty() -> f32 {
    180.0
}

/// The run's limits, from `rules.ron`: numbers a game mode may set its own way. Every one has a
/// default, so a mode's file names only what it changes.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Rules {
    /// How many different tomes a hero holds; once full, only those are raised.
    #[serde(default = "four")]
    pub tome_slots: u32,
    /// The rank a tome stops at.
    #[serde(default = "five_u8")]
    pub tome_max_rank: u8,
    /// The most copies of one item a hero holds. An item whose chance tops out sooner stops there
    /// ([`crate::Item::max_copies`]).
    #[serde(default = "five_u8")]
    pub item_max_copies: u8,
    /// Seconds between chances of a new shrine, while fewer than a zone's
    /// [`ZoneDef::shrines_at_once`] stand.
    #[serde(default = "ninety")]
    pub shrine_every: f32,
    /// Seconds after a camp's elite dies before a new one takes its place.
    #[serde(default = "one_eighty")]
    pub elite_respawn: f32,
    /// Every enemy's speed from its entry is multiplied by this...
    #[serde(default = "enemy_speed")]
    pub enemy_speed: f32,
    /// ...but held to at most this many yards a second (an entry already faster keeps its own),
    /// so a hero at the base 7 can still pull away from the horde without help.
    #[serde(default = "enemy_speed_cap")]
    pub enemy_speed_cap: f32,
    /// The horde comes from behind the way a hero runs. Halfway to each boss a swarm sends it
    /// from every side instead, for `surround_secs`.
    #[serde(default = "surround_secs")]
    pub surround_secs: f32,
    /// Health, in ordinary enemies: a rare (a camp's guard) and an elite (the horde's and an
    /// idol's) of their own kind's; a boss of the horde's strongest kind at the time, and the
    /// run's last boss (the `final_boss`th) more.
    #[serde(default = "rare_health")]
    pub rare_health: f32,
    #[serde(default = "elite_health")]
    pub elite_health: f32,
    #[serde(default = "boss_health")]
    pub boss_health: f32,
    #[serde(default = "final_boss_health")]
    pub final_boss_health: f32,
    #[serde(default = "final_boss")]
    pub final_boss: u32,
}

fn rare_health() -> f32 {
    30.0
}
fn final_boss_health() -> f32 {
    75.0
}
fn final_boss() -> u32 {
    3
}

fn elite_health() -> f32 {
    15.0
}
fn boss_health() -> f32 {
    50.0
}


fn enemy_speed() -> f32 {
    1.35
}
fn enemy_speed_cap() -> f32 {
    6.65
}
fn surround_secs() -> f32 {
    12.0
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            tome_slots: four(),
            tome_max_rank: five_u8(),
            item_max_copies: five_u8(),
            shrine_every: ninety(),
            elite_respawn: one_eighty(),
            enemy_speed: enemy_speed(),
            enemy_speed_cap: enemy_speed_cap(),
            surround_secs: surround_secs(),
            rare_health: rare_health(),
            elite_health: elite_health(),
            boss_health: boss_health(),
            final_boss_health: final_boss_health(),
            final_boss: final_boss(),
        }
    }
}

/// A race's passives, from `races.ron`: what every hero of `race` (a `ChrRaces.dbc` id) gets.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RaceDef {
    pub race: u8,
    pub name: String,
    pub passives: Vec<Racial>,
}

/// One racial passive: its name and line for the menu, the stats it adds, and whether it shows
/// treasure on the minimap.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Racial {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub stats: Vec<(crate::stats::Stat, f32)>,
    #[serde(default)]
    pub treasure: bool,
}

impl RaceDef {
    /// Whether one of its passives shows chests, shrines and pots on the minimap.
    pub fn treasure(&self) -> bool {
        self.passives.iter().any(|p| p.treasure)
    }
}

/// All the game's data, its cross-references checked.
#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub abilities: Vec<AbilityDef>,
    pub enemies: Vec<EnemyDef>,
    pub zones: Vec<ZoneDef>,
    /// [`Rules::default`] until [`Content::with_rules`] reads a file.
    pub rules: Rules,
    /// None until [`Content::with_races`] reads `races.ron`.
    pub races: Vec<RaceDef>,
}

#[derive(Debug)]
pub enum ContentError {
    /// A file is not valid RON for its type.
    Parse { file: &'static str, message: String },
    /// Two entries in one file share a key.
    DuplicateKey { file: &'static str, key: String },
    /// An entry refers to a key that does not exist.
    UnknownKey {
        file: &'static str,
        owner: String,
        key: String,
    },
    /// An entry is present but cannot work (e.g. a zone with no enemies).
    Invalid {
        file: &'static str,
        owner: String,
        why: &'static str,
    },
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Parse { file, message } => write!(f, "{file}: {message}"),
            ContentError::DuplicateKey { file, key } => {
                write!(f, "{file}: key \"{key}\" is used twice")
            }
            ContentError::UnknownKey { file, owner, key } => {
                write!(
                    f,
                    "{file}: \"{owner}\" refers to \"{key}\", which does not exist"
                )
            }
            ContentError::Invalid { file, owner, why } => write!(f, "{file}: \"{owner}\" {why}"),
        }
    }
}

impl std::error::Error for ContentError {}

/// The four RON files' text.
pub struct ContentSources<'a> {
    pub classes: &'a str,
    pub abilities: &'a str,
    pub enemies: &'a str,
    pub zones: &'a str,
}

fn parse<T: for<'de> Deserialize<'de>>(
    file: &'static str,
    text: &str,
) -> Result<Vec<T>, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse {
        file,
        message: e.to_string(),
    })
}

fn index<'a>(
    file: &'static str,
    keys: impl Iterator<Item = &'a str>,
) -> Result<HashMap<String, usize>, ContentError> {
    let mut map = HashMap::new();
    for (i, key) in keys.enumerate() {
        if map.insert(key.to_owned(), i).is_some() {
            return Err(ContentError::DuplicateKey {
                file,
                key: key.to_owned(),
            });
        }
    }
    Ok(map)
}

fn resolve(
    map: &HashMap<String, usize>,
    file: &'static str,
    owner: &str,
    key: &str,
) -> Result<usize, ContentError> {
    map.get(key)
        .copied()
        .ok_or_else(|| ContentError::UnknownKey {
            file,
            owner: owner.to_owned(),
            key: key.to_owned(),
        })
}

impl Content {
    /// Parse the four files and resolve every key.
    pub fn from_ron(src: ContentSources<'_>) -> Result<Self, ContentError> {
        let mut classes: Vec<ClassDef> = parse("classes.ron", src.classes)?;
        let mut abilities: Vec<AbilityDef> = parse("abilities.ron", src.abilities)?;
        for a in &mut abilities {
            a.tag_set = Tags::of(&a.tags);
        }
        let enemies: Vec<EnemyDef> = parse("enemies.ron", src.enemies)?;
        let mut zones: Vec<ZoneDef> = parse("zones.ron", src.zones)?;

        let ability_keys = index("abilities.ron", abilities.iter().map(|a| a.key.as_str()))?;
        let enemy_keys = index("enemies.ron", enemies.iter().map(|e| e.key.as_str()))?;
        index("classes.ron", classes.iter().map(|c| c.key.as_str()))?;
        index("zones.ron", zones.iter().map(|z| z.key.as_str()))?;

        for class in &mut classes {
            class.start_ix = resolve(&ability_keys, "classes.ron", &class.key, &class.start)?;
            class.pool_ix = class
                .pool
                .iter()
                .map(|k| resolve(&ability_keys, "classes.ron", &class.key, k))
                .collect::<Result<_, _>>()?;
            class.defensive_ix = Some(class.defensive.as_str())
                .filter(|k| !k.is_empty())
                .map(|k| resolve(&ability_keys, "classes.ron", &class.key, k))
                .transpose()?;
            if class.looks.is_empty() {
                return Err(ContentError::Invalid {
                    file: "classes.ron",
                    owner: class.key.clone(),
                    why: "has no looks",
                });
            }
        }
        for zone in &mut zones {
            zone.enemy_ix = zone
                .enemies
                .iter()
                .map(|k| resolve(&enemy_keys, "zones.ron", &zone.key, k))
                .collect::<Result<_, _>>()?;
            zone.rare_ix = zone
                .rares
                .iter()
                .map(|k| resolve(&enemy_keys, "zones.ron", &zone.key, k))
                .collect::<Result<_, _>>()?;
            zone.boss_ix = Some(zone.boss.as_str())
                .filter(|k| !k.is_empty())
                .map(|k| resolve(&enemy_keys, "zones.ron", &zone.key, k))
                .transpose()?;
            if zone.enemy_ix.is_empty() {
                return Err(ContentError::Invalid {
                    file: "zones.ron",
                    owner: zone.key.clone(),
                    why: "has no enemies",
                });
            }
        }
        Ok(Self {
            classes,
            abilities,
            enemies,
            zones,
            rules: Rules::default(),
            races: Vec::new(),
        })
    }

    /// These contents with the rules in `text` (`rules.ron`) in place of the defaults.
    pub fn with_rules(mut self, text: &str) -> Result<Self, ContentError> {
        self.rules = ron::from_str(text).map_err(|e| ContentError::Parse {
            file: "rules.ron",
            message: e.to_string(),
        })?;
        Ok(self)
    }

    /// These contents with the racial passives in `text` (`races.ron`).
    pub fn with_races(mut self, text: &str) -> Result<Self, ContentError> {
        self.races = ron::from_str(text).map_err(|e| ContentError::Parse {
            file: "races.ron",
            message: e.to_string(),
        })?;
        Ok(self)
    }

    /// The passives of `race` (a `ChrRaces.dbc` id), if it has any.
    pub fn race(&self, race: u8) -> Option<&RaceDef> {
        self.races.iter().find(|r| r.race == race)
    }

    pub fn ability(&self, key: &str) -> Option<usize> {
        self.abilities.iter().position(|a| a.key == key)
    }

    pub fn class(&self, key: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.key == key)
    }

    pub fn zone(&self, key: &str) -> Option<usize> {
        self.zones.iter().position(|z| z.key == key)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const CLASSES: &str = r#"[
        (key: "mage", name: "Mage", class_id: 8, power: 0, looks: [1294],
         base_hp: 120.0, base_regen: 0.0, speed: 7.5, start: "frostbolt", pool: ["frostbolt", "nova"]),
    ]"#;
    pub(crate) const ABILITIES: &str = r#"[
        (key: "frostbolt", name: "Frostbolt", visual: 116, kind: Bolt, damage: 12.0,
         cooldown: 1.0, range: 24.0, missile_speed: 28.0),
        (key: "nova", name: "Frost Nova", visual: 122, kind: Nova, damage: 8.0,
         cooldown: 5.0, radius: 9.0),
        (key: "static_shard", name: "Static Shard", visual: 421, kind: Strike, damage: 0.0),
        (key: "ember_core", name: "Ember Core", visual: 2120, kind: Nova, damage: 0.0),
        (key: "frost_ward", name: "Frost Ward", visual: 122, kind: Nova, damage: 0.0),
        (key: "detonation", name: "Arcane Detonation", visual: 1449, kind: Nova, damage: 0.0),
    ]"#;
    pub(crate) const ENEMIES: &str = r#"[
        (key: "wolf", name: "Young Wolf", display: 447, level: 1, hp: 10.0, speed: 5.0,
         damage: 2.0, xp: 1),
        (key: "hogger", name: "Hogger", display: 384, level: 11, scale: 1.9, hp: 3000.0,
         speed: 4.2, damage: 40.0, xp: 150),
    ]"#;
    pub(crate) const ZONES: &str = r#"[
        (key: "elwynn", name: "Elwynn Forest", map: 0, center: (-9548.5, 115.0, 60.0),
         leash: 150.0, enemies: ["wolf"], boss: "hogger",
         interactables: [(kind: TitanShrine, count: 2), (kind: Chest, count: 3),
                         (kind: CursedAltar, count: 1)], shrines_at_once: 4),
    ]"#;

    pub(crate) fn sample() -> Content {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: ZONES,
        })
        .expect("sample content is valid")
    }

    #[test]
    fn keys_resolve_to_indices() {
        let c = sample();
        let mage = &c.classes[c.class("mage").unwrap()];
        assert_eq!(c.abilities[mage.start_ix].key, "frostbolt");
        assert_eq!(mage.pool_ix.len(), 2);
        assert_eq!(c.enemies[c.zones[0].enemy_ix[0]].name, "Young Wolf");
        assert_eq!(
            c.zones[0].boss_ix.map(|b| c.enemies[b].name.as_str()),
            Some("Hogger")
        );
    }

    #[test]
    fn defaults_fill_omitted_fields() {
        let c = sample();
        assert_eq!(c.abilities[0].count, 1);
        assert_eq!(c.enemies[0].scale, 1.0);
        assert_eq!(c.classes[0].sheath, 1);
    }

    #[test]
    fn unknown_key_is_reported_with_its_owner() {
        let err = Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: r#"[(key: "z", name: "Z", map: 0, center: (0.0, 0.0, 0.0), leash: 10.0, enemies: ["dragon"])]"#,
        })
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "zones.ron: \"z\" refers to \"dragon\", which does not exist"
        );
    }

    #[test]
    fn duplicate_key_is_reported() {
        let twice = r#"[
            (key: "wolf", name: "A", display: 1, level: 1, hp: 1.0, speed: 1.0, damage: 1.0, xp: 1),
            (key: "wolf", name: "B", display: 1, level: 1, hp: 1.0, speed: 1.0, damage: 1.0, xp: 1),
        ]"#;
        let err = Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: twice,
            zones: ZONES,
        })
        .unwrap_err();
        assert!(matches!(err, ContentError::DuplicateKey { .. }));
    }

    #[test]
    fn rules_left_out_keep_their_defaults() {
        let c = sample().with_rules("(tome_slots: 2)").unwrap();
        assert_eq!(c.rules.tome_slots, 2);
        assert_eq!(c.rules.tome_max_rank, Rules::default().tome_max_rank);
        let err = sample().with_rules("(tome_slots: \"many\")").unwrap_err();
        assert!(err.to_string().starts_with("rules.ron:"));
    }

    #[test]
    fn bad_ron_is_a_parse_error_naming_the_file() {
        let err = Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: "[ (key: ",
            enemies: ENEMIES,
            zones: ZONES,
        })
        .unwrap_err();
        assert!(err.to_string().starts_with("abilities.ron:"));
    }
}
