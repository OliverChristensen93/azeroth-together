//! Paths over the height map's floors (spec §4.2): A* from floor to floor, a step to a
//! neighbouring column allowed when its floor is within [`MAX_STEP`] of this one, then smoothed
//! into straight legs a unit can walk ([`HeightMap::walk_line`]). A unit keeps one column clear
//! of anything it cannot step onto, so it walks beside a wall rather than half inside it, and
//! never through a doorway narrower than three columns.
//!
//! Every search has a budget of floors to look at: a goal shut away (a sealed house, a cave the
//! map has no way into) costs at most that, never a frozen host. [`Navigator`] keeps the paths
//! asked for again and again (a patrol's legs) so each is searched once.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use super::{HeightMap, MAX_STEP};
use crate::V3;

/// A floor: its column, and which of the column's floors (bottom first).
type Node = u64;

/// The eight neighbours, the four straight ones first.
const NEIGHBOURS: [(i64, i64); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];
/// The cost of a straight and a diagonal step (√2 ≈ 1.4), in tenths of a column: whole numbers,
/// so the same search always ends the same way.
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
/// How far (columns) a start or goal off a walkable floor looks for one.
const SNAP_REACH: i64 = 4;
/// How far up or down (yards) a start's or goal's floor may be from the height asked.
const SNAP_HEIGHT: f32 = 1.5;
/// The most columns one straight leg of a smoothed path spans: it bounds the smoothing's work.
const LEG_CELLS: usize = 40;
/// Paths kept before the cache starts over.
const CACHE_CAP: usize = 1024;

fn node(col: usize, level: usize) -> Node {
    (col as u64) << 8 | level as u64
}

fn node_col(n: Node) -> usize {
    (n >> 8) as usize
}

fn node_level(n: Node) -> usize {
    (n & 0xFF) as usize
}

/// The octile distance between two columns in [`STRAIGHT`] units: never more than a path's cost.
fn octile(dx: i64, dy: i64) -> u32 {
    let (a, b) = (dx.unsigned_abs(), dy.unsigned_abs());
    let (lo, hi) = (a.min(b), a.max(b));
    (u64::from(DIAGONAL) * lo + u64::from(STRAIGHT) * (hi - lo)) as u32
}

impl HeightMap {
    /// Column `(ix, iy)`'s floor nearest `z` within `reach`, as (its level, its height); `None`
    /// off the map, on a blocked column, or with no floor in reach.
    fn level_near(&self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<(usize, f32)> {
        let col = self.grid.at(ix, iy)?;
        if self.is_blocked(col) {
            return None;
        }
        self.level_within(col, z, reach)
    }

    /// Whether a unit can stand on the floor at `z` in column `(ix, iy)`: it is there, and every
    /// neighbour has a floor it could step to, so nothing it cannot stand on is within a column.
    pub(crate) fn walkable(&self, ix: i64, iy: i64, z: f32) -> bool {
        self.level_near(ix, iy, z, MAX_STEP).is_some()
            && NEIGHBOURS
                .iter()
                .all(|&(dx, dy)| self.level_near(ix + dx, iy + dy, z, MAX_STEP).is_some())
    }

    fn node_cell(&self, n: Node) -> (i64, i64) {
        let (ix, iy) = self.grid.cell_of(node_col(n));
        (i64::from(ix), i64::from(iy))
    }

    fn node_z(&self, n: Node) -> f32 {
        self.floors_at(node_col(n)).as_slice()[node_level(n)]
    }

    fn node_point(&self, n: Node) -> V3 {
        let (ix, iy) = self.grid.cell_of(node_col(n));
        V3::at(self.grid.center(ix, iy), self.node_z(n))
    }

    /// The walkable floor nearest `p`: its own column's if a unit can stand there, else the
    /// nearest within [`SNAP_REACH`] columns.
    fn snap(&self, p: V3) -> Option<Node> {
        if !p.is_finite() {
            return None;
        }
        let (cx, cy) = self.grid.column_of(p.ground())?;
        for r in 0..=SNAP_REACH {
            let mut best: Option<(f32, Node)> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let (ix, iy) = (i64::from(cx) + dx, i64::from(cy) + dy);
                    let Some((level, z)) = self.level_near(ix, iy, p.z, SNAP_HEIGHT) else {
                        continue;
                    };
                    if !self.walkable(ix, iy, z) {
                        continue;
                    }
                    let d = self.grid.center(ix as u32, iy as u32).distance(p.ground());
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, node(self.grid.index(ix as u32, iy as u32), level)));
                    }
                }
            }
            if let Some((_, n)) = best {
                return Some(n);
            }
        }
        None
    }

    /// A* from `start` to `goal`, looking at no more than `budget` floors. Ties go to the floor
    /// farther along, which keeps a search over open ground near its straight line.
    fn search(&self, start: Node, goal: Node, budget: usize) -> Option<Vec<Node>> {
        let (gx, gy) = self.node_cell(goal);
        let (sx, sy) = self.node_cell(start);
        let mut open = BinaryHeap::new();
        let mut best: HashMap<Node, (u32, Node)> = HashMap::new();
        let mut stand: HashMap<Node, bool> = HashMap::new();
        best.insert(start, (0, start));
        open.push(Reverse((octile(sx - gx, sy - gy), u32::MAX, start)));
        let mut looked = 0usize;
        while let Some(Reverse((_, inv_g, here))) = open.pop() {
            let g = u32::MAX - inv_g;
            if here == goal {
                return Some(walk_back(&best, start, goal));
            }
            if best.get(&here).is_some_and(|&(bg, _)| bg < g) {
                continue;
            }
            looked += 1;
            if looked > budget {
                return None;
            }
            let (ix, iy) = self.node_cell(here);
            let z = self.node_z(here);
            for &(dx, dy) in &NEIGHBOURS {
                let (nx, ny) = (ix + dx, iy + dy);
                let Some((level, nz)) = self.level_near(nx, ny, z, MAX_STEP) else {
                    continue;
                };
                let diagonal = dx != 0 && dy != 0;
                // No cutting a corner: both straight neighbours must be steppable too.
                if diagonal
                    && (self.level_near(ix + dx, iy, z, MAX_STEP).is_none()
                        || self.level_near(ix, iy + dy, z, MAX_STEP).is_none())
                {
                    continue;
                }
                let next = node(self.grid.index(nx as u32, ny as u32), level);
                if !*stand
                    .entry(next)
                    .or_insert_with(|| self.walkable(nx, ny, nz))
                {
                    continue;
                }
                let ng = g + if diagonal { DIAGONAL } else { STRAIGHT };
                if best.get(&next).is_none_or(|&(bg, _)| ng < bg) {
                    best.insert(next, (ng, here));
                    open.push(Reverse((
                        ng + octile(nx - gx, ny - gy),
                        u32::MAX - ng,
                        next,
                    )));
                }
            }
        }
        None
    }

    /// The floors' points, straightened: from each point, the farthest later one (within
    /// [`LEG_CELLS`]) a unit can walk to in a straight line. The first point is left out.
    fn smooth(&self, nodes: &[Node]) -> Vec<V3> {
        let points: Vec<V3> = nodes.iter().map(|&n| self.node_point(n)).collect();
        if points.len() == 1 {
            return points;
        }
        let mut out = Vec::new();
        let mut anchor = 0;
        while anchor + 1 < points.len() {
            let mut reach = anchor + 1;
            while reach + 1 < points.len()
                && reach + 1 - anchor <= LEG_CELLS
                && self.walk_line(points[anchor], points[reach + 1])
            {
                reach += 1;
            }
            out.push(points[reach]);
            anchor = reach;
        }
        out
    }

    /// Whether a unit can walk straight from `a` to `b`: every quarter yard of the way has a
    /// floor it can stand on within [`MAX_STEP`] of the last, and it arrives at `b`'s height.
    pub fn walk_line(&self, a: V3, b: V3) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let d = b.ground() - a.ground();
        let steps = ((d.length() / (self.grid.cell * 0.5)).ceil() as usize).max(1);
        let mut z = a.z;
        for k in 1..=steps {
            let p = a.ground() + d * (k as f32 / steps as f32);
            let Some((ix, iy)) = self.grid.column_of(p) else {
                return false;
            };
            let (ix, iy) = (i64::from(ix), i64::from(iy));
            let Some((_, fz)) = self.level_near(ix, iy, z, MAX_STEP) else {
                return false;
            };
            if !self.walkable(ix, iy, fz) {
                return false;
            }
            z = fz;
        }
        (z - b.z).abs() <= MAX_STEP
    }
}

/// The floors from `start` to `goal` by the parents A* left.
fn walk_back(best: &HashMap<Node, (u32, Node)>, start: Node, goal: Node) -> Vec<Node> {
    let mut out = vec![goal];
    let mut at = goal;
    while at != start {
        at = best[&at].1;
        out.push(at);
    }
    out.reverse();
    out
}

/// Paths over one height map, with the ones asked for again kept.
pub struct Navigator {
    map: Arc<HeightMap>,
    cache: HashMap<(Node, Node), Arc<[V3]>>,
}

impl Navigator {
    pub fn new(map: Arc<HeightMap>) -> Self {
        Self {
            map,
            cache: HashMap::new(),
        }
    }

    pub fn map(&self) -> &Arc<HeightMap> {
        &self.map
    }

    /// How many paths are kept.
    pub fn cached(&self) -> usize {
        self.cache.len()
    }

    /// A path from `from` to `to`: the points to walk through, the start left out and the goal's
    /// floor last; `None` when either end has no walkable floor near it or no path is found
    /// within `budget` floors. With `cache`, the path is kept and handed out again for the same
    /// start and goal floors (spec §4.2: a patrol's legs).
    pub fn find_path(&mut self, from: V3, to: V3, cache: bool, budget: usize) -> Option<Arc<[V3]>> {
        let map = self.map.clone();
        let start = map.snap(from)?;
        let goal = map.snap(to)?;
        if cache {
            if let Some(path) = self.cache.get(&(start, goal)) {
                return Some(path.clone());
            }
        }
        let nodes = map.search(start, goal, budget)?;
        let path: Arc<[V3]> = map.smooth(&nodes).into();
        if cache {
            if self.cache.len() >= CACHE_CAP {
                self.cache.clear();
            }
            self.cache.insert((start, goal), path.clone());
        }
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::testmap::{flat, house};
    use super::super::HeightMapBuilder;
    use super::*;
    use crate::V2;

    const BIG: usize = 40_000;

    fn nav(b: HeightMapBuilder) -> Navigator {
        Navigator::new(Arc::new(b.finish()))
    }

    /// Every quarter yard along the path, from `from`.
    fn samples(from: V3, path: &[V3]) -> Vec<V2> {
        let mut out = vec![from.ground()];
        let mut at = from.ground();
        for p in path {
            let d = p.ground() - at;
            let n = ((d.length() / 0.25).ceil() as usize).max(1);
            for k in 1..=n {
                out.push(at + d * (k as f32 / n as f32));
            }
            at = p.ground();
        }
        out
    }

    fn length(from: V3, path: &[V3]) -> f32 {
        let mut at = from.ground();
        let mut len = 0.0;
        for p in path {
            len += at.distance(p.ground());
            at = p.ground();
        }
        len
    }

    fn inside(p: V2, min: V2, max: V2) -> bool {
        p.x > min.x && p.x < max.x && p.y > min.y && p.y < max.y
    }

    #[test]
    fn open_ground_is_a_few_straight_legs() {
        let mut n = nav(flat(50.0));
        let from = V3::new(5.0, 5.0, 0.0);
        let path = n
            .find_path(from, V3::new(40.0, 30.0, 0.0), false, BIG)
            .unwrap();
        assert!(path.len() <= 4, "{} legs", path.len());
        // No longer than the grid's own octile path (45.4 yd), and near the straight 43.
        assert!(length(from, &path) <= 46.0);
        assert!(path.last().unwrap().ground().distance(V2::new(40.0, 30.0)) < 0.5);
        assert!(path.windows(2).all(|w| n.map().walk_line(w[0], w[1])));
    }

    #[test]
    fn a_path_goes_around_a_house_not_through_it() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            None,
            false,
        );
        let mut n = nav(b);
        let from = V3::new(25.0, 10.0, 0.0);
        let path = n
            .find_path(from, V3::new(25.0, 40.0, 0.0), false, BIG)
            .unwrap();
        for s in samples(from, &path) {
            assert!(
                !inside(s, V2::new(20.0, 20.0), V2::new(30.0, 30.0)),
                "{s:?}"
            );
        }
        assert!(length(from, &path) > 31.0);
    }

    #[test]
    fn a_path_into_a_house_goes_through_its_door() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            false,
        );
        let mut n = nav(b);
        let from = V3::new(10.0, 25.0, 0.0);
        let path = n
            .find_path(from, V3::new(25.0, 25.0, 0.0), false, BIG)
            .unwrap();
        let crossing: Vec<V2> = samples(from, &path)
            .into_iter()
            .filter(|s| s.x >= 20.0 && s.x <= 20.5)
            .collect();
        assert!(!crossing.is_empty());
        assert!(
            crossing.iter().all(|s| s.y > 23.75 && s.y < 26.25),
            "{crossing:?}"
        );
    }

    #[test]
    fn a_closed_house_cannot_be_entered() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            None,
            false,
        );
        let mut n = nav(b);
        assert!(n
            .find_path(
                V3::new(10.0, 25.0, 0.0),
                V3::new(25.0, 25.0, 0.0),
                false,
                BIG
            )
            .is_none());
    }

    #[test]
    fn stairs_lead_to_the_upper_floor() {
        let mut b = flat(30.0);
        for k in 0..6 {
            let x = 10.0 + 0.5 * k as f32;
            b.add_box(
                V2::new(x, 10.0),
                V2::new(x + 0.5, 12.0),
                0.0,
                0.5 * (k + 1) as f32,
            );
        }
        b.add_box(V2::new(13.0, 10.0), V2::new(16.0, 16.0), 2.8, 3.0);
        let mut n = nav(b);
        let path = n
            .find_path(
                V3::new(5.0, 11.0, 0.0),
                V3::new(15.0, 14.0, 3.0),
                false,
                BIG,
            )
            .unwrap();
        assert!((path.last().unwrap().z - 3.0).abs() < 0.01);
    }

    #[test]
    fn a_ledge_too_high_is_not_climbed() {
        let mut b = flat(50.0);
        b.add_box(V2::new(30.0, 30.0), V2::new(35.0, 35.0), 0.0, 1.0);
        let mut n = nav(b);
        assert!(n
            .find_path(
                V3::new(20.0, 20.0, 0.0),
                V3::new(32.0, 32.0, 1.0),
                false,
                BIG
            )
            .is_none());
    }

    #[test]
    fn a_search_gives_up_at_the_budget_but_finds_the_goal_with_more() {
        // The goal is reachable, but only round the end of a long wall: a search that ignored its
        // budget would find it either way.
        let mut b = flat(400.0);
        b.add_box(V2::new(100.0, 0.0), V2::new(100.5, 350.0), 0.0, 3.0);
        let mut n = nav(b);
        let (from, to) = (V3::new(50.0, 50.0, 0.0), V3::new(150.0, 50.0, 0.0));
        assert!(n.find_path(from, to, false, 5_000).is_none());
        assert!(n.find_path(from, to, false, 2_000_000).is_some());
    }

    #[test]
    fn a_shut_away_goal_costs_only_its_budget() {
        let mut b = flat(400.0);
        house(
            &mut b,
            V2::new(200.0, 200.0),
            V2::new(210.0, 210.0),
            None,
            false,
        );
        let mut n = nav(b);
        assert!(n
            .find_path(
                V3::new(10.0, 10.0, 0.0),
                V3::new(205.0, 205.0, 0.0),
                false,
                5_000
            )
            .is_none());
    }

    #[test]
    fn a_far_goal_needs_a_big_enough_budget() {
        let mut n = nav(flat(400.0));
        let (from, to) = (V3::new(10.0, 10.0, 0.0), V3::new(390.0, 390.0, 0.0));
        assert!(n.find_path(from, to, false, 50).is_none());
        assert!(n.find_path(from, to, false, BIG).is_some());
    }

    #[test]
    fn a_path_never_leaves_the_map() {
        let mut n = nav(flat(50.0));
        let from = V3::new(0.75, 5.0, 0.0);
        let path = n
            .find_path(from, V3::new(0.75, 45.0, 0.0), false, BIG)
            .unwrap();
        assert!(samples(from, &path).iter().skip(1).all(|s| s.x >= 0.5));
        assert!(n
            .find_path(from, V3::new(60.0, 25.0, 0.0), false, BIG)
            .is_none());
        assert!(n
            .find_path(V3::new(-3.0, 25.0, 0.0), from, false, BIG)
            .is_none());
    }

    #[test]
    fn a_unit_keeps_a_cell_from_a_wall() {
        let mut b = flat(50.0);
        b.add_box(V2::new(20.0, 0.0), V2::new(20.5, 50.0), 0.0, 3.0);
        let m = b.finish();
        assert!(!m.walkable(39, 10, 0.0));
        assert!(m.walkable(38, 10, 0.0));
    }

    #[test]
    fn the_cache_hands_out_the_same_path() {
        let mut n = nav(flat(50.0));
        let (from, to) = (V3::new(5.0, 5.0, 0.0), V3::new(40.0, 30.0, 0.0));
        let a = n.find_path(from, to, true, BIG).unwrap();
        let b = n.find_path(from, to, true, BIG).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(n.cached(), 1);
        n.find_path(from, V3::new(30.0, 40.0, 0.0), false, BIG)
            .unwrap();
        assert_eq!(n.cached(), 1);
    }

    #[test]
    fn the_same_search_gives_the_same_path() {
        let build = || {
            let mut b = flat(50.0);
            house(
                &mut b,
                V2::new(20.0, 20.0),
                V2::new(30.0, 30.0),
                None,
                false,
            );
            nav(b)
        };
        let (from, to) = (V3::new(25.0, 10.0, 0.0), V3::new(25.0, 40.0, 0.0));
        assert_eq!(
            build().find_path(from, to, false, BIG),
            build().find_path(from, to, false, BIG)
        );
    }
}
