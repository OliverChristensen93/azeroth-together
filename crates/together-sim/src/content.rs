//! The game's data: classes, abilities, enemies and zones, read from RON files (`content/` at the
//! repository root) so they can be added to and tuned without recompiling.
//!
//! Everything refers to everything else by `key`; [`Content::from_ron`] resolves those keys to
//! indices once and fails with a [`ContentError`] naming the first one that does not exist.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

use crate::interact::Placement;

/// How an ability picks its targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AbilityKind {
    /// A missile at the nearest `count` enemies in `range`; lands after `missile_speed` travel, and
    /// splashes everything within `radius` of the target when `radius` is above 0.
    Bolt,
    /// An instant hit on the nearest `count` enemies in `range`.
    Strike,
    /// Everything within `radius` of the caster, at once.
    Nova,
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
    /// The shrines, altars and chests scattered about the arena each run; none when left out.
    #[serde(default)]
    pub interactables: Vec<Placement>,
    /// [`ZoneDef::enemies`] resolved to indices into [`Content::enemies`].
    #[serde(skip)]
    pub enemy_ix: Vec<usize>,
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
    /// Yards per second.
    pub speed: f32,
    /// The ability every run starts with.
    pub start: String,
    /// The abilities level-up cards offer.
    pub pool: Vec<String>,
    #[serde(skip)]
    pub start_ix: usize,
    #[serde(skip)]
    pub pool_ix: Vec<usize>,
}

fn one_u8() -> u8 {
    1
}

fn white() -> u32 {
    0xFF_FF_FF
}

/// All the game's data, its cross-references checked.
#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub abilities: Vec<AbilityDef>,
    pub enemies: Vec<EnemyDef>,
    pub zones: Vec<ZoneDef>,
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
        let abilities: Vec<AbilityDef> = parse("abilities.ron", src.abilities)?;
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
        })
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
         base_hp: 120.0, speed: 7.5, start: "frostbolt", pool: ["frostbolt", "nova"]),
    ]"#;
    pub(crate) const ABILITIES: &str = r#"[
        (key: "frostbolt", name: "Frostbolt", visual: 116, kind: Bolt, damage: 12.0,
         cooldown: 1.0, range: 24.0, missile_speed: 28.0),
        (key: "nova", name: "Frost Nova", visual: 122, kind: Nova, damage: 8.0,
         cooldown: 5.0, radius: 9.0),
    ]"#;
    pub(crate) const ENEMIES: &str = r#"[
        (key: "wolf", name: "Young Wolf", display: 447, level: 1, hp: 10.0, speed: 5.0,
         damage: 2.0, xp: 1),
    ]"#;
    pub(crate) const ZONES: &str = r#"[
        (key: "elwynn", name: "Elwynn Forest", map: 0, center: (-9548.5, 115.0, 60.0),
         leash: 150.0, enemies: ["wolf"],
         interactables: [(kind: TitanShrine, count: 2), (kind: Chest, count: 3),
                         (kind: CursedAltar, count: 1)]),
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
