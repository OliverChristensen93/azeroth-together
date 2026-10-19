//! Places the night uses (spec §7.3, §8.1): risk zones, roads and the ravens' flocks from
//! `map.ron`, and the signature monsters of `monsters.ron`.

use serde::Deserialize;

use crate::{Bounds, SpawnDef, V2};

fn one() -> f32 {
    1.0
}

fn six() -> f32 {
    6.0
}

/// A risk zone, or the gentler outskirts (spec §8.1): enemies placed inside have their health and
/// their blows times these, and containers and bodies inside roll their tables `loot_rolls` more
/// times. Death works the same everywhere.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneDef {
    pub key: String,
    pub name: String,
    pub area: Bounds,
    pub hp: f32,
    pub damage: f32,
    pub loot_rolls: u8,
}

#[derive(Deserialize)]
pub(crate) struct ZoneFile {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) area: Vec<(f32, f32)>,
    #[serde(default = "one")]
    pub(crate) hp: f32,
    #[serde(default = "one")]
    pub(crate) damage: f32,
    #[serde(default)]
    pub(crate) loot_rolls: u8,
}

/// A road (spec §7.3): the Dark Riders ride it and see far along it, Stitches walks one, and a
/// scent trail across one is broken.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadDef {
    pub key: String,
    pub name: String,
    /// Yards across.
    pub width: f32,
    pub points: Vec<V2>,
}

#[derive(Deserialize)]
pub(crate) struct RoadFile {
    pub(crate) key: String,
    pub(crate) name: String,
    #[serde(default = "six")]
    pub(crate) width: f32,
    pub(crate) points: Vec<(f32, f32)>,
}

impl RoadDef {
    /// Whether `p` stands on the road: within half its width of its line.
    pub fn holds(&self, p: V2) -> bool {
        self.points
            .windows(2)
            .any(|w| segment_distance(p, w[0], w[1]) <= self.width * 0.5)
    }
}

/// How far `p` is from the segment from `a` to `b`.
pub(crate) fn segment_distance(p: V2, a: V2, b: V2) -> f32 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// A road's points from end to end and back, without walking either end twice: a patrol route,
/// which loops from its last point to its first.
pub(crate) fn there_and_back(points: &[V2]) -> Vec<(f32, f32)> {
    let mut route: Vec<(f32, f32)> = points.iter().map(|p| (p.x, p.y)).collect();
    route.extend(
        points
            .iter()
            .rev()
            .skip(1)
            .take(points.len().saturating_sub(2))
            .map(|p| (p.x, p.y)),
    );
    route
}

/// A signature monster (spec §7.3): its spawn lines (in `Content::spawns`) are placed only in the
/// raids that roll it out, and every raid for one `always` out.
#[derive(Clone, Debug, PartialEq)]
pub struct MonsterDef {
    pub key: String,
    pub name: String,
    /// The chance a raid rolls it out.
    pub odds: f32,
    pub always: bool,
    /// Its spawn lines, indices into `Content::spawns`.
    pub lines: Vec<usize>,
}

#[derive(Deserialize)]
pub(crate) struct MonsterFile {
    pub(crate) key: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) odds: f32,
    #[serde(default)]
    pub(crate) always: bool,
    pub(crate) spawns: Vec<SpawnDef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_road_holds_what_is_within_half_its_width() {
        let r = RoadDef {
            key: "r".into(),
            name: "R".into(),
            width: 6.0,
            points: vec![V2::new(0.0, 0.0), V2::new(10.0, 0.0), V2::new(10.0, 10.0)],
        };
        assert!(r.holds(V2::new(5.0, 2.9)));
        assert!(!r.holds(V2::new(5.0, 3.1)));
        assert!(r.holds(V2::new(12.5, 6.0)));
        assert!(!r.holds(V2::new(-3.5, 0.0)));
    }

    #[test]
    fn a_road_there_and_back_walks_each_end_once() {
        let p = |x: f32| V2::new(x, 0.0);
        assert_eq!(
            there_and_back(&[p(1.0), p(2.0), p(3.0), p(4.0)]),
            vec![
                (1.0, 0.0),
                (2.0, 0.0),
                (3.0, 0.0),
                (4.0, 0.0),
                (3.0, 0.0),
                (2.0, 0.0)
            ]
        );
        assert_eq!(
            there_and_back(&[p(1.0), p(2.0)]),
            vec![(1.0, 0.0), (2.0, 0.0)]
        );
    }
}
