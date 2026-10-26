//! The game's data: classes, abilities, enemies and zones, read from RON files
//! (`content/arpg/` at the repository root) so they can be tuned without recompiling.
//!
//! Everything refers to everything else by `key`; [`Content::from_ron`] resolves those keys to
//! indices once and fails with a [`ContentError`] naming the first one that does not exist.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A spell's school: what its numbers are coloured and what talents will later boost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum School {
    Physical,
    Frost,
    Fire,
    Arcane,
    Nature,
    Shadow,
    Holy,
}

/// Where an ability goes. Every shape is aimed: it takes the caster and the ground point under
/// the cursor, never a target.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Shape {
    /// A missile that flies from the caster toward the aim and hits the first enemy its path
    /// touches (`pierce` more after that), or flies out at `range`. `count` missiles fan out over
    /// `spread` degrees; `splash` hits everything that near where one strikes.
    Projectile {
        speed: f32,
        #[serde(default = "half")]
        radius: f32,
        #[serde(default)]
        pierce: u32,
        #[serde(default = "one")]
        count: u32,
        #[serde(default)]
        spread: f32,
        #[serde(default)]
        splash: f32,
    },
    /// An area at the aim (pulled in to `range`): `damage` lands `delay` seconds after the cast on
    /// everything within `radius`, then, for `duration` seconds, `tick_damage` every `tick`.
    Ground {
        radius: f32,
        #[serde(default)]
        delay: f32,
        #[serde(default)]
        duration: f32,
        #[serde(default = "one_f")]
        tick: f32,
        #[serde(default)]
        tick_damage: f32,
    },
    /// Everything within `range` of the caster and `angle` degrees wide, pointed at the aim.
    Cone { angle: f32 },
    /// Everything within `radius` of the caster.
    Nova { radius: f32 },
    /// The caster itself: `immune` makes it untouchable (and unable to act) for the duration,
    /// `absorb` soaks that much damage.
    Guard {
        duration: f32,
        #[serde(default)]
        immune: bool,
        #[serde(default)]
        absorb: f32,
    },
    /// The caster moves toward the aim, up to `range`, at once.
    Blink,
}

/// What a hit leaves on an enemy, besides its damage.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Effects {
    /// Seconds of Chill: [`CHILL_SLOW`](crate::CHILL_SLOW) slower.
    pub chill: f32,
    /// Seconds of Freeze: no moving, no attacking; every hit on it is a critical strike (Shatter).
    pub freeze: f32,
    /// Fire damage over `burn_secs` seconds, on top of the hit.
    pub burn: f32,
    pub burn_secs: f32,
    /// Yards an enemy is pushed away from the caster.
    pub knockback: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AbilityDef {
    pub key: String,
    pub name: String,
    /// The Spell.dbc id whose cast, missile and impact visuals (and icon) the client shows.
    pub visual: u32,
    pub school: School,
    pub shape: Shape,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub cooldown: f32,
    #[serde(default)]
    pub mana: f32,
    /// Seconds before it goes off; 0 is instant.
    #[serde(default)]
    pub cast_time: f32,
    /// A channel: the shape goes off every `channel_tick` seconds for `channel` seconds, at the
    /// aim as it is then (a Ground shape is one area that lasts as long as the channel). Moving
    /// or casting something else stops it.
    #[serde(default)]
    pub channel: f32,
    #[serde(default = "one_f")]
    pub channel_tick: f32,
    /// The caster keeps walking while it casts (instants always may).
    #[serde(default)]
    pub move_while_casting: bool,
    /// It starts the global cooldown and waits for it, as most spells do; escapes (Blink, Ice
    /// Block) are off it so they always answer.
    #[serde(default = "yes")]
    pub gcd: bool,
    /// How far from the caster it reaches: a missile's flight, a ground spell's farthest aim, a
    /// cone's length, a blink's jump.
    #[serde(default = "default_range")]
    pub range: f32,
    #[serde(default)]
    pub effects: Effects,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub key: String,
    pub name: String,
    /// The ability on each slot, in slot order (the client binds the keys).
    pub abilities: Vec<String>,
    pub hp: f32,
    pub mana: f32,
    /// Mana back every second.
    pub mana_regen: f32,
    /// Yards a second.
    pub speed: f32,
    /// The share of hits that are critical strikes (Shatter makes every hit on a frozen one so).
    #[serde(default = "default_crit")]
    pub crit: f32,
    /// Seconds of global cooldown a spell on it starts (its cast time counts toward it).
    #[serde(default = "default_gcd")]
    pub gcd: f32,
    /// The dodge roll every hero has: yards covered, seconds it takes, and seconds after it before
    /// the next. It gets the hero out of the way; it does not make them untouchable.
    #[serde(default = "default_dodge")]
    pub dodge: Dodge,
    /// Stamina, which the roll spends: the most there is, and how much comes back a second once
    /// `stamina_delay` seconds have passed since the last roll.
    #[serde(default = "default_stamina")]
    pub stamina: f32,
    #[serde(default = "default_stamina_regen")]
    pub stamina_regen: f32,
    #[serde(default = "default_stamina_delay")]
    pub stamina_delay: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Dodge {
    pub distance: f32,
    pub time: f32,
    pub recover: f32,
    /// Stamina it spends.
    #[serde(default = "default_dodge_cost")]
    pub cost: f32,
}

/// How an enemy fights. Every attack is telegraphed: a windup the client draws, then the blow
/// lands on whatever is still where it was aimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behaviour {
    /// Walks up and swings in an arc ahead of it.
    Melee,
    /// Keeps its distance and throws a missile at where the hero stood.
    Ranged,
    /// Walks up and slams the ground around itself.
    Slam,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDef {
    pub key: String,
    pub name: String,
    /// The CreatureDisplayInfo id the client draws.
    pub display: u32,
    #[serde(default = "one_f")]
    pub scale: f32,
    #[serde(default = "one_u8")]
    pub level: u8,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub behaviour: Behaviour,
    /// How close it must get to attack (a Ranged enemy's throw).
    pub reach: f32,
    /// Seconds of telegraph before the blow lands.
    pub windup: f32,
    /// Seconds it stands after a blow.
    pub recover: f32,
    #[serde(default = "default_radius")]
    pub radius: f32,
    /// A Ranged enemy's missile: its speed and Spell.dbc visual.
    #[serde(default)]
    pub missile_speed: f32,
    #[serde(default)]
    pub missile_visual: u32,
    /// The missile's school.
    #[serde(default = "default_missile_school")]
    pub missile_school: School,
    /// Bosses and the like: frozen and stunned for half as long.
    #[serde(default)]
    pub tough: bool,
    /// How near a hero must come before it notices them, in yards.
    #[serde(default = "default_aggro")]
    pub aggro: f32,
    /// How far from home it chases before it gives up and runs back, whole again.
    #[serde(default = "default_leash")]
    pub leash: f32,
}

/// A camp: enemies of one kind standing around a spot, as WoW places them, each coming back
/// `respawn` seconds after it dies.
#[derive(Clone, Debug, Deserialize)]
pub struct CampDef {
    pub enemy: String,
    /// The camp's middle, in WoW coordinates (x, y).
    pub at: (f32, f32),
    #[serde(default = "one")]
    pub count: u32,
    /// How far from the middle they stand, at most.
    #[serde(default = "default_spread")]
    pub spread: f32,
    #[serde(default = "default_respawn")]
    pub respawn: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneDef {
    pub key: String,
    pub name: String,
    /// The Map.dbc id (0 Eastern Kingdoms, 1 Kalimdor).
    pub map: u32,
    /// Where the heroes start, and where they wake after falling, in WoW coordinates (x, y, z).
    pub start: (f32, f32, f32),
    pub camps: Vec<CampDef>,
}

/// The game's data, every key resolved.
#[derive(Clone, Debug)]
pub struct Content {
    pub abilities: Vec<AbilityDef>,
    pub classes: Vec<ClassDef>,
    pub enemies: Vec<EnemyDef>,
    pub zones: Vec<ZoneDef>,
    /// Each class's slots as ability indices.
    pub(crate) class_slots: Vec<Vec<usize>>,
    /// Each zone's camps' enemies, as enemy indices.
    pub(crate) zone_camps: Vec<Vec<usize>>,
}

#[derive(Debug)]
pub enum ContentError {
    Io(String, String),
    Parse(&'static str, String),
    UnknownAbility { class: String, ability: String },
    UnknownEnemy { zone: String, enemy: String },
    Duplicate(&'static str, String),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Io(path, e) => write!(f, "cannot read {path}: {e}"),
            ContentError::Parse(file, e) => write!(f, "{file}: {e}"),
            ContentError::UnknownAbility { class, ability } => {
                write!(f, "class `{class}` names unknown ability `{ability}`")
            }
            ContentError::UnknownEnemy { zone, enemy } => {
                write!(f, "zone `{zone}` names unknown enemy `{enemy}`")
            }
            ContentError::Duplicate(what, key) => write!(f, "two {what} share the key `{key}`"),
        }
    }
}

impl std::error::Error for ContentError {}

impl Content {
    /// The four files' text: abilities, classes, enemies, zones.
    pub fn from_ron(
        abilities: &str,
        classes: &str,
        enemies: &str,
        zones: &str,
    ) -> Result<Content, ContentError> {
        let abilities: Vec<AbilityDef> = parse("abilities.ron", abilities)?;
        let classes: Vec<ClassDef> = parse("classes.ron", classes)?;
        let enemies: Vec<EnemyDef> = parse("enemies.ron", enemies)?;
        let zones: Vec<ZoneDef> = parse("zones.ron", zones)?;

        let ability_ix = index("abilities", abilities.iter().map(|a| &a.key))?;
        let enemy_ix = index("enemies", enemies.iter().map(|e| &e.key))?;
        index("classes", classes.iter().map(|c| &c.key))?;
        index("zones", zones.iter().map(|z| &z.key))?;

        let class_slots = classes
            .iter()
            .map(|c| {
                c.abilities
                    .iter()
                    .map(|a| {
                        ability_ix
                            .get(a.as_str())
                            .copied()
                            .ok_or_else(|| ContentError::UnknownAbility {
                                class: c.key.clone(),
                                ability: a.clone(),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let zone_camps = zones
            .iter()
            .map(|z| {
                z.camps
                    .iter()
                    .map(|s| {
                        enemy_ix
                            .get(s.enemy.as_str())
                            .copied()
                            .ok_or_else(|| ContentError::UnknownEnemy {
                                zone: z.key.clone(),
                                enemy: s.enemy.clone(),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Content {
            abilities,
            classes,
            enemies,
            zones,
            class_slots,
            zone_camps,
        })
    }

    /// Reads `abilities.ron`, `classes.ron`, `enemies.ron` and `zones.ron` from `dir`.
    pub fn from_dir(dir: &Path) -> Result<Content, ContentError> {
        let read = |name: &str| {
            let path = dir.join(name);
            std::fs::read_to_string(&path)
                .map_err(|e| ContentError::Io(path.display().to_string(), e.to_string()))
        };
        Content::from_ron(
            &read("abilities.ron")?,
            &read("classes.ron")?,
            &read("enemies.ron")?,
            &read("zones.ron")?,
        )
    }

    pub fn ability(&self, key: &str) -> Option<usize> {
        self.abilities.iter().position(|a| a.key == key)
    }

    pub fn class(&self, key: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.key == key)
    }

    pub fn enemy(&self, key: &str) -> Option<usize> {
        self.enemies.iter().position(|e| e.key == key)
    }

    pub fn zone(&self, key: &str) -> Option<usize> {
        self.zones.iter().position(|z| z.key == key)
    }

    /// A class's slots, as ability indices.
    pub fn slots(&self, class: usize) -> &[usize] {
        &self.class_slots[class]
    }
}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse(file, e.to_string()))
}

fn index<'a>(
    what: &'static str,
    keys: impl Iterator<Item = &'a String>,
) -> Result<HashMap<&'a str, usize>, ContentError> {
    let mut map = HashMap::new();
    for (i, k) in keys.enumerate() {
        if map.insert(k.as_str(), i).is_some() {
            return Err(ContentError::Duplicate(what, k.clone()));
        }
    }
    Ok(map)
}

fn one() -> u32 {
    1
}
fn one_u8() -> u8 {
    1
}
fn one_f() -> f32 {
    1.0
}
fn half() -> f32 {
    0.5
}
fn default_range() -> f32 {
    30.0
}
fn default_crit() -> f32 {
    0.05
}
fn default_radius() -> f32 {
    0.7
}
fn default_dodge() -> Dodge {
    Dodge {
        distance: 5.5,
        time: 0.35,
        recover: 0.3,
        cost: default_dodge_cost(),
    }
}
fn default_dodge_cost() -> f32 {
    40.0
}
fn default_gcd() -> f32 {
    0.9
}
fn default_stamina() -> f32 {
    100.0
}
fn default_stamina_regen() -> f32 {
    25.0
}
fn default_stamina_delay() -> f32 {
    0.8
}
fn yes() -> bool {
    true
}
fn default_missile_school() -> School {
    School::Fire
}
fn default_aggro() -> f32 {
    9.0
}
fn default_leash() -> f32 {
    35.0
}
fn default_spread() -> f32 {
    5.0
}
fn default_respawn() -> f32 {
    60.0
}
