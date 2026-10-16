//! Cards: what a level-up, a shrine, an altar or a chest offers, how rare each is, and the numbers
//! an ability has at a rank.
//!
//! A card's rarity decides how strong it is: a new spell starts with a damage bonus, an upgrade adds
//! one on top of its rank, and a tome or a pact's boon is worth more stat. The client shows these
//! numbers on the cards, so they come from here, where the simulation uses them too.

use serde::{Deserialize, Serialize};

use crate::augments::Augment;
use crate::content::{AbilityDef, AbilityKind};
use crate::items::Item;
use crate::rng::Rng;
use crate::spell::{CastSpec, Tag, Targeting};
use crate::stats::{Pact, Stat, Stats};

/// The highest rank an ability reaches: one rank per upgrade card. Ranks 5, 10 and 15 each bring
/// an augment; the power is in the upgrades' [`Boost`]s, not the rank.
pub const MAX_RANK: u8 = 15;

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
        let weight =
            |r: Rarity| f64::from(r.weight()) * f64::from(1.0 + luck.max(0.0)).powi(r as i32);
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
    /// Raise a known spell one rank, with up to three boosts of the card's rarity.
    Upgrade(usize, [Option<Boost>; 3]),
    /// Raise a stat by a tome of the card's rarity ([`Stat::tome_amount`]).
    Tome(Stat),
    /// A Cursed Altar's bargain: a boon of the card's rarity, and its bane.
    Pact(Pact),
    /// One more copy of an item; the card's rarity is the item's own.
    Item(Item),
    /// When nothing else is left to deal: a purse of gold ([`fallback_gold`]).
    Gold,
    /// When nothing else is left to deal: health back to full.
    Heal,
    /// When nothing else is left to deal: one more reroll.
    Reroll,
    /// An augment for a known spell (an index into the content's abilities) at its milestone.
    Augment(usize, Augment),
}

impl CardKind {
    /// The cards that fill a hand when spells, tomes and items have run out.
    pub const FALLBACKS: [CardKind; 3] = [CardKind::Gold, CardKind::Heal, CardKind::Reroll];
}

/// The gold a [`CardKind::Gold`] card gives at the party's `level`, before gold gain.
pub fn fallback_gold(level: u32) -> u32 {
    20 + 5 * level
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
            CardKind::Learn(a) | CardKind::Upgrade(a, _) | CardKind::Augment(a, _) => Some(a),
            _ => None,
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

/// An ability's own numbers with the damage `bonus` its learning card gave (0.25 is +25%); its
/// upgrades ([`SpellMods`]) and augments come on top when it is cast.
pub fn ability_stats(def: &AbilityDef, bonus: f32) -> AbilityStats {
    AbilityStats {
        damage: def.damage * (1.0 + bonus),
        cooldown: def.cooldown.max(MIN_COOLDOWN),
        count: def.count,
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

/// What an upgrade card improves on a spell. Which a card can roll depends on the spell's tags
/// ([`Boost::fits`]); how much, and how many at once, on the card's rarity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Boost {
    Damage,
    Cooldown,
    /// Range, for a spell that picks its targets.
    Reach,
    /// Area, splash or cone size.
    Area,
    /// How long a ground area or a burn lasts.
    Duration,
    /// How often a burn or a ground area ticks.
    Ticks,
    Shield,
    Projectiles,
    Crit,
    CritDamage,
    /// How much its Chill slows.
    Slow,
    /// How long its Freeze holds.
    Freeze,
}

impl Boost {
    pub const ALL: [Boost; 12] = [
        Boost::Damage,
        Boost::Cooldown,
        Boost::Reach,
        Boost::Area,
        Boost::Duration,
        Boost::Ticks,
        Boost::Shield,
        Boost::Projectiles,
        Boost::Crit,
        Boost::CritDamage,
        Boost::Slow,
        Boost::Freeze,
    ];

    /// What a common card gives.
    fn base(self) -> f32 {
        match self {
            Boost::Damage => 0.12,
            Boost::Cooldown => 0.06,
            Boost::Reach => 0.10,
            Boost::Area => 0.10,
            Boost::Duration => 0.12,
            Boost::Ticks => 0.10,
            Boost::Shield => 0.12,
            Boost::Projectiles => 1.0,
            Boost::Crit => 0.04,
            Boost::CritDamage => 0.15,
            Boost::Slow => 0.05,
            Boost::Freeze => 0.3,
        }
    }

    /// What a card of `rarity` gives: more the rarer. Projectiles come whole, and only on rare cards
    /// and better ([`Boost::fits`]).
    pub fn amount(self, rarity: Rarity) -> f32 {
        if self == Boost::Projectiles {
            return if rarity == Rarity::Legendary {
                2.0
            } else {
                1.0
            };
        }
        let scale = match rarity {
            Rarity::Common => 1.0,
            Rarity::Uncommon | Rarity::Rare => 1.5,
            Rarity::Epic => 2.0,
            Rarity::Legendary => 2.5,
        };
        self.base() * scale
    }

    /// The card's line for `amount` of it: "Cooldown -9%".
    pub fn describe(self, amount: f32) -> String {
        let pct = (amount * 100.0).round();
        match self {
            Boost::Damage => format!("Damage +{pct}%"),
            Boost::Cooldown => format!("Cooldown -{pct}%"),
            Boost::Reach => format!("Range +{pct}%"),
            Boost::Area => format!("Area +{pct}%"),
            Boost::Duration => format!("Duration +{pct}%"),
            Boost::Ticks => format!("Tick rate +{pct}%"),
            Boost::Shield => format!("Shield +{pct}%"),
            Boost::Projectiles => format!("Projectiles +{}", amount.round()),
            Boost::Crit => format!("Crit chance +{pct}%"),
            Boost::CritDamage => format!("Crit damage +{pct}%"),
            Boost::Slow => format!("Slow +{pct}%"),
            Boost::Freeze => format!("Freeze +{amount:.1} sec"),
        }
    }

    /// Whether it means anything for a spell of `kind` cast as `spec`, on a card of `rarity`.
    pub fn fits(self, spec: &CastSpec, kind: AbilityKind, rarity: Rarity) -> bool {
        let t = spec.tags;
        match self {
            Boost::Damage => kind != AbilityKind::Shield,
            Boost::Cooldown => true,
            Boost::Reach => {
                matches!(
                    kind,
                    AbilityKind::Bolt | AbilityKind::Strike | AbilityKind::Ground
                ) && matches!(
                    spec.targeting,
                    Targeting::Nearest | Targeting::ElitesFirst | Targeting::Densest
                )
            }
            Boost::Area => t.any(&[Tag::Area, Tag::Cone, Tag::Ground]) || spec.radius > 0.0,
            Boost::Duration => t.any(&[Tag::Ground, Tag::DamageOverTime]),
            Boost::Ticks => t.has(Tag::DamageOverTime),
            Boost::Shield => t.has(Tag::Shield),
            Boost::Projectiles => t.has(Tag::Projectile) && rarity >= Rarity::Rare,
            Boost::Crit | Boost::CritDamage => t.has(Tag::Direct),
            Boost::Slow => t.has(Tag::Slow),
            Boost::Freeze => t.has(Tag::Freeze),
        }
    }
}

/// How many boosts an upgrade card of `rarity` gives.
pub fn boost_lines(rarity: Rarity) -> usize {
    match rarity {
        Rarity::Common | Rarity::Uncommon => 1,
        Rarity::Rare | Rarity::Epic => 2,
        Rarity::Legendary => 3,
    }
}

/// Everything a spell's upgrades have added up to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpellMods {
    pub damage: f32,
    pub cooldown: f32,
    pub reach: f32,
    pub area: f32,
    pub duration: f32,
    pub ticks: f32,
    pub shield: f32,
    pub projectiles: f32,
    pub crit: f32,
    pub crit_damage: f32,
    pub slow: f32,
    pub freeze: f32,
}

impl SpellMods {
    fn slot(&mut self, boost: Boost) -> &mut f32 {
        match boost {
            Boost::Damage => &mut self.damage,
            Boost::Cooldown => &mut self.cooldown,
            Boost::Reach => &mut self.reach,
            Boost::Area => &mut self.area,
            Boost::Duration => &mut self.duration,
            Boost::Ticks => &mut self.ticks,
            Boost::Shield => &mut self.shield,
            Boost::Projectiles => &mut self.projectiles,
            Boost::Crit => &mut self.crit,
            Boost::CritDamage => &mut self.crit_damage,
            Boost::Slow => &mut self.slow,
            Boost::Freeze => &mut self.freeze,
        }
    }

    pub fn add(&mut self, boost: Boost, amount: f32) {
        *self.slot(boost) += amount;
    }

    pub fn get(&self, boost: Boost) -> f32 {
        *self.clone().slot(boost)
    }

    /// Every boost it holds, in [`Boost::ALL`]'s order, with its total.
    pub fn totals(&self) -> Vec<(Boost, f32)> {
        Boost::ALL
            .into_iter()
            .map(|b| (b, self.get(b)))
            .filter(|&(_, v)| v > 1e-4)
            .collect()
    }

    /// These upgrades on a cast's numbers.
    pub fn apply(&self, s: &mut CastSpec) {
        s.damage *= 1.0 + self.damage;
        s.cooldown = (s.cooldown * (1.0 - self.cooldown).max(0.3)).max(MIN_COOLDOWN);
        s.range *= 1.0 + self.reach;
        s.radius *= 1.0 + self.area;
        s.ground *= 1.0 + self.duration;
        s.dot_secs *= 1.0 + self.duration;
        s.tick_rate *= 1.0 + self.ticks;
        s.shield *= 1.0 + self.shield;
        s.count += self.projectiles.round() as u32;
        s.crit += self.crit;
        s.crit_mult += self.crit_damage;
        s.slow = (s.slow + self.slow).min(0.8);
        if s.freeze > 0.0 {
            s.freeze += self.freeze;
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
    fn a_learning_bonus_grows_damage() {
        let def = crate::content::tests::sample().abilities[0].clone();
        let plain = ability_stats(&def, 0.0);
        let boosted = ability_stats(&def, 0.5);
        assert!((boosted.damage - plain.damage * 1.5).abs() < 1e-4);
    }

    #[test]
    fn boosts_scale_with_rarity_and_fit_their_tags() {
        assert!(Boost::Damage.amount(Rarity::Epic) > Boost::Damage.amount(Rarity::Common));
        assert_eq!(Boost::Projectiles.amount(Rarity::Rare), 1.0);
        assert_eq!(Boost::Cooldown.describe(0.09), "Cooldown -9%");
        let bolt = crate::augments::tests::spec(&[Tag::Frost, Tag::Projectile, Tag::Direct]);
        assert!(Boost::Projectiles.fits(&bolt, AbilityKind::Bolt, Rarity::Epic));
        assert!(!Boost::Projectiles.fits(&bolt, AbilityKind::Bolt, Rarity::Common));
        assert!(Boost::Crit.fits(&bolt, AbilityKind::Bolt, Rarity::Common));
        assert!(!Boost::Shield.fits(&bolt, AbilityKind::Bolt, Rarity::Common));
        assert!(!Boost::Ticks.fits(&bolt, AbilityKind::Bolt, Rarity::Common));
    }

    #[test]
    fn spell_mods_add_up_and_shape_a_cast() {
        let mut m = SpellMods::default();
        m.add(Boost::Damage, 0.2);
        m.add(Boost::Damage, 0.1);
        m.add(Boost::Projectiles, 1.0);
        let mut s = crate::augments::tests::spec(&[Tag::Projectile]);
        m.apply(&mut s);
        assert!((s.damage - 13.0).abs() < 1e-4);
        assert_eq!(s.count, 2);
        assert_eq!(m.totals().len(), 2);
    }

    #[test]
    fn hero_stats_sharpen_every_ability() {
        let def = crate::content::tests::sample().abilities[1].clone();
        let plain = ability_stats(&def, 0.0);
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
