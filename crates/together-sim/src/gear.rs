//! Meta equipment: armour and weapons a hero brings into a run from outside it.
//!
//! A hero wears up to one [`GearPiece`] in each [`Slot`] (head, shoulders, back, chest, wrists,
//! hands, waist, legs, feet, main hand and off hand; a two-handed weapon fills both hands). The
//! pieces come from a stash kept between runs ([`crate::Armory`]) and start every run
//! **dormant**: a hero switches one on by paying gold for it during the run, the same gold that
//! buys chests, so each coin is "a chest, or a piece I know" ([`GearPiece::price`]). A piece that
//! is on adds its stat lines to the hero's [`crate::Stats`], and a [`Unique`] also changes one
//! spell.
//!
//! Everything about a piece follows from its few stored fields: its slot, [`Rarity`], item level,
//! armour type, a `seed` and, for a legendary, its [`Unique`]. The seed rolls its stat lines and
//! its name, so a piece is small on the wire and in the save file, and a rebalance reaches pieces
//! already found.
//!
//! **The power budget.** Gear keeps well inside the agreed 40% late-game budget for talents and
//! gear together: a full set of eleven legendary pieces at the highest item level adds
//! [`FULL_SET_TOMES`] common tomes' worth of stats (about +22% damage, were it all damage), and a
//! more usual set of rare and epic pieces from the middle zones about half that. Uniques add their
//! spell's change on top, which pays off only when the run offers that spell.

use serde::{Deserialize, Serialize};

use crate::cards::Rarity;
use crate::rng::Rng;
use crate::spell::{CastSpec, Tag, CHILL_SECS, CHILL_SLOW};
use crate::stats::Stat;

/// Where a piece is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Slot {
    Head,
    Shoulders,
    Back,
    Chest,
    Wrists,
    Hands,
    Waist,
    Legs,
    Feet,
    MainHand,
    OffHand,
}

impl Slot {
    pub const COUNT: usize = 11;

    /// In the order the character sheet lists them.
    pub const ALL: [Slot; Slot::COUNT] = [
        Slot::Head,
        Slot::Shoulders,
        Slot::Back,
        Slot::Chest,
        Slot::Wrists,
        Slot::Hands,
        Slot::Waist,
        Slot::Legs,
        Slot::Feet,
        Slot::MainHand,
        Slot::OffHand,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Shoulders => "Shoulders",
            Slot::Back => "Back",
            Slot::Chest => "Chest",
            Slot::Wrists => "Wrists",
            Slot::Hands => "Hands",
            Slot::Waist => "Waist",
            Slot::Legs => "Legs",
            Slot::Feet => "Feet",
            Slot::MainHand => "Main Hand",
            Slot::OffHand => "Off Hand",
        }
    }

    /// Its index in [`Slot::ALL`], which is also how a [`crate::PlayerInput`] names it.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Slot> {
        Slot::ALL.get(i).copied()
    }

    pub fn is_weapon(self) -> bool {
        matches!(self, Slot::MainHand | Slot::OffHand)
    }

    /// Its share of the power budget: the eleven add up to [`FULL_SET_WEIGHT`]. A two-handed
    /// weapon counts as both hands.
    fn weight(self) -> f32 {
        match self {
            Slot::Head | Slot::Shoulders | Slot::OffHand => 1.0,
            Slot::Back | Slot::Wrists => 0.6,
            Slot::Chest | Slot::Legs => 1.2,
            Slot::Hands | Slot::Waist | Slot::Feet => 0.8,
            Slot::MainHand => 2.0,
        }
    }

    /// The stats its pieces roll, each with how often.
    fn pool(self) -> &'static [(Stat, u32)] {
        match self {
            Slot::Head => &[
                (Stat::Damage, 3),
                (Stat::Area, 3),
                (Stat::Crit, 2),
                (Stat::Health, 2),
                (Stat::XpGain, 1),
            ],
            Slot::Shoulders => &[
                (Stat::Area, 3),
                (Stat::Damage, 2),
                (Stat::CritDamage, 2),
                (Stat::Armor, 2),
            ],
            Slot::Back => &[
                (Stat::Speed, 2),
                (Stat::Armor, 2),
                (Stat::Regen, 2),
                (Stat::Pickup, 2),
            ],
            Slot::Chest => &[
                (Stat::Health, 3),
                (Stat::Armor, 3),
                (Stat::Damage, 2),
                (Stat::Regen, 2),
            ],
            Slot::Wrists => &[
                (Stat::Cooldown, 2),
                (Stat::Regen, 2),
                (Stat::Crit, 2),
                (Stat::GoldGain, 1),
            ],
            Slot::Hands => &[(Stat::Cooldown, 3), (Stat::Crit, 3), (Stat::CritDamage, 2)],
            Slot::Waist => &[
                (Stat::Health, 2),
                (Stat::Pickup, 2),
                (Stat::GoldGain, 2),
                (Stat::Luck, 2),
            ],
            Slot::Legs => &[
                (Stat::Health, 3),
                (Stat::Armor, 2),
                (Stat::Damage, 2),
                (Stat::Area, 2),
            ],
            Slot::Feet => &[(Stat::Speed, 4), (Stat::Pickup, 2), (Stat::Regen, 1)],
            Slot::MainHand => &[
                (Stat::Damage, 4),
                (Stat::Crit, 2),
                (Stat::CritDamage, 2),
                (Stat::Cooldown, 2),
            ],
            Slot::OffHand => &[
                (Stat::Cooldown, 3),
                (Stat::Area, 2),
                (Stat::Crit, 2),
                (Stat::Health, 1),
                (Stat::Armor, 1),
            ],
        }
    }
}

/// The eleven slots' budget shares added up.
pub const FULL_SET_WEIGHT: f32 = 11.0;
/// Common tomes' worth of stats per budget share, for a legendary piece at the highest item
/// level: a full such set is [`FULL_SET_TOMES`].
const TOMES_PER_WEIGHT: f32 = 0.25;
/// What a full set of legendary pieces at [`MAX_ITEM_LEVEL`] adds, in common tomes.
pub const FULL_SET_TOMES: f32 = FULL_SET_WEIGHT * TOMES_PER_WEIGHT;

/// The highest item level a zone gives today (Gates of Ahn'Qiraj); a piece's stats reach their
/// full size there. A difficulty ladder (9g, not built) would add item levels above it.
pub const MAX_ITEM_LEVEL: u16 = 65;

/// The cloth, leather, mail or plate a piece is made of: the finder's class decides it, and with
/// it the piece's look and name. It changes nothing in play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Armor {
    Cloth,
    Leather,
    Mail,
    Plate,
}

impl Armor {
    pub const ALL: [Armor; 4] = [Armor::Cloth, Armor::Leather, Armor::Mail, Armor::Plate];

    pub fn name(self) -> &'static str {
        match self {
            Armor::Cloth => "Cloth",
            Armor::Leather => "Leather",
            Armor::Mail => "Mail",
            Armor::Plate => "Plate",
        }
    }

    /// What a class wears, by its key: casters cloth, rogues and druids leather, hunters and
    /// shamans mail, warriors and paladins plate.
    pub fn for_class(key: &str) -> Armor {
        match key {
            "rogue" | "druid" => Armor::Leather,
            "hunter" | "shaman" => Armor::Mail,
            "warrior" | "paladin" => Armor::Plate,
            _ => Armor::Cloth,
        }
    }
}

/// What a weapon slot's piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    /// Two-handed.
    Staff,
    /// Two-handed.
    Greatsword,
    Sword,
    Mace,
    Axe,
    Dagger,
    Shield,
    /// An orb or tome held in the off hand.
    Orb,
}

impl WeaponKind {
    pub fn two_hand(self) -> bool {
        matches!(self, WeaponKind::Staff | WeaponKind::Greatsword)
    }

    fn noun(self) -> &'static str {
        match self {
            WeaponKind::Staff => "Staff",
            WeaponKind::Greatsword => "Greatsword",
            WeaponKind::Sword => "Sword",
            WeaponKind::Mace => "Mace",
            WeaponKind::Axe => "Axe",
            WeaponKind::Dagger => "Dagger",
            WeaponKind::Shield => "Shield",
            WeaponKind::Orb => "Orb",
        }
    }

    /// The kinds a slot's piece of `armor` can be.
    fn choices(slot: Slot, armor: Armor) -> &'static [WeaponKind] {
        use WeaponKind::*;
        match (slot, armor) {
            (Slot::OffHand, Armor::Cloth) => &[Orb],
            (Slot::OffHand, Armor::Leather) => &[Dagger, Sword, Orb],
            (Slot::OffHand, _) => &[Shield],
            (_, Armor::Cloth) => &[Staff, Staff, Dagger, Sword],
            (_, Armor::Leather) => &[Dagger, Sword, Mace, Staff],
            (_, Armor::Mail) => &[Axe, Mace, Staff, Sword],
            (_, Armor::Plate) => &[Greatsword, Sword, Mace, Axe],
        }
    }
}

/// A legendary piece that changes one spell. Every legendary is one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unique {
    /// Chain Lightning jumps further and slows.
    Thunderfury,
    /// Arcane Explosion pulls enemies in.
    Atiesh,
    /// Arcane Missiles fires more missiles.
    RobeOfTheArchmage,
    /// Fireball's burst is wider and burns.
    MantleOfTheBlackwingCabal,
    /// Frost Nova reaches further and freezes longer.
    GlacialCloak,
    /// Frostbolt's Chill can Freeze.
    FrostfireSandals,
    /// Flamestrike burns longer and comes sooner.
    GlovesOfTheHypnoticFlame,
    /// Cleave strikes more enemies, further.
    Ashkandi,
    /// Corruption afflicts more enemies.
    NemesisSkullcap,
}

/// How far Atiesh's pull reaches, and how far it drags an enemy toward the caster.
pub const ATIESH_REACH: f32 = 16.0;
pub const ATIESH_PULL: f32 = 4.0;

impl Unique {
    pub const ALL: [Unique; 9] = [
        Unique::Thunderfury,
        Unique::Atiesh,
        Unique::RobeOfTheArchmage,
        Unique::MantleOfTheBlackwingCabal,
        Unique::GlacialCloak,
        Unique::FrostfireSandals,
        Unique::GlovesOfTheHypnoticFlame,
        Unique::Ashkandi,
        Unique::NemesisSkullcap,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Unique::Thunderfury => "Thunderfury, Blessed Blade of the Windseeker",
            Unique::Atiesh => "Atiesh, Greatstaff of the Guardian",
            Unique::RobeOfTheArchmage => "Robe of the Archmage",
            Unique::MantleOfTheBlackwingCabal => "Mantle of the Blackwing Cabal",
            Unique::GlacialCloak => "Glacial Cloak",
            Unique::FrostfireSandals => "Frostfire Sandals",
            Unique::GlovesOfTheHypnoticFlame => "Gloves of the Hypnotic Flame",
            Unique::Ashkandi => "Ashkandi, Greatsword of the Brotherhood",
            Unique::NemesisSkullcap => "Nemesis Skullcap",
        }
    }

    pub fn slot(self) -> Slot {
        match self {
            Unique::Thunderfury | Unique::Atiesh | Unique::Ashkandi => Slot::MainHand,
            Unique::RobeOfTheArchmage => Slot::Chest,
            Unique::MantleOfTheBlackwingCabal => Slot::Shoulders,
            Unique::GlacialCloak => Slot::Back,
            Unique::FrostfireSandals => Slot::Feet,
            Unique::GlovesOfTheHypnoticFlame => Slot::Hands,
            Unique::NemesisSkullcap => Slot::Head,
        }
    }

    /// What it is made of: the armour it always is, whoever finds it.
    pub fn armor(self) -> Armor {
        match self {
            Unique::Thunderfury | Unique::Ashkandi => Armor::Plate,
            _ => Armor::Cloth,
        }
    }

    fn weapon(self) -> Option<WeaponKind> {
        match self {
            Unique::Thunderfury => Some(WeaponKind::Sword),
            Unique::Atiesh => Some(WeaponKind::Staff),
            Unique::Ashkandi => Some(WeaponKind::Greatsword),
            _ => None,
        }
    }

    /// The ability key of the spell it changes.
    pub fn spell(self) -> &'static str {
        match self {
            Unique::Thunderfury => "chain_lightning",
            Unique::Atiesh => "arcane_explosion",
            Unique::RobeOfTheArchmage => "arcane_missiles",
            Unique::MantleOfTheBlackwingCabal => "fireball",
            Unique::GlacialCloak => "frost_nova",
            Unique::FrostfireSandals => "frostbolt",
            Unique::GlovesOfTheHypnoticFlame => "flamestrike",
            Unique::Ashkandi => "cleave",
            Unique::NemesisSkullcap => "corruption",
        }
    }

    /// What it does, as its tooltip says after "Equip:".
    pub fn effect(self) -> &'static str {
        match self {
            Unique::Thunderfury => {
                "Chain Lightning jumps to 2 more enemies and Chills everything it strikes."
            }
            Unique::Atiesh => {
                "Arcane Explosion pulls enemies within 16 yards 4 yards toward you before it bursts, and reaches 20% further."
            }
            Unique::RobeOfTheArchmage => "Arcane Missiles fires 2 more missiles at each target.",
            Unique::MantleOfTheBlackwingCabal => {
                "Fireball's burst is 50% wider and leaves enemies Burning for 30% of the hit."
            }
            Unique::GlacialCloak => "Frost Nova reaches 30% further and Freezes 1 second longer.",
            Unique::FrostfireSandals => "Frostbolt's Chill has a 20% chance to Freeze.",
            Unique::GlovesOfTheHypnoticFlame => {
                "Flamestrike's burning ground lasts 3 seconds longer, and Flamestrike comes 20% sooner."
            }
            Unique::Ashkandi => "Cleave strikes 2 more enemies and reaches 30% further.",
            Unique::NemesisSkullcap => "Corruption afflicts 2 more enemies.",
        }
    }

    /// A line of lore under the effect, in WoW's flavour-text yellow.
    pub fn flavor(self) -> &'static str {
        match self {
            Unique::Thunderfury => "The prison of Thunderaan, Prince of Air.",
            Unique::Atiesh => "The staff of Medivh, last Guardian of Tirisfal.",
            Unique::RobeOfTheArchmage => "Stitched with threads of pure mana.",
            Unique::MantleOfTheBlackwingCabal => "Worn by the sorcerers of Nefarian's court.",
            Unique::GlacialCloak => "Cold enough to frost the air around it.",
            Unique::FrostfireSandals => "Fire and frost, bound at every step.",
            Unique::GlovesOfTheHypnoticFlame => "Taken from the hands of a Firelord.",
            Unique::Ashkandi => "Blessed by the Brotherhood of the Light.",
            Unique::NemesisSkullcap => "It whispers of things best left unsaid.",
        }
    }

    /// Its stat lines' shares of its budget.
    fn stat_shares(self) -> &'static [(Stat, f32)] {
        match self {
            Unique::Thunderfury => &[(Stat::Damage, 0.5), (Stat::Crit, 0.3), (Stat::Speed, 0.2)],
            Unique::Atiesh => &[
                (Stat::Area, 0.4),
                (Stat::Damage, 0.35),
                (Stat::Cooldown, 0.25),
            ],
            Unique::RobeOfTheArchmage => &[
                (Stat::Damage, 0.4),
                (Stat::Cooldown, 0.3),
                (Stat::Health, 0.3),
            ],
            Unique::MantleOfTheBlackwingCabal => &[(Stat::Damage, 0.5), (Stat::Crit, 0.5)],
            Unique::GlacialCloak => &[(Stat::Armor, 0.4), (Stat::Area, 0.3), (Stat::Health, 0.3)],
            Unique::FrostfireSandals => {
                &[(Stat::Speed, 0.5), (Stat::Crit, 0.3), (Stat::Damage, 0.2)]
            }
            Unique::GlovesOfTheHypnoticFlame => &[
                (Stat::Damage, 0.4),
                (Stat::Crit, 0.3),
                (Stat::CritDamage, 0.3),
            ],
            Unique::Ashkandi => &[
                (Stat::Damage, 0.5),
                (Stat::CritDamage, 0.3),
                (Stat::Health, 0.2),
            ],
            Unique::NemesisSkullcap => &[
                (Stat::Damage, 0.4),
                (Stat::Cooldown, 0.3),
                (Stat::Regen, 0.3),
            ],
        }
    }

    /// Change a cast of spell `key`, if it is this unique's spell. Atiesh's pull happens in the
    /// world, as the explosion goes off.
    pub(crate) fn shape(self, key: &str, s: &mut CastSpec) {
        if key != self.spell() {
            return;
        }
        match self {
            Unique::Thunderfury => {
                s.count += 2;
                s.chill = s.chill.max(CHILL_SECS);
                s.slow = s.slow.max(CHILL_SLOW);
                s.tags = s.tags.with(Tag::Slow);
            }
            Unique::Atiesh => s.radius *= 1.2,
            Unique::RobeOfTheArchmage => s.volley += 2,
            Unique::MantleOfTheBlackwingCabal => {
                s.radius *= 1.5;
                s.burn += 0.3;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Unique::GlacialCloak => {
                s.radius *= 1.3;
                s.freeze += 1.0;
            }
            Unique::FrostfireSandals => s.freeze_chance += 0.2,
            Unique::GlovesOfTheHypnoticFlame => {
                if s.ground > 0.0 {
                    s.ground += 3.0;
                }
                s.cooldown *= 0.8;
            }
            Unique::Ashkandi => {
                s.count += 2;
                s.range *= 1.3;
            }
            Unique::NemesisSkullcap => s.count += 2,
        }
    }
}

/// How much of the budget a rarity gets.
fn rarity_share(r: Rarity) -> f32 {
    match r {
        Rarity::Common => 0.45,
        Rarity::Uncommon => 0.6,
        Rarity::Rare => 0.75,
        Rarity::Epic => 0.9,
        Rarity::Legendary => 1.0,
    }
}

/// Stat lines a piece of this rarity rolls.
fn lines(r: Rarity) -> usize {
    match r {
        Rarity::Common => 1,
        Rarity::Uncommon | Rarity::Rare => 2,
        Rarity::Epic | Rarity::Legendary => 3,
    }
}

/// How big a piece of `item_level` is against one at [`MAX_ITEM_LEVEL`]: a quarter for the
/// lowest, growing evenly to all of it; past it (a harder difficulty) up to half again.
pub fn level_share(item_level: u16) -> f32 {
    (0.25 + 0.75 * f32::from(item_level) / f32::from(MAX_ITEM_LEVEL)).min(1.5)
}

/// The item level a zone of `zone_level` drops: a boss's pieces are three levels better. The
/// difficulty ladder (9g) would add `torment` steps of ten.
pub fn item_level(zone_level: u16, boss: bool, torment: u8) -> u16 {
    zone_level + if boss { 3 } else { 0 } + 10 * u16::from(torment)
}

/// One piece of equipment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GearPiece {
    pub slot: Slot,
    pub rarity: Rarity,
    pub item_level: u16,
    pub armor: Armor,
    /// Rolls its stat lines, its name and, for a weapon, what kind it is.
    pub seed: u32,
    /// A legendary's unique effect: always set on a legendary, never on anything else.
    #[serde(default)]
    pub unique: Option<Unique>,
}

impl GearPiece {
    /// A piece of everything but a [`Unique`]; a legendary rarity is held to epic, since every
    /// legendary is a unique.
    pub fn new(slot: Slot, rarity: Rarity, item_level: u16, armor: Armor, seed: u32) -> Self {
        Self {
            slot,
            rarity: rarity.min(Rarity::Epic),
            item_level,
            armor,
            seed,
            unique: None,
        }
    }

    /// The legendary `unique` at `item_level`.
    pub fn unique(unique: Unique, item_level: u16, seed: u32) -> Self {
        Self {
            slot: unique.slot(),
            rarity: Rarity::Legendary,
            item_level,
            armor: unique.armor(),
            seed,
            unique: Some(unique),
        }
    }

    /// Whether its fields agree with each other: a unique in its own slot and only on a
    /// legendary, a sane item level. A guest's gear that fails this is left at the door.
    pub fn valid(&self) -> bool {
        let unique_ok = match self.unique {
            Some(u) => u.slot() == self.slot && self.rarity == Rarity::Legendary,
            None => self.rarity != Rarity::Legendary,
        };
        unique_ok && (1..=MAX_ITEM_LEVEL + 50).contains(&self.item_level)
    }

    /// What a weapon slot's piece is; `None` for armour.
    pub fn weapon(&self) -> Option<WeaponKind> {
        if !self.slot.is_weapon() {
            return None;
        }
        if let Some(w) = self.unique.and_then(Unique::weapon) {
            return Some(w);
        }
        let choices = WeaponKind::choices(self.slot, self.armor);
        Some(choices[(self.seed as usize / 7) % choices.len()])
    }

    /// A two-handed weapon, which leaves no room for an off hand.
    pub fn two_hand(&self) -> bool {
        self.weapon().is_some_and(WeaponKind::two_hand)
    }

    /// Its share of the power budget, in common tomes.
    pub fn power(&self) -> f32 {
        let weight = if self.two_hand() {
            Slot::MainHand.weight() + Slot::OffHand.weight()
        } else {
            self.slot.weight()
        };
        weight * TOMES_PER_WEIGHT * rarity_share(self.rarity) * level_share(self.item_level)
    }

    /// Its stat lines, as [`crate::Stats::add`] takes them, biggest first.
    pub fn stats(&self) -> Vec<(Stat, f32)> {
        let shares: Vec<(Stat, f32)> = match self.unique {
            Some(u) => u.stat_shares().to_vec(),
            None => self.rolled_shares(),
        };
        let power = self.power();
        let mut out: Vec<(Stat, f32)> = shares
            .into_iter()
            .map(|(stat, share)| {
                let v = stat.tome_amount(Rarity::Common) * power * share;
                // Whole tenths of a percent (hundredths of a point of regeneration), as shown.
                let step = if stat == Stat::Regen { 0.01 } else { 0.001 };
                (stat, ((v / step).round() * step).max(step))
            })
            .collect();
        out.sort_by(|a, b| {
            let worth = |&(s, v): &(Stat, f32)| v / s.tome_amount(Rarity::Common);
            worth(b).total_cmp(&worth(a))
        });
        out
    }

    /// The stats the seed picks from the slot's pool, each with its share of the budget.
    fn rolled_shares(&self) -> Vec<(Stat, f32)> {
        let mut rng = Rng::new(u64::from(self.seed) ^ 0x6765_6172);
        let mut pool: Vec<(Stat, u32)> = self.slot.pool().to_vec();
        if self.weapon() == Some(WeaponKind::Shield) {
            pool.push((Stat::Armor, 3));
        }
        let mut picked: Vec<(Stat, f32)> = Vec::new();
        for i in 0..lines(self.rarity).min(pool.len()) {
            let total: u32 = pool.iter().map(|&(_, w)| w).sum();
            let mut roll = (rng.next_u64() % u64::from(total.max(1))) as u32;
            let at = pool
                .iter()
                .position(|&(_, w)| {
                    if roll < w {
                        true
                    } else {
                        roll -= w;
                        false
                    }
                })
                .unwrap_or(0);
            let (stat, _) = pool.remove(at);
            // The first line is the piece's main stat; the others share what is left.
            let weight = if i == 0 { 1.4 } else { rng.range(0.5, 1.0) };
            picked.push((stat, weight));
        }
        let sum: f32 = picked.iter().map(|&(_, w)| w).sum();
        picked
            .into_iter()
            .map(|(s, w)| (s, w / sum.max(1e-6)))
            .collect()
    }

    /// Its name: a unique's own, else its material, what it is and, from uncommon up, a suffix
    /// for its main stat ("Runed Silk Hood of the Owl").
    pub fn name(&self) -> String {
        if let Some(u) = self.unique {
            return u.name().to_string();
        }
        let band = usize::from(self.item_level.min(64) / 13).min(4);
        let noun = match self.weapon() {
            Some(w) => w.noun(),
            None => armor_noun(self.slot, self.armor),
        };
        let material = match self.weapon() {
            Some(WeaponKind::Staff | WeaponKind::Orb) => {
                ["Oaken", "Willow", "Ironwood", "Darkwood", "Arcane"][band]
            }
            Some(_) => ["Copper", "Bronze", "Iron", "Mithril", "Thorium"][band],
            None => material(self.armor, band),
        };
        let prefix = match self.rarity {
            Rarity::Rare => ["Runed ", "Ornate ", "Gleaming ", "Imbued "][(self.seed % 4) as usize],
            Rarity::Epic => ["Ancient ", "Eternal ", "Dread ", "Mythic "][(self.seed % 4) as usize],
            _ => "",
        };
        let suffix = match (self.rarity, self.stats().first()) {
            (Rarity::Common, _) | (_, None) => String::new(),
            (_, Some(&(stat, _))) => format!(" {}", suffix(stat)),
        };
        format!("{prefix}{material} {noun}{suffix}")
    }

    /// The gold it costs to switch on, with `already_on` pieces already switched on this run:
    /// its rarity's price, a little more for a higher item level, and ten more for every piece
    /// already on (so the order pieces are bought in never changes what all of them cost).
    pub fn price(&self, already_on: u32) -> u32 {
        let base = match self.rarity {
            Rarity::Common => 15.0,
            Rarity::Uncommon => 25.0,
            Rarity::Rare => 40.0,
            Rarity::Epic => 60.0,
            Rarity::Legendary => 90.0,
        };
        let level = 0.75 + f32::from(self.item_level) / 130.0;
        let raw = base * level;
        ((raw / 5.0).round() * 5.0) as u32 + 10 * already_on
    }
}

/// An armour piece's noun.
fn armor_noun(slot: Slot, armor: Armor) -> &'static str {
    let row: [&str; 9] = match armor {
        Armor::Cloth => [
            "Hood", "Mantle", "Cloak", "Robe", "Cuffs", "Gloves", "Cord", "Pants", "Sandals",
        ],
        Armor::Leather => [
            "Cap",
            "Shoulderpads",
            "Cloak",
            "Tunic",
            "Bracers",
            "Gloves",
            "Belt",
            "Leggings",
            "Boots",
        ],
        Armor::Mail => [
            "Coif",
            "Spaulders",
            "Cape",
            "Hauberk",
            "Wristguards",
            "Gauntlets",
            "Girdle",
            "Leggings",
            "Greaves",
        ],
        Armor::Plate => [
            "Helm",
            "Pauldrons",
            "Cape",
            "Breastplate",
            "Bracers",
            "Gauntlets",
            "Girdle",
            "Legplates",
            "Sabatons",
        ],
    };
    row.get(slot.index()).copied().unwrap_or("Charm")
}

/// The stuff an armour piece is made of, by its item-level band.
fn material(armor: Armor, band: usize) -> &'static str {
    match armor {
        Armor::Cloth => ["Linen", "Woolen", "Silk", "Mageweave", "Runecloth"][band],
        Armor::Leather => ["Rawhide", "Hide", "Thick Hide", "Wild", "Rugged"][band],
        Armor::Mail => ["Ringed", "Chain", "Scaled", "Mithril", "Thorium"][band],
        Armor::Plate => ["Copper", "Bronze", "Iron", "Mithril", "Thorium"][band],
    }
}

/// The suffix a piece whose main stat is `stat` takes, after WoW's "of the Owl".
fn suffix(stat: Stat) -> &'static str {
    match stat {
        Stat::Damage => "of the Tiger",
        Stat::Cooldown => "of the Owl",
        Stat::Area => "of the Sorcerer",
        Stat::Speed => "of the Cheetah",
        Stat::Health => "of the Bear",
        Stat::Regen => "of the Whale",
        Stat::Armor => "of Defense",
        Stat::Pickup => "of the Magpie",
        Stat::Luck => "of Fortune",
        Stat::XpGain => "of the Sage",
        Stat::GoldGain => "of Greed",
        Stat::Crit => "of the Eagle",
        Stat::CritDamage => "of Slaying",
        Stat::Projectiles => "of Multitudes",
    }
}

/// A stat line as a tooltip gives it, with tenths where they matter: "+2.4% Damage",
/// "+0.15 Health a second".
pub fn describe_line(stat: Stat, amount: f32) -> String {
    match stat {
        Stat::Regen => format!("+{amount:.2} health a second"),
        Stat::Projectiles => format!("+{amount:.0} projectiles"),
        _ => {
            let pct = amount * 100.0;
            let noun = match stat {
                Stat::Cooldown => "cooldown speed",
                Stat::Speed => "move speed",
                Stat::Health => "max health",
                Stat::Pickup => "pickup range",
                Stat::XpGain => "experience",
                Stat::GoldGain => "gold",
                Stat::Crit => "critical strike chance",
                Stat::CritDamage => "critical strike damage",
                other => return format!("+{pct:.1}% {}", other.name().to_lowercase()),
            };
            format!("+{pct:.1}% {noun}")
        }
    }
}

/// A piece a hero brought into the run, and whether its gold has been paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worn {
    pub piece: GearPiece,
    pub on: bool,
}

/// A hero's equipment in a run: what it brought (dormant until switched on) and what it found.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroGear {
    /// One piece a slot at most, in [`Slot::ALL`]'s order.
    pub worn: Vec<Worn>,
    /// Pieces that dropped for this hero this run; they go to the stash, for the next run.
    pub found: Vec<GearPiece>,
}

impl HeroGear {
    /// What a hero brings: [`loadout`] of `pieces`, all dormant.
    pub fn bring(pieces: &[GearPiece]) -> Self {
        Self {
            worn: loadout(pieces)
                .into_iter()
                .map(|piece| Worn { piece, on: false })
                .collect(),
            found: Vec::new(),
        }
    }

    /// The pieces switched on.
    pub fn active(&self) -> impl Iterator<Item = &GearPiece> {
        self.worn.iter().filter(|w| w.on).map(|w| &w.piece)
    }

    pub fn switched_on(&self) -> u32 {
        self.worn.iter().filter(|w| w.on).count() as u32
    }

    /// The piece in `slot`, on or not.
    pub fn in_slot(&self, slot: Slot) -> Option<&Worn> {
        self.worn.iter().find(|w| w.piece.slot == slot)
    }

    /// What switching on the dormant piece in `slot` costs; `None` if there is none.
    pub fn price(&self, slot: Slot) -> Option<u32> {
        let w = self.in_slot(slot).filter(|w| !w.on)?;
        Some(w.piece.price(self.switched_on()))
    }

    /// Whether `unique` is worn and switched on.
    pub fn has(&self, unique: Unique) -> bool {
        self.active().any(|p| p.unique == Some(unique))
    }

    /// The dormant piece to switch on next with `gold`: the most powerful one it pays for.
    pub fn next_affordable(&self, gold: u32) -> Option<Slot> {
        let n = self.switched_on();
        self.worn
            .iter()
            .filter(|w| !w.on && w.piece.price(n) <= gold)
            .max_by(|a, b| {
                (a.piece.rarity, a.piece.power())
                    .partial_cmp(&(b.piece.rarity, b.piece.power()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|w| w.piece.slot)
    }

    /// Change a cast of spell `key` by every unique switched on.
    pub(crate) fn shape(&self, key: &str, s: &mut CastSpec) {
        for p in self.active() {
            if let Some(u) = p.unique {
                u.shape(key, s);
            }
        }
    }
}

/// `pieces` as one hero can wear them: valid ones only, the first for each slot, and no off hand
/// beside a two-handed weapon; in [`Slot::ALL`]'s order.
pub fn loadout(pieces: &[GearPiece]) -> Vec<GearPiece> {
    let mut out: Vec<GearPiece> = Vec::new();
    for p in pieces.iter().filter(|p| p.valid()) {
        if out.iter().any(|q| q.slot == p.slot) {
            continue;
        }
        out.push(*p);
    }
    if out.iter().any(|p| p.two_hand()) {
        out.retain(|p| p.slot != Slot::OffHand);
    }
    out.sort_by_key(|p| p.slot);
    out
}

/// The chance a boss, and an elite, drops a piece for each hero.
pub const BOSS_DROP: f32 = 0.5;
pub const ELITE_DROP: f32 = 0.012;

/// A dropped piece's rarity: in 1000, from Common to Legendary, a boss's and an elite's.
const BOSS_RARITY: [u32; 5] = [250, 350, 250, 120, 30];
const ELITE_RARITY: [u32; 5] = [450, 350, 150, 45, 5];

/// The piece that drops for a hero: a rarity rolled with its `luck` (as cards roll theirs), a
/// slot at random, `armor` for its class; a legendary is a unique, three times likelier one for a
/// spell in `spells` (the hero's class's pool).
pub(crate) fn roll_drop(
    rng: &mut Rng,
    boss: bool,
    luck: f32,
    item_level: u16,
    armor: Armor,
    spells: &[&str],
) -> GearPiece {
    let table = if boss { BOSS_RARITY } else { ELITE_RARITY };
    let weight = |i: usize| f64::from(table[i]) * f64::from(1.0 + luck.max(0.0)).powi(i as i32);
    let total: f64 = (0..5).map(weight).sum();
    let mut pick = f64::from(rng.f32()) * total;
    let mut rarity = Rarity::Common;
    for (i, r) in Rarity::ALL.into_iter().enumerate() {
        if pick < weight(i) {
            rarity = r;
            break;
        }
        pick -= weight(i);
    }
    let seed = rng.next_u64() as u32;
    if rarity == Rarity::Legendary {
        let w = |u: &Unique| if spells.contains(&u.spell()) { 3 } else { 1 };
        let total: u32 = Unique::ALL.iter().map(w).sum();
        let mut roll = (rng.next_u64() % u64::from(total)) as u32;
        let unique = Unique::ALL
            .into_iter()
            .find(|u| {
                if roll < w(u) {
                    true
                } else {
                    roll -= w(u);
                    false
                }
            })
            .unwrap_or(Unique::Thunderfury);
        return GearPiece::unique(unique, item_level, seed);
    }
    let slot = Slot::ALL[rng.index(Slot::COUNT)];
    GearPiece::new(slot, rarity, item_level, armor, seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(slot: Slot, rarity: Rarity, level: u16) -> GearPiece {
        GearPiece::new(slot, rarity, level, Armor::Cloth, 1234)
    }

    #[test]
    fn the_slots_add_up_to_the_full_set() {
        let sum: f32 = Slot::ALL.iter().map(|s| s.weight()).sum();
        assert!((sum - FULL_SET_WEIGHT).abs() < 1e-4);
    }

    #[test]
    fn a_full_best_set_stays_well_inside_the_power_budget() {
        // Best in every slot: Legendary rarity (as if every slot had a unique) at the top item
        // level, one-handed weapon and off hand.
        let mut tomes = 0.0;
        for slot in Slot::ALL {
            let p = (0..100)
                .map(|seed| GearPiece {
                    rarity: Rarity::Legendary,
                    seed,
                    ..piece(slot, Rarity::Common, MAX_ITEM_LEVEL)
                })
                .find(|p| !p.two_hand())
                .expect("a one-handed roll");
            tomes += p.power();
        }
        // A two-hander is worth exactly a main hand and an off hand.
        let staff = GearPiece::unique(Unique::Atiesh, MAX_ITEM_LEVEL, 1);
        let one = (Slot::MainHand.weight() + Slot::OffHand.weight()) * TOMES_PER_WEIGHT;
        assert!((staff.power() - one).abs() < 1e-5);
        assert!(tomes <= FULL_SET_TOMES + 1e-4, "{tomes}");
        // A common tome is +8% damage: the set is worth under a quarter more power, against the
        // 40% agreed for talents and gear together.
        assert!(tomes * 0.08 < 0.25, "{}", tomes * 0.08);
        // Every stat line adds up to the piece's power.
        let p = piece(Slot::Chest, Rarity::Epic, 40);
        let worth: f32 = p
            .stats()
            .iter()
            .map(|&(s, v)| v / s.tome_amount(Rarity::Common))
            .sum();
        assert!((worth - p.power()).abs() < 0.02, "{worth} vs {}", p.power());
    }

    #[test]
    fn rarer_and_higher_pieces_are_stronger() {
        let low = piece(Slot::Head, Rarity::Rare, 10).power();
        let high = piece(Slot::Head, Rarity::Rare, 60).power();
        assert!(high > low * 2.0);
        assert!(
            piece(Slot::Head, Rarity::Epic, 30).power()
                > piece(Slot::Head, Rarity::Uncommon, 30).power()
        );
        assert_eq!(lines(Rarity::Common), 1);
        assert_eq!(piece(Slot::Legs, Rarity::Epic, 30).stats().len(), 3);
        assert_eq!(piece(Slot::Legs, Rarity::Common, 30).stats().len(), 1);
    }

    #[test]
    fn the_same_seed_rolls_the_same_piece() {
        let a = piece(Slot::Hands, Rarity::Rare, 25);
        assert_eq!(a.stats(), a.stats());
        assert_eq!(a.name(), a.name());
        let other = GearPiece { seed: 99, ..a };
        // Different seeds usually differ somewhere.
        let differs = (0..20u32).any(|s| {
            let b = GearPiece {
                seed: s * 7919,
                ..a
            };
            b.stats() != a.stats()
        });
        assert!(differs, "{other:?}");
    }

    #[test]
    fn names_read_like_wow_items() {
        let p = piece(Slot::Head, Rarity::Common, 10);
        assert_eq!(p.name(), "Linen Hood");
        let u = GearPiece::unique(Unique::Thunderfury, 40, 1);
        assert_eq!(u.name(), "Thunderfury, Blessed Blade of the Windseeker");
        let r = piece(Slot::Feet, Rarity::Rare, 30);
        assert!(r.name().contains("Silk Sandals of the"), "{}", r.name());
    }

    #[test]
    fn a_legendary_is_always_a_unique_and_a_unique_keeps_its_slot() {
        let p = GearPiece::new(Slot::Head, Rarity::Legendary, 20, Armor::Cloth, 1);
        assert_eq!(p.rarity, Rarity::Epic);
        assert!(GearPiece::unique(Unique::Atiesh, 30, 5).valid());
        let wrong = GearPiece {
            slot: Slot::Feet,
            ..GearPiece::unique(Unique::Atiesh, 30, 5)
        };
        assert!(!wrong.valid());
        let fake = GearPiece {
            rarity: Rarity::Legendary,
            ..p
        };
        assert!(!fake.valid());
        assert!(GearPiece::unique(Unique::Atiesh, 30, 5).two_hand());
    }

    #[test]
    fn a_loadout_has_one_piece_a_slot_and_no_off_hand_beside_a_two_hander() {
        let staff = GearPiece::unique(Unique::Atiesh, 30, 5);
        let orb = GearPiece::new(Slot::OffHand, Rarity::Rare, 30, Armor::Cloth, 3);
        let hood = piece(Slot::Head, Rarity::Rare, 30);
        let hood2 = piece(Slot::Head, Rarity::Epic, 30);
        let l = loadout(&[orb, hood, staff, hood2]);
        assert_eq!(l, vec![hood, staff]);
        assert_eq!(loadout(&[orb, hood]), vec![hood, orb]);
    }

    #[test]
    fn prices_rise_with_rarity_level_and_pieces_already_on() {
        let c = piece(Slot::Head, Rarity::Common, 10).price(0);
        let e = piece(Slot::Head, Rarity::Epic, 10).price(0);
        assert!(e > c * 2, "{c} {e}");
        assert!(piece(Slot::Head, Rarity::Epic, 60).price(0) > e);
        assert_eq!(piece(Slot::Head, Rarity::Epic, 10).price(3), e + 30);
        // Order does not matter: two pieces cost the same in either order.
        let (a, b) = (
            piece(Slot::Head, Rarity::Rare, 30),
            piece(Slot::Feet, Rarity::Common, 30),
        );
        assert_eq!(a.price(0) + b.price(1), b.price(0) + a.price(1));
    }

    #[test]
    fn the_next_piece_is_the_best_one_the_gold_pays_for() {
        let gear = HeroGear::bring(&[
            piece(Slot::Head, Rarity::Common, 10),
            piece(Slot::Chest, Rarity::Epic, 10),
        ]);
        let common = gear.price(Slot::Head).unwrap();
        let epic = gear.price(Slot::Chest).unwrap();
        assert_eq!(gear.next_affordable(common), Some(Slot::Head));
        assert_eq!(gear.next_affordable(epic), Some(Slot::Chest));
        assert_eq!(gear.next_affordable(0), None);
    }

    #[test]
    fn uniques_change_only_their_own_spell() {
        let content = crate::content::tests::sample();
        let def = &content.abilities[0];
        let base = CastSpec::base(0, def);
        let mut s = base.clone();
        Unique::Thunderfury.shape("frostbolt", &mut s);
        assert_eq!(s, base);
        Unique::Thunderfury.shape("chain_lightning", &mut s);
        assert_eq!(s.count, base.count + 2);
        assert!(s.chill > 0.0 && s.tags.has(Tag::Slow));
        for u in Unique::ALL {
            assert!(!u.effect().is_empty() && !u.flavor().is_empty());
            assert!(GearPiece::unique(u, 30, 1).valid(), "{u:?}");
        }
    }

    #[test]
    fn drops_lean_rarer_from_bosses_and_with_luck() {
        let mut rng = Rng::new(9);
        let count = |rng: &mut Rng, boss: bool, luck: f32| {
            (0..4000)
                .filter(|_| {
                    roll_drop(rng, boss, luck, 30, Armor::Cloth, &[]).rarity >= Rarity::Epic
                })
                .count()
        };
        let elite = count(&mut rng, false, 0.0);
        let boss = count(&mut rng, true, 0.0);
        let lucky = count(&mut rng, true, 1.0);
        assert!(boss > elite * 2, "{elite} {boss}");
        assert!(lucky > boss, "{boss} {lucky}");
        for _ in 0..200 {
            let p = roll_drop(&mut rng, true, 0.0, 33, Armor::Plate, &[]);
            assert!(p.valid(), "{p:?}");
        }
    }

    #[test]
    fn a_class_finds_its_own_spells_uniques_more_often() {
        let mut rng = Rng::new(4);
        let mut mage = 0;
        let mut total = 0;
        for _ in 0..20000 {
            let p = roll_drop(
                &mut rng,
                true,
                3.0,
                30,
                Armor::Cloth,
                &["frostbolt", "fireball", "arcane_explosion"],
            );
            if let Some(u) = p.unique {
                total += 1;
                if matches!(
                    u,
                    Unique::FrostfireSandals | Unique::MantleOfTheBlackwingCabal | Unique::Atiesh
                ) {
                    mage += 1;
                }
            }
        }
        // Three of nine uniques at triple weight: 9 of 15, against 3 of 9 unweighted.
        let share = mage as f32 / total as f32;
        assert!(share > 0.5, "{share}");
    }

    #[test]
    fn stat_lines_read_with_tenths() {
        assert_eq!(describe_line(Stat::Damage, 0.024), "+2.4% damage");
        assert_eq!(
            describe_line(Stat::Crit, 0.006),
            "+0.6% critical strike chance"
        );
        assert_eq!(describe_line(Stat::Regen, 0.15), "+0.15 health a second");
    }
}
