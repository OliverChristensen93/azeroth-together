//! A two-component vector in WoW's ground plane: `x` north, `y` west, in yards.

use std::ops::{Add, AddAssign, Mul, Sub};

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

    pub fn dot(self, other: V2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Both components are numbers: a pose from a broken or hostile client may not be.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
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

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

/// A point in WoW's world: `x` north, `y` west, `z` up, in yards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The point `z` yards up over the ground point `p`.
    pub const fn at(p: V2, z: f32) -> Self {
        Self { x: p.x, y: p.y, z }
    }

    /// Where it stands on the ground plane.
    pub const fn ground(self) -> V2 {
        V2::new(self.x, self.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_stands_on_its_ground_point() {
        let p = V3::at(V2::new(1.0, 2.0), 3.0);
        assert_eq!(p, V3::new(1.0, 2.0, 3.0));
        assert_eq!(p.ground(), V2::new(1.0, 2.0));
        assert!(p.is_finite());
        assert!(!V3::new(0.0, 0.0, f32::NAN).is_finite());
    }

    #[test]
    fn normalize_has_unit_length_and_zero_stays_zero() {
        assert!((V2::new(3.0, 4.0).normalize_or_zero().length() - 1.0).abs() < 1e-6);
        assert_eq!(V2::ZERO.normalize_or_zero(), V2::ZERO);
    }

    #[test]
    fn dot_and_finiteness() {
        assert_eq!(V2::new(1.0, 2.0).dot(V2::new(3.0, 4.0)), 11.0);
        assert!(V2::new(1.0, 2.0).is_finite());
        assert!(!V2::new(f32::NAN, 0.0).is_finite());
        assert!(!V2::new(0.0, f32::INFINITY).is_finite());
    }
}
