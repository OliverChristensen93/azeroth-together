//! The fight's arithmetic, pure: dice for damage and crits, which way a unit faces another, how
//! far an ability reaches, how far a caster may move before its cast breaks, and a hero's dodge
//! charges. The rest of [`super`] uses it on the world.

use serde::{Deserialize, Serialize};

use crate::{CombatRules, DodgeRules, Facing, Moving, V2};

/// A small random number generator (xorshift64*): the same fight every time with the same inputs.
#[derive(Clone, Debug)]
pub(crate) struct Dice(u64);

impl Dice {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..1`.
    pub(crate) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number from `min` up to `max`, whichever way round they are. The range is closed: both
    /// ends can come up, since damage's `max` is inclusive in content. Rounding in the f32 add
    /// may give `max` exactly, and that is kept, not clamped. A bound that is not a number rolls
    /// nothing (0), so a broken table never spreads NaN through the damage.
    pub(crate) fn between(&mut self, min: f32, max: f32) -> f32 {
        if !(min.is_finite() && max.is_finite()) {
            return 0.0;
        }
        let (lo, hi) = (min.min(max), min.max(max));
        lo + (hi - lo) * self.unit()
    }

    /// Whether a chance of `p` (0 to 1) comes up.
    pub(crate) fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

/// Whether the way `to` lies within `half` degrees either side of `facing` (ground directions;
/// a unit on top of another is taken to face it). Nothing is within an arc that is not finite:
/// a NaN direction would otherwise read as "no direction", which is within any arc.
pub(crate) fn within_arc(facing: V2, to: V2, half: f32) -> bool {
    if !(facing.is_finite() && to.is_finite() && half.is_finite()) {
        return false;
    }
    let (f, d) = (facing.normalize_or_zero(), to.normalize_or_zero());
    f == V2::ZERO || d == V2::ZERO || f.dot(d) >= half.to_radians().cos() - 1e-5
}

/// Whether an attacker at `from` facing `facing` stands as `want` asks toward a target at `at`
/// facing `at_facing`: in front, the target within `front` degrees of straight ahead; behind,
/// that, and the attacker in the target's rear half (WoW's facing rules).
pub(crate) fn stands_right(
    want: Facing,
    from: V2,
    facing: V2,
    at: V2,
    at_facing: V2,
    front: f32,
) -> bool {
    // Any unit with a point or a facing that is not finite stands nowhere, whatever `want` asks.
    if !(from.is_finite() && facing.is_finite() && at.is_finite() && at_facing.is_finite()) {
        return false;
    }
    let ahead = within_arc(facing, at - from, front);
    match want {
        Facing::Any => true,
        Facing::Front => ahead,
        Facing::Behind => {
            ahead
                && at_facing
                    .normalize_or_zero()
                    .dot((from - at).normalize_or_zero())
                    < 0.0
        }
    }
}

/// Whether a target `distance` yards off is within `range`, its size (`reach`) added.
pub(crate) fn in_range(distance: f32, range: f32, r: &CombatRules) -> bool {
    // An infinite range would reach across the map; a NaN distance or range is no distance at all.
    distance.is_finite() && range.is_finite() && distance <= range + r.reach
}

/// How far (yards, straight from where it began) a caster running at `run` yards a second may
/// have gone `elapsed` seconds into a cast before it breaks: anywhere for an instant; a nudge for
/// a rooted cast; for a slowed one full speed during `speed_grace` (the slow reaching a joined
/// client), `cast_move_speed` after it, and the nudge. A speed or time that is not finite gets
/// the nudge alone, the tightest bound a cast can have and still allow the caster's own nudge.
pub(crate) fn allowance(moving: Moving, run: f32, elapsed: f32, r: &CombatRules) -> f32 {
    match moving {
        Moving::Free => f32::INFINITY,
        Moving::Rooted => r.cast_slack,
        Moving::Slowed if !(run.is_finite() && elapsed.is_finite()) => r.cast_slack,
        Moving::Slowed => {
            // A negative speed or time is no speed or time: it gives no ground.
            let (run, elapsed) = (run.max(0.0), elapsed.max(0.0));
            let grace = elapsed.min(r.speed_grace);
            r.cast_slack + run * (grace + (elapsed - grace) * r.cast_move_speed)
        }
    }
}

/// A hero's dodge charges (spec §6.1): one comes back `recharge` seconds after the last came
/// back, or after the first was spent from full.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Charges {
    pub have: u8,
    /// Seconds until the next comes back (0 when full).
    pub next_in: f32,
}

impl Charges {
    /// `n` charges, all there.
    pub fn full(n: u8) -> Self {
        Self {
            have: n,
            next_in: 0.0,
        }
    }

    /// Spend one; whether there was one.
    pub(crate) fn spend(&mut self, r: &DodgeRules) -> bool {
        if self.have == 0 {
            return false;
        }
        if self.have >= r.charges {
            self.next_in = r.recharge;
        }
        self.have -= 1;
        true
    }

    /// `dt` seconds pass. A step that is not a finite time, or runs backwards, passes nothing: a
    /// NaN would otherwise leave `next_in` NaN for good, and a negative step would lengthen the
    /// wait.
    pub(crate) fn tick(&mut self, dt: f32, r: &DodgeRules) {
        if !dt.is_finite() || dt < 0.0 {
            return;
        }
        if self.have >= r.charges {
            self.next_in = 0.0;
            return;
        }
        self.next_in -= dt;
        if self.next_in <= 1e-4 {
            self.have += 1;
            self.next_in = if self.have < r.charges {
                self.next_in + r.recharge
            } else {
                0.0
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dice_are_the_same_every_run_and_stay_in_bounds() {
        let roll = || {
            let mut d = Dice::new(7);
            (0..50).map(|_| d.between(10.0, 20.0)).collect::<Vec<_>>()
        };
        let a = roll();
        assert_eq!(a, roll());
        assert!(a.iter().all(|x| (10.0..=20.0).contains(x)));
        let mut d = Dice::new(7);
        assert_eq!(d.between(5.0, 5.0), 5.0);
        assert!(!(0..100).any(|_| d.chance(0.0)));
        assert!((0..100).all(|_| d.chance(1.0)));
    }

    #[test]
    fn in_front_is_the_half_circle_ahead() {
        let north = V2::new(1.0, 0.0);
        assert!(within_arc(north, V2::new(3.0, 2.9), 90.0));
        assert!(within_arc(north, V2::new(0.0, 5.0), 90.0));
        assert!(!within_arc(north, V2::new(-0.5, 5.0), 90.0));
        assert!(within_arc(north, V2::ZERO, 90.0));
        let r = CombatRules::default();
        let at = V2::new(5.0, 0.0);
        assert!(stands_right(
            Facing::Front,
            V2::ZERO,
            north,
            at,
            north,
            r.front
        ));
        assert!(!stands_right(
            Facing::Front,
            V2::ZERO,
            north * -1.0,
            at,
            north,
            r.front
        ));
        assert!(stands_right(
            Facing::Any,
            V2::ZERO,
            north * -1.0,
            at,
            north,
            r.front
        ));
    }

    #[test]
    fn behind_needs_the_targets_back_and_the_attacker_facing_it() {
        let north = V2::new(1.0, 0.0);
        let target = V2::new(5.0, 0.0);
        // The target faces north and we stand south of it, facing it: behind.
        assert!(stands_right(
            Facing::Behind,
            V2::ZERO,
            north,
            target,
            north,
            90.0
        ));
        // The target turned to face us: in front of it.
        assert!(!stands_right(
            Facing::Behind,
            V2::ZERO,
            north,
            target,
            north * -1.0,
            90.0
        ));
        // Behind it but looking away.
        assert!(!stands_right(
            Facing::Behind,
            V2::ZERO,
            north * -1.0,
            target,
            north,
            90.0
        ));
    }

    #[test]
    fn range_counts_the_targets_size() {
        let r = CombatRules::default();
        assert!(in_range(6.4, 5.0, &r));
        assert!(!in_range(6.6, 5.0, &r));
    }

    #[test]
    fn a_rooted_cast_allows_a_nudge_a_slowed_one_the_slowed_pace_after_a_grace() {
        let r = CombatRules::default();
        assert_eq!(allowance(Moving::Free, 7.0, 1.0, &r), f32::INFINITY);
        assert_eq!(allowance(Moving::Rooted, 7.0, 1.0, &r), 0.5);
        // 0.2 s in: still within the grace, at full speed.
        assert!((allowance(Moving::Slowed, 7.0, 0.2, &r) - (0.5 + 1.4)).abs() < 1e-4);
        // 1.3 s in: 0.3 s at full speed, then a second at half.
        assert!((allowance(Moving::Slowed, 7.0, 1.3, &r) - (0.5 + 2.1 + 3.5)).abs() < 1e-4);
    }

    #[test]
    fn charges_come_back_one_at_a_time() {
        let r = DodgeRules::default();
        let mut c = Charges::full(r.charges);
        assert!(c.spend(&r));
        assert!(c.spend(&r));
        assert!(!c.spend(&r));
        assert_eq!(c.have, 0);
        for _ in 0..79 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 0);
        for _ in 0..2 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 1);
        for _ in 0..81 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 2);
        assert_eq!(c.next_in, 0.0);
    }

    #[test]
    fn a_non_finite_number_hits_nothing() {
        let north = V2::new(1.0, 0.0);
        let nan = V2::new(f32::NAN, 0.0);
        let inf = V2::new(f32::INFINITY, 0.0);
        let r = CombatRules::default();
        let at = V2::new(5.0, 0.0);
        // Arcs: no facing, way or half-angle that is not finite is ever in one.
        assert!(!within_arc(nan, north, 90.0));
        assert!(!within_arc(north, nan, 90.0));
        assert!(!within_arc(inf, north, 90.0));
        assert!(!within_arc(north, north, f32::NAN));
        assert!(!within_arc(north, north, f32::INFINITY));
        // Standing: every point and facing must be finite, whatever the facing asks.
        assert!(!stands_right(
            Facing::Any,
            V2::ZERO,
            nan,
            at,
            north,
            r.front
        ));
        assert!(!stands_right(Facing::Any, nan, north, at, north, r.front));
        assert!(!stands_right(
            Facing::Front,
            V2::ZERO,
            north,
            at,
            nan,
            r.front
        ));
        assert!(!stands_right(
            Facing::Behind,
            V2::ZERO,
            north,
            at,
            north,
            f32::NAN
        ));
        // Range: a distance or a range that is not a finite number is never in it.
        assert!(!in_range(f32::NAN, 5.0, &r));
        assert!(!in_range(f32::INFINITY, 5.0, &r));
        assert!(!in_range(6.0, f32::NAN, &r));
        assert!(!in_range(6.0, f32::INFINITY, &r));
        // Allowance: a speed or a time that is not finite gets the rooted cast's nudge.
        assert_eq!(allowance(Moving::Slowed, f32::NAN, 1.0, &r), r.cast_slack);
        assert_eq!(allowance(Moving::Slowed, 7.0, f32::NAN, &r), r.cast_slack);
        assert_eq!(
            allowance(Moving::Slowed, f32::INFINITY, 1.0, &r),
            r.cast_slack
        );
    }

    #[test]
    fn between_takes_the_interval_either_way_round_and_no_bound_that_is_not_a_number() {
        let mut d = Dice::new(3);
        for _ in 0..50 {
            let x = d.between(20.0, 10.0);
            assert!((10.0..=20.0).contains(&x), "{x}");
        }
        assert_eq!(d.between(f32::NAN, 10.0), 0.0);
        assert_eq!(d.between(10.0, f32::INFINITY), 0.0);
    }

    #[test]
    fn the_dice_roll_the_same_numbers_on_every_platform() {
        // Pinned from this generator's integer arithmetic: a change here changes every fight, and
        // integer arithmetic gives the same numbers on every platform.
        let mut one = Dice::new(1);
        assert_eq!(
            [one.next(), one.next(), one.next()],
            [
                5180492295206395165,
                12380297144915551517,
                13389498078930870103
            ]
        );
        let mut seeded = Dice::new(12345);
        assert_eq!(
            [seeded.next(), seeded.next(), seeded.next()],
            [
                10977518812293740004,
                13893246733018840292,
                1412386850724336324
            ]
        );
    }

    #[test]
    fn a_negative_speed_or_time_gives_no_ground() {
        let r = CombatRules::default();
        // A negative time is no time: the cast has allowed only its nudge.
        assert_eq!(allowance(Moving::Slowed, 7.0, -1.0, &r), r.cast_slack);
        // A negative speed is no speed: no slack past the nudge.
        assert_eq!(allowance(Moving::Slowed, -7.0, 1.0, &r), r.cast_slack);
    }

    #[test]
    fn a_step_backwards_in_time_passes_nothing() {
        let r = DodgeRules::default();
        let mut c = Charges::full(r.charges);
        assert!(c.spend(&r));
        let before = c;
        c.tick(-3.0, &r);
        assert_eq!(c, before);
    }

    #[test]
    fn spending_and_refilling_keep_the_timer_honest() {
        let r = DodgeRules::default();
        // A spend from full starts the timer at a full recharge.
        let mut c = Charges::full(r.charges);
        assert!(c.spend(&r));
        assert_eq!(c.next_in, r.recharge);
        // A spend below full leaves the timer where it was.
        c.tick(0.1, &r);
        let timer = c.next_in;
        assert!(c.spend(&r));
        assert_eq!(c.have, 0);
        assert_eq!(c.next_in, timer);
        // A step longer than the timer refills one charge and carries the overshoot on: the next
        // charge's wait is the rest of the recharge less the overshoot.
        c.tick(10.0, &r);
        assert_eq!(c.have, 1);
        assert!(
            (c.next_in - (timer - 10.0 + r.recharge)).abs() < 1e-4,
            "{}",
            c.next_in
        );
        // Ticks at full leave the charges at the most there are.
        let mut full = Charges::full(r.charges);
        for _ in 0..5 {
            full.tick(1.0, &r);
        }
        assert_eq!(full.have, r.charges);
        assert_eq!(full.next_in, 0.0);
    }

    #[test]
    fn a_tick_that_is_not_a_time_passes_nothing() {
        let r = DodgeRules::default();
        let mut c = Charges::full(r.charges);
        assert!(c.spend(&r));
        let before = c;
        c.tick(f32::NAN, &r);
        c.tick(f32::INFINITY, &r);
        assert_eq!(c, before);
    }
}
