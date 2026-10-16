//! Nightfall's content: its classes, its map, its enemies and where they go, and its rules, read
//! from RON (`content/nightfall/`) so they change without a rebuild, and checked as they load so a mistake is a clear message at
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
    /// The class can stealth (spec §6.3's Rogue toggle, the stealthed rows of §5.1).
    #[serde(default)]
    pub stealth: bool,
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
    /// Its lit places (spec §5.1).
    pub lit: Vec<LitArea>,
}

/// A lit place: a hero standing in it is easier to see (spec §5.1: campfires, Darkshire's lamps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitArea {
    pub at: V2,
    /// Yards.
    pub radius: f32,
}

#[derive(Deserialize)]
struct MapFile {
    key: String,
    name: String,
    map: u32,
    bounds: Vec<(f32, f32)>,
    entries: Vec<(f32, f32, f32)>,
    #[serde(default)]
    lit: Vec<(f32, f32, f32)>,
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
    /// The world's knowledge (spec §4); its defaults when `rules.ron` leaves it out.
    #[serde(default)]
    pub world: WorldRules,
    /// The senses (spec §5); the spec's values when `rules.ron` leaves any out.
    #[serde(default)]
    pub senses: SenseRules,
}

/// How the host uses what it knows of the world.
#[derive(Clone, Debug, Deserialize)]
pub struct WorldRules {
    /// A teammate this near (yards) shares what they see (spec §4.3).
    pub share_sight_within: f32,
    /// The most path searches the host makes a step, so many enemies choosing a way at once
    /// never stall it.
    pub paths_per_step: u32,
    /// How long a wanderer rests between walks, seconds (shortest, longest).
    pub wander_pause: (f32, f32),
}

impl Default for WorldRules {
    fn default() -> Self {
        Self {
            share_sight_within: 10.0,
            paths_per_step: 4,
            wander_pause: (3.0, 8.0),
        }
    }
}

/// The senses (spec §5): how visible and how loud heroes are and how enemies grow aware of them.
/// Every number is a starting value to playtest; each left out takes the spec's.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SenseRules {
    /// Visibility 0..100 by stance (spec §5.1).
    pub visibility: StanceTable,
    /// Visibility multipliers: a lit lantern, standing in a lit area, a fire spell's flash.
    pub lantern: f32,
    pub lit: f32,
    pub flash: f32,
    /// How long a flash lasts, seconds.
    pub flash_for: f32,
    /// How far an enemy notices a hero of visibility 100 straight ahead, yards.
    pub notice_range: f32,
    /// The front cone's width, degrees: every enemy's, and a sentry's wider one (spec §7.1).
    pub cone: f32,
    pub sentry_cone: f32,
    /// The rear cone's width, degrees; between the two lie the sides.
    pub behind_cone: f32,
    /// The notice range's share at the sides and behind (the front is 1).
    pub side: f32,
    pub behind: f32,
    /// Footsteps (spec §5.2).
    pub steps: StepNoise,
    /// Every other noise (spec §5.2).
    pub loudness: LoudnessTable,
    /// A listener without line of sight to a noise hears it within this share of its radius.
    pub walls: f32,
    /// The rings by the player frame show the loudest noise of the last this-many seconds.
    pub noise_memory: f32,
    pub awareness: AwarenessRules,
    /// An alerted enemy closes to this many yards of its hero.
    pub chase_reach: f32,
    /// The first hit on an enemy not alerted to its attacker is multiplied by this (spec §5.4).
    pub ambush: f32,
    /// A stealthed hero's speed, as a share of their own (spec §6.3, §11).
    pub stealth_speed: f32,
    /// The heartbeat plays while an enemy alerted to you is this near, yards (spec §10).
    pub heartbeat_within: f32,
    /// The night grade's vignette (strength, inner edge from the centre, both 0..1) at visibility
    /// 100 and at 0: it tightens as the hero hides (spec §10).
    pub vignette_seen: (f32, f32),
    pub vignette_hidden: (f32, f32),
}

impl Default for SenseRules {
    fn default() -> Self {
        Self {
            visibility: StanceTable::default(),
            lantern: 1.6,
            lit: 1.5,
            flash: 2.0,
            flash_for: 1.0,
            notice_range: 35.0,
            cone: 120.0,
            sentry_cone: 180.0,
            behind_cone: 120.0,
            side: 0.6,
            behind: 0.3,
            steps: StepNoise::default(),
            loudness: LoudnessTable::default(),
            walls: 0.5,
            noise_memory: 1.0,
            awareness: AwarenessRules::default(),
            chase_reach: 2.0,
            ambush: 1.5,
            stealth_speed: 0.7,
            heartbeat_within: 20.0,
            vignette_seen: (0.45, 0.35),
            vignette_hidden: (0.85, 0.1),
        }
    }
}

impl SenseRules {
    /// What is wrong with these numbers, if anything: each check is one the senses rely on.
    fn problem(&self) -> Option<&'static str> {
        let v = &self.visibility;
        let a = &self.awareness;
        let l = &self.loudness;
        let s = &self.steps;
        let in01 = |x: f32| (0.0..=1.0).contains(&x);
        // Range checks like `x >= 0.0` let infinity through, and a NaN fails them only by luck, so
        // every number is first required to be finite.
        let every_number = [
            v.run,
            v.walk,
            v.still,
            v.sneak,
            v.sneak_still,
            self.lantern,
            self.lit,
            self.flash,
            self.flash_for,
            self.notice_range,
            self.cone,
            self.sentry_cone,
            self.behind_cone,
            self.side,
            self.behind,
            s.run,
            s.walk,
            s.sneak,
            s.every,
            l.quiet,
            l.normal,
            l.loud,
            l.hit,
            l.open,
            l.dig,
            l.ravens,
            l.extract,
            self.walls,
            self.noise_memory,
            a.fill,
            a.fill_min,
            a.noise,
            a.drain,
            a.suspicious,
            a.check,
            a.alert,
            a.shout,
            a.lose_after,
            a.search_for,
            self.chase_reach,
            self.ambush,
            self.stealth_speed,
            self.heartbeat_within,
            self.vignette_seen.0,
            self.vignette_seen.1,
            self.vignette_hidden.0,
            self.vignette_hidden.1,
        ];
        let checks: [(bool, &'static str); 13] = [
            (
                every_number.into_iter().all(f32::is_finite),
                "every number must be finite",
            ),
            (
                [v.run, v.walk, v.still, v.sneak, v.sneak_still]
                    .into_iter()
                    .all(|x| (0.0..=100.0).contains(&x)),
                "visibility values must be 0 to 100",
            ),
            (
                [self.lantern, self.lit, self.flash]
                    .into_iter()
                    .all(|x| x >= 1.0)
                    && self.flash_for >= 0.0,
                "lantern, lit and flash must be 1 or more, flash_for 0 or more",
            ),
            (self.notice_range > 0.0, "notice_range must be above 0"),
            (
                self.cone > 0.0
                    && self.sentry_cone > 0.0
                    && self.behind_cone >= 0.0
                    && self.cone.max(self.sentry_cone) + self.behind_cone <= 360.0,
                "cone, sentry_cone and behind_cone must leave room for each other in 360 degrees",
            ),
            (
                in01(self.side) && in01(self.behind),
                "side and behind must be 0 to 1",
            ),
            (
                [s.run, s.walk, s.sneak].into_iter().all(|x| x >= 0.0) && s.every > 0.0,
                "steps need radii of 0 or more and `every` above 0",
            ),
            (
                [
                    l.quiet, l.normal, l.loud, l.hit, l.open, l.dig, l.ravens, l.extract,
                ]
                .into_iter()
                .all(|x| x >= 0.0),
                "loudness radii must be 0 or more",
            ),
            (
                in01(self.walls) && self.noise_memory > 0.0,
                "walls must be 0 to 1 and noise_memory above 0",
            ),
            (
                0.0 < a.suspicious && a.suspicious < a.check && a.check < a.alert,
                "awareness needs 0 < suspicious < check < alert",
            ),
            (
                a.fill >= a.fill_min
                    && a.fill_min >= 0.0
                    && a.noise >= 0.0
                    && a.drain >= 0.0
                    && a.shout >= 0.0
                    && a.lose_after >= 0.0
                    && a.search_for >= 0.0,
                "awareness rates, reaches and times must be 0 or more, fill at least fill_min",
            ),
            (
                self.chase_reach > 0.0
                    && self.ambush >= 1.0
                    && self.stealth_speed > 0.0
                    && self.stealth_speed <= 1.0
                    && self.heartbeat_within >= 0.0,
                "chase_reach above 0, ambush 1 or more, stealth_speed above 0 up to 1, heartbeat_within 0 or more",
            ),
            (
                [
                    self.vignette_seen.0,
                    self.vignette_seen.1,
                    self.vignette_hidden.0,
                    self.vignette_hidden.1,
                ]
                .into_iter()
                .all(in01),
                "vignette values must be 0 to 1",
            ),
        ];
        checks.into_iter().find(|(ok, _)| !ok).map(|(_, why)| why)
    }
}

/// Visibility 0..100 for each stance (spec §5.1's table).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StanceTable {
    pub run: f32,
    pub walk: f32,
    pub still: f32,
    pub sneak: f32,
    pub sneak_still: f32,
}

impl Default for StanceTable {
    fn default() -> Self {
        Self {
            run: 100.0,
            walk: 55.0,
            still: 35.0,
            sneak: 20.0,
            sneak_still: 8.0,
        }
    }
}

/// How far a footstep carries by stance, yards, and how often a moving hero steps, seconds
/// (spec §5.2).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StepNoise {
    pub run: f32,
    pub walk: f32,
    pub sneak: f32,
    pub every: f32,
}

impl Default for StepNoise {
    fn default() -> Self {
        Self {
            run: 12.0,
            walk: 4.0,
            sneak: 0.0,
            every: 0.5,
        }
    }
}

/// How far other noises carry, yards (spec §5.2): abilities by loudness, a hit landing or a unit
/// dying, opening a chest or searching a body, digging a grave, ravens taking off, a second of
/// extracting.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LoudnessTable {
    pub quiet: f32,
    pub normal: f32,
    pub loud: f32,
    pub hit: f32,
    pub open: f32,
    pub dig: f32,
    pub ravens: f32,
    pub extract: f32,
}

impl Default for LoudnessTable {
    fn default() -> Self {
        Self {
            quiet: 8.0,
            normal: 20.0,
            loud: 40.0,
            hit: 15.0,
            open: 15.0,
            dig: 30.0,
            ravens: 40.0,
            extract: 25.0,
        }
    }
}

/// How an enemy's suspicion of a hero rises and falls, and what it does at each level (spec §5.3).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AwarenessRules {
    /// Seeing a hero fills suspicion by `fill × (1 − distance / notice range)` a second, at least
    /// `fill_min`.
    pub fill: f32,
    pub fill_min: f32,
    /// A noise heard adds this much.
    pub noise: f32,
    /// Neither seen nor heard, it drains this much a second.
    pub drain: f32,
    /// From `suspicious` an enemy turns to look ("?"), from `check` it goes to look, at `alert` it
    /// is alerted ("!").
    pub suspicious: f32,
    pub check: f32,
    pub alert: f32,
    /// An alerted enemy shouts to every ally this near, yards.
    pub shout: f32,
    /// Seconds out of sight before an alerted enemy searches, and how long it looks around there.
    pub lose_after: f32,
    pub search_for: f32,
}

impl Default for AwarenessRules {
    fn default() -> Self {
        Self {
            fill: 60.0,
            fill_min: 10.0,
            noise: 35.0,
            drain: 10.0,
            suspicious: 40.0,
            check: 60.0,
            alert: 100.0,
            shout: 15.0,
            lose_after: 5.0,
            search_for: 15.0,
        }
    }
}

/// A kind of enemy: one of Duskwood's creatures (spec §7.1).
#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDef {
    pub key: String,
    pub name: String,
    /// `CreatureDisplayInfo.dbc` id: the model.
    pub display: u32,
    #[serde(default = "one_f32")]
    pub scale: f32,
    pub level: u32,
    pub hp: f32,
    /// Walking (patrols, wanderers), yards a second.
    pub walk_speed: f32,
    /// Running, for the chases of later sub-projects, yards a second.
    pub run_speed: f32,
}

fn one_f32() -> f32 {
    1.0
}

fn one_u32() -> u32 {
    1
}

fn two_f32() -> f32 {
    2.0
}

/// What a group of enemies does (spec §7.1). Points are WoW `(x, y)` yards.
#[derive(Clone, Debug, Deserialize)]
pub enum Behaviour {
    /// Stands at a post, facing `facing` degrees (0 north, 90 west).
    Sentry { at: (f32, f32), facing: f32 },
    /// Lies at a post facing `facing`: hears but does not see until alerted (spec §7.1).
    Sleeper { at: (f32, f32), facing: f32 },
    /// Walks its route point by point and back to the first, resting `pause` seconds at each.
    Patrol {
        route: Vec<(f32, f32)>,
        #[serde(default = "two_f32")]
        pause: f32,
    },
    /// Roams within `radius` yards of `center`, resting between walks.
    Wander { center: (f32, f32), radius: f32 },
}

/// A group of enemies at a place.
#[derive(Clone, Debug, Deserialize)]
pub struct SpawnDef {
    /// The point of interest, for the content's author and the host's warnings.
    pub place: String,
    /// An [`EnemyDef::key`].
    pub kind: String,
    #[serde(default = "one_u32")]
    pub count: u32,
    /// The height to look for a floor near (a house's upper storey, a crypt); without it, the
    /// floor nearest the terrain.
    #[serde(default)]
    pub z: Option<f32>,
    pub behaviour: Behaviour,
}

/// The most enemies one spawn line places.
const MAX_SPAWN_COUNT: u32 = 20;

/// The text of each content file.
pub struct ContentSources<'a> {
    pub classes: &'a str,
    pub map: &'a str,
    pub rules: &'a str,
    pub enemies: &'a str,
    pub spawns: &'a str,
}

#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub map: MapDef,
    pub rules: Rules,
    pub enemies: Vec<EnemyDef>,
    pub spawns: Vec<SpawnDef>,
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
        let enemies: Vec<EnemyDef> = parse("enemies.ron", src.enemies)?;
        let spawns: Vec<SpawnDef> = parse("spawns.ron", src.spawns)?;
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
        if map.lit.iter().any(|&(_, _, r)| !positive(r)) {
            return invalid("map.ron: a lit area needs a radius above 0".into());
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
        for (i, e) in enemies.iter().enumerate() {
            if enemies[..i].iter().any(|d| d.key == e.key) {
                return invalid(format!("enemy \"{}\" is listed twice", e.key));
            }
            if !(positive(e.hp) && positive(e.walk_speed) && positive(e.scale)) {
                return invalid(format!(
                    "enemies.ron: \"{}\" needs hp, walk_speed and scale above 0 (and finite)",
                    e.key
                ));
            }
            // Negated for NaN as above; an infinite run speed would cross the map in one step.
            if !(e.run_speed.is_finite() && e.run_speed >= e.walk_speed) {
                return invalid(format!(
                    "enemies.ron: \"{}\" needs a finite run_speed of at least walk_speed",
                    e.key
                ));
            }
            if e.level == 0 {
                return invalid(format!("enemies.ron: \"{}\" needs level 1 or more", e.key));
            }
        }
        let inside = |(x, y): (f32, f32)| bounds.contains(V2::new(x, y));
        for s in &spawns {
            let who = format!("spawns.ron: \"{}\"", s.place);
            if !enemies.iter().any(|e| e.key == s.kind) {
                return invalid(format!("{who}: no enemy \"{}\" in enemies.ron", s.kind));
            }
            if s.count == 0 || s.count > MAX_SPAWN_COUNT {
                return invalid(format!("{who}: count must be 1 to {MAX_SPAWN_COUNT}"));
            }
            if s.z.is_some_and(|z| !z.is_finite()) {
                return invalid(format!("{who}: z must be a finite number"));
            }
            match &s.behaviour {
                Behaviour::Sentry { at, facing } | Behaviour::Sleeper { at, facing } => {
                    if !inside(*at) {
                        return invalid(format!("{who}: its post {at:?} is outside the bounds"));
                    }
                    if !facing.is_finite() {
                        return invalid(format!("{who}: facing must be a finite number"));
                    }
                }
                Behaviour::Patrol { route, pause } => {
                    if route.len() < 2 {
                        return invalid(format!("{who}: a patrol needs 2 points or more"));
                    }
                    if let Some(p) = route.iter().find(|p| !inside(**p)) {
                        return invalid(format!("{who}: route point {p:?} is outside the bounds"));
                    }
                    if !non_negative(*pause) {
                        return invalid(format!("{who}: pause must be 0 or more"));
                    }
                }
                Behaviour::Wander { center, radius } => {
                    if !inside(*center) {
                        return invalid(format!(
                            "{who}: its centre {center:?} is outside the bounds"
                        ));
                    }
                    if !positive(*radius) {
                        return invalid(format!("{who}: radius must be above 0"));
                    }
                }
            }
        }
        let w = &rules.world;
        // Negated for NaN as above.
        if !non_negative(w.share_sight_within)
            || w.paths_per_step == 0
            || !(non_negative(w.wander_pause.0)
                && w.wander_pause.1.is_finite()
                && w.wander_pause.0 <= w.wander_pause.1)
        {
            return invalid(
                "rules.ron: world needs share_sight_within of 0 or more, paths_per_step of 1 or more and wander_pause (shortest, longest) from 0 up"
                    .into(),
            );
        }
        if let Some(why) = rules.senses.problem() {
            return invalid(format!("rules.ron: senses: {why}"));
        }
        Ok(Self {
            classes,
            map: MapDef {
                key: map.key,
                name: map.name,
                map: map.map,
                bounds,
                entries: map.entries.iter().map(|&(x, y, z)| [x, y, z]).collect(),
                lit: map
                    .lit
                    .iter()
                    .map(|&(x, y, radius)| LitArea {
                        at: V2::new(x, y),
                        radius,
                    })
                    .collect(),
            },
            rules,
            enemies,
            spawns,
        })
    }

    /// The enemy kind with this key, by index.
    pub fn enemy(&self, key: &str) -> Option<usize> {
        self.enemies.iter().position(|e| e.key == key)
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

    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
        (key: "spider", name: "Spider", display: 955, scale: 0.9, level: 26, hp: 800.0, walk_speed: 2.2, run_speed: 7.0),
    ]"#;
    const SPAWNS: &str = r#"[
        (place: "Gate", kind: "skeleton", behaviour: Sentry(at: (60.0, 50.0), facing: 90.0)),
        (place: "Road", kind: "skeleton", behaviour: Patrol(route: [(20.0, 20.0), (80.0, 20.0)])),
        (place: "Nest", kind: "spider", count: 3, z: Some(1.0), behaviour: Wander(center: (30.0, 70.0), radius: 10.0)),
    ]"#;

    fn load(classes: &str, map: &str, rules: &str) -> Result<Content, ContentError> {
        load_all(classes, map, rules, ENEMIES, SPAWNS)
    }

    fn load_all(
        classes: &str,
        map: &str,
        rules: &str,
        enemies: &str,
        spawns: &str,
    ) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes,
            map,
            rules,
            enemies,
            spawns,
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
    fn enemies_and_their_places_load() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.enemy("spider"), Some(1));
        assert_eq!(c.enemy("worgen"), None);
        assert_eq!(c.enemies[0].scale, 1.0);
        assert_eq!(c.spawns.len(), 3);
        assert_eq!(c.spawns[0].count, 1);
        assert_eq!(c.spawns[2].z, Some(1.0));
        match &c.spawns[1].behaviour {
            Behaviour::Patrol { route, pause } => {
                assert_eq!(route.len(), 2);
                assert_eq!(*pause, 2.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_world_rules_default_when_absent_and_are_checked_when_given() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.rules.world.share_sight_within, 10.0);
        assert_eq!(c.rules.world.paths_per_step, 4);
        let rules = RULES.replacen(
            "(hero_level: 30,",
            "(hero_level: 30, world: (share_sight_within: 10.0, paths_per_step: 0, wander_pause: (3.0, 8.0)),",
            1,
        );
        assert!(matches!(
            load(CLASSES, MAP, &rules),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn each_world_rule_is_checked() {
        let with = |world: &str| {
            let rules = RULES.replacen(
                "(hero_level: 30,",
                &format!("(hero_level: 30, world: {world},"),
                1,
            );
            load(CLASSES, MAP, &rules)
        };
        assert!(
            with("(share_sight_within: 0.0, paths_per_step: 1, wander_pause: (0.0, 0.0))").is_ok()
        );
        for bad in [
            "(share_sight_within: -1.0, paths_per_step: 4, wander_pause: (3.0, 8.0))",
            "(share_sight_within: NaN, paths_per_step: 4, wander_pause: (3.0, 8.0))",
            "(share_sight_within: 10.0, paths_per_step: 4, wander_pause: (8.0, 3.0))",
            "(share_sight_within: 10.0, paths_per_step: 4, wander_pause: (-1.0, 3.0))",
            "(share_sight_within: 10.0, paths_per_step: 4, wander_pause: (NaN, 3.0))",
            "(share_sight_within: 10.0, paths_per_step: 4, wander_pause: (3.0, inf))",
        ] {
            refused_naming(with(bad), "world");
        }
    }

    #[test]
    fn a_spawn_of_an_unknown_kind_is_refused() {
        let spawns = SPAWNS.replace("kind: \"spider\"", "kind: \"wolf\"");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &spawns),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_spawn_outside_the_bounds_is_refused() {
        let post = SPAWNS.replace("at: (60.0, 50.0)", "at: (160.0, 50.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &post),
            Err(ContentError::Invalid(_))
        ));
        let route = SPAWNS.replace("(80.0, 20.0)", "(80.0, -20.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &route),
            Err(ContentError::Invalid(_))
        ));
        let centre = SPAWNS.replace("center: (30.0, 70.0)", "center: (30.0, 170.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &centre),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_patrol_needs_two_points_and_a_wander_a_radius() {
        let one = SPAWNS.replace("[(20.0, 20.0), (80.0, 20.0)]", "[(20.0, 20.0)]");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &one),
            Err(ContentError::Invalid(_))
        ));
        for bad in ["0.0", "-5.0", "NaN"] {
            let none = SPAWNS.replace("radius: 10.0", &format!("radius: {bad}"));
            refused_naming(load_all(CLASSES, MAP, RULES, ENEMIES, &none), "radius");
        }
    }

    #[test]
    fn a_spawn_count_is_one_to_twenty() {
        for bad in ["0", "21"] {
            let spawns = SPAWNS.replace("count: 3", &format!("count: {bad}"));
            refused_naming(load_all(CLASSES, MAP, RULES, ENEMIES, &spawns), "count");
        }
        let most = SPAWNS.replace("count: 3", "count: 20");
        assert!(load_all(CLASSES, MAP, RULES, ENEMIES, &most).is_ok());
    }

    #[test]
    fn a_patrols_pause_and_a_spawns_numbers_are_finite() {
        let pause = SPAWNS.replace("(80.0, 20.0)])", "(80.0, 20.0)], pause: -1.0)");
        refused_naming(load_all(CLASSES, MAP, RULES, ENEMIES, &pause), "pause");
        let facing = SPAWNS.replace("facing: 90.0", "facing: NaN");
        refused_naming(load_all(CLASSES, MAP, RULES, ENEMIES, &facing), "facing");
        let z = SPAWNS.replace("z: Some(1.0)", "z: Some(inf)");
        refused_naming(load_all(CLASSES, MAP, RULES, ENEMIES, &z), "z must");
    }

    #[test]
    fn an_enemy_kind_twice_or_without_health_is_refused() {
        let twice = ENEMIES.replace("\"spider\", name", "\"skeleton\", name");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, &twice, SPAWNS),
            Err(ContentError::Invalid(_))
        ));
        let dead = ENEMIES.replace("hp: 900.0", "hp: 0.0");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, &dead, SPAWNS),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn an_enemys_numbers_are_finite_and_above_zero() {
        for (from, to) in [
            ("hp: 900.0", "hp: -1.0"),
            ("hp: 900.0", "hp: NaN"),
            ("hp: 900.0", "hp: inf"),
            ("walk_speed: 2.5", "walk_speed: 0.0"),
            ("walk_speed: 2.5", "walk_speed: NaN"),
            ("scale: 0.9", "scale: 0.0"),
            ("scale: 0.9", "scale: -1.0"),
            ("scale: 0.9", "scale: inf"),
            ("level: 28", "level: 0"),
        ] {
            let enemies = ENEMIES.replacen(from, to, 1);
            refused_naming(
                load_all(CLASSES, MAP, RULES, &enemies, SPAWNS),
                "enemies.ron",
            );
        }
    }

    #[test]
    fn an_enemy_runs_at_least_as_fast_as_it_walks() {
        // The skeleton walks at 2.5: running at 2.0 is slower, and NaN and infinity are no speed.
        for bad in ["2.0", "NaN", "inf"] {
            let enemies = ENEMIES.replacen("run_speed: 7.0", &format!("run_speed: {bad}"), 1);
            refused_naming(load_all(CLASSES, MAP, RULES, &enemies, SPAWNS), "run_speed");
        }
        let same = ENEMIES.replacen("run_speed: 7.0", "run_speed: 2.5", 1);
        assert!(load_all(CLASSES, MAP, RULES, &same, SPAWNS).is_ok());
    }

    #[test]
    fn the_senses_default_to_the_spec_when_left_out() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        let s = &c.rules.senses;
        assert_eq!(s.notice_range, 35.0);
        assert_eq!(s.visibility.walk, 55.0);
        assert_eq!(s.steps.run, 12.0);
        assert_eq!(s.loudness.dig, 30.0);
        assert_eq!(s.awareness.alert, 100.0);
        assert_eq!(s.stealth_speed, 0.7);
        assert!(c.map.lit.is_empty());
        assert!(!c.classes[0].stealth);
    }

    #[test]
    fn senses_given_in_part_keep_the_rest() {
        let rules = RULES.replacen(
            "(hero_level: 30,",
            "(hero_level: 30, senses: (notice_range: 30.0, awareness: (drain: 5.0)),",
            1,
        );
        let c = load(CLASSES, MAP, &rules).unwrap();
        assert_eq!(c.rules.senses.notice_range, 30.0);
        assert_eq!(c.rules.senses.awareness.drain, 5.0);
        assert_eq!(c.rules.senses.awareness.noise, 35.0);
        assert_eq!(c.rules.senses.visibility.run, 100.0);
    }

    #[test]
    fn senses_that_cannot_work_are_refused() {
        for bad in [
            "awareness: (suspicious: 70.0)",
            "stealth_speed: 0.0",
            "walls: 1.5",
            "visibility: (run: 120.0)",
            "cone: 300.0",
            "steps: (every: 0.0)",
        ] {
            let rules = RULES.replacen(
                "(hero_level: 30,",
                &format!("(hero_level: 30, senses: ({bad}),"),
                1,
            );
            assert!(
                matches!(load(CLASSES, MAP, &rules), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn senses_that_are_not_finite_numbers_are_refused() {
        // Each of these passes a bare `>= 0` or `> 0` check, so infinity needs its own refusal.
        for bad in [
            "notice_range: inf",
            "cone: inf",
            "steps: (run: inf)",
            "steps: (every: inf)",
            "loudness: (dig: inf)",
            "noise_memory: inf",
            "awareness: (noise: inf)",
            "awareness: (shout: inf)",
            "awareness: (fill: inf)",
            "awareness: (suspicious: NaN)",
            "chase_reach: inf",
            "ambush: inf",
            "lantern: inf",
            "flash_for: inf",
            "heartbeat_within: inf",
            "behind_cone: inf",
        ] {
            let rules = RULES.replacen(
                "(hero_level: 30,",
                &format!("(hero_level: 30, senses: ({bad}),"),
                1,
            );
            refused_naming(load(CLASSES, MAP, &rules), "senses");
        }
    }

    #[test]
    fn a_class_may_stealth() {
        let classes = CLASSES.replace(
            "base_hp: 100.0, speed: 7.0),",
            "base_hp: 100.0, speed: 7.0, stealth: true),",
        );
        let c = load(&classes, MAP, RULES).unwrap();
        assert!(c.classes[0].stealth);
        assert!(!c.classes[1].stealth);
    }

    #[test]
    fn lit_areas_load_and_need_a_radius() {
        let map = MAP.replace("entries:", "lit: [(20.0, 30.0, 6.0)], entries:");
        let c = load(CLASSES, &map, RULES).unwrap();
        assert_eq!(
            c.map.lit,
            vec![LitArea {
                at: V2::new(20.0, 30.0),
                radius: 6.0
            }]
        );
        let none = MAP.replace("entries:", "lit: [(20.0, 30.0, 0.0)], entries:");
        assert!(matches!(
            load(CLASSES, &none, RULES),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_sleeper_lies_inside_the_bounds() {
        let sleeper = SPAWNS.replace(
            "Sentry(at: (60.0, 50.0), facing: 90.0)",
            "Sleeper(at: (60.0, 50.0), facing: 90.0)",
        );
        let c = load_all(CLASSES, MAP, RULES, ENEMIES, &sleeper).unwrap();
        assert!(matches!(c.spawns[0].behaviour, Behaviour::Sleeper { .. }));
        let out = sleeper.replace("at: (60.0, 50.0)", "at: (160.0, 50.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &out),
            Err(ContentError::Invalid(_))
        ));
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
