//! Nightfall's content: its classes, its map and its rules, read from RON (`content/nightfall/`)
//! so they change without a rebuild, and checked as they load so a mistake is a clear message at
//! startup rather than a strange run.

use std::fmt;

use serde::Deserialize;

use crate::{Bounds, V2};

/// One of the class's weapons as WoW's virtual item fields want it.
#[derive(Clone, Debug, Deserialize)]
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
    /// A line or two for the menu.
    #[serde(default)]
    pub description: String,
    /// The class colour (`RAID_CLASS_COLORS`), `0xRRGGBB`.
    #[serde(default = "white")]
    pub color: u32,
    /// `ChrClasses.dbc` id.
    pub class_id: u8,
    /// `UNIT_FIELD_BYTES_0` byte 3: 0 mana, 1 rage, 3 energy.
    pub power: u8,
    /// `ChrRaces.dbc` id and sex (0 male, 1 female) every hero of the class wears for now.
    pub race: u8,
    pub sex: u8,
    #[serde(default)]
    pub weapons: Vec<WeaponDef>,
    /// The sheath state the hero stands in: 1 melee, 2 ranged.
    #[serde(default = "one_u8")]
    pub sheath: u8,
    pub base_hp: f32,
    /// Running speed, yards per second.
    pub speed: f32,
}

fn white() -> u32 {
    0xFF_FF_FF
}

fn one_u8() -> u8 {
    1
}

/// The map a run plays on.
#[derive(Clone, Debug)]
pub struct MapDef {
    pub key: String,
    pub name: String,
    /// `Map.dbc` id.
    pub map: u32,
    pub bounds: Bounds,
    /// Where a party can start, WoW `(x, y, z)`.
    pub entries: Vec<[f32; 3]>,
}

#[derive(Deserialize)]
struct MapFile {
    key: String,
    name: String,
    map: u32,
    bounds: Vec<(f32, f32)>,
    entries: Vec<(f32, f32, f32)>,
}

/// The light a hero carries: a small glow so a player sees their feet.
#[derive(Clone, Debug, Deserialize)]
pub struct HeroLight {
    /// Linear RGB, 0..1.
    pub color: (f32, f32, f32),
    pub intensity: f32,
    /// Yards.
    pub range: f32,
}

/// How dark and how near: the night, the fog and the view distance.
#[derive(Clone, Debug, Deserialize)]
pub struct Atmosphere {
    /// Minute of the day the light stands at (0 is midnight).
    pub night_minute: u32,
    /// Fog from `fog_start` to `fog_end` yards, in `fog_color` (sRGB 0..1).
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_color: (f32, f32, f32),
    /// How far the world is drawn, yards.
    pub view_distance: f32,
    pub hero_light: HeroLight,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rules {
    /// Every hero's level: fixed, there is no levelling in a run.
    pub hero_level: u32,
    pub atmosphere: Atmosphere,
}

/// The text of each content file.
pub struct ContentSources<'a> {
    pub classes: &'a str,
    pub map: &'a str,
    pub rules: &'a str,
}

#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub map: MapDef,
    pub rules: Rules,
}

/// Content that does not load, and why.
#[derive(Debug, PartialEq)]
pub enum ContentError {
    Parse { file: &'static str, error: String },
    Invalid(String),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Parse { file, error } => write!(f, "content/nightfall/{file}: {error}"),
            ContentError::Invalid(why) => write!(f, "content/nightfall: {why}"),
        }
    }
}

impl std::error::Error for ContentError {}

/// A finite number above zero (NaN and infinity are not).
fn positive(x: f32) -> bool {
    x.is_finite() && x > 0.0
}

/// A finite number, zero or more.
fn non_negative(x: f32) -> bool {
    x.is_finite() && x >= 0.0
}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse {
        file,
        error: e.to_string(),
    })
}

impl Content {
    // The float checks are negated comparisons on purpose, so a NaN fails them.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn from_ron(src: ContentSources) -> Result<Self, ContentError> {
        let classes: Vec<ClassDef> = parse("classes.ron", src.classes)?;
        let map: MapFile = parse("map.ron", src.map)?;
        let rules: Rules = parse("rules.ron", src.rules)?;
        let invalid = |why: String| Err(ContentError::Invalid(why));

        if classes.is_empty() {
            return invalid("classes.ron has no class".into());
        }
        for (i, c) in classes.iter().enumerate() {
            if classes[..i].iter().any(|d| d.key == c.key) {
                return invalid(format!("class \"{}\" is listed twice", c.key));
            }
            if !positive(c.speed) {
                return invalid(format!(
                    "class \"{}\": speed {} must be a positive number",
                    c.key, c.speed
                ));
            }
            if !positive(c.base_hp) {
                return invalid(format!(
                    "class \"{}\": base_hp {} must be a positive number",
                    c.key, c.base_hp
                ));
            }
        }
        let bounds = Bounds::new(map.bounds.iter().map(|&(x, y)| V2::new(x, y)).collect())
            .map_err(|why| ContentError::Invalid(format!("map.ron: {why}")))?;
        if map.entries.is_empty() {
            return invalid("map.ron has no entry point".into());
        }
        for &(x, y, _) in &map.entries {
            if !bounds.contains(V2::new(x, y)) {
                return invalid(format!("map.ron: entry ({x}, {y}) is outside the bounds"));
            }
        }
        if rules.hero_level == 0 {
            return invalid("rules.ron: hero_level must be at least 1".into());
        }
        let a = &rules.atmosphere;
        if a.night_minute >= 1440 {
            return invalid(format!(
                "rules.ron: night_minute {} is past 1439",
                a.night_minute
            ));
        }
        // Written as a negated `<` so that a NaN on either side is rejected too.
        if !(a.fog_start < a.fog_end) {
            return invalid("rules.ron: fog_start must be less than fog_end".into());
        }
        // Negated for NaN as above: the world drawn short of the fog's end would show its edge.
        if !(a.view_distance >= a.fog_end) {
            return invalid(format!(
                "rules.ron: view_distance {} is short of fog_end {}",
                a.view_distance, a.fog_end
            ));
        }
        let light = &a.hero_light;
        if !non_negative(light.intensity) || !non_negative(light.range) {
            return invalid(format!(
                "rules.ron: hero_light intensity {} and range {} must be numbers, not negative",
                light.intensity, light.range
            ));
        }
        Ok(Self {
            classes,
            map: MapDef {
                key: map.key,
                name: map.name,
                map: map.map,
                bounds,
                entries: map.entries.iter().map(|&(x, y, z)| [x, y, z]).collect(),
            },
            rules,
        })
    }

    /// The class with this key, by index.
    pub fn class(&self, key: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 100.0, speed: 7.0),
        (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 90.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
        entries: [(50.0, 50.0, 10.0)])"#;
    const RULES: &str = r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
        fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#;

    fn load(classes: &str, map: &str, rules: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes,
            map,
            rules,
        })
    }

    #[test]
    fn good_content_loads() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.class("mage"), Some(1));
        assert_eq!(c.class("warlock"), None);
        assert_eq!(c.map.entries, vec![[50.0, 50.0, 10.0]]);
        assert!(c.map.bounds.contains(V2::new(50.0, 50.0)));
        assert_eq!(c.rules.hero_level, 30);
    }

    #[test]
    fn a_parse_error_names_its_file() {
        match load(CLASSES, "(key: ", RULES) {
            Err(ContentError::Parse { file, .. }) => assert_eq!(file, "map.ron"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_entry_outside_the_bounds_is_refused() {
        let map = MAP.replace("(50.0, 50.0, 10.0)", "(150.0, 50.0, 10.0)");
        assert!(matches!(
            load(CLASSES, &map, RULES),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_map_needs_an_entry() {
        let map = MAP.replace("[(50.0, 50.0, 10.0)]", "[]");
        assert!(matches!(
            load(CLASSES, &map, RULES),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn fog_must_start_before_it_ends() {
        let rules = RULES.replace("fog_start: 10.0", "fog_start: 50.0");
        assert!(matches!(
            load(CLASSES, MAP, &rules),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn the_night_minute_is_a_minute_of_the_day() {
        let rules = RULES.replace("night_minute: 0", "night_minute: 1440");
        assert!(matches!(
            load(CLASSES, MAP, &rules),
            Err(ContentError::Invalid(_))
        ));
    }

    /// `result` is refused, and the reason names `what`.
    fn refused_naming(result: Result<Content, ContentError>, what: &str) {
        match result {
            Err(ContentError::Invalid(why)) => assert!(why.contains(what), "{why}"),
            other => panic!("expected a refusal naming {what}: {other:?}"),
        }
    }

    #[test]
    fn a_class_speed_must_be_a_positive_number() {
        for bad in ["0.0", "-7.0", "NaN", "inf"] {
            let classes = CLASSES.replacen("speed: 7.0", &format!("speed: {bad}"), 1);
            refused_naming(load(&classes, MAP, RULES), "speed");
        }
    }

    #[test]
    fn a_class_base_hp_must_be_a_positive_number() {
        for bad in ["0.0", "-100.0", "NaN", "inf"] {
            let classes = CLASSES.replacen("base_hp: 100.0", &format!("base_hp: {bad}"), 1);
            refused_naming(load(&classes, MAP, RULES), "base_hp");
        }
    }

    #[test]
    fn the_hero_level_is_at_least_one() {
        let rules = RULES.replace("hero_level: 30", "hero_level: 0");
        refused_naming(load(CLASSES, MAP, &rules), "hero_level");
    }

    #[test]
    fn the_world_is_drawn_at_least_to_the_fogs_end() {
        let rules = RULES.replace("view_distance: 120.0", "view_distance: 30.0");
        refused_naming(load(CLASSES, MAP, &rules), "view_distance");
        let rules = RULES.replace("view_distance: 120.0", "view_distance: NaN");
        refused_naming(load(CLASSES, MAP, &rules), "view_distance");
        let rules = RULES.replace("view_distance: 120.0", "view_distance: 40.0");
        assert!(
            load(CLASSES, MAP, &rules).is_ok(),
            "exactly at the fog's end is fine"
        );
    }

    #[test]
    fn the_hero_light_is_finite_and_not_negative() {
        for bad in ["intensity: -0.6", "intensity: NaN", "intensity: inf"] {
            let rules = RULES.replace("intensity: 0.6", bad);
            refused_naming(load(CLASSES, MAP, &rules), "hero_light");
        }
        for bad in ["range: -9.0", "range: NaN"] {
            let rules = RULES.replace("range: 9.0", bad);
            refused_naming(load(CLASSES, MAP, &rules), "hero_light");
        }
    }

    #[test]
    fn class_keys_are_unique_and_present() {
        let twice = CLASSES.replace("\"mage\", name", "\"rogue\", name");
        assert!(matches!(
            load(&twice, MAP, RULES),
            Err(ContentError::Invalid(_))
        ));
        assert!(matches!(
            load("[]", MAP, RULES),
            Err(ContentError::Invalid(_))
        ));
    }
}
