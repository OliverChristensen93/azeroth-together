//! Cards: what a level-up, a shrine, an altar or a chest offers, how rare each is, and the numbers
//! an ability has at a rank.
//!
//! A card's rarity decides how strong it is: a new spell starts with a damage bonus, an upgrade adds
//! one on top of its rank, and a tome or a pact's boon is worth more stat. The client shows these
//! numbers on the cards, so they come from here, where the simulation uses them too.

use serde::{Deserialize, Serialize};

use crate::content::AbilityDef;
use crate::rng::Rng;
use crate::stats::{Pact, Stat, Stats};

/// The highest rank an ability reaches.
pub const MAX_RANK: u8 = 8;

/// How rare a card is, in WoW's item-quality order and colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }

    /// WoW's quality colour, `0xRRGGBB`.
    pub fn color(self) -> u32 {
        match self {
            Rarity::Common => 0xFF_FF_FF,
            Rarity::Uncommon => 0x1E_FF_00,
            Rarity::Rare => 0x00_70_DD,
            Rarity::Epic => 0xA3_35_EE,
            Rarity::Legendary => 0xFF_80_00,
        }
    }

    /// The damage bonus a newly learned spell starts with.
    pub fn learn_bonus(self) -> f32 {
        match self {
            Rarity::Common => 0.0,
            Rarity::Uncommon => 0.15,
            Rarity::Rare => 0.25,
            Rarity::Epic => 0.35,
            Rarity::Legendary => 0.5,
        }
    }

    /// The damage bonus an upgrade adds on top of its rank.
    pub fn upgrade_bonus(self) -> f32 {
        match self {
            Rarity::Common => 0.0,
            Rarity::Uncommon => 0.08,
            Rarity::Rare => 0.15,
            Rarity::Epic => 0.25,
            Rarity::Legendary => 0.4,
        }
    }

    /// How many common tomes a tome of this rarity is worth.
    pub fn scale(self) -> f32 {
        match self {
            Rarity::Common => 1.0,
            Rarity::Uncommon => 1.4,
            Rarity::Rare => 1.9,
            Rarity::Epic => 2.6,
            Rarity::Legendary => 3.5,
        }
    }

    /// Chance in 1000 of a card being this rare, without luck.
    fn weight(self) -> u32 {
        match self {
            Rarity::Common => 600,
            Rarity::Uncommon => 250,
            Rarity::Rare => 100,
            Rarity::Epic => 40,
            Rarity::Legendary => 10,
        }
    }

    /// A rarity at random. `luck` (a hero's [`Stat::Luck`]) makes every tier above Common likelier,
    /// the rarer ones the more: 0.5 luck makes Uncommon 1.5 times as likely, Legendary 5 times.
    pub(crate) fn roll(rng: &mut Rng, luck: f32) -> Rarity {
        let weight = |r: Rarity| f64::from(r.weight()) * f64::from(1.0 + luck.max(0.0)).powi(r as i32);
        let total: f64 = Rarity::ALL.iter().map(|&r| weight(r)).sum();
        let mut pick = f64::from(rng.f32()) * total;
        for r in Rarity::ALL {
            if pick < weight(r) {
                return r;
            }
            pick -= weight(r);
        }
        Rarity::Legendary
    }
}

/// What a card does when picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardKind {
    /// Teach a new spell: an index into the content's abilities.
    Learn(usize),
    /// Raise a known spell one rank.
    Upgrade(usize),
    /// Raise a stat by a tome of the card's rarity ([`Stat::tome_amount`]).
    Tome(Stat),
    /// A Cursed Altar's bargain: a boon of the card's rarity, and its bane.
    Pact(Pact),
}

/// A card in a hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub kind: CardKind,
    pub rarity: Rarity,
}

impl Card {
    /// The ability it teaches or raises, if it is a spell card.
    pub fn ability(&self) -> Option<usize> {
        match self.kind {
            CardKind::Learn(a) | CardKind::Upgrade(a) => Some(a),
            CardKind::Tome(_) | CardKind::Pact(_) => None,
        }
    }
}

/// What an ability does at a rank, with a damage `bonus` (0.25 is +25%).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilityStats {
    /// Per hit.
    pub damage: f32,
    /// Seconds between casts.
    pub cooldown: f32,
    /// Targets per cast, for the single-target kinds.
    pub count: u32,
    pub range: f32,
    pub radius: f32,
}

/// The shortest a cooldown gets.
const MIN_COOLDOWN: f32 = 0.2;

/// An ability's numbers at `rank` (1 up to [`MAX_RANK`]): each rank adds 30% damage and takes 7%
/// off the cooldown, and ranks 3, 5 and 7 each add a target.
pub fn ability_stats(def: &AbilityDef, rank: u8, bonus: f32) -> AbilityStats {
    let rank = rank.clamp(1, MAX_RANK);
    let extra = [3u8, 5, 7].iter().filter(|&&r| rank >= r).count() as u32;
    AbilityStats {
        damage: def.damage * (1.0 + 0.3 * f32::from(rank - 1)) * (1.0 + bonus),
        cooldown: (def.cooldown * 0.93f32.powi(i32::from(rank) - 1)).max(MIN_COOLDOWN),
        count: def.count + extra,
        range: def.range,
        radius: def.radius,
    }
}

impl AbilityStats {
    /// These numbers with a hero's [`Stats`] on top: more damage, shorter cooldowns, wider areas
    /// and more targets.
    pub fn with(self, stats: &Stats) -> AbilityStats {
        AbilityStats {
            damage: self.damage * (1.0 + stats.get(Stat::Damage)),
            cooldown: (self.cooldown * (1.0 - stats.get(Stat::Cooldown))).max(MIN_COOLDOWN),
            count: self.count + stats.get(Stat::Projectiles).round() as u32,
            range: self.range,
            radius: self.radius * (1.0 + stats.get(Stat::Area)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tally(luck: f32) -> [u32; 5] {
        let mut rng = Rng::new(9);
        let mut seen = [0u32; 5];
        for _ in 0..100_000 {
            seen[Rarity::roll(&mut rng, luck) as usize] += 1;
        }
        seen
    }

    #[test]
    fn rarities_roll_in_their_proportions() {
        let seen = tally(0.0);
        assert!(seen[0] > 55_000 && seen[0] < 65_000, "{seen:?}");
        assert!(seen[4] > 500 && seen[4] < 1_500, "{seen:?}");
    }

    #[test]
    fn luck_makes_rare_cards_likelier() {
        let plain = tally(0.0);
        let lucky = tally(1.0);
        assert!(lucky[0] < plain[0], "{lucky:?} vs {plain:?}");
        assert!(lucky[4] > plain[4] * 8, "{lucky:?} vs {plain:?}");
    }

    #[test]
    fn ranks_grow_damage_and_shrink_cooldown() {
        let def = crate::content::tests::sample().abilities[0].clone();
        let r1 = ability_stats(&def, 1, 0.0);
        let r3 = ability_stats(&def, 3, 0.0);
        assert!(r3.damage > r1.damage && r3.cooldown < r1.cooldown);
        assert_eq!(r3.count, r1.count + 1);
        let boosted = ability_stats(&def, 1, 0.5);
        assert!((boosted.damage - r1.damage * 1.5).abs() < 1e-4);
    }

    #[test]
    fn hero_stats_sharpen_every_ability() {
        let def = crate::content::tests::sample().abilities[1].clone();
        let plain = ability_stats(&def, 1, 0.0);
        let mut stats = Stats::default();
        stats.add(Stat::Damage, 0.5);
        stats.add(Stat::Cooldown, 0.2);
        stats.add(Stat::Area, 0.25);
        stats.add(Stat::Projectiles, 2.0);
        let sharp = plain.with(&stats);
        assert!((sharp.damage - plain.damage * 1.5).abs() < 1e-4);
        assert!((sharp.cooldown - plain.cooldown * 0.8).abs() < 1e-4);
        assert!((sharp.radius - plain.radius * 1.25).abs() < 1e-4);
        assert_eq!(sharp.count, plain.count + 2);
        assert_eq!(plain.with(&Stats::default()), plain);
    }
}
