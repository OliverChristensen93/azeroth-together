//! Paths over the height map's floors (spec §4.2): A* from floor to floor, a step to a
//! neighbouring column allowed when its floor is within [`MAX_STEP`] of this one, then smoothed
//! into straight legs a unit can walk ([`HeightMap::walk_line`]). A unit keeps one column clear
//! of anything it cannot step onto, so it walks beside a wall rather than half inside it, and
//! never through a doorway narrower than three columns.
//!
//! Every search has a budget of floors to look at: a goal shut away (a sealed house, a cave the
//! map has no way into) costs at most that, never a frozen host. [`Navigator`] keeps the paths
//! asked for again and again (a patrol's legs) so each is searched once, reads the floors through
//! tiles it works out as searches reach them, and, for the enemies' planning, shares one budget
//! of floors among a step's searches and remembers for a while the searches that found nothing.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use super::walk::{Cached, Ground, Place, Walk, NEIGHBOURS};
use super::{HeightMap, HEADROOM, MAX_STEP};
use crate::{TICK_HZ, V3};

/// A floor: its column, and which of the column's floors (bottom first).
type Node = u64;

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
/// A failed search is remembered by the squares its ends stand in, this many columns (4 yd) a
/// side, and by the storey ([`HEADROOM`] bands of height): a groupmate, or the same enemy a
/// moment later, asking from near there to near there is told at once.
const FAIL_SQUARE: i64 = 8;
/// Steps a failed search is remembered for: 3 s, less than a sentry's 5 s retry, so the retry
/// still searches.
const FAIL_STEPS: u64 = 3 * TICK_HZ as u64;
/// Failures kept before the stale ones are swept out.
const FAIL_CAP: usize = 256;

/// The Fx hash (rustc's): one multiply a word. The search's maps are keyed by floors the host
/// itself made, so they need speed, not SipHash's defence against chosen keys.
#[derive(Default)]
struct FxHasher(u64);

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

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

    /// Whether a unit can walk straight from `a` to `b`: every quarter yard of the way has a
    /// floor it can stand on within [`MAX_STEP`] of the last, and it arrives at `b`'s height.
    pub fn walk_line(&self, a: V3, b: V3) -> bool {
        walk_line(&mut &*self, a, b)
    }

    /// The floor a unit can stand on nearest `p`, as a path's ends are found: in `p`'s own
    /// column within [`SNAP_HEIGHT`] of `p.z`, else in the nearest column within [`SNAP_REACH`]
    /// (2 yd) that has one, at that column's middle. `None` when there is none: `p` is on a tree
    /// trunk's or a rock's top, or in a wall, with no ground near its height beside it.
    pub fn stand_near(&self, p: V3) -> Option<V3> {
        stand_at(&mut &*self, p)
    }
}

/// [`HeightMap::stand_near`] over any [`Ground`].
fn stand_at(ground: &mut impl Ground, p: V3) -> Option<V3> {
    let n = snap(ground, p)?;
    let map = ground.map();
    let at = map.node_point(n);
    let own = map.grid.column_of(p.ground()) == Some(map.grid.cell_of(node_col(n)));
    Some(if own { V3::at(p.ground(), at.z) } else { at })
}

/// The walkable floor nearest `p`: its own column's if a unit can stand there, else the nearest
/// within [`SNAP_REACH`] columns.
fn snap(ground: &mut impl Ground, p: V3) -> Option<Node> {
    if !p.is_finite() {
        return None;
    }
    let grid = ground.map().grid;
    let (cx, cy) = grid.column_of(p.ground())?;
    for r in 0..=SNAP_REACH {
        let mut best: Option<(f32, Node)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let (ix, iy) = (i64::from(cx) + dx, i64::from(cy) + dy);
                let Some(foot) = ground.foothold(ix, iy, p.z, SNAP_HEIGHT) else {
                    continue;
                };
                if !foot.stands {
                    continue;
                }
                let d = grid.center(ix as u32, iy as u32).distance(p.ground());
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, node(grid.index(ix as u32, iy as u32), foot.level)));
                }
            }
        }
        if let Some((_, n)) = best {
            return Some(n);
        }
    }
    None
}

/// One tile's share of a search: for each of its floors, the cheapest cost found to it and the
/// way it was reached.
#[derive(Default)]
struct TileState {
    tile: usize,
    /// `u32::MAX` for a floor not reached yet.
    cost: Vec<u32>,
    /// The step that reached it: its [`NEIGHBOURS`] index times eight, plus the level it came
    /// from.
    from: Vec<u8>,
}

/// What a search keeps between one search and the next, so a warm search allocates nothing. Its
/// state is dense arrays beside the [`Walk`]'s tiles rather than a map keyed by floor: a search
/// looks a floor up eight times for each it opens.
#[derive(Default)]
struct Scratch {
    /// Floors to look at: ([`rank`], the floor).
    open: BinaryHeap<Reverse<(u64, Node)>>,
    /// Each tile's index in `state` during a search; `u32::MAX` for one not reached.
    slot: Vec<u32>,
    state: Vec<TileState>,
    /// States of earlier searches, kept for their allocations.
    spare: Vec<TileState>,
}

impl Scratch {
    fn reset(&mut self, tiles: usize) {
        self.open.clear();
        if self.slot.len() != tiles {
            self.slot = vec![u32::MAX; tiles];
        }
        for s in self.state.drain(..) {
            self.slot[s.tile] = u32::MAX;
            self.spare.push(s);
        }
    }

    /// The search's state for `place`'s tile.
    fn at(&mut self, place: Place) -> &mut TileState {
        let mut slot = self.slot[place.tile];
        if slot == u32::MAX {
            let mut s = self.spare.pop().unwrap_or_default();
            s.tile = place.tile;
            s.cost.clear();
            s.cost.resize(place.floors, u32::MAX);
            s.from.clear();
            s.from.resize(place.floors, 0);
            slot = self.state.len() as u32;
            self.slot[place.tile] = slot;
            self.state.push(s);
        }
        &mut self.state[slot as usize]
    }
}

/// A floor's place in the open list, smallest first: its cost so far plus the estimate left, then
/// its cost so far inverted, so a tie goes to the floor farther along. One word, so the heap moves
/// little.
fn rank(cost: u32, estimate: u32) -> u64 {
    u64::from(cost.saturating_add(estimate)) << 32 | u64::from(u32::MAX - cost)
}

/// A* from `start` to `goal`, looking at no more than `budget` floors; also says how many it
/// looked at. Ties go to the floor farther along, which keeps a search over open ground near its
/// straight line. Every floor it looks at is one a unit can stand on (the start is snapped to
/// one, and only such floors are opened), so each has a steppable floor in all eight neighbours
/// and a diagonal step never cuts a corner.
fn search(
    ground: &mut Cached<'_>,
    scratch: &mut Scratch,
    start: Node,
    goal: Node,
    budget: usize,
) -> (Option<Vec<Node>>, usize) {
    let map = ground.map;
    let grid = map.grid;
    let ((gx, gy), (sx, sy)) = (map.node_cell(goal), map.node_cell(start));
    scratch.reset(ground.walk.tiles());
    let Some((place, _)) = ground.place_of(sx, sy, node_level(start)) else {
        return (None, 0);
    };
    scratch.at(place).cost[place.floor] = 0;
    scratch
        .open
        .push(Reverse((rank(0, octile(sx - gx, sy - gy)), start)));
    let mut looked = 0usize;
    while let Some(Reverse((key, here))) = scratch.open.pop() {
        let g = u32::MAX - key as u32;
        if here == goal {
            return (Some(walk_back(ground, scratch, start, goal)), looked);
        }
        let (ix, iy) = map.node_cell(here);
        let Some((place, z)) = ground.place_of(ix, iy, node_level(here)) else {
            continue;
        };
        if scratch.at(place).cost[place.floor] < g {
            continue;
        }
        if looked >= budget {
            return (None, looked);
        }
        looked += 1;
        ground.each_step(ix, iy, z, |way, foot, place| {
            let (dx, dy) = NEIGHBOURS[way];
            let (nx, ny) = (ix + dx, iy + dy);
            let ng = g + if dx != 0 && dy != 0 {
                DIAGONAL
            } else {
                STRAIGHT
            };
            let s = scratch.at(place);
            if ng < s.cost[place.floor] {
                s.cost[place.floor] = ng;
                s.from[place.floor] = (way * 8 + node_level(here)) as u8;
                let next = node(grid.index(nx as u32, ny as u32), foot.level);
                let key = rank(ng, octile(nx - gx, ny - gy));
                scratch.open.push(Reverse((key, next)));
            }
        });
    }
    (None, looked)
}

/// The floors' points, straightened: from each point, the farthest later one (within
/// [`LEG_CELLS`]) a unit can walk to in a straight line. The first point is left out.
fn smooth(ground: &mut impl Ground, nodes: &[Node]) -> Vec<V3> {
    let points: Vec<V3> = nodes.iter().map(|&n| ground.map().node_point(n)).collect();
    if points.len() == 1 {
        return points;
    }
    let mut out = Vec::new();
    let mut anchor = 0;
    while anchor + 1 < points.len() {
        let mut reach = anchor + 1;
        while reach + 1 < points.len()
            && reach + 1 - anchor <= LEG_CELLS
            && walk_line(ground, points[anchor], points[reach + 1])
        {
            reach += 1;
        }
        out.push(points[reach]);
        anchor = reach;
    }
    out
}

/// [`HeightMap::walk_line`] over any [`Ground`].
fn walk_line(ground: &mut impl Ground, a: V3, b: V3) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let grid = ground.map().grid;
    let d = b.ground() - a.ground();
    let steps = ((d.length() / (grid.cell * 0.5)).ceil() as usize).max(1);
    let mut z = a.z;
    for k in 1..=steps {
        let p = a.ground() + d * (k as f32 / steps as f32);
        let Some((ix, iy)) = grid.column_of(p) else {
            return false;
        };
        let (ix, iy) = (i64::from(ix), i64::from(iy));
        let Some(foot) = ground.foothold(ix, iy, z, MAX_STEP) else {
            return false;
        };
        if !foot.stands {
            return false;
        }
        z = foot.z;
    }
    (z - b.z).abs() <= MAX_STEP
}

/// The floors from `start` to `goal` by the steps A* left.
fn walk_back(ground: &mut Cached<'_>, scratch: &mut Scratch, start: Node, goal: Node) -> Vec<Node> {
    let grid = ground.map.grid;
    let mut out = vec![goal];
    let mut at = goal;
    while at != start {
        let (ix, iy) = ground.map.node_cell(at);
        let Some((place, _)) = ground.place_of(ix, iy, node_level(at)) else {
            break;
        };
        let from = scratch.at(place).from[place.floor] as usize;
        let (dx, dy) = NEIGHBOURS[from / 8];
        let (px, py) = (ix - dx, iy - dy);
        at = node(grid.index(px as u32, py as u32), from % 8);
        out.push(at);
    }
    out.reverse();
    out
}

/// What [`Navigator::plan_path`] came to.
#[derive(Clone, Debug, PartialEq)]
pub enum PathPlan {
    /// The points to walk through, as [`Navigator::find_path`] gives them.
    Found(Arc<[V3]>),
    /// Searched now, and no path: an end has no walkable floor near it, or the budget ran out.
    NoPath,
    /// A search from near here to near there found nothing a moment ago, with at least this
    /// budget: it is not made again yet.
    KnownNoPath,
    /// This step's floors are spent: ask again next step. Nothing was searched; not a failure.
    Later,
}

/// Where a failed search's ends stood: their [`FAIL_SQUARE`]s and storeys.
type FailKey = [i64; 6];

/// Paths over one height map, with the ones asked for again kept.
pub struct Navigator {
    map: Arc<HeightMap>,
    walk: Walk,
    cache: HashMap<(Node, Node), Arc<[V3]>>,
    scratch: Scratch,
    /// The searches that found nothing lately: until which step each is remembered, and its
    /// budget.
    failed: FxMap<FailKey, (u64, usize)>,
    /// Steps begun ([`Navigator::begin_step`]).
    step: u64,
    /// Floors this step's searches may still look at.
    step_left: usize,
    /// Floors looked at by every search so far.
    looked: u64,
}

impl Navigator {
    pub fn new(map: Arc<HeightMap>) -> Self {
        Self {
            walk: Walk::new(&map),
            map,
            cache: HashMap::new(),
            scratch: Scratch::default(),
            failed: FxMap::default(),
            step: 0,
            step_left: usize::MAX,
            looked: 0,
        }
    }

    pub fn map(&self) -> &Arc<HeightMap> {
        &self.map
    }

    /// How many paths are kept.
    pub fn cached(&self) -> usize {
        self.cache.len()
    }

    /// Floors looked at by every search so far: what path finding has cost, counted rather than
    /// timed, so tests can pin it.
    pub fn floors_looked_at(&self) -> u64 {
        self.looked
    }

    /// Tiles of floors worked out so far (each 32 by 32 columns): what a search's first visit to
    /// a place costs on top of its floors.
    pub fn tiles_built(&self) -> u64 {
        self.walk.built()
    }

    /// A new step: [`Navigator::plan_path`]'s searches share `floors` until the next.
    pub fn begin_step(&mut self, floors: usize) {
        self.step += 1;
        self.step_left = floors;
        if self.failed.len() > FAIL_CAP {
            let now = self.step;
            self.failed.retain(|_, &mut (until, _)| until > now);
        }
    }

    /// A path from `from` to `to`: the points to walk through, the start left out and the goal's
    /// floor last; `None` when either end has no walkable floor near it or no path is found
    /// within `budget` floors. With `cache`, the path is kept and handed out again for the same
    /// start and goal floors (spec §4.2: a patrol's legs). Searched whatever the step's budget
    /// and the failures remembered: those are [`Navigator::plan_path`]'s.
    pub fn find_path(&mut self, from: V3, to: V3, cache: bool, budget: usize) -> Option<Arc<[V3]>> {
        let (start, goal) = self.ends(from, to)?;
        if cache {
            if let Some(path) = self.cache.get(&(start, goal)) {
                return Some(path.clone());
            }
        }
        self.run(start, goal, cache, budget)
    }

    /// [`Navigator::find_path`] for an enemy's plan: a kept path is handed out at once; a search
    /// that found nothing a moment ago from near here to near there ([`FAIL_SQUARE`],
    /// [`FAIL_STEPS`]) is not made again; and a search is made only while the step's floors
    /// ([`Navigator::begin_step`]) still cover its whole `budget`, so it is never cut short by
    /// the step and one step never looks at more floors than it was given.
    pub fn plan_path(&mut self, from: V3, to: V3, cache: bool, budget: usize) -> PathPlan {
        let Some((start, goal)) = self.ends(from, to) else {
            return PathPlan::NoPath;
        };
        if cache {
            if let Some(path) = self.cache.get(&(start, goal)) {
                return PathPlan::Found(path.clone());
            }
        }
        let key = self.fail_key(start, goal);
        if let Some(&(until, failed)) = self.failed.get(&key) {
            if until > self.step && budget <= failed {
                return PathPlan::KnownNoPath;
            }
        }
        if budget > self.step_left {
            return PathPlan::Later;
        }
        match self.run(start, goal, cache, budget) {
            Some(path) => PathPlan::Found(path),
            None => {
                self.failed.insert(key, (self.step + FAIL_STEPS, budget));
                PathPlan::NoPath
            }
        }
    }

    /// [`HeightMap::stand_near`], read through the tiles.
    pub fn stand_near(&mut self, p: V3) -> Option<V3> {
        self.walk.trim();
        let mut ground = Cached {
            map: &self.map,
            walk: &mut self.walk,
        };
        stand_at(&mut ground, p)
    }

    /// The start's and goal's walkable floors.
    fn ends(&mut self, from: V3, to: V3) -> Option<(Node, Node)> {
        self.walk.trim();
        let mut ground = Cached {
            map: &self.map,
            walk: &mut self.walk,
        };
        Some((snap(&mut ground, from)?, snap(&mut ground, to)?))
    }

    /// Searches and smooths a path, keeping it with `cache`, and counts what it looked at.
    fn run(&mut self, start: Node, goal: Node, cache: bool, budget: usize) -> Option<Arc<[V3]>> {
        let mut ground = Cached {
            map: &self.map,
            walk: &mut self.walk,
        };
        let (nodes, looked) = search(&mut ground, &mut self.scratch, start, goal, budget);
        self.looked += looked as u64;
        self.step_left = self.step_left.saturating_sub(looked);
        let path: Arc<[V3]> = smooth(&mut ground, &nodes?).into();
        if cache {
            if self.cache.len() >= CACHE_CAP {
                self.cache.clear();
            }
            self.cache.insert((start, goal), path.clone());
        }
        Some(path)
    }

    fn fail_key(&self, start: Node, goal: Node) -> FailKey {
        let at = |n: Node| {
            let (ix, iy) = self.map.node_cell(n);
            let storey = (self.map.node_z(n) / HEADROOM).floor() as i64;
            [
                ix.div_euclid(FAIL_SQUARE),
                iy.div_euclid(FAIL_SQUARE),
                storey,
            ]
        };
        let ([a, b, c], [d, e, f]) = (at(start), at(goal));
        [a, b, c, d, e, f]
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

    /// Flat ground 400 yd across with a closed house (no door) over [200, 210]².
    fn sealed() -> Navigator {
        let mut b = flat(400.0);
        house(
            &mut b,
            V2::new(200.0, 200.0),
            V2::new(210.0, 210.0),
            None,
            false,
        );
        nav(b)
    }

    const INSIDE: V3 = V3::new(205.0, 205.0, 0.0);

    #[test]
    fn a_sealed_goal_costs_its_budget_once_then_is_remembered_a_while() {
        let mut n = sealed();
        let from = V3::new(150.0, 150.0, 0.0);
        n.begin_step(usize::MAX);
        assert_eq!(n.plan_path(from, INSIDE, true, 30_000), PathPlan::NoPath);
        assert_eq!(n.floors_looked_at(), 30_000);
        // The same ends, a groupmate a yard off, or a smaller budget: told at once, unsearched.
        assert_eq!(
            n.plan_path(from, INSIDE, true, 30_000),
            PathPlan::KnownNoPath
        );
        let near = V3::new(150.5, 151.0, 0.0);
        assert_eq!(
            n.plan_path(near, INSIDE, true, 4_000),
            PathPlan::KnownNoPath
        );
        assert_eq!(n.floors_looked_at(), 30_000);
        // A bigger budget is a different question.
        assert_eq!(n.plan_path(from, INSIDE, false, 31_000), PathPlan::NoPath);
        assert_eq!(n.floors_looked_at(), 61_000);
        // Far off, the failure says nothing.
        assert_eq!(
            n.plan_path(V3::new(50.0, 50.0, 0.0), INSIDE, false, 4_000),
            PathPlan::NoPath
        );
        assert_eq!(n.floors_looked_at(), 65_000);
        // Once forgotten, it is searched again.
        for _ in 0..FAIL_STEPS {
            n.begin_step(usize::MAX);
        }
        assert_eq!(n.plan_path(from, INSIDE, true, 30_000), PathPlan::NoPath);
        assert_eq!(n.floors_looked_at(), 95_000);
    }

    #[test]
    fn a_search_waits_for_a_step_that_can_pay_for_all_of_it() {
        let mut n = sealed();
        let (from, open) = (V3::new(150.0, 150.0, 0.0), V3::new(160.0, 150.0, 0.0));
        let leg = n.find_path(from, open, true, BIG).unwrap();
        let spent = n.floors_looked_at();
        n.begin_step(5_000);
        assert_eq!(n.plan_path(from, INSIDE, false, 4_000), PathPlan::NoPath);
        // 1 000 floors are left: a 4 000 search waits, unsearched and not failed.
        let other = V3::new(100.0, 100.0, 0.0);
        assert_eq!(n.plan_path(other, INSIDE, false, 4_000), PathPlan::Later);
        assert_eq!(n.floors_looked_at(), spent + 4_000);
        // A kept path costs nothing, so it is handed out whatever is left.
        assert_eq!(n.plan_path(from, open, true, 4_000), PathPlan::Found(leg));
        n.begin_step(5_000);
        assert_eq!(n.plan_path(other, INSIDE, false, 4_000), PathPlan::NoPath);
    }

    #[test]
    fn a_failed_search_works_out_each_tile_once() {
        let mut n = sealed();
        let from = V3::new(150.0, 150.0, 0.0);
        assert!(n.find_path(from, INSIDE, false, 30_000).is_none());
        // 30 000 floors are about 30 tiles of 1 024; the frontier and the snap add a ring.
        let tiles = n.tiles_built();
        assert!(tiles <= 60, "{tiles} tiles");
        assert!(n.find_path(from, INSIDE, false, 30_000).is_none());
        assert_eq!(n.tiles_built(), tiles);
    }

    #[test]
    fn the_tiles_give_the_paths_the_map_gives() {
        // The search reads the tiles; `walk_line` on the map itself reads the spans: every leg of
        // a path through a doorway and round a house must check out on both.
        let mut b = flat(80.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            true,
        );
        house(
            &mut b,
            V2::new(40.0, 40.0),
            V2::new(55.0, 52.0),
            None,
            false,
        );
        let mut n = nav(b);
        let from = V3::new(10.0, 25.0, 0.0);
        for to in [
            V3::new(25.0, 25.0, 0.0),
            V3::new(70.0, 70.0, 0.0),
            V3::new(60.0, 45.0, 0.0),
        ] {
            let path = n.find_path(from, to, false, BIG).unwrap();
            let mut at = from;
            for &p in path.iter() {
                assert!(n.map().walk_line(at, p), "{at:?} -> {p:?}");
                at = p;
            }
        }
    }
}
