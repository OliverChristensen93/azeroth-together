//! Line of sight through the height map's columns (spec §4.2): a ray walks from column to column
//! along its ground line (Amanatides and Woo's grid walk), and in each it is blocked by any span
//! across the heights the ray passes there. Columns off the map are open air: outside a run's
//! bounds the bake knows of nothing.
//!
//! An end's own column is coarse (half a yard), so a hero pressed against a wall can share a
//! column with it. There, a span holding the end itself is passed over: the hero is beside the
//! wall, not inside it. A wall wholly inside that one column is the cost; walls are thicker than
//! that, or straddle a column edge, nearly everywhere.

use super::{HeightMap, Span};
use crate::V3;

impl HeightMap {
    /// Whether nothing solid stands between `a` and `b`. An end that is not a number sees
    /// nothing.
    pub fn line_of_sight(&self, a: V3, b: V3) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let g = self.grid;
        let (ax, ay) = ((a.x - g.origin.x) / g.cell, (a.y - g.origin.y) / g.cell);
        let (bx, by) = ((b.x - g.origin.x) / g.cell, (b.y - g.origin.y) / g.cell);
        let (dx, dy) = (bx - ax, by - ay);
        let start = (ax.floor() as i64, ay.floor() as i64);
        let end = (bx.floor() as i64, by.floor() as i64);
        let step_x: i64 = if dx > 0.0 { 1 } else { -1 };
        let step_y: i64 = if dy > 0.0 { 1 } else { -1 };
        let (mut next_x, delta_x) = edges(ax, dx);
        let (mut next_y, delta_y) = edges(ay, dy);
        let (mut ix, mut iy) = start;
        let mut t = 0.0f32;
        // One more than the columns the walk can visit, and two to spare against rounding.
        let cells = (end.0 - start.0).abs() + (end.1 - start.1).abs() + 3;
        for _ in 0..cells {
            let t_out = next_x.min(next_y).min(1.0);
            let za = a.z + (b.z - a.z) * t;
            let zb = a.z + (b.z - a.z) * t_out;
            let mut ends = [f32::NAN; 2];
            if (ix, iy) == start {
                ends[0] = a.z;
            }
            if (ix, iy) == end {
                ends[1] = b.z;
            }
            if self.solid_across(ix, iy, za.min(zb), za.max(zb), ends) {
                return false;
            }
            if (ix, iy) == end || t_out >= 1.0 {
                break;
            }
            if next_x < next_y {
                ix += step_x;
                t = next_x;
                next_x += delta_x;
            } else {
                iy += step_y;
                t = next_y;
                next_y += delta_y;
            }
        }
        true
    }

    /// Whether column `(ix, iy)` is solid anywhere from `lo` to `hi`, passing over a span that
    /// holds one of `ends` (not a number for none).
    fn solid_across(&self, ix: i64, iy: i64, lo: f32, hi: f32, ends: [f32; 2]) -> bool {
        let Some(col) = self.grid.at(ix, iy) else {
            return false;
        };
        let holds = |s: &Span, z: f32| s.lo <= z && z <= s.hi;
        self.spans_at(col)
            .as_slice()
            .iter()
            .any(|s| s.lo <= hi && s.hi >= lo && !holds(s, ends[0]) && !holds(s, ends[1]))
    }
}

/// How far along the ray (0..1) the first column edge past `from` lies, moving `d` columns over
/// the whole ray, and how far apart edges are; infinite when the ray does not move that way.
fn edges(from: f32, d: f32) -> (f32, f32) {
    if d > 0.0 {
        ((from.floor() + 1.0 - from) / d, 1.0 / d)
    } else if d < 0.0 {
        ((from.floor() - from) / d, -1.0 / d)
    } else {
        (f32::INFINITY, f32::INFINITY)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testmap::{flat, house};
    use crate::{V2, V3};

    fn eye(x: f32, y: f32) -> V3 {
        V3::new(x, y, 1.7)
    }

    #[test]
    fn open_ground_is_seen_across() {
        let m = flat(50.0).finish();
        assert!(m.line_of_sight(eye(10.0, 10.0), eye(15.0, 40.0)));
    }

    #[test]
    fn a_house_stands_in_the_way() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            None,
            false,
        );
        let m = b.finish();
        assert!(!m.line_of_sight(eye(10.0, 25.0), eye(40.0, 25.0)));
        // Past its corner it does not.
        assert!(m.line_of_sight(eye(10.0, 10.0), eye(15.0, 40.0)));
    }

    #[test]
    fn a_door_is_seen_through() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            false,
        );
        let m = b.finish();
        assert!(m.line_of_sight(eye(10.0, 25.0), eye(25.0, 25.0)));
    }

    #[test]
    fn a_low_wall_is_seen_over_and_a_tall_one_is_not() {
        let mut low = flat(50.0);
        low.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 0.8);
        assert!(low.finish().line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
        let mut tall = flat(50.0);
        tall.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 2.5);
        assert!(!tall
            .finish()
            .line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
    }

    #[test]
    fn a_hill_hides_what_is_behind_it() {
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 5.0), V2::new(26.0, 15.0), 0.0, 3.0);
        assert!(!b.finish().line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
    }

    #[test]
    fn nobody_sees_through_a_roof() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            Some(25.0),
            true,
        );
        let m = b.finish();
        assert!(!m.line_of_sight(eye(25.0, 25.0), V3::new(25.1, 25.1, 10.0)));
    }

    #[test]
    fn a_hero_against_a_wall_sees_away_from_it_but_not_through_it() {
        // The hero's eye is in the wall's own column.
        let at = eye(24.3, 10.0);
        let mut thin = flat(50.0);
        thin.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        assert!(thin.finish().line_of_sight(at, eye(10.0, 10.0)));
        let mut thick = flat(50.0);
        thick.add_box(V2::new(24.0, 0.0), V2::new(25.5, 50.0), 0.0, 3.0);
        let m = thick.finish();
        assert!(m.line_of_sight(at, eye(10.0, 10.0)));
        assert!(!m.line_of_sight(at, eye(40.0, 10.0)));
    }

    #[test]
    fn rays_leaving_the_map_see_open_air_outside() {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            None,
            false,
        );
        let m = b.finish();
        assert!(m.line_of_sight(eye(40.0, 40.0), eye(60.0, 60.0)));
        assert!(m.line_of_sight(eye(-10.0, -10.0), eye(-5.0, -5.0)));
        assert!(!m.line_of_sight(eye(-5.0, 25.0), eye(55.0, 25.0)));
    }

    #[test]
    fn the_same_column_and_broken_points() {
        let m = flat(50.0).finish();
        assert!(m.line_of_sight(eye(5.1, 5.1), V3::new(5.2, 5.2, 1.0)));
        assert!(!m.line_of_sight(V3::new(f32::NAN, 5.0, 1.7), eye(10.0, 10.0)));
        assert!(!m.line_of_sight(eye(5.0, 5.0), V3::new(10.0, 10.0, f32::INFINITY)));
    }

    #[test]
    fn under_the_ground_is_not_seen_through() {
        let m = flat(50.0).finish();
        assert!(!m.line_of_sight(V3::new(10.0, 10.0, 1.0), V3::new(20.0, 10.0, -1.0)));
    }
}
