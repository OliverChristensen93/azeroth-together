//! Building a [`HeightMap`]: terrain heights and solid spans in any order, merged at the end.

use super::{dequantize, quantize, Grid, HeightMap, Span, MERGE_GAP, TERRAIN_DEPTH};
use crate::V2;

/// A height map under construction. Spans are kept as they come (a bake adds millions, many over
/// the same column) and merged per column in [`HeightMapBuilder::finish`], so the order things
/// are added in never changes the map.
pub struct HeightMapBuilder {
    grid: Grid,
    /// Each column's terrain height; not a number for none.
    ground: Vec<f32>,
    raw: Vec<(u32, Span)>,
    blocked: Vec<u64>,
}

impl HeightMapBuilder {
    /// A grid with no terrain and nothing on it.
    pub fn new(grid: Grid) -> Self {
        let n = grid.columns();
        Self {
            grid,
            ground: vec![f32::NAN; n],
            raw: Vec::new(),
            blocked: vec![0; n.div_ceil(64)],
        }
    }

    /// A grid of flat terrain at height `z`.
    pub fn flat(grid: Grid, z: f32) -> Self {
        let mut b = Self::new(grid);
        b.ground.fill(z);
        b
    }

    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Column `col`'s terrain height; not a number takes its terrain away (a hole).
    pub fn set_ground(&mut self, col: usize, z: f32) {
        if let Some(g) = self.ground.get_mut(col) {
            *g = z;
        }
    }

    /// Solid from `lo` to `hi` (either order) in column `col`. A span that is not a number, or a
    /// column off the grid, is dropped: a broken model must not break the map.
    pub fn add_span(&mut self, col: usize, lo: f32, hi: f32) {
        if col >= self.ground.len() || !lo.is_finite() || !hi.is_finite() {
            return;
        }
        let span = Span {
            lo: lo.min(hi),
            hi: lo.max(hi),
        };
        self.raw.push((col as u32, span));
    }

    /// No unit may walk into column `col` (an impassable chunk, outside the run's bounds). It
    /// blocks nothing else: sight passes.
    pub fn block(&mut self, col: usize) {
        if let Some(w) = self.blocked.get_mut(col / 64) {
            *w |= 1 << (col % 64);
        }
    }

    /// The columns whose centres lie in `[min, max)`. A bound that is not finite (infinite, or not
    /// a number) selects nothing, like a span that is not a number: the box of a broken model must
    /// not fill the map.
    pub fn columns_in(&self, min: V2, max: V2) -> Vec<usize> {
        if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite()) {
            return Vec::new();
        }
        let g = self.grid;
        let range = |lo: f32, hi: f32, origin: f32, n: u32| {
            let first = ((lo - origin) / g.cell - 0.5).ceil().max(0.0) as i64;
            let last = (((hi - origin) / g.cell - 0.5).ceil() as i64 - 1).min(n as i64 - 1);
            first..=last
        };
        let mut out = Vec::new();
        for iy in range(min.y, max.y, g.origin.y, g.ny) {
            for ix in range(min.x, max.x, g.origin.x, g.nx) {
                out.push(g.index(ix as u32, iy as u32));
            }
        }
        out
    }

    /// Solid from `lo` to `hi` over every column whose centre is in `[min, max)`.
    pub fn add_box(&mut self, min: V2, max: V2, lo: f32, hi: f32) {
        for col in self.columns_in(min, max) {
            self.add_span(col, lo, hi);
        }
    }

    /// Every column whose centre is in `[min, max)` blocked.
    pub fn block_box(&mut self, min: V2, max: V2) {
        for col in self.columns_in(min, max) {
            self.block(col);
        }
    }

    /// The map: each column's terrain shell and spans sorted and merged where they touch
    /// ([`MERGE_GAP`]); a column that is terrain alone keeps only its height.
    pub fn finish(mut self) -> HeightMap {
        let ground: Vec<i16> = self.ground.iter().map(|&z| quantize(z)).collect();
        self.raw
            .sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.lo.total_cmp(&b.1.lo)));
        let (mut cols, mut starts, mut spans) = (Vec::new(), Vec::new(), Vec::new());
        let mut list: Vec<Span> = Vec::new();
        let mut i = 0;
        while i < self.raw.len() {
            let col = self.raw[i].0;
            list.clear();
            // The shell as stored, so a column's terrain is the same height whichever way it is
            // kept.
            if let Some(g) = dequantize(ground[col as usize]) {
                list.push(Span {
                    lo: g - TERRAIN_DEPTH,
                    hi: g,
                });
            }
            while i < self.raw.len() && self.raw[i].0 == col {
                list.push(self.raw[i].1);
                i += 1;
            }
            list.sort_unstable_by(|a, b| a.lo.total_cmp(&b.lo));
            cols.push(col);
            starts.push(spans.len() as u32);
            let mut cur = list[0];
            for s in &list[1..] {
                if s.lo <= cur.hi + MERGE_GAP {
                    cur.hi = cur.hi.max(s.hi);
                } else {
                    spans.push(cur);
                    cur = *s;
                }
            }
            spans.push(cur);
        }
        starts.push(spans.len() as u32);
        HeightMap {
            grid: self.grid,
            ground,
            cols,
            starts,
            spans,
            blocked: self.blocked,
        }
    }
}
