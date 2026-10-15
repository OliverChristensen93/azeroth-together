//! Power-ups and the other things that lie on the ground for a hero to grab: a health shard, or
//! a power-up that works for the whole party at once, as in a zombie round.
//!
//! Most power-ups last a few seconds ([`PowerUp::duration`]); a Detonation and a Magnet act once,
//! the moment they are taken. They turn up now and then near the heroes, and from broken pots.

use serde::{Deserialize, Serialize};

use crate::math::V2;

/// A short boon for the whole party.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PowerUp {
    /// Everyone runs faster.
    Swiftness,
    /// Everyone's spells hit much harder.
    Berserk,
    /// Everyone's cooldowns run twice as fast.
    Bloodlust,
    /// Nothing can hurt anyone.
    DivineShield,
    /// Twice the gold.
    Fortune,
    /// A blast around whoever took it that slays every ordinary enemy near and wounds bosses.
    Detonation,
    /// All experience and gold on the ground flies to whoever took it.
    Magnet,
}

impl PowerUp {
    pub const ALL: [PowerUp; 7] = [
        PowerUp::Swiftness,
        PowerUp::Berserk,
        PowerUp::Bloodlust,
        PowerUp::DivineShield,
        PowerUp::Fortune,
        PowerUp::Detonation,
        PowerUp::Magnet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PowerUp::Swiftness => "Swiftness",
            PowerUp::Berserk => "Berserk",
            PowerUp::Bloodlust => "Bloodlust",
            PowerUp::DivineShield => "Divine Shield",
            PowerUp::Fortune => "Goblin Fortune",
            PowerUp::Detonation => "Arcane Detonation",
            PowerUp::Magnet => "Magnet",
        }
    }

    /// What it does, for the HUD.
    pub fn description(self) -> &'static str {
        match self {
            PowerUp::Swiftness => "+50% move speed",
            PowerUp::Berserk => "Double damage",
            PowerUp::Bloodlust => "Cooldowns twice as fast",
            PowerUp::DivineShield => "Nothing can hurt you",
            PowerUp::Fortune => "Double gold",
            PowerUp::Detonation => "Slays every enemy nearby",
            PowerUp::Magnet => "Pulls in all experience and gold",
        }
    }

    /// Seconds it lasts, or `None` for one that acts once.
    pub fn duration(self) -> Option<f32> {
        match self {
            PowerUp::Swiftness | PowerUp::Berserk | PowerUp::Bloodlust => Some(10.0),
            PowerUp::DivineShield => Some(6.0),
            PowerUp::Fortune => Some(20.0),
            PowerUp::Detonation | PowerUp::Magnet => None,
        }
    }
}

/// [`PowerUp::Swiftness`]'s extra speed, [`PowerUp::Berserk`]'s extra damage, and
/// [`PowerUp::Bloodlust`]'s share of each cooldown.
pub const SWIFTNESS: f32 = 0.5;
pub const BERSERK: f32 = 1.0;
pub const BLOODLUST: f32 = 0.5;
/// [`PowerUp::Detonation`]'s reach, and its share of a boss's health.
pub const DETONATION_RADIUS: f32 = 25.0;
pub const DETONATION_BOSS: f32 = 0.15;

/// What lies on the ground to grab, besides experience and coins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PickupKind {
    /// Heals whoever takes it by [`HEALTH_SHARD`] of their health.
    Health,
    PowerUp(PowerUp),
}

/// A health shard's share of its taker's health.
pub const HEALTH_SHARD: f32 = 0.25;

/// Something on the ground, gone after `left` seconds if nobody takes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pickup {
    pub id: u32,
    pub pos: V2,
    pub kind: PickupKind,
    pub left: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_blast_and_the_magnet_act_at_once() {
        for p in PowerUp::ALL {
            let instant = matches!(p, PowerUp::Detonation | PowerUp::Magnet);
            assert_eq!(p.duration().is_none(), instant, "{p:?}");
            assert!(!p.description().is_empty());
        }
    }
}
