//! Augments: how a spell grows past its numbers. At rank 5 and rank 15 a spell takes a
//! **general** augment, from a shared library open to any spell with the right tags; at rank 10 it
//! takes one of its own three **capstones**, always the same three.
//!
//! A general augment changes a cast's numbers or adds a behaviour ([`Augment::shape`] does the
//! numbers); a capstone is a mechanic of its own, which the world checks where it acts (on a cast,
//! a hit, a crit, a kill, a blow taken).

use serde::{Deserialize, Serialize};

use crate::spell::{CastSpec, Tag, Tags, Targeting};

/// What a spell's next augment is: a general one (ranks 5 and 15) or one of its capstones
/// (rank 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    General,
    Capstone,
}

/// The rank at which a spell takes its `n`th augment (from 0): 5, 10, then 15.
pub fn milestone_rank(n: usize) -> u8 {
    match n {
        0 => 5,
        1 => 10,
        _ => 15,
    }
}

/// Who may roll a general augment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requires {
    /// Any spell.
    Any,
    /// A spell with any of these tags.
    AnyOf(&'static [Tag]),
    /// A spell with all of these tags.
    AllOf(&'static [Tag]),
    /// A spell that can Chill, Freeze or Stun.
    Control,
    /// A spell whose cooldown is at least this many seconds (tenths of a second, to stay `Eq`).
    Cooldown(u32),
    /// A ground spell, or a nova around the caster, that does not already pick its own spot.
    Placeable,
}

/// An augment, general or capstone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Augment {
    // --- General: conversions ---
    Frostfire,
    Spellfire,
    Spellfrost,
    // --- General: projectiles ---
    Multishot,
    Piercing,
    Returning,
    // --- General: areas ---
    Wide,
    Concentrated,
    Echo,
    Ring,
    // --- General: ground ---
    Lingering,
    Mobile,
    Targeted,
    // --- General: damage over time ---
    Contagion,
    Quickened,
    Molten,
    // --- General: direct hits ---
    Precise,
    Executioner,
    Ignite,
    // --- General: control ---
    DeepChill,
    ColdBite,
    LastingFrost,
    Permafrost,
    Shatter,
    BruteForce,
    Concussive,
    // --- General: shields ---
    Fortified,
    Reflective,
    // --- General: any ---
    TwinCharges,
    Heavy,
    Twin,
    Hasted,
    // --- Capstones: Frostbolt ---
    GlacialSpike,
    Shatterbolt,
    WintersChill,
    // --- Capstones: Fireball ---
    HotStreak,
    CriticalMass,
    LivingBomb,
    // --- Capstones: Arcane Missiles ---
    ArcaneCharge,
    MissileStorm,
    ChannelledFocus,
    // --- Capstones: Frost Nova ---
    Frostbite,
    ColdSnap,
    FrozenCore,
    // --- Capstones: Arcane Explosion ---
    ArcanePotency,
    BlinkDetonation,
    Implosion,
    // --- Capstones: Blizzard ---
    FinalGale,
    Whiteout,
    FrozenOrb,
    // --- Capstones: Flamestrike ---
    Firestorm,
    Meteor,
    PhoenixGround,
    // --- Capstones: Fire Blast ---
    Impact,
    Inferno,
    Ignition,
    // --- Capstones: Cone of Cold ---
    FrozenCone,
    ShardBurst,
    WakeOfWinter,
    // --- Capstones: Blast Wave ---
    BlazingSpeed,
    LivingWave,
    DragonsBreath,
    // --- Capstones: Pyroblast ---
    PresenceOfMind,
    Kindling,
    Combustion,
    // --- Capstones: Ice Barrier ---
    ShatteringBarrier,
    IceFloes,
    FrostArmor,
}

impl Augment {
    /// Every general augment, in the library's order.
    pub const GENERAL: [Augment; 32] = [
        Augment::Frostfire,
        Augment::Spellfire,
        Augment::Spellfrost,
        Augment::Multishot,
        Augment::Piercing,
        Augment::Returning,
        Augment::Wide,
        Augment::Concentrated,
        Augment::Echo,
        Augment::Ring,
        Augment::Lingering,
        Augment::Mobile,
        Augment::Targeted,
        Augment::Contagion,
        Augment::Quickened,
        Augment::Molten,
        Augment::Precise,
        Augment::Executioner,
        Augment::Ignite,
        Augment::DeepChill,
        Augment::ColdBite,
        Augment::LastingFrost,
        Augment::Permafrost,
        Augment::Shatter,
        Augment::BruteForce,
        Augment::Concussive,
        Augment::Fortified,
        Augment::Reflective,
        Augment::TwinCharges,
        Augment::Heavy,
        Augment::Twin,
        Augment::Hasted,
    ];

    /// Every capstone, three per spell, in spell order.
    pub const CAPSTONES: [Augment; 36] = [
        Augment::GlacialSpike,
        Augment::Shatterbolt,
        Augment::WintersChill,
        Augment::HotStreak,
        Augment::CriticalMass,
        Augment::LivingBomb,
        Augment::ArcaneCharge,
        Augment::MissileStorm,
        Augment::ChannelledFocus,
        Augment::Frostbite,
        Augment::ColdSnap,
        Augment::FrozenCore,
        Augment::ArcanePotency,
        Augment::BlinkDetonation,
        Augment::Implosion,
        Augment::FinalGale,
        Augment::Whiteout,
        Augment::FrozenOrb,
        Augment::Firestorm,
        Augment::Meteor,
        Augment::PhoenixGround,
        Augment::Impact,
        Augment::Inferno,
        Augment::Ignition,
        Augment::FrozenCone,
        Augment::ShardBurst,
        Augment::WakeOfWinter,
        Augment::BlazingSpeed,
        Augment::LivingWave,
        Augment::DragonsBreath,
        Augment::PresenceOfMind,
        Augment::Kindling,
        Augment::Combustion,
        Augment::ShatteringBarrier,
        Augment::IceFloes,
        Augment::FrostArmor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Augment::Frostfire => "Frostfire",
            Augment::Spellfire => "Spellfire",
            Augment::Spellfrost => "Spellfrost",
            Augment::Multishot => "Multishot",
            Augment::Piercing => "Piercing",
            Augment::Returning => "Returning",
            Augment::Wide => "Wide",
            Augment::Concentrated => "Concentrated",
            Augment::Echo => "Echo",
            Augment::Ring => "Ring",
            Augment::Lingering => "Lingering",
            Augment::Mobile => "Mobile",
            Augment::Targeted => "Targeted",
            Augment::Contagion => "Contagion",
            Augment::Quickened => "Quickened",
            Augment::Molten => "Molten",
            Augment::Precise => "Precise",
            Augment::Executioner => "Executioner",
            Augment::Ignite => "Ignite",
            Augment::DeepChill => "Deep Chill",
            Augment::ColdBite => "Cold Bite",
            Augment::LastingFrost => "Lasting Frost",
            Augment::Permafrost => "Permafrost",
            Augment::Shatter => "Shatter",
            Augment::BruteForce => "Brute Force",
            Augment::Concussive => "Concussive",
            Augment::Fortified => "Fortified",
            Augment::Reflective => "Reflective",
            Augment::TwinCharges => "Twin Charges",
            Augment::Heavy => "Heavy",
            Augment::Twin => "Twin",
            Augment::Hasted => "Hasted",
            Augment::GlacialSpike => "Glacial Spike",
            Augment::Shatterbolt => "Shatterbolt",
            Augment::WintersChill => "Winter's Chill",
            Augment::HotStreak => "Hot Streak",
            Augment::CriticalMass => "Critical Mass",
            Augment::LivingBomb => "Living Bomb",
            Augment::ArcaneCharge => "Arcane Charge",
            Augment::MissileStorm => "Missile Storm",
            Augment::ChannelledFocus => "Channelled Focus",
            Augment::Frostbite => "Frostbite",
            Augment::ColdSnap => "Cold Snap",
            Augment::FrozenCore => "Frozen Core",
            Augment::ArcanePotency => "Arcane Potency",
            Augment::BlinkDetonation => "Blink Detonation",
            Augment::Implosion => "Implosion",
            Augment::FinalGale => "Final Gale",
            Augment::Whiteout => "Whiteout",
            Augment::FrozenOrb => "Frozen Orb",
            Augment::Firestorm => "Firestorm",
            Augment::Meteor => "Meteor",
            Augment::PhoenixGround => "Phoenix Ground",
            Augment::Impact => "Impact",
            Augment::Inferno => "Inferno",
            Augment::Ignition => "Ignition",
            Augment::FrozenCone => "Frozen Cone",
            Augment::ShardBurst => "Shard Burst",
            Augment::WakeOfWinter => "Wake of Winter",
            Augment::BlazingSpeed => "Blazing Speed",
            Augment::LivingWave => "Living Wave",
            Augment::DragonsBreath => "Dragon's Breath",
            Augment::PresenceOfMind => "Presence of Mind",
            Augment::Kindling => "Kindling",
            Augment::Combustion => "Combustion",
            Augment::ShatteringBarrier => "Shattering Barrier",
            Augment::IceFloes => "Ice Floes",
            Augment::FrostArmor => "Frost Armor",
        }
    }

    /// What it does, for its card.
    pub fn description(self) -> &'static str {
        match self {
            Augment::Frostfire => "Counts as Fire and Frost. A Fire spell also Chills; a Frost spell also leaves Burning.",
            Augment::Spellfire => "Counts as Fire and Arcane. +15% area.",
            Augment::Spellfrost => "Counts as Frost and Arcane. −15% cooldown.",
            Augment::Multishot => "+2 projectiles, −25% damage each.",
            Augment::Piercing => "Passes through 2 more enemies.",
            Augment::Returning => "Flies back, hitting its target again.",
            Augment::Wide => "+40% area, −20% damage.",
            Augment::Concentrated => "−30% area, +40% damage.",
            Augment::Echo => "Repeats at half strength 1 second later.",
            Augment::Ring => "Hits a ring from 5 to 12 yards instead of a circle. +50% damage.",
            Augment::Lingering => "Lasts 50% longer.",
            Augment::Mobile => "Follows you.",
            Augment::Targeted => "Lands on the thickest crowd within 25 yards instead of around you.",
            Augment::Contagion => "When a Burning enemy dies, its burn jumps to a nearby enemy.",
            Augment::Quickened => "Its damage over time runs 30% faster.",
            Augment::Molten => "Its damage over time deals double and lasts twice as long; its direct hit −30%.",
            Augment::Precise => "+20% critical strike chance.",
            Augment::Executioner => "+100% damage to enemies below 30% health.",
            Augment::Ignite => "Leaves the target Burning for 40% of the hit over 4 seconds.",
            Augment::DeepChill => "Its Chill has a 15% chance to Freeze.",
            Augment::ColdBite => "+25% damage to Chilled enemies.",
            Augment::LastingFrost => "Its Freeze lasts 1 second longer.",
            Augment::Permafrost => "Enemies it Freezes stay Chilled after they thaw.",
            Augment::Shatter => "+50% critical strike chance against Frozen enemies.",
            Augment::BruteForce => "+120% damage, but it no longer Chills, Freezes or Stuns.",
            Augment::Concussive => "Knocks back 4 yards farther and Stuns for half a second.",
            Augment::Fortified => "The shield absorbs 30% more; while it holds, +15% damage.",
            Augment::Reflective => "20% of what the shield absorbs is dealt back to the attacker.",
            Augment::TwinCharges => "2 charges.",
            Augment::Heavy => "+60% damage, +40% cooldown.",
            Augment::Twin => "Casts twice, −40% damage each.",
            Augment::Hasted => "−25% cooldown, −15% damage.",
            Augment::GlacialSpike => "Every 4th Frostbolt is a Glacial Spike: +200% damage, and it Freezes its target.",
            Augment::Shatterbolt => "Always crits Frozen enemies, and those crits splash 50% within 5 yards.",
            Augment::WintersChill => "Each hit makes the target take +2% crit chance from every source, up to 10%. The whole party benefits.",
            Augment::HotStreak => "Two Fire crits in a row cast Pyroblast: yours, with its augments, if you know it.",
            Augment::CriticalMass => "When the hit crits, the burst crits too. Your Fire crits deal 2.5× instead of 2×.",
            Augment::LivingBomb => "Its target becomes a bomb that explodes after 3 seconds for 150% within 8 yards; enemies it hits become smaller bombs.",
            Augment::ArcaneCharge => "Each cast gives an Arcane Charge (up to 4, +10% Arcane damage each). At 4, your next Arcane Explosion spends them for +100% damage and area.",
            Augment::MissileStorm => "Every 10 seconds, a missile at every enemy in range.",
            Augment::ChannelledFocus => "While you stand still, it fires twice as often and gains +15% damage per second, up to +60%.",
            Augment::Frostbite => "A Frozen enemy that dies shatters for 50% of its health as Frost damage within 4 yards.",
            Augment::ColdSnap => "Hitting 10 or more enemies resets your other Frost spells' cooldowns (at most every 8 seconds).",
            Augment::FrozenCore => "Below 30% health, Frost Nova casts itself and you take 50% less damage for 3 seconds (every 15 seconds at most).",
            Augment::ArcanePotency => "Hitting 15 or more enemies makes your next Arcane spell deal triple damage.",
            Augment::BlinkDetonation => "Your movement ability casts Arcane Explosion where you leave and where you land.",
            Augment::Implosion => "First pulls every enemy within 12 yards toward you, then explodes.",
            Augment::FinalGale => "Lasts 50% longer; when it ends, it casts Frost Nova at its centre.",
            Augment::Whiteout => "Enemies inside take +20% damage from every source.",
            Augment::FrozenOrb => "Becomes an orb rolling away from you the way you face, chilling and damaging all it passes.",
            Augment::Firestorm => "2 more Flamestrikes on other crowds, −40% damage each.",
            Augment::Meteor => "+200% damage, +100% cooldown, and it Stuns for 1.5 seconds.",
            Augment::PhoenixGround => "Standing in your own burning ground heals 2% of your health a second and gives +20% Fire damage.",
            Augment::Impact => "Stuns its target for 1 second and spreads the target's Burning to 3 nearby enemies.",
            Augment::Inferno => "A Fire Blast that kills casts Fire Blast again at the next closest enemy.",
            Augment::Ignition => "Always crits Burning enemies.",
            Augment::FrozenCone => "The first 5 enemies hit are Frozen instead of Chilled.",
            Augment::ShardBurst => "+100% damage against Frozen enemies.",
            Augment::WakeOfWinter => "Your movement ability casts Cone of Cold behind you.",
            Augment::BlazingSpeed => "Each enemy hit gives +3% move speed for 4 seconds, up to 30%.",
            Augment::LivingWave => "Leaves enemies Burning; a Burning enemy that dies sends out a small Blast Wave.",
            Augment::DragonsBreath => "A cone ahead of you instead of a ring; +60% damage; Stuns for 2 seconds.",
            Augment::PresenceOfMind => "Every 3rd Pyroblast fires a second Pyroblast at another target.",
            Augment::Kindling => "Every Fire crit takes half a second off Pyroblast's cooldown.",
            Augment::Combustion => "Consumes every Burning effect on its target and deals their remaining damage at once, doubled.",
            Augment::ShatteringBarrier => "When the shield breaks or ends, it deals 3× what it absorbed within 15 yards and casts Frost Nova.",
            Augment::IceFloes => "While the shield holds, your Frost spells' cooldowns run 25% faster.",
            Augment::FrostArmor => "Enemies that strike the shield are Chilled and take 20% of the blow as Frost damage.",
        }
    }

    /// The spell a capstone belongs to (its ability key), `None` for a general augment.
    pub fn capstone_of(self) -> Option<&'static str> {
        Some(match self {
            Augment::GlacialSpike | Augment::Shatterbolt | Augment::WintersChill => "frostbolt",
            Augment::HotStreak | Augment::CriticalMass | Augment::LivingBomb => "fireball",
            Augment::ArcaneCharge | Augment::MissileStorm | Augment::ChannelledFocus => {
                "arcane_missiles"
            }
            Augment::Frostbite | Augment::ColdSnap | Augment::FrozenCore => "frost_nova",
            Augment::ArcanePotency | Augment::BlinkDetonation | Augment::Implosion => {
                "arcane_explosion"
            }
            Augment::FinalGale | Augment::Whiteout | Augment::FrozenOrb => "blizzard",
            Augment::Firestorm | Augment::Meteor | Augment::PhoenixGround => "flamestrike",
            Augment::Impact | Augment::Inferno | Augment::Ignition => "fire_blast",
            Augment::FrozenCone | Augment::ShardBurst | Augment::WakeOfWinter => "cone_of_cold",
            Augment::BlazingSpeed | Augment::LivingWave | Augment::DragonsBreath => "blast_wave",
            Augment::PresenceOfMind | Augment::Kindling | Augment::Combustion => "pyroblast",
            Augment::ShatteringBarrier | Augment::IceFloes | Augment::FrostArmor => "ice_barrier",
            _ => return None,
        })
    }

    /// The three capstones of the spell `key`, in order; empty for a spell without any.
    pub fn capstones_of(key: &str) -> Vec<Augment> {
        Augment::CAPSTONES
            .into_iter()
            .filter(|a| a.capstone_of() == Some(key))
            .collect()
    }

    /// A conversion: it adds a school, and a spell takes at most one.
    pub fn is_conversion(self) -> bool {
        matches!(
            self,
            Augment::Frostfire | Augment::Spellfire | Augment::Spellfrost
        )
    }

    /// Who may roll a general augment; capstones are never rolled.
    pub fn requires(self) -> Requires {
        use Tag::*;
        match self {
            Augment::Frostfire => Requires::AnyOf(&[Fire, Frost]),
            Augment::Spellfire => Requires::AnyOf(&[Fire, Arcane]),
            Augment::Spellfrost => Requires::AnyOf(&[Frost, Arcane]),
            Augment::Multishot | Augment::Piercing | Augment::Returning => {
                Requires::AnyOf(&[Projectile])
            }
            Augment::Wide | Augment::Concentrated | Augment::Echo | Augment::Ring => {
                Requires::AnyOf(&[Area])
            }
            Augment::Lingering | Augment::Mobile => Requires::AnyOf(&[Ground]),
            Augment::Targeted => Requires::Placeable,
            Augment::Contagion | Augment::Quickened | Augment::Molten => {
                Requires::AnyOf(&[DamageOverTime])
            }
            Augment::Precise | Augment::Executioner => Requires::AnyOf(&[Direct]),
            Augment::Ignite => Requires::AllOf(&[Fire, Direct]),
            Augment::DeepChill | Augment::ColdBite => Requires::AnyOf(&[Slow]),
            Augment::LastingFrost | Augment::Permafrost | Augment::Shatter => {
                Requires::AnyOf(&[Freeze])
            }
            Augment::BruteForce => Requires::Control,
            Augment::Concussive => Requires::AnyOf(&[Knockback]),
            Augment::Fortified | Augment::Reflective => Requires::AnyOf(&[Shield]),
            Augment::TwinCharges => Requires::Cooldown(30),
            _ => Requires::Any,
        }
    }

    /// How many of the tags this augment asks for are among `tags`: how well it fits a hero's
    /// other spells, which makes it likelier to be offered.
    pub fn synergy(self, tags: Tags) -> u32 {
        match self.requires() {
            Requires::AnyOf(want) | Requires::AllOf(want) => {
                want.iter().filter(|&&t| tags.has(t)).count() as u32
            }
            _ => 0,
        }
    }

    /// Whether a general augment may be offered for a spell cast as `spec`, which already has
    /// `taken` augments.
    pub fn fits(self, spec: &CastSpec, taken: &[Augment]) -> bool {
        if self.capstone_of().is_some() || taken.contains(&self) {
            return false;
        }
        if self.is_conversion() && taken.iter().any(|a| a.is_conversion()) {
            return false;
        }
        // A shield deals no damage itself: damage trades mean nothing to it.
        if spec.tags.has(Tag::Shield) && matches!(self, Augment::Heavy | Augment::Twin) {
            return false;
        }
        match self.requires() {
            Requires::Any => true,
            Requires::AnyOf(tags) => spec.tags.any(tags),
            Requires::AllOf(tags) => tags.iter().all(|&t| spec.tags.has(t)),
            Requires::Control => spec.has_control(),
            Requires::Cooldown(tenths) => spec.cooldown * 10.0 >= tenths as f32,
            Requires::Placeable => {
                (spec.tags.has(Tag::Ground) && spec.targeting != Targeting::Densest)
                    || spec.targeting == Targeting::Around
            }
        }
    }

    /// The change an augment makes to a cast's numbers. Capstones that change numbers do so here
    /// too; the rest act in the world.
    pub fn shape(self, s: &mut CastSpec) {
        match self {
            Augment::Frostfire => {
                if s.tags.has(Tag::Fire) && s.chill == 0.0 {
                    s.chill = crate::spell::CHILL_SECS;
                    s.tags = s.tags.with(Tag::Slow);
                }
                if s.tags.has(Tag::Frost) {
                    s.burn += 0.3;
                    s.tags = s.tags.with(Tag::DamageOverTime);
                }
                s.tags = s.tags.with(Tag::Fire).with(Tag::Frost);
            }
            Augment::Spellfire => {
                s.tags = s.tags.with(Tag::Fire).with(Tag::Arcane);
                s.radius *= 1.15;
            }
            Augment::Spellfrost => {
                s.tags = s.tags.with(Tag::Frost).with(Tag::Arcane);
                s.cooldown *= 0.85;
            }
            Augment::Multishot => {
                s.count += 2;
                s.damage *= 0.75;
            }
            Augment::Piercing => s.pierce += 2,
            Augment::Returning => s.returning = true,
            Augment::Wide => {
                s.radius *= 1.4;
                s.damage *= 0.8;
            }
            Augment::Concentrated => {
                s.radius *= 0.7;
                s.damage *= 1.4;
            }
            Augment::Echo => s.echo = true,
            Augment::Ring => {
                s.ring = 5.0;
                s.radius = s.radius.max(12.0);
                s.damage *= 1.5;
            }
            Augment::Lingering => s.ground *= 1.5,
            Augment::Mobile => s.follows = true,
            Augment::Targeted => s.targeted = true,
            Augment::Contagion => s.contagion = true,
            Augment::Quickened => s.dot_speed *= 1.3,
            Augment::Molten => {
                s.dot_mult *= 2.0;
                s.dot_speed *= 0.5;
                s.damage *= 0.7;
            }
            Augment::Precise => s.crit += 0.2,
            Augment::Executioner => s.execute += 1.0,
            Augment::Ignite => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::DeepChill => s.freeze_chance += 0.15,
            Augment::ColdBite => s.vs_chilled += 0.25,
            Augment::LastingFrost => s.freeze += 1.0,
            Augment::Permafrost => s.permafrost = true,
            Augment::Shatter => s.shatter += 0.5,
            Augment::BruteForce => {
                s.chill = 0.0;
                s.freeze = 0.0;
                s.freeze_chance = 0.0;
                s.stun = 0.0;
                s.tags = s
                    .tags
                    .without(Tag::Slow)
                    .without(Tag::Freeze)
                    .without(Tag::Stun);
                s.damage *= 2.2;
            }
            Augment::Concussive => {
                s.knockback += 4.0;
                s.stun = s.stun.max(0.5);
            }
            Augment::Fortified => s.shield *= 1.3,
            Augment::TwinCharges => s.charges = 2,
            Augment::Heavy => {
                s.damage *= 1.6;
                s.cooldown *= 1.4;
            }
            Augment::Twin => {
                s.twin = true;
                s.damage *= 0.6;
            }
            Augment::Hasted => {
                s.cooldown *= 0.75;
                s.damage *= 0.85;
            }
            // Capstones that change the cast's numbers.
            Augment::FinalGale => s.ground *= 1.5,
            Augment::Meteor => {
                s.damage *= 3.0;
                s.cooldown *= 2.0;
                s.stun = s.stun.max(1.5);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::DragonsBreath => {
                s.targeting = Targeting::Ahead;
                s.damage *= 1.6;
                s.stun = s.stun.max(2.0);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::LivingWave => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::FrozenOrb => s.follows = false,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spell::Tags;

    pub(crate) fn spec(tags: &[Tag]) -> CastSpec {
        CastSpec {
            ability: 0,
            tags: Tags::of(tags),
            targeting: Targeting::Nearest,
            damage: 10.0,
            cooldown: 2.0,
            count: 1,
            volley: 1,
            pierce: 0,
            range: 20.0,
            radius: 3.0,
            ring: 0.0,
            missile_speed: 20.0,
            crit: 0.05,
            crit_mult: 2.0,
            chill: 0.0,
            freeze: 0.0,
            freeze_chance: 0.0,
            permafrost: false,
            stun: 0.0,
            knockback: 0.0,
            burn: 0.0,
            dot_mult: 1.0,
            dot_speed: 1.0,
            contagion: false,
            ground: 0.0,
            follows: false,
            targeted: false,
            echo: false,
            twin: false,
            returning: false,
            vs_chilled: 0.0,
            vs_frozen: 0.0,
            execute: 0.0,
            shatter: 0.0,
            shield: 0.0,
            charges: 1,
        }
    }

    #[test]
    fn three_capstones_for_each_of_twelve_spells() {
        let mut spells: Vec<&str> = Augment::CAPSTONES
            .iter()
            .filter_map(|a| a.capstone_of())
            .collect();
        assert_eq!(spells.len(), 36);
        spells.dedup();
        assert_eq!(spells.len(), 12);
        assert!(Augment::GENERAL.iter().all(|a| a.capstone_of().is_none()));
        assert_eq!(Augment::capstones_of("fireball").len(), 3);
    }

    #[test]
    fn generals_go_only_where_their_tags_are() {
        let bolt = spec(&[Tag::Fire, Tag::Projectile, Tag::Direct]);
        assert!(Augment::Multishot.fits(&bolt, &[]));
        assert!(Augment::Ignite.fits(&bolt, &[]));
        assert!(!Augment::Wide.fits(&bolt, &[]));
        assert!(!Augment::Shatter.fits(&bolt, &[]));
        assert!(
            !Augment::GlacialSpike.fits(&bolt, &[]),
            "a capstone is never rolled"
        );
    }

    #[test]
    fn a_spell_takes_one_conversion_and_no_augment_twice() {
        let bolt = spec(&[Tag::Fire, Tag::Projectile]);
        assert!(Augment::Spellfire.fits(&bolt, &[]));
        assert!(!Augment::Spellfire.fits(&bolt, &[Augment::Frostfire]));
        assert!(!Augment::Multishot.fits(&bolt, &[Augment::Multishot]));
    }

    #[test]
    fn frostfire_makes_a_fire_spell_chill_and_count_as_frost() {
        let mut s = spec(&[Tag::Fire, Tag::Projectile]);
        Augment::Frostfire.shape(&mut s);
        assert!(s.tags.has(Tag::Frost) && s.tags.has(Tag::Fire) && s.tags.has(Tag::Slow));
        assert!(s.chill > 0.0);
    }

    #[test]
    fn brute_force_trades_control_for_damage() {
        let mut s = spec(&[Tag::Frost, Tag::Area, Tag::Freeze]);
        s.freeze = 2.0;
        assert!(Augment::BruteForce.fits(&s, &[]));
        Augment::BruteForce.shape(&mut s);
        assert_eq!(s.freeze, 0.0);
        assert!(!s.tags.has(Tag::Freeze));
        assert!((s.damage - 22.0).abs() < 1e-4);
    }
}
