//! A hero's stat block, and the pacts that trade one stat for a curse.
//!
//! Every hero carries [`Stats`]: bonuses on top of its class and its spells, raised by tomes
//! (stat cards), pacts and items. Each [`Stat`] is a plain number: a fraction for the percentage
//! stats (0.08 is +8%), health a second for [`Stat::Regen`], whole targets for
//! [`Stat::Projectiles`]. The world reads them where they matter: casting, moving, taking a hit,
//! picking things up and rolling a card's rarity.
//!
//! A [`Pact`] is a Cursed Altar's offer: a strong boon for this hero paired with a [`Bane`], which
//! either weakens the hero or makes the whole party's horde stronger ([`Difficulty`]).

use serde::{Deserialize, Serialize};

use crate::cards::Rarity;

/// One number in a hero's stat block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    /// More damage from every spell.
    Damage,
    /// Shorter cooldowns: 0.1 takes a tenth off each.
    Cooldown,
    /// Wider novas and splashes.
    Area,
    /// Faster running.
    Speed,
    /// More maximum health.
    Health,
    /// Health back every second.
    Regen,
    /// Less damage from every blow: 0.1 takes a tenth off.
    Armor,
    /// Experience and gold are collected from further away.
    Pickup,
    /// Rarer cards and items.
    Luck,
    /// More experience from every gem.
    XpGain,
    /// More gold from every coin and shrine.
    GoldGain,
    /// A better chance of a critical strike, which hits twice as hard.
    Crit,
    /// More targets for every missile and strike.
    Projectiles,
}

impl Stat {
    pub const COUNT: usize = 13;

    pub const ALL: [Stat; Stat::COUNT] = [
        Stat::Damage,
        Stat::Cooldown,
        Stat::Area,
        Stat::Speed,
        Stat::Health,
        Stat::Regen,
        Stat::Armor,
        Stat::Pickup,
        Stat::Luck,
        Stat::XpGain,
        Stat::GoldGain,
        Stat::Crit,
        Stat::Projectiles,
    ];

    /// The stats tomes raise: all but [`Stat::Projectiles`], which only pacts and a golden shrine
    /// give.
    pub const TOMES: [Stat; Stat::COUNT - 1] = [
        Stat::Damage,
        Stat::Cooldown,
        Stat::Area,
        Stat::Speed,
        Stat::Health,
        Stat::Regen,
        Stat::Armor,
        Stat::Pickup,
        Stat::Luck,
        Stat::XpGain,
        Stat::GoldGain,
        Stat::Crit,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stat::Damage => "Damage",
            Stat::Cooldown => "Cooldown",
            Stat::Area => "Area",
            Stat::Speed => "Move Speed",
            Stat::Health => "Max Health",
            Stat::Regen => "Regeneration",
            Stat::Armor => "Armor",
            Stat::Pickup => "Pickup Range",
            Stat::Luck => "Luck",
            Stat::XpGain => "Experience",
            Stat::GoldGain => "Gold",
            Stat::Crit => "Critical Strike",
            Stat::Projectiles => "Projectiles",
        }
    }

    /// The tome that raises it, for the card's title.
    pub fn tome(self) -> &'static str {
        match self {
            Stat::Damage => "Tome of Might",
            Stat::Cooldown => "Tome of Haste",
            Stat::Area => "Tome of Reach",
            Stat::Speed => "Tome of the Cheetah",
            Stat::Health => "Tome of Fortitude",
            Stat::Regen => "Tome of Renewal",
            Stat::Armor => "Tome of Stoneskin",
            Stat::Pickup => "Tome of the Magpie",
            Stat::Luck => "Tome of Fortune",
            Stat::XpGain => "Tome of Wisdom",
            Stat::GoldGain => "Tome of Commerce",
            Stat::Crit => "Tome of Precision",
            Stat::Projectiles => "Tome of Multitudes",
        }
    }

    /// What a common tome adds.
    fn base(self) -> f32 {
        match self {
            Stat::Damage => 0.08,
            Stat::Cooldown => 0.04,
            Stat::Area => 0.08,
            Stat::Speed => 0.05,
            Stat::Health => 0.10,
            Stat::Regen => 0.4,
            Stat::Armor => 0.03,
            Stat::Pickup => 0.20,
            Stat::Luck => 0.10,
            Stat::XpGain => 0.08,
            Stat::GoldGain => 0.12,
            Stat::Crit => 0.03,
            Stat::Projectiles => 1.0,
        }
    }

    /// What a tome of `rarity` adds: [`Rarity::scale`] times a common one's. Projectiles come
    /// one at a time, whatever the rarity.
    pub fn tome_amount(self, rarity: Rarity) -> f32 {
        match self {
            Stat::Projectiles => 1.0,
            _ => self.base() * rarity.scale(),
        }
    }

    /// The range the stat is held to when read: cooldowns and armor never take everything off,
    /// and nothing is lost below a quarter of itself.
    fn bounds(self) -> (f32, f32) {
        match self {
            Stat::Cooldown => (-1.0, 0.6),
            Stat::Armor => (-0.5, 0.75),
            Stat::Regen => (0.0, 1000.0),
            Stat::Crit => (-0.05, 0.95),
            Stat::Projectiles => (0.0, 10.0),
            _ => (-0.75, 100.0),
        }
    }

    /// `amount` of this stat as a card shows it: "+8% damage", "-15% max health",
    /// "+0.4 health a second", "+1 target for every missile and strike".
    pub fn describe(self, amount: f32) -> String {
        let sign = if amount < 0.0 { "-" } else { "+" };
        let a = amount.abs();
        let noun = match self {
            Stat::Regen => return format!("{sign}{a:.1} health a second"),
            Stat::Projectiles => {
                let n = a.round() as u32;
                let s = if n == 1 { "" } else { "s" };
                return format!("{sign}{n} target{s} for every missile and strike");
            }
            Stat::Damage => "damage",
            Stat::Cooldown => "cooldown speed",
            Stat::Area => "area",
            Stat::Speed => "move speed",
            Stat::Health => "max health",
            Stat::Armor => "armor",
            Stat::Pickup => "pickup range",
            Stat::Luck => "luck",
            Stat::XpGain => "experience",
            Stat::GoldGain => "gold",
            Stat::Crit => "critical strike chance",
        };
        format!("{sign}{:.0}% {noun}", a * 100.0)
    }
}

/// A hero's bonuses, one number per [`Stat`], all zero for a fresh hero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats([f32; Stat::COUNT]);

impl Stats {
    /// The stat as the world uses it, held to its bounds.
    pub fn get(&self, stat: Stat) -> f32 {
        let (lo, hi) = stat.bounds();
        self.0[stat as usize].clamp(lo, hi)
    }

    /// Add to (or, negative, take from) a stat. The sum is kept as it is and only bounded when
    /// read, so a boon and a bane cancel whatever order they came in.
    pub fn add(&mut self, stat: Stat, amount: f32) {
        self.0[stat as usize] += amount;
    }
}

/// How much harder the party's horde has become, as fractions on top of the run's own growth:
/// 0.05 is 5% more. Greed and pacts raise it; nothing lowers it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Difficulty {
    pub enemy_health: f32,
    pub enemy_damage: f32,
    pub enemy_speed: f32,
    /// More enemies alive at once.
    pub horde: f32,
}

impl Difficulty {
    /// The average of the four, for a one-number readout.
    pub fn overall(&self) -> f32 {
        (self.enemy_health + self.enemy_damage + self.enemy_speed + self.horde) / 4.0
    }
}

/// The price of a pact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bane {
    /// This hero loses some of a stat.
    Weaker(Stat),
    /// Every enemy, for everyone, gets more health.
    EnemyHealth,
    /// Every enemy hits harder.
    EnemyDamage,
    /// Every enemy runs faster.
    EnemySpeed,
    /// More enemies at once.
    Horde,
}

impl Bane {
    /// Every bane an altar can ask for.
    pub const ALL: [Bane; 8] = [
        Bane::Weaker(Stat::Health),
        Bane::Weaker(Stat::Speed),
        Bane::Weaker(Stat::Armor),
        Bane::Weaker(Stat::Cooldown),
        Bane::EnemyHealth,
        Bane::EnemyDamage,
        Bane::EnemySpeed,
        Bane::Horde,
    ];

    /// How much it takes: a stat's loss (as [`Stats::add`] takes it, so negative), or the
    /// difficulty it adds.
    pub fn amount(self) -> f32 {
        match self {
            Bane::Weaker(Stat::Health) => -0.15,
            Bane::Weaker(Stat::Armor) => -0.10,
            Bane::Weaker(_) => -0.08,
            Bane::EnemyHealth | Bane::EnemyDamage => 0.12,
            Bane::EnemySpeed => 0.10,
            Bane::Horde => 0.15,
        }
    }

    /// Whether the whole party pays it, rather than just the hero who took the pact.
    pub fn shared(self) -> bool {
        !matches!(self, Bane::Weaker(_))
    }

    /// The bane as a card shows it.
    pub fn describe(self) -> String {
        let pct = (self.amount().abs() * 100.0).round();
        match self {
            Bane::Weaker(stat) => stat.describe(self.amount()),
            Bane::EnemyHealth => format!("Enemies have +{pct:.0}% health"),
            Bane::EnemyDamage => format!("Enemies deal +{pct:.0}% damage"),
            Bane::EnemySpeed => format!("Enemies are {pct:.0}% faster"),
            Bane::Horde => format!("+{pct:.0}% more enemies"),
        }
    }

    /// Pay it: from the hero's stats, or onto the party's difficulty.
    pub fn apply(self, stats: &mut Stats, difficulty: &mut Difficulty) {
        let a = self.amount();
        match self {
            Bane::Weaker(stat) => stats.add(stat, a),
            Bane::EnemyHealth => difficulty.enemy_health += a,
            Bane::EnemyDamage => difficulty.enemy_damage += a,
            Bane::EnemySpeed => difficulty.enemy_speed += a,
            Bane::Horde => difficulty.horde += a,
        }
    }
}

/// A pact's boon, in tomes of the same rarity.
pub const PACT_BOON: f32 = 2.5;

/// A Cursed Altar's offer: a boon worth [`PACT_BOON`] tomes of its rarity, for a bane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pact {
    pub boon: Stat,
    pub bane: Bane,
}

impl Pact {
    /// What the boon adds at `rarity`.
    pub fn boon_amount(self, rarity: Rarity) -> f32 {
        match self.boon {
            Stat::Projectiles => 1.0,
            stat => stat.tome_amount(rarity) * PACT_BOON,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_are_bounded_when_read_not_when_added() {
        let mut s = Stats::default();
        s.add(Stat::Cooldown, 0.5);
        s.add(Stat::Cooldown, 0.5);
        assert_eq!(s.get(Stat::Cooldown), 0.6, "cooldowns never vanish");
        // A bane after the cap still takes from the real sum.
        s.add(Stat::Cooldown, -0.3);
        assert!((s.get(Stat::Cooldown) - 0.6).abs() < 1e-6);
        s.add(Stat::Cooldown, -0.5);
        assert!((s.get(Stat::Cooldown) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn rarer_tomes_add_more_except_projectiles() {
        let common = Stat::Damage.tome_amount(Rarity::Common);
        let legendary = Stat::Damage.tome_amount(Rarity::Legendary);
        assert!(legendary > common * 3.0);
        assert_eq!(Stat::Projectiles.tome_amount(Rarity::Legendary), 1.0);
    }

    #[test]
    fn descriptions_read_like_cards() {
        assert_eq!(Stat::Damage.describe(0.08), "+8% damage");
        assert_eq!(Stat::Health.describe(-0.15), "-15% max health");
        assert_eq!(Stat::Regen.describe(0.4), "+0.4 health a second");
        assert_eq!(
            Stat::Projectiles.describe(1.0),
            "+1 target for every missile and strike"
        );
        assert_eq!(Bane::EnemySpeed.describe(), "Enemies are 10% faster");
    }

    #[test]
    fn a_bane_weakens_the_hero_or_hardens_the_horde() {
        let mut s = Stats::default();
        let mut d = Difficulty::default();
        Bane::Weaker(Stat::Health).apply(&mut s, &mut d);
        assert!(s.get(Stat::Health) < 0.0);
        assert_eq!(d, Difficulty::default());
        Bane::Horde.apply(&mut s, &mut d);
        assert!(d.horde > 0.0);
        assert!(Bane::Horde.shared() && !Bane::Weaker(Stat::Speed).shared());
    }

    #[test]
    fn a_pact_boon_outweighs_a_tome() {
        let pact = Pact {
            boon: Stat::Damage,
            bane: Bane::EnemyHealth,
        };
        assert!(pact.boon_amount(Rarity::Rare) > Stat::Damage.tome_amount(Rarity::Rare) * 2.0);
    }
}
