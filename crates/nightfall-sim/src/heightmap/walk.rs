//! What a path search asks of the ground, column by column: the floor nearest a height, and
//! whether a unit can stand on it. [`HeightMap`] answers from its span list; [`Walk`] keeps the
//! answers a tile of columns at a time, worked out the first time a search goes there, so a
//! search's inner loop reads a small array instead of searching the map's span list (millions of
//! columns) nine times a neighbour.

use super::{dequantize, floors_of, HeightMap, MAX_FLOORS, MAX_STEP};

/// The eight neighbours, the four straight ones first.
pub(super) const NEIGHBOURS: [(i64, i64); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Columns along a tile's side.
const TILE: i64 = 32;
/// Columns in a tile.
const TILE_COLS: usize = (TILE * TILE) as usize;
/// Tiles kept before the cache starts over: about 7 KB each, so under 30 MB however far the
/// searches roam (Duskwood is 17 000 tiles).
const MAX_TILES: usize = 4096;

/// A column's floor nearest a height, as a search steps onto it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Foothold {
    /// Which of the column's floors, bottom first.
    pub(super) level: usize,
    pub(super) z: f32,
    /// Whether a unit can stand on it: every neighbour has a floor within [`MAX_STEP`] of it, so
    /// nothing it cannot step onto is within a column.
    pub(super) stands: bool,
}

/// The floors a search steps over, and which a unit can stand on.
pub(super) trait Ground {
    fn map(&self) -> &HeightMap;

    /// Column `(ix, iy)`'s floor nearest `z` within `reach` (a tie to the lower); `None` off the
    /// map, on a blocked column, or with no floor in reach.
    fn foothold(&mut self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<Foothold>;
}

impl HeightMap {
    /// Column `(ix, iy)`'s floor nearest `z` within `reach`, as (its level, its height); `None`
    /// off the map, on a blocked column, or with no floor in reach.
    pub(super) fn level_near(&self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<(usize, f32)> {
        let col = self.grid.at(ix, iy)?;
        if self.is_blocked(col) {
            return None;
        }
        self.level_within(col, z, reach)
    }
}

impl Ground for &HeightMap {
    fn map(&self) -> &HeightMap {
        self
    }

    fn foothold(&mut self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<Foothold> {
        let (level, z) = self.level_near(ix, iy, z, reach)?;
        Some(Foothold {
            level,
            z,
            stands: self.walkable(ix, iy, z),
        })
    }
}

/// The nearest of `floors` to `z` within `reach`, as [`HeightMap::level_near`] has it.
fn nearest(floors: &[f32], z: f32, reach: f32) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (level, &f) in floors.iter().enumerate() {
        let d = (f - z).abs();
        if d <= reach && best.is_none_or(|(_, b)| d < (b - z).abs()) {
            best = Some((level, f));
        }
    }
    best
}

/// The first index at or after `from` of the sorted `cols` whose column is `target` or more:
/// galloping, so a target a little way on costs a few looks near `from`, not a search of the
/// whole list.
fn seek(cols: &[u32], from: usize, target: usize) -> usize {
    let mut lo = from;
    let mut step = 1;
    loop {
        let hi = lo + step;
        if hi >= cols.len() || cols[hi] as usize >= target {
            let hi = hi.min(cols.len());
            return lo + cols[lo.min(hi)..hi].partition_point(|&m| (m as usize) < target);
        }
        lo = hi;
        step *= 2;
    }
}

/// One tile's columns: their floors (none on a blocked column) and which a unit can stand on.
struct Tile {
    /// Where each column's floors start in `z`, the columns row by row, and one more: the end.
    start: Box<[u16]>,
    z: Vec<f32>,
    /// Beside `z`: whether a unit can stand on that floor.
    stand: Vec<bool>,
}

impl Tile {
    fn floors(&self, local: usize) -> std::ops::Range<usize> {
        self.start[local] as usize..self.start[local + 1] as usize
    }
}

/// The tiles worked out so far over one map.
pub(super) struct Walk {
    tiles_x: i64,
    tiles_y: i64,
    tiles: Vec<Option<Box<Tile>>>,
    kept: usize,
    /// Tiles worked out, ever: what a search's first visit to a place costs, for tests.
    built: u64,
    /// A tile's floors with a column of its neighbours' all round, while it is worked out.
    halo: Vec<HaloColumn>,
}

impl Walk {
    pub(super) fn new(map: &HeightMap) -> Self {
        let tiles_x = i64::from(map.grid.nx).div_euclid(TILE) + 1;
        let tiles_y = i64::from(map.grid.ny).div_euclid(TILE) + 1;
        Self {
            tiles_x,
            tiles_y,
            tiles: (0..tiles_x * tiles_y).map(|_| None).collect(),
            kept: 0,
            built: 0,
            halo: Vec::new(),
        }
    }

    pub(super) fn built(&self) -> u64 {
        self.built
    }

    /// Forgets every tile once more than [`MAX_TILES`] are kept. Called between searches only,
    /// so a search never loses what it is reading.
    pub(super) fn trim(&mut self) {
        if self.kept > MAX_TILES {
            self.tiles.iter_mut().for_each(|t| *t = None);
            self.kept = 0;
        }
    }

    /// How many tiles the map has room for.
    pub(super) fn tiles(&self) -> usize {
        self.tiles.len()
    }

    /// The tile holding column `(ix, iy)`, worked out if it is not yet, its index, and the
    /// column's place in it; `None` off the map.
    fn tile(&mut self, map: &HeightMap, ix: i64, iy: i64) -> Option<(&Tile, usize, usize)> {
        map.grid.at(ix, iy)?;
        let (tx, ty) = (ix / TILE, iy / TILE);
        let t = (ty * self.tiles_x + tx) as usize;
        if self.tiles[t].is_none() {
            let tile = self.build(map, tx, ty);
            self.tiles[t] = Some(Box::new(tile));
            self.kept += 1;
            self.built += 1;
        }
        let local = ((iy - ty * TILE) * TILE + (ix - tx * TILE)) as usize;
        Some((self.tiles[t].as_deref()?, t, local))
    }

    /// Works out tile `(tx, ty)`: the floors of its columns and a ring of columns round it, read
    /// row by row (the map's complex columns are found once a row, galloping on from the row
    /// before, not searched for a column at a time), then which of its own floors a unit can
    /// stand on.
    fn build(&mut self, map: &HeightMap, tx: i64, ty: i64) -> Tile {
        debug_assert!(tx < self.tiles_x && ty < self.tiles_y);
        let g = &map.grid;
        let side = TILE + 2;
        let (x0, y0) = (tx * TILE - 1, ty * TILE - 1);
        self.halo.clear();
        let mut k = 0;
        for hy in 0..side {
            let iy = y0 + hy;
            if let Some(first) = (x0.max(0)..x0 + side).find_map(|ix| g.at(ix, iy)) {
                k = seek(&map.cols, k, first);
            }
            for hx in 0..side {
                let mut floors = HaloColumn::default();
                if let Some(col) = g.at(x0 + hx, iy) {
                    let complex = map.cols.get(k).is_some_and(|&m| m as usize == col);
                    if complex && !map.is_blocked(col) {
                        let (a, b) = (map.starts[k] as usize, map.starts[k + 1] as usize);
                        floors = HaloColumn::of(floors_of(&map.spans[a..b]).as_slice());
                    } else if !complex && !map.is_blocked(col) {
                        if let Some(z) = dequantize(map.ground[col]) {
                            floors = HaloColumn::of(&[z]);
                        }
                    }
                    if complex {
                        k += 1;
                    }
                }
                self.halo.push(floors);
            }
        }
        let halo = &self.halo;
        let mut start = Vec::with_capacity(TILE_COLS + 1);
        let mut z = Vec::with_capacity(TILE_COLS);
        let mut stand = Vec::with_capacity(TILE_COLS);
        for ly in 0..TILE {
            for lx in 0..TILE {
                start.push(z.len() as u16);
                let h = |dx: i64, dy: i64| &halo[((ly + 1 + dy) * side + lx + 1 + dx) as usize];
                let own = h(0, 0);
                if own.len == 0 {
                    continue;
                }
                let around = NEIGHBOURS.map(|(dx, dy)| h(dx, dy));
                for &f in &own.z[..own.len as usize] {
                    z.push(f);
                    stand.push(around.iter().all(|n| n.reaches(f)));
                }
            }
        }
        start.push(z.len() as u16);
        Tile {
            start: start.into_boxed_slice(),
            z,
            stand,
        }
    }
}

/// A column's floors while a tile is worked out: a fixed array (the rest no floor at all), so
/// the test for a floor within a step of another runs over all eight at once.
#[derive(Clone, Copy)]
struct HaloColumn {
    len: u8,
    z: [f32; MAX_FLOORS],
}

impl Default for HaloColumn {
    fn default() -> Self {
        Self {
            len: 0,
            z: [f32::INFINITY; MAX_FLOORS],
        }
    }
}

impl HaloColumn {
    fn of(floors: &[f32]) -> Self {
        let mut z = [f32::INFINITY; MAX_FLOORS];
        z[..floors.len()].copy_from_slice(floors);
        Self {
            len: floors.len() as u8,
            z,
        }
    }

    /// Whether one of these floors is within [`MAX_STEP`] of `f`.
    fn reaches(&self, f: f32) -> bool {
        self.z
            .iter()
            .fold(false, |any, &g| any | ((g - f).abs() <= MAX_STEP))
    }
}

/// A map read through its [`Walk`].
pub(super) struct Cached<'a> {
    pub(super) map: &'a HeightMap,
    pub(super) walk: &'a mut Walk,
}

impl Ground for Cached<'_> {
    fn map(&self) -> &HeightMap {
        self.map
    }

    fn foothold(&mut self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<Foothold> {
        self.place_near(ix, iy, z, reach).map(|(foot, _)| foot)
    }
}

/// Where a floor is kept in the [`Walk`]: a search keeps its own state in arrays beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Place {
    /// The tile's index.
    pub(super) tile: usize,
    /// The floor's index among the tile's floors.
    pub(super) floor: usize,
    /// How many floors the tile has.
    pub(super) floors: usize,
}

impl Cached<'_> {
    /// The floors a unit standing at height `z` in column `(ix, iy)` can step to and stand on, one
    /// for each neighbour that has one: `f` is given the neighbour's [`NEIGHBOURS`] index, the
    /// floor, and where it is kept. A column inside its tile reads its neighbours straight from
    /// the tile, the search's inner loop.
    pub(super) fn each_step(
        &mut self,
        ix: i64,
        iy: i64,
        z: f32,
        mut f: impl FnMut(usize, Foothold, Place),
    ) {
        let (tx, ty) = (ix.div_euclid(TILE), iy.div_euclid(TILE));
        let (lx, ly) = (ix - tx * TILE, iy - ty * TILE);
        let inside =
            ix >= 0 && iy >= 0 && (1..TILE - 1).contains(&lx) && (1..TILE - 1).contains(&ly);
        let t = (ty * self.walk.tiles_x + tx) as usize;
        if let Some(Some(tile)) = self.walk.tiles.get(t).filter(|_| inside) {
            let local = ly * TILE + lx;
            for (way, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
                let floors = tile.floors((local + dy * TILE + dx) as usize);
                let Some((level, z)) = nearest(&tile.z[floors.clone()], z, MAX_STEP) else {
                    continue;
                };
                let floor = floors.start + level;
                if tile.stand[floor] {
                    let foot = Foothold {
                        level,
                        z,
                        stands: true,
                    };
                    let place = Place {
                        tile: t,
                        floor,
                        floors: tile.z.len(),
                    };
                    f(way, foot, place);
                }
            }
            return;
        }
        for (way, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
            if let Some((foot, place)) = self.place_near(ix + dx, iy + dy, z, MAX_STEP) {
                if foot.stands {
                    f(way, foot, place);
                }
            }
        }
    }

    /// [`Ground::foothold`], and where the floor is kept.
    pub(super) fn place_near(
        &mut self,
        ix: i64,
        iy: i64,
        z: f32,
        reach: f32,
    ) -> Option<(Foothold, Place)> {
        let (tile, t, local) = self.walk.tile(self.map, ix, iy)?;
        let floors = tile.floors(local);
        let (level, z) = nearest(&tile.z[floors.clone()], z, reach)?;
        let floor = floors.start + level;
        let foot = Foothold {
            level,
            z,
            stands: tile.stand[floor],
        };
        let place = Place {
            tile: t,
            floor,
            floors: tile.z.len(),
        };
        Some((foot, place))
    }

    /// Where floor `level` of column `(ix, iy)` is kept, and its height.
    pub(super) fn place_of(&mut self, ix: i64, iy: i64, level: usize) -> Option<(Place, f32)> {
        let (tile, t, local) = self.walk.tile(self.map, ix, iy)?;
        let floors = tile.floors(local);
        let floor = floors.start + level;
        (floor < floors.end).then(|| {
            let place = Place {
                tile: t,
                floor,
                floors: tile.z.len(),
            };
            (place, tile.z[floor])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::testmap::{flat, house};
    use super::*;
    use crate::V2;

    /// The tiles answer every column of a map with stairs, a house, a slab, a hole and blocked
    /// columns exactly as the map does, tile edges and the map's own edge included.
    #[test]
    fn the_tiles_answer_as_the_map_does() {
        let mut b = flat(40.0);
        house(
            &mut b,
            V2::new(10.0, 10.0),
            V2::new(20.0, 20.0),
            Some(15.0),
            true,
        );
        for k in 0..6 {
            let x = 22.0 + 0.5 * k as f32;
            b.add_box(
                V2::new(x, 22.0),
                V2::new(x + 0.5, 24.0),
                0.0,
                0.5 * (k + 1) as f32,
            );
        }
        b.add_box(V2::new(25.0, 22.0), V2::new(30.0, 30.0), 2.8, 3.0);
        b.block_box(V2::new(31.0, 2.0), V2::new(33.5, 4.0));
        let hole = b.grid().index(70, 5);
        b.set_ground(hole, f32::NAN);
        b.add_span(hole, -6.0, -5.5);
        let map = b.finish();
        let mut walk = Walk::new(&map);
        let mut cached = Cached {
            map: &map,
            walk: &mut walk,
        };
        let mut plain = &map;
        let n = i64::from(map.grid().nx);
        for iy in -1..=n {
            for ix in -1..=n {
                for z in [-5.5, 0.0, 0.4, 1.5, 3.0, 3.3] {
                    for reach in [MAX_STEP, 1.5] {
                        assert_eq!(
                            cached.foothold(ix, iy, z, reach),
                            plain.foothold(ix, iy, z, reach),
                            "{ix} {iy} {z}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn seeking_finds_the_first_column_at_or_past_the_target() {
        let cols = [2u32, 5, 9, 10, 40, 41, 300];
        for from in 0..=cols.len() {
            for target in 0..310 {
                let want = cols.partition_point(|&m| (m as usize) < target).max(from);
                assert_eq!(seek(&cols, from, target), want, "{from} {target}");
            }
        }
        assert_eq!(seek(&[], 0, 5), 0);
    }

    #[test]
    fn too_many_tiles_start_over() {
        let map = flat(1100.0).finish();
        let mut walk = Walk::new(&map);
        let mut cached = Cached {
            map: &map,
            walk: &mut walk,
        };
        let n = i64::from(map.grid().nx);
        for iy in (0..n).step_by(TILE as usize) {
            for ix in (0..n).step_by(TILE as usize) {
                cached.foothold(ix, iy, 0.0, MAX_STEP);
            }
        }
        assert!(walk.kept > MAX_TILES);
        walk.trim();
        assert_eq!(walk.kept, 0);
        let mut cached = Cached {
            map: &map,
            walk: &mut walk,
        };
        assert!(cached
            .foothold(5, 5, 0.0, MAX_STEP)
            .is_some_and(|f| f.stands));
    }
}
