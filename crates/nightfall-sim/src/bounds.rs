//! A run's edge: a simple polygon in WoW's ground plane. Heroes are kept inside it, so nobody
//! walks off the end of the map or out of the run.

use crate::V2;

#[derive(Clone, Debug, PartialEq)]
pub struct Bounds {
    points: Vec<V2>,
}

impl Bounds {
    /// A polygon through `points` in order (either winding); at least three.
    pub fn new(points: Vec<V2>) -> Result<Self, String> {
        if points.len() < 3 {
            return Err(format!("bounds need 3 points or more, got {}", points.len()));
        }
        if points.iter().any(|p| !p.is_finite()) {
            return Err("bounds have a point that is not a number".into());
        }
        Ok(Self { points })
    }

    pub fn points(&self) -> &[V2] {
        &self.points
    }

    /// Whether `p` lies inside (even-odd rule; a point on an edge may go either way).
    pub fn contains(&self, p: V2) -> bool {
        let mut inside = false;
        let n = self.points.len();
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + n - 1) % n];
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// `p` itself when inside, else the nearest point on the edge.
    pub fn clamp(&self, p: V2) -> V2 {
        if self.contains(p) {
            return p;
        }
        let n = self.points.len();
        let mut best = self.points[0];
        let mut best_d = f32::INFINITY;
        for i in 0..n {
            let q = nearest_on_segment(p, self.points[i], self.points[(i + 1) % n]);
            let d = q.distance(p);
            if d < best_d {
                best_d = d;
                best = q;
            }
        }
        best
    }
}

/// The point of the segment `a`–`b` nearest `p`.
fn nearest_on_segment(p: V2, a: V2, b: V2) -> V2 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 1e-12 {
        return a;
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    a + ab * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(100.0, 0.0),
            V2::new(100.0, 100.0),
            V2::new(0.0, 100.0),
        ])
        .unwrap()
    }

    /// An L: the square without its top-right quarter.
    fn ell() -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(100.0, 0.0),
            V2::new(100.0, 50.0),
            V2::new(50.0, 50.0),
            V2::new(50.0, 100.0),
            V2::new(0.0, 100.0),
        ])
        .unwrap()
    }

    #[test]
    fn fewer_than_three_points_is_no_polygon() {
        assert!(Bounds::new(vec![V2::ZERO, V2::new(1.0, 0.0)]).is_err());
    }

    #[test]
    fn inside_and_outside() {
        let b = square();
        assert!(b.contains(V2::new(50.0, 50.0)));
        assert!(!b.contains(V2::new(150.0, 50.0)));
        assert!(!b.contains(V2::new(-1.0, 50.0)));
        assert!(!ell().contains(V2::new(75.0, 75.0)));
        assert!(ell().contains(V2::new(25.0, 75.0)));
    }

    #[test]
    fn a_point_inside_is_left_alone() {
        assert_eq!(square().clamp(V2::new(20.0, 30.0)), V2::new(20.0, 30.0));
    }

    #[test]
    fn a_point_outside_goes_to_the_nearest_edge() {
        let b = square();
        assert_eq!(b.clamp(V2::new(150.0, 40.0)), V2::new(100.0, 40.0));
        assert_eq!(b.clamp(V2::new(-10.0, -10.0)), V2::new(0.0, 0.0));
        // In the L's missing corner, the nearer of its two inner edges.
        let c = ell().clamp(V2::new(60.0, 90.0));
        assert_eq!(c, V2::new(50.0, 90.0));
    }
}
