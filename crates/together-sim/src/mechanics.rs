//! How elites and bosses fight beyond walking up and swinging: charges, slams, leaps, casters,
//! frenzies, healers, summoners, and the bosses' ground patches and death grips.
//!
//! The rule (agreed): **every mechanic can be avoided by moving.** Each one is telegraphed on the
//! ground first (a [`Shape`] that goes off a moment later), and none forces damage on a hero who
//! steps out in time. Ordinary enemies never use them: a creature's mechanics are listed in
//! `content/enemies.ron`, and only its elites and bosses act on them. Freezing or stunning an
//! elite while it winds up cancels what it was about to do.

use serde::{Deserialize, Serialize};

/// One thing an elite or boss can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mechanic {
    /// Winds up, then dashes along a marked line, hitting whoever stands on it.
    Charge,
    /// Keeps its distance and marks small circles under heroes that burst a moment later.
    Caster,
    /// Marks a circle around itself, then slams: whoever is inside is hit and thrown out.
    Slam,
    /// Marks a circle under a hero from range, then leaps into it.
    Leap,
    /// Faster and quicker to strike below half health.
    Frenzy,
    /// Heals the enemies around it every few seconds.
    Healer,
    /// Calls two more enemies to its side every few seconds.
    Summoner,
    /// A boss's patches of fire (or poison, or sand) around every hero: walk through the gaps.
    Patches,
    /// A boss marks the ground under a hero; whoever is still inside is pulled to it.
    DeathGrip,
}

impl Mechanic {
    /// One sentence for the boss bar or an elite's target frame.
    pub fn hint(self) -> &'static str {
        match self {
            Mechanic::Charge => "Charges along a marked line: step aside.",
            Mechanic::Caster => "Casts from range: its circles burst a moment later.",
            Mechanic::Slam => "Slams the ground around it: leave the circle.",
            Mechanic::Leap => "Leaps at you from range: leave the circle under you.",
            Mechanic::Frenzy => "Frenzies below half health.",
            Mechanic::Healer => "Heals the enemies around it: pull them away, or burst it.",
            Mechanic::Summoner => "Calls more enemies to its side.",
            Mechanic::Patches => "Fire bursts around you: move through the gaps.",
            Mechanic::DeathGrip => "Marks the ground: anyone still inside is pulled to it.",
        }
    }

    /// Seconds between uses, and before the first.
    pub(crate) fn cooldown(self) -> (f32, f32) {
        match self {
            Mechanic::Charge => (7.0, 3.0),
            Mechanic::Caster => (3.0, 1.5),
            Mechanic::Slam => (8.0, 2.0),
            Mechanic::Leap => (9.0, 4.0),
            Mechanic::Frenzy => (f32::MAX, f32::MAX),
            Mechanic::Healer => (3.0, 2.0),
            Mechanic::Summoner => (9.0, 4.0),
            Mechanic::Patches => (10.0, 6.0),
            Mechanic::DeathGrip => (15.0, 10.0),
        }
    }
}

/// The ground a telegraph marks: a circle, or a line from its position to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Circle { radius: f32 },
    Line { to: crate::math::V2, width: f32 },
}

impl Shape {
    /// Whether `p` stands in this shape laid at `origin`, with `pad` yards of body to spare.
    pub fn contains(&self, origin: crate::math::V2, p: crate::math::V2, pad: f32) -> bool {
        match *self {
            Shape::Circle { radius } => p.distance(origin) <= radius + pad,
            Shape::Line { to, width } => {
                let (d, off) = (to - origin, p - origin);
                let len2 = d.x * d.x + d.y * d.y;
                let t = if len2 > 0.0 {
                    ((off.x * d.x + off.y * d.y) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                p.distance(origin + d * t) <= width * 0.5 + pad
            }
        }
    }
}
