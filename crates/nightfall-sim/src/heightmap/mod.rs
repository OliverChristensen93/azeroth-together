//! The baked height map: what the host knows of the world's solid shape, in the style of
//! Recast's heightfield (spec §4.1). The ground plane is cut into square columns ([`CELL`]
//! yards); each column holds its solid spans, bottom first. The top of a span with [`HEADROOM`]
//! of air over it is a floor, so a column can hold several: a house's two storeys, a bridge over
//! a road, a crypt under a hill. Line of sight and paths run on it (later modules).
//!
//! The terrain is a shell [`TERRAIN_DEPTH`] yards thick under its surface rather than solid to
//! the bottom of the world, so a cave dug under a hillside keeps its air. Most columns are
//! terrain alone and are stored as one quantized height; the few with anything else on them
//! (walls, floors, trunks, rocks) keep their whole span list. `nightfall-bake` builds a map from
//! the player's WoW data; tests build small ones by hand with [`HeightMapBuilder`].

mod build;
mod codec;
mod path;
mod sight;
#[cfg(test)]
pub(crate) mod testmap;

use std::fmt;

pub use build::HeightMapBuilder;
pub use path::Navigator;

use crate::V2;

/// Yards across a column: a doorway is four or five of them, and Duskwood's bounds are 17
/// million.
pub const CELL: f32 = 0.5;
/// How deep under its surface the terrain is solid. Not to the bottom of the world: a crypt or a
/// mine dug under a hillside keeps its air, and no ray reaches this deep except through a hole.
pub const TERRAIN_DEPTH: f32 = 2.0;
/// Solids closer than this (yards) are one: a floor laid on the ground, a wall standing on it.
pub const MERGE_GAP: f32 = 0.2;
/// Air a unit needs over a floor to stand on it, yards.
pub const HEADROOM: f32 = 2.0;
/// The highest step a unit takes from one column to the next, yards: stairs and slopes up to
/// about 55°, never a ledge or a table.
pub const MAX_STEP: f32 = 0.7;
/// The most floors a column reports; more is a stack of thin slabs, not storeys.
pub const MAX_FLOORS: usize = 8;

/// Terrain heights are kept in eighths of a yard as `i16`: two bytes a column instead of eight,
/// across 17 million columns.
const Z_SCALE: f32 = 8.0;
/// A column with no terrain: a hole (a cave's mouth), or off every tile.
const NO_GROUND: i16 = i16::MIN;

/// Solid from `lo` to `hi`, yards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: f32,
    pub hi: f32,
}

/// Where the columns lie: column `(ix, iy)` covers `x` from `origin.x + ix·cell` and `y` from
/// `origin.y + iy·cell`, one `cell` each way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub origin: V2,
    pub cell: f32,
    pub nx: u32,
    pub ny: u32,
}

impl Grid {
    pub fn columns(&self) -> usize {
        self.nx as usize * self.ny as usize
    }

    /// The column holding ground point `p`, if any.
    pub fn column_of(&self, p: V2) -> Option<(u32, u32)> {
        if !p.is_finite() {
            return None;
        }
        let fx = ((p.x - self.origin.x) / self.cell).floor();
        let fy = ((p.y - self.origin.y) / self.cell).floor();
        if fx < 0.0 || fy < 0.0 || fx >= self.nx as f32 || fy >= self.ny as f32 {
            return None;
        }
        Some((fx as u32, fy as u32))
    }

    /// Column `(ix, iy)`'s index, rows of `nx` columns.
    pub fn index(&self, ix: u32, iy: u32) -> usize {
        iy as usize * self.nx as usize + ix as usize
    }

    /// The column at an index, as `(ix, iy)`.
    pub fn cell_of(&self, col: usize) -> (u32, u32) {
        let nx = self.nx as usize;
        ((col % nx) as u32, (col / nx) as u32)
    }

    /// The middle of column `(ix, iy)`.
    pub fn center(&self, ix: u32, iy: u32) -> V2 {
        V2::new(
            self.origin.x + (ix as f32 + 0.5) * self.cell,
            self.origin.y + (iy as f32 + 0.5) * self.cell,
        )
    }

    /// Column `(ix, iy)`'s index, or `None` off the grid.
    pub(crate) fn at(&self, ix: i64, iy: i64) -> Option<usize> {
        (ix >= 0 && iy >= 0 && ix < self.nx as i64 && iy < self.ny as i64)
            .then(|| self.index(ix as u32, iy as u32))
    }
}

/// A terrain height as stored; not a number is no terrain.
fn quantize(z: f32) -> i16 {
    if z.is_finite() {
        (z * Z_SCALE).round().clamp(-32767.0, 32767.0) as i16
    } else {
        NO_GROUND
    }
}

fn dequantize(q: i16) -> Option<f32> {
    (q != NO_GROUND).then(|| f32::from(q) / Z_SCALE)
}

/// A column's solid spans, bottom first: its stored list, or the terrain's shell alone.
pub struct SpanList<'a> {
    one: [Span; 1],
    many: Option<&'a [Span]>,
    has_one: bool,
}

impl SpanList<'_> {
    pub fn as_slice(&self) -> &[Span] {
        match self.many {
            Some(many) => many,
            None if self.has_one => &self.one,
            None => &[],
        }
    }
}

/// A column's floors, bottom first: the tops of its spans with [`HEADROOM`] over them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Floors {
    len: usize,
    z: [f32; MAX_FLOORS],
}

impl Floors {
    pub fn as_slice(&self) -> &[f32] {
        &self.z[..self.len]
    }

    fn push(&mut self, z: f32) {
        if self.len < MAX_FLOORS {
            self.z[self.len] = z;
            self.len += 1;
        }
    }
}

/// The world's solid shape over a [`Grid`].
#[derive(PartialEq)]
pub struct HeightMap {
    grid: Grid,
    /// Each column's terrain height, quantized ([`quantize`]).
    ground: Vec<i16>,
    /// The columns with more than terrain, ascending, and where each one's spans start in
    /// `spans` (one more entry than `cols`, the last the end).
    cols: Vec<u32>,
    starts: Vec<u32>,
    spans: Vec<Span>,
    /// One bit a column: no unit may walk into it.
    blocked: Vec<u64>,
}

// Written out: a derived `Debug` would print 17 million heights into a failing test's output.
impl fmt::Debug for HeightMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeightMap")
            .field("grid", &self.grid)
            .field("complex_columns", &self.cols.len())
            .field("spans", &self.spans.len())
            .finish()
    }
}

impl HeightMap {
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// How many columns hold more than terrain.
    pub fn complex_columns(&self) -> usize {
        self.cols.len()
    }

    fn col_at(&self, p: V2) -> Option<usize> {
        let (ix, iy) = self.grid.column_of(p)?;
        Some(self.grid.index(ix, iy))
    }

    /// Column `col`'s solid spans, bottom first; none off the grid.
    pub fn spans_at(&self, col: usize) -> SpanList<'_> {
        let none = Span { lo: 0.0, hi: 0.0 };
        if let Ok(k) = self.cols.binary_search(&(col as u32)) {
            let (a, b) = (self.starts[k] as usize, self.starts[k + 1] as usize);
            return SpanList {
                one: [none],
                many: Some(&self.spans[a..b]),
                has_one: false,
            };
        }
        match self.ground.get(col).copied().and_then(dequantize) {
            Some(g) => SpanList {
                one: [Span {
                    lo: g - TERRAIN_DEPTH,
                    hi: g,
                }],
                many: None,
                has_one: true,
            },
            None => SpanList {
                one: [none],
                many: None,
                has_one: false,
            },
        }
    }

    /// Column `col`'s floors, bottom first.
    pub fn floors_at(&self, col: usize) -> Floors {
        let list = self.spans_at(col);
        let spans = list.as_slice();
        let mut floors = Floors::default();
        for (i, s) in spans.iter().enumerate() {
            let room = spans
                .get(i + 1)
                .map_or(f32::INFINITY, |above| above.lo - s.hi);
            if room >= HEADROOM {
                floors.push(s.hi);
            }
        }
        floors
    }

    /// Whether no unit may walk into column `col`; every column off the grid is.
    pub fn is_blocked(&self, col: usize) -> bool {
        col >= self.grid.columns() || self.blocked[col / 64] >> (col % 64) & 1 == 1
    }

    /// The terrain's height under `p` (not what stands on it); `None` off the map or in a hole.
    pub fn ground_at(&self, p: V2) -> Option<f32> {
        dequantize(self.ground[self.col_at(p)?])
    }

    /// The floor of column `col` nearest `z` and within `reach` of it, as (which floor, bottom
    /// first; its height). A tie goes to the lower floor.
    pub(crate) fn level_within(&self, col: usize, z: f32, reach: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (level, &f) in self.floors_at(col).as_slice().iter().enumerate() {
            let d = (f - z).abs();
            if d <= reach && best.is_none_or(|(_, b)| d < (b - z).abs()) {
                best = Some((level, f));
            }
        }
        best
    }

    /// The floor under `p` nearest height `z`, if one is within `reach`.
    pub fn floor_within(&self, p: V2, z: f32, reach: f32) -> Option<f32> {
        if !z.is_finite() {
            return None;
        }
        self.level_within(self.col_at(p)?, z, reach).map(|(_, f)| f)
    }

    /// The floor under `p` nearest height `z`: for placing a unit.
    pub fn floor_near(&self, p: V2, z: f32) -> Option<f32> {
        self.floor_within(p, z, f32::INFINITY)
    }

    /// The highest floor under `p` that a unit at height `z` stands on or over (a step up
    /// included).
    pub fn floor_below(&self, p: V2, z: f32) -> Option<f32> {
        let col = self.col_at(p)?;
        self.floors_at(col)
            .as_slice()
            .iter()
            .copied()
            .rfind(|&f| f <= z + MAX_STEP)
    }

    /// Whether something solid stands over the floor a unit at `p`, height `z`, stands on: a
    /// roof, an upper storey, a bridge (spec §4.2; noise through walls and rain will use it).
    pub fn is_indoors(&self, p: V2, z: f32) -> bool {
        let (Some(col), Some(floor)) = (self.col_at(p), self.floor_below(p, z)) else {
            return false;
        };
        self.spans_at(col).as_slice().iter().any(|s| s.lo > floor)
    }
}

#[cfg(test)]
mod tests {
    use super::testmap::{flat, house};
    use super::*;

    fn col(m: &HeightMap, x: f32, y: f32) -> usize {
        let g = m.grid();
        let (ix, iy) = g.column_of(V2::new(x, y)).unwrap();
        g.index(ix, iy)
    }

    #[test]
    fn a_flat_map_has_one_floor_everywhere() {
        let m = flat(20.0).finish();
        let p = V2::new(5.0, 5.0);
        assert_eq!(m.ground_at(p), Some(0.0));
        assert_eq!(m.floor_near(p, 7.0), Some(0.0));
        assert_eq!(m.floor_below(p, 0.0), Some(0.0));
        assert_eq!(m.floor_below(p, -1.0), None);
        assert_eq!(m.floors_at(col(&m, 5.0, 5.0)).as_slice(), &[0.0]);
        assert_eq!(m.complex_columns(), 0);
        assert!(!m.is_indoors(p, 0.0));
    }

    #[test]
    fn a_slab_overhead_makes_a_second_storey() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(10.0, 10.0), 3.0, 3.2);
        let m = b.finish();
        let p = V2::new(7.0, 7.0);
        assert_eq!(m.floors_at(col(&m, 7.0, 7.0)).as_slice(), &[0.0, 3.2]);
        assert_eq!(m.floor_near(p, 3.0), Some(3.2));
        assert_eq!(m.floor_below(p, 2.0), Some(0.0));
        assert_eq!(m.floor_within(p, 1.8, 1.0), None);
        assert!(m.is_indoors(p, 0.0));
        assert!(!m.is_indoors(p, 3.2));
    }

    #[test]
    fn a_low_ceiling_leaves_no_room_to_stand_under_it() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(10.0, 10.0), 1.0, 1.2);
        let m = b.finish();
        assert_eq!(m.floors_at(col(&m, 7.0, 7.0)).as_slice(), &[1.2]);
    }

    #[test]
    fn touching_solids_merge_into_one() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(5.5, 5.5), 0.0, 1.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(5.5, 5.5), 1.1, 2.0);
        let m = b.finish();
        let c = col(&m, 5.25, 5.25);
        assert_eq!(
            m.spans_at(c).as_slice(),
            &[Span {
                lo: -TERRAIN_DEPTH,
                hi: 2.0
            }]
        );
        assert_eq!(m.floors_at(c).as_slice(), &[2.0]);
    }

    #[test]
    fn the_order_things_are_added_in_does_not_matter() {
        let mut a = flat(10.0);
        a.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 3.0, 3.2);
        a.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 0.0, 1.0);
        let mut b = flat(10.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 0.0, 1.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 3.0, 3.2);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn heights_are_kept_to_an_eighth_of_a_yard() {
        let mut b = flat(4.0);
        b.set_ground(0, 1.03);
        let m = b.finish();
        let g = m.ground_at(V2::new(0.25, 0.25)).unwrap();
        assert!((g - 1.03).abs() <= 1.0 / 16.0, "{g}");
    }

    #[test]
    fn a_roofed_house_is_indoors_and_its_yard_is_not() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            true,
        );
        let m = b.finish();
        assert!(m.is_indoors(V2::new(25.0, 25.0), 0.0));
        assert!(!m.is_indoors(V2::new(10.0, 10.0), 0.0));
    }

    #[test]
    fn off_the_map_there_is_nothing_to_stand_on() {
        let m = flat(20.0).finish();
        assert_eq!(m.floor_near(V2::new(-1.0, 5.0), 0.0), None);
        assert_eq!(m.ground_at(V2::new(5.0, 25.0)), None);
        assert!(!m.is_indoors(V2::new(-1.0, 5.0), 0.0));
        assert!(m.is_blocked(m.grid().columns()));
        assert_eq!(m.floor_near(V2::new(f32::NAN, 5.0), 0.0), None);
        assert_eq!(m.floor_near(V2::new(5.0, 5.0), f32::NAN), None);
    }

    #[test]
    fn blocked_columns_are_remembered() {
        let mut b = flat(20.0);
        b.block_box(V2::new(0.0, 0.0), V2::new(1.0, 1.0));
        let m = b.finish();
        assert!(m.is_blocked(col(&m, 0.25, 0.75)));
        assert!(!m.is_blocked(col(&m, 1.25, 0.25)));
    }

    #[test]
    fn a_hole_in_the_ground_has_only_what_lies_under_it() {
        let mut b = flat(20.0);
        let c = b.grid().index(4, 4);
        b.set_ground(c, f32::NAN);
        b.add_span(c, -6.0, -5.5);
        let m = b.finish();
        assert_eq!(m.floors_at(c).as_slice(), &[-5.5]);
        assert_eq!(m.ground_at(V2::new(2.25, 2.25)), None);
    }

    #[test]
    fn a_box_covers_the_columns_whose_centres_it_holds() {
        let b = flat(10.0);
        let cols = b.columns_in(V2::new(1.0, 1.0), V2::new(2.0, 1.5));
        let g = b.grid();
        assert_eq!(cols, vec![g.index(2, 2), g.index(3, 2)]);
        assert!(b
            .columns_in(V2::new(-5.0, -5.0), V2::new(-1.0, -1.0))
            .is_empty());
    }
}
