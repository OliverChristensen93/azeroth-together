//! A two-component vector in WoW's ground plane: `x` north, `y` west, in yards.

use std::ops::{Add, AddAssign, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: V2) -> f32 {
        (other - self).length()
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// This vector scaled to length 1, or zero when it is (nearly) zero.
    pub fn normalize_or_zero(self) -> V2 {
        let len = self.length();
        if len > 1e-6 {
            self * (1.0 / len)
        } else {
            V2::ZERO
        }
    }

    /// The point `len` yards along `angle` (radians) from the origin.
    pub fn from_angle(angle: f32, len: f32) -> V2 {
        V2::new(angle.cos() * len, angle.sin() * len)
    }

    /// This vector's angle in radians, as [`V2::from_angle`] takes it.
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// This vector turned by `angle` radians.
    pub fn rotate(self, angle: f32) -> V2 {
        let (s, c) = angle.sin_cos();
        V2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// How far `p` lies from the segment `a`-`b`, and how far along it (0 at `a`, 1 at `b`) its
    /// nearest point is.
    pub fn segment_distance(p: V2, a: V2, b: V2) -> (f32, f32) {
        let ab = b - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 1e-9 {
            ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p.distance(a + ab * t), t)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_distance_finds_the_nearest_point() {
        let (d, t) = V2::segment_distance(V2::new(5.0, 2.0), V2::ZERO, V2::new(10.0, 0.0));
        assert!((d - 2.0).abs() < 1e-5);
        assert!((t - 0.5).abs() < 1e-5);
        let (d, t) = V2::segment_distance(V2::new(-3.0, 4.0), V2::ZERO, V2::new(10.0, 0.0));
        assert!((d - 5.0).abs() < 1e-5);
        assert_eq!(t, 0.0);
    }

    #[test]
    fn rotate_turns_a_quarter() {
        let v = V2::new(1.0, 0.0).rotate(std::f32::consts::FRAC_PI_2);
        assert!(v.x.abs() < 1e-6 && (v.y - 1.0).abs() < 1e-6);
    }
}
