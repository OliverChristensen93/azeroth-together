//! Noise (spec §5.2): an event with a place and a radius, made by a hero. A listener with a clear
//! line to it hears it within its radius, and through something solid within `walls` of it. It
//! points the listener at the place, never at the hero. Footsteps are noises too, as loud as the
//! stance.

use serde::{Deserialize, Serialize};

use super::Stance;
use crate::heightmap::HeightMap;
#[cfg(doc)]
use crate::World;
use crate::{LoudnessTable, PlayerId, StepNoise, CHEST_HEIGHT, V3};

/// A noise a hero made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Noise {
    pub by: PlayerId,
    /// The floor point it came from; it is heard at chest height over it.
    pub at: V3,
    /// Yards.
    pub radius: f32,
}

/// How loud a thing is (spec §5.2's table), for the noises later sub-projects make through
/// [`World::make_noise`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loudness {
    /// Abilities, by their loudness.
    Quiet,
    Normal,
    Loud,
    /// A hit landing, a unit dying.
    Hit,
    /// Opening a chest, searching a body.
    Open,
    /// Digging a grave.
    Dig,
    /// Ravens taking off.
    Ravens,
    /// A second in an exit's ring.
    Extract,
}

impl Loudness {
    /// How far it carries, yards.
    pub fn radius(self, t: &LoudnessTable) -> f32 {
        match self {
            Loudness::Quiet => t.quiet,
            Loudness::Normal => t.normal,
            Loudness::Loud => t.loud,
            Loudness::Hit => t.hit,
            Loudness::Open => t.open,
            Loudness::Dig => t.dig,
            Loudness::Ravens => t.ravens,
            Loudness::Extract => t.extract,
        }
    }
}

/// How far one footstep of a hero in `stance` carries, yards: nothing standing still.
pub(crate) fn step_radius(stance: Stance, s: &StepNoise) -> f32 {
    match stance {
        Stance::Running => s.run,
        Stance::Walking => s.walk,
        Stance::Sneaking => s.sneak,
        Stance::Still | Stance::SneakingStill => 0.0,
    }
}

/// Whether ears at `ear` hear `noise`: within its radius with a clear line to it, or within
/// `walls` of its radius through something solid. Without a map nothing is known to stand between.
/// A place, an ear or a radius that is not a number hears nothing, and a radius of 0 or less is
/// silent.
pub(crate) fn hears(ear: V3, noise: &Noise, map: Option<&HeightMap>, walls: f32) -> bool {
    if !ear.is_finite() || !noise.at.is_finite() || !noise.radius.is_finite() || noise.radius <= 0.0
    {
        return false;
    }
    let d = ear.ground().distance(noise.at.ground());
    if d > noise.radius {
        return false;
    }
    // Heard through walls too from this near: no ray needs casting.
    if d <= noise.radius * walls {
        return true;
    }
    let source = V3::at(noise.at.ground(), noise.at.z + CHEST_HEIGHT);
    map.is_none_or(|m| m.line_of_sight(ear, source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightmap::testmap::flat;
    use crate::V2;

    fn ear(x: f32, y: f32) -> V3 {
        V3::new(x, y, 1.7)
    }

    fn noise(x: f32, y: f32, radius: f32) -> Noise {
        Noise {
            by: 0,
            at: V3::new(x, y, 0.0),
            radius,
        }
    }

    #[test]
    fn footsteps_are_as_loud_as_the_stance() {
        let s = StepNoise::default();
        assert_eq!(step_radius(Stance::Running, &s), 12.0);
        assert_eq!(step_radius(Stance::Walking, &s), 4.0);
        assert_eq!(step_radius(Stance::Sneaking, &s), 0.0);
        assert_eq!(step_radius(Stance::Still, &s), 0.0);
        assert_eq!(step_radius(Stance::SneakingStill, &s), 0.0);
    }

    #[test]
    fn each_loudness_carries_as_far_as_its_row() {
        let t = LoudnessTable::default();
        let rows = [
            (Loudness::Quiet, 8.0),
            (Loudness::Normal, 20.0),
            (Loudness::Loud, 40.0),
            (Loudness::Hit, 15.0),
            (Loudness::Open, 15.0),
            (Loudness::Dig, 30.0),
            (Loudness::Ravens, 40.0),
            (Loudness::Extract, 25.0),
        ];
        for (l, r) in rows {
            assert_eq!(l.radius(&t), r, "{l:?}");
        }
    }

    #[test]
    fn in_the_open_a_noise_carries_its_radius() {
        let m = flat(50.0).finish();
        let n = noise(21.0, 10.0, 12.0);
        assert!(hears(ear(10.0, 10.0), &n, Some(&m), 0.5));
        assert!(!hears(ear(8.0, 10.0), &n, Some(&m), 0.5));
    }

    #[test]
    fn a_wall_halves_how_far_a_noise_carries() {
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        let m = b.finish();
        let n = noise(25.5, 10.0, 12.0);
        assert!(hears(ear(20.0, 10.0), &n, Some(&m), 0.5));
        assert!(!hears(ear(18.0, 10.0), &n, Some(&m), 0.5));
        // Without the map nothing is known to stand between.
        assert!(hears(ear(18.0, 10.0), &n, None, 0.5));
    }

    #[test]
    fn a_noise_or_an_ear_that_is_not_a_number_is_not_heard() {
        assert!(!hears(
            ear(10.0, 10.0),
            &noise(f32::NAN, 10.0, 12.0),
            None,
            0.5
        ));
        assert!(!hears(
            V3::new(10.0, f32::INFINITY, 1.7),
            &noise(12.0, 10.0, 12.0),
            None,
            0.5
        ));
    }

    #[test]
    fn a_radius_that_is_not_a_number_or_not_above_zero_is_silent() {
        for radius in [f32::NAN, f32::INFINITY, 0.0, -3.0] {
            let n = noise(10.0, 10.0, radius);
            assert!(!hears(ear(10.0, 10.0), &n, None, 0.5), "{radius}");
        }
    }
}
