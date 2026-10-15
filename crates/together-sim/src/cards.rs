//! Level-up cards: what they offer, how rare they are, and the numbers an ability has at a rank.
//!
//! A card's rarity decides how strong it is: a new spell starts with a damage bonus, an upgrade adds
//! one on top of its rank. The client shows these numbers on the cards, so they come from here,
//! where the simulation uses them too.

use crate::content::AbilityDef;
use crate::rng::Rng;

/// The highest rank an ability reaches.
pub const MAX_RANK: u8 = 8;

/// How rare a card is, in WoW's item-quality order and colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// Chance in 1000 of a card being this rare.
    fn weight(self) -> u32 {
        match self {
            Rarity::Common => 600,
            Rarity::Uncommon => 250,
            Rarity::Rare => 100,
            Rarity::Epic => 40,
            Rarity::Legendary => 10,
        }
    }

    pub(crate) fn roll(rng: &mut Rng) -> Rarity {
        let total: u32 = Rarity::ALL.iter().map(|r| r.weight()).sum();
        let mut pick = (rng.next_u64() % u64::from(total)) as u32;
        for r in Rarity::ALL {
            if pick < r.weight() {
                return r;
            }
            pick -= r.weight();
        }
        Rarity::Common
    }

    /// The bonus this rarity gives a card of `kind`.
    pub fn bonus(self, kind: CardKind) -> f32 {
        match kind {
            CardKind::Learn => self.learn_bonus(),
            CardKind::Upgrade => self.upgrade_bonus(),
        }
    }
}

/// Whether a card teaches a new spell or raises a known one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Learn,
    Upgrade,
}

/// A level-up card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    /// Index into the content's abilities.
    pub ability: usize,
    pub kind: CardKind,
    pub rarity: Rarity,
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

/// An ability's numbers at `rank` (1 up to [`MAX_RANK`]): each rank adds 30% damage and takes 7%
/// off the cooldown, and ranks 3, 5 and 7 each add a target.
pub fn ability_stats(def: &AbilityDef, rank: u8, bonus: f32) -> AbilityStats {
    let rank = rank.clamp(1, MAX_RANK);
    let extra = [3u8, 5, 7].iter().filter(|&&r| rank >= r).count() as u32;
    AbilityStats {
        damage: def.damage * (1.0 + 0.3 * f32::from(rank - 1)) * (1.0 + bonus),
        cooldown: (def.cooldown * 0.93f32.powi(i32::from(rank) - 1)).max(0.2),
        count: def.count + extra,
        range: def.range,
        radius: def.radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rarities_roll_in_their_proportions() {
        let mut rng = Rng::new(9);
        let mut seen = [0u32; 5];
        for _ in 0..100_000 {
            seen[Rarity::roll(&mut rng) as usize] += 1;
        }
        assert!(seen[0] > 55_000 && seen[0] < 65_000, "{seen:?}");
        assert!(seen[4] > 500 && seen[4] < 1_500, "{seen:?}");
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
}
