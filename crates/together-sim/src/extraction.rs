//! Extraction mode's data and state.
//!
//! The whole of a zone is open, inside an outline drawn round it. A safe ring (Goldshire) spawns
//! nothing and lets heat cool; out in the forest heat climbs with time and kills, and with it the
//! horde, the hunters sent to cut heroes off from home, and the rewards. Exits open on a timer: a
//! hero standing in an open one calls a flight, which takes whoever stands in its ring when it
//! lands. The rules that use all this are in `world/extraction.rs`.

use serde::{Deserialize, Serialize};

use crate::math::V2;
use crate::world::PlayerId;

/// One extraction map, as `content/extraction.ron` describes it.
#[derive(Clone, Debug, Deserialize)]
pub struct ExtractionDef {
    pub key: String,
    /// The zone it is played in, a key into `zones.ron`: its enemies, shrines, centre and map.
    pub zone: String,
    /// The playable area's outline in WoW `(x, y)`: a convex polygon, corners in order.
    pub boundary: Vec<[f32; 2]>,
    pub safe: SafeDef,
    /// Ordinary enemies further than this from every living hero are let go.
    pub recycle: f32,
    /// Ordinary enemies at most, per hero outside the safe ring, however hot: a ceiling on the
    /// horde's cost to the host and its snapshots.
    pub max_enemies_per_hero: u32,
    /// How far from the safe ring's centre the run's first shrines stand, nearest and furthest.
    pub first_shrines: (f32, f32),
    /// A ready shrine further than this from every living hero goes, making room for one near them.
    pub shrine_reach: f32,
    pub heat: HeatDef,
    pub exits: Vec<ExitDef>,
    #[serde(default)]
    pub ambush: AmbushDef,
    /// The creature display a flight comes down as.
    pub flight_display: u32,
    #[serde(default)]
    pub settings: ExtractionSettings,
    /// [`ExtractionDef::zone`] resolved to an index into [`crate::Content::zones`].
    #[serde(skip)]
    pub zone_ix: usize,
}

/// The safe ring: nothing new spawns near it, and heat cools inside it.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SafeDef {
    pub center: [f32; 2],
    pub radius: f32,
    /// Nothing spawns within `radius + margin` of the centre.
    pub margin: f32,
    /// Heat lost per second while it may cool.
    pub cool: f32,
}

/// How heat is gained, what floor it keeps, and what each skull of it does.
#[derive(Clone, Debug, Deserialize)]
pub struct HeatDef {
    /// Gained each second outside the safe ring.
    pub outside: f32,
    /// Gained per kill.
    pub kill: f32,
    /// Gained each second a flight is on its way.
    pub flight: f32,
    /// The nightfall floor starts rising this many seconds in...
    pub floor_from: f32,
    /// ...by this much a minute.
    pub floor_per_min: f32,
    /// Luck added for each point of reward multiplier over 1.
    pub reward_luck: f32,
    /// `lose_them`: no enemy within this many yards of any living hero...
    pub quiet_range: f32,
    /// ...for this many seconds drops a skull.
    pub quiet_secs: f32,
    /// The skulls from 0 up, each starting at its `at` heat.
    pub skulls: Vec<SkullDef>,
    pub hunters: HunterDef,
}

/// What one skull of heat does.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SkullDef {
    /// The heat it starts at.
    pub at: f32,
    /// The horde's size, times the arena's.
    pub spawns: f32,
    /// Roster tiers added to what the clock brings.
    pub tier: usize,
    /// Enemy health and damage, times.
    pub toughness: f32,
    /// Seconds between hunter packs; 0 for none.
    pub hunters_every: f32,
    /// XP and gold from kills, times.
    pub reward: f32,
}

/// The packs sent to cut a hot hero off from home.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct HunterDef {
    /// Enemies in a pack, fewest and most.
    pub pack: (u32, u32),
    /// How far from the hero they come, nearest and furthest.
    pub ring: (f32, f32),
    /// The cone around the way home they come from, in degrees.
    pub cone: f32,
    /// Their speed, times.
    pub speed: f32,
}

/// A pack sent to a waiting flight.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct AmbushDef {
    /// The chance a departure draws one.
    pub chance: f32,
    /// Seconds before its flight lands that the party is warned.
    pub warn: f32,
    /// Enemies in the pack at no heat, fewest and most...
    pub pack: (u32, u32),
    /// ...and this share more for each skull of the party's heat.
    pub per_skull: f32,
    /// How far from the exit they come, nearest and furthest.
    pub ring: (f32, f32),
}

impl Default for AmbushDef {
    fn default() -> Self {
        Self {
            chance: 0.0,
            warn: 20.0,
            pack: (6, 10),
            per_skull: 0.25,
            ring: (40.0, 60.0),
        }
    }
}

/// A place to leave from, and its timetable.
#[derive(Clone, Debug, Deserialize)]
pub struct ExitDef {
    pub name: String,
    pub pos: [f32; 2],
    pub radius: f32,
    /// Seconds into the run its first flight departs...
    pub first: f32,
    /// ...and between departures after that.
    pub every: f32,
    /// Seconds a flight waits on the ground before it departs.
    pub window: f32,
}

/// Rules to playtest both ways.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ExtractionSettings {
    /// One heat for the party, or one per hero.
    #[serde(default = "yes")]
    pub heat_shared: bool,
    /// A quiet spell with no enemy near drops a skull.
    #[serde(default)]
    pub lose_them: bool,
    /// The zone's boss comes on its arena timer.
    #[serde(default)]
    pub timed_boss: bool,
}

fn yes() -> bool {
    true
}

impl Default for ExtractionSettings {
    fn default() -> Self {
        Self {
            heat_shared: true,
            lose_them: false,
            timed_boss: false,
        }
    }
}

impl ExtractionDef {
    pub fn safe_center(&self) -> V2 {
        V2::new(self.safe.center[0], self.safe.center[1])
    }

    /// Inside the safe ring.
    pub fn in_safe(&self, p: V2) -> bool {
        p.distance(self.safe_center()) <= self.safe.radius
    }

    /// Too near the safe ring for anything to spawn.
    pub fn in_margin(&self, p: V2) -> bool {
        p.distance(self.safe_center()) < self.safe.radius + self.safe.margin
    }

    pub fn exit_pos(&self, exit: usize) -> V2 {
        let p = self.exits[exit].pos;
        V2::new(p[0], p[1])
    }

    pub fn bounds(&self) -> Bounds {
        Bounds::Polygon(self.boundary.iter().map(|p| V2::new(p[0], p[1])).collect())
    }

    /// The skull `heat` stands at: the last whose `at` it has reached.
    pub fn skull_index(&self, heat: f32) -> usize {
        self.heat
            .skulls
            .iter()
            .rposition(|s| heat >= s.at)
            .unwrap_or(0)
    }

    pub fn skull(&self, heat: f32) -> &SkullDef {
        &self.heat.skulls[self.skull_index(heat)]
    }

    /// The nightfall floor `elapsed` seconds in.
    pub fn floor(&self, elapsed: f32) -> f32 {
        ((elapsed - self.heat.floor_from) / 60.0 * self.heat.floor_per_min).max(0.0)
    }
}

/// Where heroes may go: the arena's circle, or an extraction map's outline.
#[derive(Clone, Debug, PartialEq)]
pub enum Bounds {
    Circle {
        center: V2,
        radius: f32,
    },
    /// A convex polygon, corners in order.
    Polygon(Vec<V2>),
}

impl Bounds {
    /// `p`, or the nearest point inside when it is outside.
    pub fn clamp(&self, p: V2) -> V2 {
        match self {
            Bounds::Circle { center, radius } => {
                let off = p - *center;
                if off.length() > *radius {
                    *center + off.normalize_or_zero() * *radius
                } else {
                    p
                }
            }
            Bounds::Polygon(poly) => {
                if contains(poly, p) {
                    p
                } else {
                    nearest_on_outline(poly, p)
                }
            }
        }
    }

    /// Whether `p` is inside with at least `margin` yards to spare.
    pub fn inside(&self, p: V2, margin: f32) -> bool {
        match self {
            Bounds::Circle { center, radius } => p.distance(*center) <= radius - margin,
            Bounds::Polygon(poly) => {
                contains(poly, p) && nearest_on_outline(poly, p).distance(p) >= margin
            }
        }
    }
}

/// Whether `p` lies inside `poly`, by the even-odd rule.
fn contains(poly: &[V2], p: V2) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The point on `poly`'s outline nearest `p`.
fn nearest_on_outline(poly: &[V2], p: V2) -> V2 {
    let mut best = poly[0];
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let ab = b - a;
        let ap = p - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let t = if len2 > 0.0 {
            ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a + ab * t;
        if q.distance(p) < best.distance(p) {
            best = q;
        }
    }
    best
}

/// Which rules a run plays by.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// A leash round the zone's centre, the horde on the clock, a boss every few minutes.
    Arena,
    Extraction(Extraction),
}

/// An extraction run's state. It is in every snapshot, so a co-op client's HUD reads it too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extraction {
    /// Index into [`crate::Content::extractions`].
    pub def: usize,
    /// The party's heat, when `heat_shared`.
    pub heat: f32,
    /// Each hero's heat, when not.
    pub heat_by: Vec<(PlayerId, f32)>,
    /// The nightfall floor now.
    pub floor: f32,
    /// The most heat anyone has had this run.
    pub peak: f32,
    /// The skulls last told of (`None` the party's, else a hero's), so a change is told once.
    pub told: Vec<(Option<PlayerId>, u8)>,
    /// Seconds no enemy has come near any living hero (`lose_them`).
    pub quiet: f32,
    /// Seconds until the next hunter pack may come.
    pub hunt: f32,
    pub exits: Vec<ExitState>,
}

/// How one exit stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExitState {
    /// Seconds into the run its next flight departs: the map's timetable, as the host tuned it.
    pub departs: f32,
    /// The next flight is on the ground, boarding.
    pub boarding: bool,
    /// Whether the next departure draws an ambush has been rolled; it does; the party has been
    /// warned; the pack has come.
    pub rolled: bool,
    pub ambush: bool,
    pub warned: bool,
    pub came: bool,
}

impl ExitState {
    /// Seconds until the next flight lands at `def`'s exit; zero or less once it is down.
    pub fn lands_in(&self, def: &ExitDef, now: f32) -> f32 {
        self.departs - def.window - now
    }
}

impl Extraction {
    pub fn new(def_ix: usize, def: &ExtractionDef) -> Self {
        Self {
            def: def_ix,
            heat: 0.0,
            heat_by: Vec::new(),
            floor: 0.0,
            peak: 0.0,
            told: Vec::new(),
            quiet: 0.0,
            hunt: 0.0,
            exits: def
                .exits
                .iter()
                .map(|e| ExitState {
                    departs: e.first,
                    ..ExitState::default()
                })
                .collect(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::content::tests::{ABILITIES, CLASSES, ENEMIES};
    use crate::content::{Content, ContentError, ContentSources};

    /// A small square world: the safe ring at the origin, exits east and south.
    pub(crate) const TEST_ZONES: &str = r#"[
        (key: "goldshire", name: "Goldshire", map: 0, center: (0.0, 0.0, 0.0), leash: 1000.0,
         enemies: ["wolf"], boss: "hogger",
         interactables: [(kind: Chest, count: 1)], shrines_at_once: 1, pots: 0),
    ]"#;

    /// `SETTINGS` is replaced by a test's settings ([`maps`]), e.g. `heat_shared: false`.
    pub(crate) const TEST_MAPS: &str = r#"[
        (key: "goldshire", zone: "goldshire",
         boundary: [(-300.0, -300.0), (300.0, -300.0), (300.0, 300.0), (-300.0, 300.0)],
         safe: (center: (0.0, 0.0), radius: 40.0, margin: 50.0, cool: 15.0),
         recycle: 120.0,
         max_enemies_per_hero: 150,
         first_shrines: (100.0, 250.0),
         shrine_reach: 300.0,
         heat: (outside: 1.0, kill: 0.5, flight: 4.0, floor_from: 180.0, floor_per_min: 40.0,
                reward_luck: 0.25, quiet_range: 40.0, quiet_secs: 10.0,
                skulls: [
                    (at: 0.0, spawns: 1.0, tier: 0, toughness: 1.0, hunters_every: 0.0, reward: 1.0),
                    (at: 100.0, spawns: 1.3, tier: 1, toughness: 1.15, hunters_every: 0.0, reward: 1.5),
                    (at: 250.0, spawns: 1.6, tier: 1, toughness: 1.3, hunters_every: 0.0, reward: 2.0),
                    (at: 450.0, spawns: 2.0, tier: 2, toughness: 1.5, hunters_every: 20.0, reward: 2.75),
                    (at: 700.0, spawns: 2.5, tier: 3, toughness: 1.75, hunters_every: 15.0, reward: 3.5),
                    (at: 1000.0, spawns: 3.0, tier: 4, toughness: 2.0, hunters_every: 10.0, reward: 5.0),
                ],
                hunters: (pack: (6, 10), ring: (50.0, 70.0), cone: 60.0, speed: 1.2)),
         exits: [
            (name: "East Gate", pos: (200.0, 0.0), radius: 15.0, first: 60.0, every: 60.0,
             window: 30.0),
            (name: "South Camp", pos: (0.0, -250.0), radius: 15.0, first: 120.0, every: 120.0,
             window: 30.0),
         ],
         flight_display: 1566,
         settings: (SETTINGS)),
    ]"#;

    /// The test maps with `settings`; none given is the default, written out, since a settings
    /// block needs at least one field.
    fn maps(settings: &str) -> String {
        let settings = if settings.is_empty() {
            "heat_shared: true"
        } else {
            settings
        };
        TEST_MAPS.replace("SETTINGS", settings)
    }

    pub(crate) fn test_content(settings: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: TEST_ZONES,
        })
        .and_then(|c| c.with_extractions(&maps(settings)))
        .unwrap_or_else(|e| panic!("test content: {e}"))
    }

    fn maps_with(from: &str, to: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: TEST_ZONES,
        })?
        .with_extractions(&maps("").replace(from, to))
    }

    #[test]
    fn a_map_parses_and_is_tied_to_its_zone() {
        let c = test_content("lose_them: true");
        assert_eq!(c.extractions.len(), 1);
        let map = &c.extractions[0];
        assert_eq!(map.zone_ix, 0);
        assert_eq!(c.zones[0].extraction_ix, Some(0));
        assert!(map.settings.heat_shared && map.settings.lose_them);
        assert_eq!(map.exits[1].name, "South Camp");
    }

    #[test]
    fn a_map_for_a_missing_zone_is_refused() {
        let err = maps_with(r#"zone: "goldshire""#, r#"zone: "nowhere""#).unwrap_err();
        assert!(matches!(err, ContentError::UnknownKey { .. }), "{err}");
    }

    #[test]
    fn an_exit_outside_the_outline_is_refused() {
        let err = maps_with("pos: (200.0, 0.0)", "pos: (900.0, 0.0)").unwrap_err();
        assert!(
            matches!(
                err,
                ContentError::Invalid {
                    why: "has an exit outside its boundary",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn skulls_must_start_at_zero_and_rise() {
        let err = maps_with("(at: 250.0,", "(at: 50.0,").unwrap_err();
        assert!(matches!(err, ContentError::Invalid { .. }), "{err}");
    }

    #[test]
    fn skulls_and_the_floor_follow_heat_and_time() {
        let c = test_content("");
        let map = &c.extractions[0];
        assert_eq!(map.skull_index(0.0), 0);
        assert_eq!(map.skull_index(99.9), 0);
        assert_eq!(map.skull_index(100.0), 1);
        assert_eq!(map.skull_index(5000.0), 5);
        assert_eq!(map.skull(460.0).hunters_every, 20.0);
        assert_eq!(map.floor(100.0), 0.0);
        assert!((map.floor(240.0) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn an_outline_holds_a_point_on_its_nearest_edge() {
        let square = Bounds::Polygon(vec![
            V2::new(-10.0, -10.0),
            V2::new(10.0, -10.0),
            V2::new(10.0, 10.0),
            V2::new(-10.0, 10.0),
        ]);
        assert_eq!(square.clamp(V2::new(3.0, 4.0)), V2::new(3.0, 4.0));
        assert_eq!(square.clamp(V2::new(25.0, 4.0)), V2::new(10.0, 4.0));
        assert_eq!(square.clamp(V2::new(-30.0, -30.0)), V2::new(-10.0, -10.0));
        assert!(square.inside(V2::new(0.0, 0.0), 5.0));
        assert!(!square.inside(V2::new(8.0, 0.0), 5.0));
        assert!(!square.inside(V2::new(11.0, 0.0), 0.0));
    }

    #[test]
    fn a_circle_holds_a_point_on_its_rim_as_the_arena_leash_does() {
        let ring = Bounds::Circle {
            center: V2::new(100.0, 0.0),
            radius: 10.0,
        };
        assert_eq!(ring.clamp(V2::new(130.0, 0.0)), V2::new(110.0, 0.0));
        assert_eq!(ring.clamp(V2::new(105.0, 0.0)), V2::new(105.0, 0.0));
        assert!(ring.inside(V2::new(104.0, 0.0), 3.0));
        assert!(!ring.inside(V2::new(108.0, 0.0), 3.0));
    }

    #[test]
    fn tuning_scales_exits_flights_heat_horde_and_regen() {
        let base = test_content("").with_rules("(hero_regen: 1.0)").unwrap();
        let tuned = base.tuned(&crate::content::Tuning {
            exits: 0.5,
            flight: 2.0,
            heat: 3.0,
            horde: 0.5,
            regen: 4.0,
        });
        let (a, b) = (&base.extractions[0], &tuned.extractions[0]);
        assert_eq!(b.exits[0].first, a.exits[0].first * 0.5);
        assert_eq!(b.exits[0].every, a.exits[0].every * 0.5);
        assert_eq!(b.exits[1].window, a.exits[1].window * 2.0);
        assert_eq!(b.heat.outside, a.heat.outside * 3.0);
        assert_eq!(b.heat.kill, a.heat.kill * 3.0);
        assert_eq!(b.heat.flight, a.heat.flight * 3.0);
        assert_eq!(b.heat.floor_per_min, a.heat.floor_per_min * 3.0);
        assert_eq!(b.heat.skulls[5].spawns, a.heat.skulls[5].spawns * 0.5);
        assert_eq!(b.max_enemies_per_hero, a.max_enemies_per_hero / 2);
        assert_eq!(tuned.rules.hero_regen, 4.0);
        let same = base.tuned(&crate::content::Tuning::default());
        assert_eq!(same.extractions[0].exits[0].first, a.exits[0].first);
        assert_eq!(same.rules.hero_regen, 1.0);
    }

    #[test]
    fn a_timetable_that_cannot_run_is_refused() {
        let err = maps_with("every: 60.0,", "every: 20.0,").unwrap_err();
        assert!(
            matches!(
                err,
                ContentError::Invalid {
                    why: "has an exit whose timetable cannot run",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn a_pack_whose_fewest_is_more_than_its_most_is_refused() {
        let err = maps_with("pack: (6, 10)", "pack: (10, 6)").unwrap_err();
        assert!(matches!(err, ContentError::Invalid { .. }), "{err}");
    }
}
