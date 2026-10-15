//! Items: passive treasures from chests and caches, each stacking with copies of itself.
//!
//! Some raise a stat when taken; the rest act on their own: on a hit, on a kill, on a blow taken
//! or on a chest opened. An item that strikes enemies casts a content ability ([`Item::proc`],
//! in no class's pool) for its visual and its row in the damage meter; its damage comes from the
//! hit or the kill that set it off.

use serde::{Deserialize, Serialize};

use crate::cards::Rarity;
use crate::stats::Stat;

/// A passive treasure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Item {
    /// A hit may arc lightning to enemies around its target.
    StaticShard,
    /// A kill may burst into flame, burning the enemies around the corpse.
    EmberCore,
    /// Every kill heals.
    VampiricFang,
    /// A chest may cost nothing.
    SkeletonKey,
    /// More luck.
    RabbitsFoot,
    /// More critical strikes.
    BerserkersRing,
    /// Blocks a blow now and then.
    FrostWard,
    /// A kill may spill gold.
    GoblinPurse,
}

impl Item {
    pub const ALL: [Item; 8] = [
        Item::StaticShard,
        Item::EmberCore,
        Item::VampiricFang,
        Item::SkeletonKey,
        Item::RabbitsFoot,
        Item::BerserkersRing,
        Item::FrostWard,
        Item::GoblinPurse,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Item::StaticShard => "Static Shard",
            Item::EmberCore => "Ember Core",
            Item::VampiricFang => "Vampiric Fang",
            Item::SkeletonKey => "Skeleton Key",
            Item::RabbitsFoot => "Rabbit's Foot",
            Item::BerserkersRing => "Berserker's Ring",
            Item::FrostWard => "Frost Ward",
            Item::GoblinPurse => "Goblin Purse",
        }
    }

    /// How rare it is: the rarer, the less often a chest has it.
    pub fn rarity(self) -> Rarity {
        match self {
            Item::RabbitsFoot | Item::GoblinPurse => Rarity::Common,
            Item::VampiricFang | Item::SkeletonKey | Item::BerserkersRing => Rarity::Uncommon,
            Item::StaticShard | Item::EmberCore => Rarity::Rare,
            Item::FrostWard => Rarity::Epic,
        }
    }

    /// The content ability whose visual and meter row its strikes use, if it strikes.
    pub fn proc(self) -> Option<&'static str> {
        match self {
            Item::StaticShard => Some("static_shard"),
            Item::EmberCore => Some("ember_core"),
            Item::FrostWard => Some("frost_ward"),
            _ => None,
        }
    }

    /// The stat it raises per copy when taken, if it is a stat item.
    pub fn stat(self) -> Option<(Stat, f32)> {
        match self {
            Item::RabbitsFoot => Some((Stat::Luck, 0.2)),
            Item::BerserkersRing => Some((Stat::Crit, 0.06)),
            _ => None,
        }
    }

    /// The chance its effect fires, with `stacks` copies: each copy adds the same again, up to a
    /// cap.
    pub fn chance(self, stacks: u8) -> f32 {
        let (each, cap) = match self {
            Item::StaticShard => (0.08, 0.5),
            Item::EmberCore => (0.12, 0.6),
            Item::SkeletonKey => (0.25, 1.0),
            Item::GoblinPurse => (0.04, 0.4),
            _ => (1.0, 1.0),
        };
        (each * f32::from(stacks)).min(cap)
    }

    /// What it does, with `stacks` copies, as its card says.
    pub fn describe(self, stacks: u8) -> String {
        let pct = |x: f32| format!("{:.0}%", x * 100.0);
        let n = f32::from(stacks);
        match self {
            Item::StaticShard => format!(
                "{} of hits arc lightning to {SHARD_CHAIN} enemies nearby for half the damage",
                pct(self.chance(stacks))
            ),
            Item::EmberCore => format!(
                "{} of kills burst, burning enemies within {EMBER_RADIUS:.0} yd",
                pct(self.chance(stacks))
            ),
            Item::VampiricFang => format!("Every kill heals {:.1} health", FANG_HEAL * n),
            Item::SkeletonKey => format!("{} of chests are free", pct(self.chance(stacks))),
            Item::RabbitsFoot | Item::BerserkersRing => {
                let (stat, each) = self.stat().expect("a stat item");
                stat.describe(each * n)
            }
            Item::FrostWard => format!(
                "Blocks a blow every {:.0} sec",
                ward_cooldown(stacks)
            ),
            Item::GoblinPurse => format!(
                "{} of kills spill extra gold",
                pct(self.chance(stacks))
            ),
        }
    }
}

/// Enemies a Static Shard's lightning arcs to, and how far it reaches from its target.
pub const SHARD_CHAIN: usize = 3;
pub const SHARD_REACH: f32 = 8.0;
/// A Static Shard arc's share of the hit that set it off.
pub const SHARD_SHARE: f32 = 0.5;
/// How far an Ember Core's burst reaches, and its share of the dead enemy's health.
pub const EMBER_RADIUS: f32 = 5.0;
pub const EMBER_SHARE: f32 = 0.4;
/// Health a Vampiric Fang heals per kill, per copy.
pub const FANG_HEAL: f32 = 0.5;
/// A Goblin Purse's spill, in the enemy's experience.
pub const PURSE_GOLD: f32 = 3.0;

/// Seconds between the blows a Frost Ward blocks, with `stacks` copies: 15, then 2 less per
/// copy, never under 5.
pub fn ward_cooldown(stacks: u8) -> f32 {
    (15.0 - 2.0 * f32::from(stacks.saturating_sub(1))).max(5.0)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_stack_up_to_a_cap() {
        assert!(Item::StaticShard.chance(2) > Item::StaticShard.chance(1));
        assert_eq!(Item::StaticShard.chance(50), 0.5);
        assert!(ward_cooldown(3) < ward_cooldown(1));
        assert_eq!(ward_cooldown(40), 5.0);
    }

    #[test]
    fn every_item_says_what_it_does() {
        for item in Item::ALL {
            assert!(!item.describe(1).is_empty(), "{item:?}");
        }
        assert_eq!(Item::RabbitsFoot.describe(2), "+40% luck");
    }
}
