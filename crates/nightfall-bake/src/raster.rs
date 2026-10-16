//! Solid geometry into columns, as Recast's heightfield rasterizer does it: each triangle is
//! clipped to every column its footprint touches, and the part inside a column is solid from its
//! lowest to its highest point there. One model's triangles gather in a [`Footprint`] first, so
//! its many faces over a column reach the builder as one or two spans, not dozens.

use nightfall_sim::{Grid, HeightMapBuilder, Span, MERGE_GAP};

/// One model's triangles, column by column, until [`Footprint::flush_into`] hands them on.
pub struct Footprint {
    grid: Grid,
    spans: Vec<(u32, Span)>,
}

impl Footprint {
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            spans: Vec::new(),
        }
    }

    /// How many column pieces are gathered.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// A triangle (WoW world points), clipped into every column it touches. One with a point
    /// that is not a number is dropped, and the parts off the grid are too.
    pub fn add_triangle(&mut self, tri: [[f32; 3]; 3]) {
        if tri.iter().flatten().any(|v| !v.is_finite()) {
            return;
        }
        let g = self.grid;
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for p in &tri {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let first = |v: f32, origin: f32| ((v - origin) / g.cell).floor().max(0.0) as i64;
        let last = |v: f32, origin: f32, n: u32| {
            ((v - origin) / g.cell).floor().min(n as f32 - 1.0) as i64
        };
        let (ix0, ix1) = (first(lo[0], g.origin.x), last(hi[0], g.origin.x, g.nx));
        let (iy0, iy1) = (first(lo[1], g.origin.y), last(hi[1], g.origin.y, g.ny));
        let mut a = [[0.0f32; 3]; 8];
        let mut b = [[0.0f32; 3]; 8];
        for iy in iy0..=iy1 {
            for ix in ix0..=ix1 {
                let x0 = g.origin.x + ix as f32 * g.cell;
                let y0 = g.origin.y + iy as f32 * g.cell;
                a[..3].copy_from_slice(&tri);
                let n = clip(&a[..3], &mut b, 0, x0, true);
                let n = clip(&b[..n], &mut a, 0, x0 + g.cell, false);
                let n = clip(&a[..n], &mut b, 1, y0, true);
                let n = clip(&b[..n], &mut a, 1, y0 + g.cell, false);
                if n == 0 {
                    continue;
                }
                let (zlo, zhi) = a[..n]
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), p| {
                        (l.min(p[2]), h.max(p[2]))
                    });
                let col = g.index(ix as u32, iy as u32) as u32;
                self.spans.push((col, Span { lo: zlo, hi: zhi }));
            }
        }
    }

    /// Everything gathered into the builder, then forgotten. `solid` (a doodad's collision hull,
    /// a closed solid) fills each column from the model's lowest to its highest point there; a
    /// building's spans only merge where they touch, so its rooms keep their air.
    pub fn flush_into(&mut self, b: &mut HeightMapBuilder, solid: bool) {
        self.spans
            .sort_unstable_by(|x, y| x.0.cmp(&y.0).then(x.1.lo.total_cmp(&y.1.lo)));
        let mut i = 0;
        while i < self.spans.len() {
            let (col, mut cur) = self.spans[i];
            i += 1;
            while i < self.spans.len() && self.spans[i].0 == col {
                let s = self.spans[i].1;
                if solid || s.lo <= cur.hi + MERGE_GAP {
                    cur.hi = cur.hi.max(s.hi);
                } else {
                    b.add_span(col as usize, cur.lo, cur.hi);
                    cur = s;
                }
                i += 1;
            }
            b.add_span(col as usize, cur.lo, cur.hi);
        }
        self.spans.clear();
    }
}

/// Sutherland–Hodgman against one axis-aligned plane: the part of `poly` whose coordinate `axis`
/// (0 is x, 1 is y) is at or above `at` when `keep_above`, at or below it otherwise, written to
/// `out`; returns its point count. A triangle clipped by four planes has at most seven points.
fn clip(
    poly: &[[f32; 3]],
    out: &mut [[f32; 3]; 8],
    axis: usize,
    at: f32,
    keep_above: bool,
) -> usize {
    let inside = |p: &[f32; 3]| {
        if keep_above {
            p[axis] >= at
        } else {
            p[axis] <= at
        }
    };
    let mut n = 0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let (ia, ib) = (inside(&a), inside(&b));
        if ia && n < out.len() {
            out[n] = a;
            n += 1;
        }
        if ia != ib && n < out.len() {
            let t = (at - a[axis]) / (b[axis] - a[axis]);
            out[n] = [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ];
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use nightfall_sim::{Grid, HeightMapBuilder, Span, CELL, V2, V3};

    use super::*;

    /// 20 yards across.
    fn grid() -> Grid {
        Grid {
            origin: V2::ZERO,
            cell: CELL,
            nx: 40,
            ny: 40,
        }
    }

    /// A box's twelve triangles.
    fn cuboid(lo: [f32; 3], hi: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
        let c = |i: usize| {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        let faces = [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ];
        faces
            .iter()
            .flat_map(|f| [[c(f[0]), c(f[1]), c(f[2])], [c(f[0]), c(f[2]), c(f[3])]])
            .collect()
    }

    #[test]
    fn a_wall_stands_in_the_columns_it_passes_through() {
        let mut b = HeightMapBuilder::flat(grid(), 0.0);
        let mut fp = Footprint::new(grid());
        // A wall along x = 10.1 from y = 5 to y = 15, 3 yards tall.
        fp.add_triangle([[10.1, 5.0, 0.0], [10.1, 15.0, 0.0], [10.1, 15.0, 3.0]]);
        fp.add_triangle([[10.1, 5.0, 0.0], [10.1, 15.0, 3.0], [10.1, 5.0, 3.0]]);
        fp.flush_into(&mut b, false);
        assert!(fp.is_empty());
        let m = b.finish();
        let g = m.grid();
        assert_eq!(m.floors_at(g.index(20, 20)).as_slice(), &[3.0]);
        assert_eq!(m.floors_at(g.index(18, 20)).as_slice(), &[0.0]);
        assert!(!m.line_of_sight(V3::new(8.0, 10.0, 1.7), V3::new(12.0, 10.0, 1.7)));
        assert!(m.line_of_sight(V3::new(8.0, 2.0, 1.7), V3::new(12.0, 2.0, 1.7)));
    }

    #[test]
    fn a_floor_overhead_is_a_second_storey() {
        let mut b = HeightMapBuilder::flat(grid(), 0.0);
        let mut fp = Footprint::new(grid());
        fp.add_triangle([[2.0, 2.0, 5.0], [6.0, 2.0, 5.0], [6.0, 6.0, 5.0]]);
        fp.add_triangle([[2.0, 2.0, 5.0], [6.0, 6.0, 5.0], [2.0, 6.0, 5.0]]);
        fp.flush_into(&mut b, false);
        let m = b.finish();
        assert_eq!(m.floors_at(m.grid().index(8, 8)).as_slice(), &[0.0, 5.0]);
        assert!(m.is_indoors(V2::new(4.25, 4.25), 0.0));
    }

    #[test]
    fn a_hull_is_solid_through_and_a_building_is_hollow() {
        let col = grid().index(7, 7);
        let mut fp = Footprint::new(grid());
        let mut solid = HeightMapBuilder::new(grid());
        for t in cuboid([3.0, 3.0, 0.0], [5.0, 5.0, 2.0]) {
            fp.add_triangle(t);
        }
        fp.flush_into(&mut solid, true);
        assert_eq!(
            solid.finish().spans_at(col).as_slice(),
            &[Span { lo: 0.0, hi: 2.0 }]
        );
        let mut hollow = HeightMapBuilder::new(grid());
        for t in cuboid([3.0, 3.0, 0.0], [5.0, 5.0, 2.0]) {
            fp.add_triangle(t);
        }
        fp.flush_into(&mut hollow, false);
        assert_eq!(
            hollow.finish().spans_at(col).as_slice(),
            &[Span { lo: 0.0, hi: 0.0 }, Span { lo: 2.0, hi: 2.0 }]
        );
    }

    #[test]
    fn a_triangle_off_the_grid_or_not_a_number_adds_nothing() {
        let mut fp = Footprint::new(grid());
        fp.add_triangle([[30.0, 30.0, 0.0], [35.0, 30.0, 0.0], [35.0, 35.0, 0.0]]);
        fp.add_triangle([[-9.0, -9.0, 0.0], [-5.0, -9.0, 0.0], [-5.0, -5.0, 0.0]]);
        fp.add_triangle([[1.0, 1.0, f32::NAN], [2.0, 1.0, 0.0], [2.0, 2.0, 0.0]]);
        assert_eq!(fp.len(), 0);
    }
}
