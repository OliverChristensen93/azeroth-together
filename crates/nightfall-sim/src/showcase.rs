//! The showcase (a playtest tool, spec §12): one of each enemy kind, a rival party and a flock of
//! ravens around the entry, so each can be seen in a single raid. [`plan`] works out where, on the
//! map the host measured; [`World::run_showcase`] puts them there. Nothing here runs unless the
//! host asks (`NIGHTFALL_SHOWCASE=1` in the client).

use crate::enemies::{Order, ERRAND_BUDGET};
use crate::{Content, Navigator, World, FLOCK_SETTLED, V2, V3};

/// Yards past the arrive-unseen ring that the nearest enemy stands, so nothing is on top of the
/// party as the raid begins.
const OUTSIDE: f32 = 8.0;
/// Yards between the radii tried for one spot.
const RADIUS_STEP: f32 = 8.0;
/// How many radii are tried: the farthest enemy stands `OUTSIDE + RADIUS_STEP * (RADII - 1)`
/// yards beyond the ring, a walk of well under a minute.
const RADII: usize = 6;
/// Degrees either side of its own bearing that a spot may be moved to find ground.
const SPREAD: i32 = 40;
/// Degrees between the bearings tried.
const SPREAD_STEP: i32 = 8;
/// Yards two showcase enemies keep apart.
const APART: f32 = 10.0;
/// Yards around the entry within which a flock is settled for the showcase.
const FLOCK_NEAR: f32 = 100.0;

/// Where the showcase puts everything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShowcasePlan {
    /// One enemy per kind of the content that a spawn line names: the spawn line it takes its
    /// senses from, the kind's key, and its spot.
    pub enemies: Vec<(usize, String, V3)>,
    /// Kinds of the content no spawn line names, so none could be placed.
    pub missing: Vec<String>,
    /// Kinds that found no spot on the map (named, left out).
    pub unplaced: Vec<String>,
    /// The point of interest nearest the entry, as an index into the rivals' points, with its
    /// spot, when the content has one.
    pub poi: Option<(usize, V2)>,
    /// The flocks to settle: those within [`FLOCK_NEAR`] yards of the entry, else the nearest.
    pub flocks: Vec<usize>,
}

/// Where a unit placed at `p` stands, if it stands on ground that is not water.
fn stands(nav: &mut Navigator, p: V2) -> Option<V3> {
    let z = nav.map().ground_at(p)?;
    let at = nav.stand_near(V3::at(p, z))?;
    (!nav.map().is_water(at.ground())).then_some(at)
}

/// The showcase's places for `content` around `entry` on `nav`'s map: each kind's spot stands,
/// is not water, is outside the arrive-unseen ring (and [`OUTSIDE`] yards past it), keeps
/// [`APART`] yards from the others, and has a path from the entry within the errand budget. The
/// kinds go round the entry in the content's order, each tried at its own bearing and then
/// [`SPREAD`] degrees either side of it, nearest radius first.
pub fn plan(content: &Content, nav: &mut Navigator, entry: V2) -> ShowcasePlan {
    let mut out = ShowcasePlan::default();
    let ring = content.rules.night.arrive_unseen + OUTSIDE;
    let from = stands(nav, entry);
    let n = content.enemies.len().max(1);
    for (i, kind) in content.enemies.iter().enumerate() {
        let key = kind.key.as_str();
        let Some(spawn) = content.spawns.iter().position(|s| s.kind == key) else {
            out.missing.push(key.to_owned());
            continue;
        };
        let base = i as f32 * std::f32::consts::TAU / n as f32;
        let mut found = None;
        'search: for step in 0..=(SPREAD / SPREAD_STEP) {
            for sign in [1, -1] {
                if step == 0 && sign == -1 {
                    continue;
                }
                let degrees = (sign * step * SPREAD_STEP) as f32;
                let angle = base + degrees.to_radians();
                for r in 0..RADII {
                    let p = entry + V2::from_angle(angle, ring + r as f32 * RADIUS_STEP);
                    if out
                        .enemies
                        .iter()
                        .any(|(_, _, at)| at.ground().distance(p) < APART)
                    {
                        continue;
                    }
                    let Some(at) = stands(nav, p) else {
                        continue;
                    };
                    let reachable = from
                        .is_some_and(|f| nav.find_path(f, at, false, ERRAND_BUDGET).is_some());
                    if reachable {
                        found = Some(at);
                        break 'search;
                    }
                }
            }
        }
        match found {
            Some(at) => out.enemies.push((spawn, key.to_owned(), at)),
            None => out.unplaced.push(key.to_owned()),
        }
    }
    out.poi = content
        .rivals
        .pois
        .iter()
        .enumerate()
        .map(|(i, p)| (i, V2::new(p.at.0, p.at.1)))
        .min_by(|a, b| entry.distance(a.1).total_cmp(&entry.distance(b.1)));
    let flocks = &content.map.flocks;
    out.flocks = flocks
        .iter()
        .enumerate()
        .filter(|(_, f)| f.distance(entry) <= FLOCK_NEAR)
        .map(|(i, _)| i)
        .collect();
    if out.flocks.is_empty() {
        out.flocks.extend(
            flocks
                .iter()
                .enumerate()
                .min_by(|a, b| entry.distance(*a.1).total_cmp(&entry.distance(*b.1)))
                .map(|(i, _)| i),
        );
    }
    out
}

impl World {
    /// The showcase, on a host whose map is attached: [`plan`]'s enemies stand where it puts
    /// them (each a sentry facing away from the entry, so a hero can sneak up), a rival party
    /// stands at the point of interest nearest the entry, and the planned flocks are settled.
    /// The lines for what could not be placed are returned, for the log. Without a map nothing
    /// is done.
    pub fn run_showcase(&mut self) -> Vec<String> {
        let Some(mut nav) = self.nav.take() else {
            return Vec::new();
        };
        let content = self.content().clone();
        let entry = content
            .map
            .entries
            .get(self.raid.roll.entry)
            .map_or(V2::default(), |e| V2::new(e[0], e[1]));
        let plan = plan(&content, &mut nav, entry);
        self.nav = Some(nav);
        let mut lines = Vec::new();
        for k in plan.missing.iter().chain(&plan.unplaced) {
            lines.push(format!("NIGHTFALL_SHOWCASE: no spot for \"{k}\""));
        }
        for (spawn, _, at) in &plan.enemies {
            let away = at.ground() - entry;
            let facing = if away.length() > 0.0 {
                V2::from_angle(away.y.atan2(away.x), 1.0)
            } else {
                V2::new(1.0, 0.0)
            };
            let order = Order::Sentry {
                post: *at,
                facing,
            };
            self.add_enemy(*spawn, *at, facing, order);
        }
        if let (Some((_, at)), true) = (plan.poi, !content.rivals.parties.is_empty()) {
            // Numbered past the roll's parties, so none of them is ever mistaken for it.
            let number = self.raid.roll.rivals.len().min(usize::from(u8::MAX)) as u8;
            let (lo, hi) = content.rivals.parties[0].aggression;
            let seed = self.raid.roll.seed ^ 0x5C0_0E5;
            self.add_party(0, number, (lo + hi) * 0.5, at, seed);
        }
        for &k in &plan.flocks {
            if let Some(f) = self.night.flocks.get_mut(k) {
                f.state = FLOCK_SETTLED;
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::heightmap::testmap::{flat, grid, pond};
    use crate::{ContentSources, HeightMap, HeightMapBuilder};

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (300.0, 0.0), (300.0, 300.0), (0.0, 300.0)],
        entries: [(150.0, 150.0, 0.0)],
        ravens: [(150.0, 170.0), (20.0, 20.0)])"#;
    const ENEMIES: &str = r#"[
        (key: "a", name: "A", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
        (key: "b", name: "B", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
        (key: "c", name: "C", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
        (key: "d", name: "D", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
    ]"#;
    const SPAWNS: &str = r#"[
        (place: "x", kind: "a", behaviour: Sentry(at: (10.0, 10.0), facing: 0.0)),
        (place: "x", kind: "b", behaviour: Sentry(at: (12.0, 10.0), facing: 0.0)),
        (place: "x", kind: "c", behaviour: Wander(center: (14.0, 10.0), radius: 3.0)),
    ]"#;
    const RULES: &str = r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
        fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#;

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: RULES,
            enemies: ENEMIES,
            spawns: SPAWNS,
            abilities: "[]",
            items: "[]",
            loot: "()",
            monsters: "[]",
            rivals: "",
        })
        .unwrap()
    }

    fn entry() -> V2 {
        V2::new(150.0, 150.0)
    }

    #[test]
    fn each_kind_with_a_spawn_line_gets_a_spot_outside_the_ring_and_apart() {
        let c = content();
        let mut nav = Navigator::new(Arc::new(flat(300.0).finish()));
        let p = plan(&c, &mut nav, entry());
        // "d" has no spawn line to take its senses from: named, not placed.
        assert_eq!(p.missing, vec!["d".to_owned()]);
        assert!(p.unplaced.is_empty());
        assert_eq!(p.enemies.len(), 3);
        for (i, (_, _, a)) in p.enemies.iter().enumerate() {
            let far = a.ground().distance(entry());
            assert!(far > c.rules.night.arrive_unseen, "{a:?} is {far} yd out");
            assert!(far < 130.0, "{a:?} is {far} yd out");
            for (_, _, b) in &p.enemies[i + 1..] {
                assert!(a.ground().distance(b.ground()) >= APART);
            }
        }
        // The flock beside the entry is settled; the far one is not.
        assert_eq!(p.flocks, vec![0]);
        // No rivals in this content.
        assert_eq!(p.poi, None);
    }

    #[test]
    fn a_pond_on_a_bearing_moves_the_spot_round_it_never_into_the_water() {
        let c = content();
        let mut b = flat(300.0);
        // Water over the whole first kind's bearing (east of the entry), 50 to 110 yd out.
        pond(&mut b, V2::new(195.0, 140.0), V2::new(265.0, 160.0));
        let map: HeightMap = b.finish();
        let mut nav = Navigator::new(Arc::new(map));
        let p = plan(&c, &mut nav, entry());
        assert_eq!(p.enemies.len(), 3);
        for (_, key, at) in &p.enemies {
            assert!(!nav.map().is_water(at.ground()), "{key} stands in water");
        }
    }

    #[test]
    fn a_map_with_no_ground_places_nothing_and_says_so() {
        let c = content();
        let b = HeightMapBuilder::new(grid(300.0)).finish();
        let mut nav = Navigator::new(Arc::new(b));
        let p = plan(&c, &mut nav, entry());
        assert!(p.enemies.is_empty());
        assert_eq!(p.unplaced.len(), 3);
    }
}
