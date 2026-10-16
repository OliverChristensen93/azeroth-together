//! How visible a hero is (spec §5.1): a number from 0 to 100 set by how they move, raised by
//! light, and how far that lets an enemy notice them, by where they stand in its view.

use serde::{Deserialize, Serialize};

use crate::{AwarenessRules, SenseRules, V2};

/// How a hero moves, as their client reports it: WoW's walk toggle, and whether they move at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gait {
    #[default]
    Still,
    Walk,
    Run,
}

/// A hero's stance: spec §5.1's rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Still,
    Walking,
    Running,
    /// In stealth and moving, at whatever pace.
    Sneaking,
    SneakingStill,
}

impl Stance {
    /// The stance of a hero moving at `gait`, in stealth or not.
    pub fn of(gait: Gait, stealthed: bool) -> Stance {
        match (stealthed, gait) {
            (true, Gait::Still) => Stance::SneakingStill,
            (true, _) => Stance::Sneaking,
            (false, Gait::Still) => Stance::Still,
            (false, Gait::Walk) => Stance::Walking,
            (false, Gait::Run) => Stance::Running,
        }
    }

    /// Whether the hero's feet are moving (and so stepping).
    pub fn moving(self) -> bool {
        matches!(self, Stance::Walking | Stance::Running | Stance::Sneaking)
    }
}

/// What lights a hero up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Light {
    pub lantern: bool,
    pub lit_area: bool,
    pub flash: bool,
}

/// A hero's visibility, 0..100: the stance's row times every light on them.
pub(crate) fn visibility(stance: Stance, light: Light, r: &SenseRules) -> f32 {
    let t = &r.visibility;
    let mut v = match stance {
        Stance::Running => t.run,
        Stance::Walking => t.walk,
        Stance::Still => t.still,
        Stance::Sneaking => t.sneak,
        Stance::SneakingStill => t.sneak_still,
    };
    if light.lantern {
        v *= r.lantern;
    }
    if light.lit_area {
        v *= r.lit;
    }
    if light.flash {
        v *= r.flash;
    }
    v.clamp(0.0, 100.0)
}

/// The share of its notice range an enemy facing `facing` has for something in direction `to`
/// from it: 1 in its front cone of `cone` degrees, `behind` in its rear cone, `side` between.
/// Something on top of it is in front.
pub(crate) fn facing_factor(facing: V2, to: V2, cone: f32, r: &SenseRules) -> f32 {
    let (f, d) = (facing.normalize_or_zero(), to.normalize_or_zero());
    if f == V2::ZERO || d == V2::ZERO {
        return 1.0;
    }
    let off = f.dot(d).clamp(-1.0, 1.0).acos().to_degrees();
    if off <= cone / 2.0 {
        1.0
    } else if off >= 180.0 - r.behind_cone / 2.0 {
        r.behind
    } else {
        r.side
    }
}

/// How far an enemy notices a hero of `visibility` standing at `facing` of its view, never past
/// the `fog` (spec §5.1).
pub(crate) fn notice_range(visibility: f32, facing: f32, fog: f32, r: &SenseRules) -> f32 {
    (r.notice_range * visibility / 100.0 * facing).min(fog)
}

/// How fast seeing a hero `distance` yards off, inside a notice range of `range`, fills suspicion,
/// a second (spec §5.3).
pub(crate) fn fill_rate(distance: f32, range: f32, a: &AwarenessRules) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    (a.fill * (1.0 - distance / range)).max(a.fill_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stance_has_its_visibility() {
        let r = SenseRules::default();
        let none = Light::default();
        assert_eq!(visibility(Stance::Running, none, &r), 100.0);
        assert_eq!(visibility(Stance::Walking, none, &r), 55.0);
        assert_eq!(visibility(Stance::Still, none, &r), 35.0);
        assert_eq!(visibility(Stance::Sneaking, none, &r), 20.0);
        assert_eq!(visibility(Stance::SneakingStill, none, &r), 8.0);
    }

    #[test]
    fn the_stance_comes_from_the_gait_and_stealth() {
        assert_eq!(Stance::of(Gait::Run, false), Stance::Running);
        assert_eq!(Stance::of(Gait::Walk, false), Stance::Walking);
        assert_eq!(Stance::of(Gait::Still, false), Stance::Still);
        assert_eq!(Stance::of(Gait::Run, true), Stance::Sneaking);
        assert_eq!(Stance::of(Gait::Walk, true), Stance::Sneaking);
        assert_eq!(Stance::of(Gait::Still, true), Stance::SneakingStill);
        assert!(Stance::Sneaking.moving());
        assert!(!Stance::SneakingStill.moving());
        assert!(!Stance::Still.moving());
    }

    #[test]
    fn light_multiplies_and_never_passes_100() {
        let r = SenseRules::default();
        let lantern = Light {
            lantern: true,
            ..Light::default()
        };
        assert!((visibility(Stance::Walking, lantern, &r) - 88.0).abs() < 1e-4);
        let all = Light {
            lantern: true,
            lit_area: true,
            flash: true,
        };
        assert_eq!(visibility(Stance::Walking, all, &r), 100.0);
        let flash = Light {
            flash: true,
            ..Light::default()
        };
        assert_eq!(visibility(Stance::SneakingStill, flash, &r), 16.0);
    }

    #[test]
    fn in_front_at_the_sides_and_behind() {
        let r = SenseRules::default();
        let north = V2::new(1.0, 0.0);
        assert_eq!(facing_factor(north, V2::new(1.0, 0.0), 120.0, &r), 1.0);
        assert_eq!(facing_factor(north, V2::new(1.0, 1.0), 120.0, &r), 1.0);
        assert_eq!(facing_factor(north, V2::new(0.0, 1.0), 120.0, &r), 0.6);
        assert_eq!(facing_factor(north, V2::new(-1.0, 0.3), 120.0, &r), 0.3);
        // A sentry's wide cone takes in what stands nearly at its side.
        assert_eq!(facing_factor(north, V2::new(0.2, 1.0), 180.0, &r), 1.0);
        // On top of it: in front.
        assert_eq!(facing_factor(north, V2::ZERO, 120.0, &r), 1.0);
    }

    #[test]
    fn the_notice_range_follows_visibility_and_facing_and_stops_at_the_fog() {
        let r = SenseRules::default();
        assert_eq!(notice_range(100.0, 1.0, 40.0, &r), 35.0);
        assert!((notice_range(55.0, 1.0, 40.0, &r) - 19.25).abs() < 1e-4);
        assert!((notice_range(20.0, 0.3, 40.0, &r) - 2.1).abs() < 1e-4);
        assert_eq!(notice_range(100.0, 1.0, 30.0, &r), 30.0);
    }

    #[test]
    fn suspicion_fills_faster_near_and_never_slower_than_the_least() {
        let a = AwarenessRules::default();
        assert_eq!(fill_rate(0.0, 20.0, &a), 60.0);
        assert_eq!(fill_rate(10.0, 20.0, &a), 30.0);
        assert_eq!(fill_rate(19.5, 20.0, &a), 10.0);
        assert_eq!(fill_rate(5.0, 0.0, &a), 0.0);
    }
}
