//! Augments: how a spell grows past its numbers. At rank 5 and rank 15 a spell takes a
//! **general** augment, from a shared library open to any spell with the right tags; at rank 10 it
//! takes one of its own three **capstones**, always the same three.
//!
//! A general augment changes a cast's numbers or adds a behaviour ([`Augment::shape`] does the
//! numbers); a capstone is a mechanic of its own, which the world checks where it acts (on a cast,
//! a hit, a crit, a kill, a blow taken).

use serde::{Deserialize, Serialize};

use crate::spell::{CastSpec, Cone, Tag, Tags, Targeting};

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
    // --- General: cones ---
    Reversed,
    FourWinds,
    FullCircle,
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
    // --- The other classes' general augments ---
    Shadowflame,
    Twilight,
    SacredFlame,
    Thunderstrike,
    Balance,
    Frostbrand,
    Crusader,
    Shadowblade,
    Lacerate,
    Hemorrhage,
    Crippling,
    Virulent,
    Dread,
    Panic,
    Bramble,
    Grasping,
    Crushing,
    Forked,
    Conductive,
    Pack,
    FeralBond,
    Frenzy,
    BoomingVoice,
    Commanding,
    DeepReserves,
    Overflowing,
    MendingLink,
    Renewing,
    Sanguine,
    Bloodthirsty,
    Sweeping,
    // --- Capstones: warrior ---
    BloodAndThunder,
    Rampage,
    CleavingSwings,
    Overpower,
    HeroicThrow,
    ChampionsGuard,
    MortalWounds,
    SuddenDeath,
    ColossusSmash,
    Bladestorm,
    FuriousSpin,
    SecondWind,
    Shockwave,
    Aftershock,
    Thunderstruck,
    GushingWound,
    BloodFrenzy,
    DeepWounds,
    ExecutionersChain,
    Massacre,
    VictoryRush,
    BloodCraze,
    UnbridledWrath,
    Bloodbath,
    Revenge,
    ShieldBash,
    LastStand,
    RallyingCry,
    CripplingPresence,
    WarCry,
    ChallengingShout,
    ShatteringRoar,
    Fearsome,
    ExposeArmor,
    Devastate,
    SunderingBlows,
    // --- Capstones: rogue ---
    CombatPotency,
    BladeDance,
    RelentlessStrikes,
    Ambush,
    Opportunity,
    SealFate,
    RestlessBlades,
    ColdBlood,
    Ruthlessness,
    VenomousWounds,
    CrimsonTempest,
    Bloodletter,
    AdrenalineRush,
    FlowingSteel,
    SerratedBlades,
    PoisonedKnives,
    LethalFan,
    FanThePack,
    Envenom,
    ToxicCloud,
    MindNumbingPoison,
    ImprovedGouge,
    DirtyTricks,
    GougedEyes,
    MarkedForDeath,
    KidneyBurst,
    QuickRecovery,
    Adrenaline,
    RestlessTempo,
    ImprovedSlice,
    Counterattack,
    Nimble,
    CheatDeath,
    BlindingPowder,
    SmokeBomb,
    Sap,
    // --- Capstones: priest ---
    Atonement,
    DivineFury,
    SearingLight,
    Martyrdom,
    PurgeTheWicked,
    HolyFervor,
    PsychicHorror,
    ShadowyApparitions,
    ShadowyInsight,
    SpreadingPain,
    Misery,
    DeathsEcho,
    MindSear,
    PsychicLink,
    MindTrauma,
    Plaguebearer,
    VampiricEmbrace,
    Insatiable,
    SearingNova,
    DivineAegis,
    Inspiration,
    ShootingStars,
    Starfall,
    LunarGuidance,
    HolyWordSanctify,
    BlessedRecovery,
    InnerFocus,
    BodyAndSoul,
    Rapture,
    BorrowedTime,
    Sanctuary,
    LightsWrath,
    SpringOfLight,
    TouchOfWeakness,
    HexOfDomination,
    SpreadingHex,
    // --- Capstones: warlock ---
    Nightfall,
    ShadowBoltVolley,
    ImprovedShadowBolt,
    SiphonLife,
    SeedOfCorruption,
    EverlastingAffliction,
    CurseOfDoom,
    AmplifyCurse,
    CurseOfExhaustion,
    FireAndBrimstone,
    Backdraft,
    Emberstorm,
    FirestormRain,
    Aftermath,
    Pyroclasm,
    InfernalPact,
    RitualOfFire,
    HellfireWard,
    Horrify,
    DeathsEmbrace,
    DarkBargain,
    SoulHarvest,
    DrainAura,
    ImprovedDrain,
    Nightmare,
    Shadowfury,
    DemonicPresence,
    BurningImp,
    DemonicSacrifice,
    ImprovedImp,
    Sacrifice,
    Torment,
    ConsumeShadows,
    ImprovedSearingPain,
    SoulFire,
    FelFlame,
    // --- Capstones: hunter ---
    ArcaneBarrage,
    TrickShot,
    LockAndLoad,
    SplinterArrows,
    SpreadShot,
    SerpentSpread,
    CarefulAim,
    SniperTraining,
    PiercingShots,
    SpreadingVenom,
    NoxiousStings,
    ScorpidSting,
    ImprovedConcussion,
    DazeWave,
    FrostShot,
    Arrowstorm,
    ExplosiveVolley,
    HailOfArrows,
    Minefield,
    ScorchedEarth,
    BigGameTrap,
    FreezingTrap,
    FrozenWake,
    DeepFreeze,
    BestialWrath,
    KillCommand,
    Intimidation,
    ImprovedScatter,
    WyvernSting,
    ScatterBurst,
    ImprovedMark,
    MarkedForTheKill,
    HuntersPrey,
    RapidKilling,
    TrueshotAura,
    QuickShots,
    // --- Capstones: shaman ---
    LightningOverload,
    Stormkeeper,
    RollingThunder,
    ChainReaction,
    StormNexus,
    Electrified,
    Fulmination,
    EarthenGrip,
    ShockMastery,
    LavaBurst,
    VolcanicFury,
    SearingFlames,
    FrozenPower,
    FrostNovaShock,
    Icefury,
    SearingBurn,
    SearingPact,
    TwinFires,
    FireNova,
    LiquidMagma,
    MoltenEarth,
    Earthgrab,
    EarthquakeTotem,
    Stoneclaw,
    HealingTide,
    Cloudburst,
    Stoneskin,
    Thunderstorm,
    StaticShield,
    StaticCharge,
    AncestralFortitude,
    NaturesSwiftness,
    TidalFocus,
    Windfury,
    Stormlash,
    MaelstromWeapon,
    // --- Capstones: paladin ---
    HammerOfTheRighteous,
    WrathOfTheExecutioner,
    BlessedHammer,
    JudgementOfLight,
    DivineStorm,
    JudgementOfTheCrusader,
    Exorcist,
    ArtOfWar,
    BanishEvil,
    InfusionOfLight,
    GlimmerOfLight,
    Daybreak,
    SacredGround,
    HallowedGround,
    ConsecratedChill,
    Wrathful,
    DivinePurpose,
    StunningWrath,
    FistOfJustice,
    ShockwaveHammer,
    LastingJustice,
    AvengersShield,
    HolyRetribution,
    Reckoning,
    HolyPower,
    DivineFavor,
    LightOfDawn,
    AuraMastery,
    Devotion,
    Sanctity,
    SealOfLight,
    SealOfRighteousness,
    SealOfWisdom,
    ImprovedLayOnHands,
    MartyrsHands,
    HandOfProtection,
}

impl Augment {
    /// Every general augment, in the library's order.
    pub const GENERAL: [Augment; 66] = [
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
        Augment::Reversed,
        Augment::FourWinds,
        Augment::FullCircle,
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
        Augment::Shadowflame,
        Augment::Twilight,
        Augment::SacredFlame,
        Augment::Thunderstrike,
        Augment::Balance,
        Augment::Frostbrand,
        Augment::Crusader,
        Augment::Shadowblade,
        Augment::Lacerate,
        Augment::Hemorrhage,
        Augment::Crippling,
        Augment::Virulent,
        Augment::Dread,
        Augment::Panic,
        Augment::Bramble,
        Augment::Grasping,
        Augment::Crushing,
        Augment::Forked,
        Augment::Conductive,
        Augment::Pack,
        Augment::FeralBond,
        Augment::Frenzy,
        Augment::BoomingVoice,
        Augment::Commanding,
        Augment::DeepReserves,
        Augment::Overflowing,
        Augment::MendingLink,
        Augment::Renewing,
        Augment::Sanguine,
        Augment::Bloodthirsty,
        Augment::Sweeping,
    ];

    /// Every capstone, three per spell, in spell order.
    pub const CAPSTONES: [Augment; 288] = [
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
        Augment::BloodAndThunder,
        Augment::Rampage,
        Augment::CleavingSwings,
        Augment::Overpower,
        Augment::HeroicThrow,
        Augment::ChampionsGuard,
        Augment::MortalWounds,
        Augment::SuddenDeath,
        Augment::ColossusSmash,
        Augment::Bladestorm,
        Augment::FuriousSpin,
        Augment::SecondWind,
        Augment::Shockwave,
        Augment::Aftershock,
        Augment::Thunderstruck,
        Augment::GushingWound,
        Augment::BloodFrenzy,
        Augment::DeepWounds,
        Augment::ExecutionersChain,
        Augment::Massacre,
        Augment::VictoryRush,
        Augment::BloodCraze,
        Augment::UnbridledWrath,
        Augment::Bloodbath,
        Augment::Revenge,
        Augment::ShieldBash,
        Augment::LastStand,
        Augment::RallyingCry,
        Augment::CripplingPresence,
        Augment::WarCry,
        Augment::ChallengingShout,
        Augment::ShatteringRoar,
        Augment::Fearsome,
        Augment::ExposeArmor,
        Augment::Devastate,
        Augment::SunderingBlows,
        Augment::CombatPotency,
        Augment::BladeDance,
        Augment::RelentlessStrikes,
        Augment::Ambush,
        Augment::Opportunity,
        Augment::SealFate,
        Augment::RestlessBlades,
        Augment::ColdBlood,
        Augment::Ruthlessness,
        Augment::VenomousWounds,
        Augment::CrimsonTempest,
        Augment::Bloodletter,
        Augment::AdrenalineRush,
        Augment::FlowingSteel,
        Augment::SerratedBlades,
        Augment::PoisonedKnives,
        Augment::LethalFan,
        Augment::FanThePack,
        Augment::Envenom,
        Augment::ToxicCloud,
        Augment::MindNumbingPoison,
        Augment::ImprovedGouge,
        Augment::DirtyTricks,
        Augment::GougedEyes,
        Augment::MarkedForDeath,
        Augment::KidneyBurst,
        Augment::QuickRecovery,
        Augment::Adrenaline,
        Augment::RestlessTempo,
        Augment::ImprovedSlice,
        Augment::Counterattack,
        Augment::Nimble,
        Augment::CheatDeath,
        Augment::BlindingPowder,
        Augment::SmokeBomb,
        Augment::Sap,
        Augment::Atonement,
        Augment::DivineFury,
        Augment::SearingLight,
        Augment::Martyrdom,
        Augment::PurgeTheWicked,
        Augment::HolyFervor,
        Augment::PsychicHorror,
        Augment::ShadowyApparitions,
        Augment::ShadowyInsight,
        Augment::SpreadingPain,
        Augment::Misery,
        Augment::DeathsEcho,
        Augment::MindSear,
        Augment::PsychicLink,
        Augment::MindTrauma,
        Augment::Plaguebearer,
        Augment::VampiricEmbrace,
        Augment::Insatiable,
        Augment::SearingNova,
        Augment::DivineAegis,
        Augment::Inspiration,
        Augment::ShootingStars,
        Augment::Starfall,
        Augment::LunarGuidance,
        Augment::HolyWordSanctify,
        Augment::BlessedRecovery,
        Augment::InnerFocus,
        Augment::BodyAndSoul,
        Augment::Rapture,
        Augment::BorrowedTime,
        Augment::Sanctuary,
        Augment::LightsWrath,
        Augment::SpringOfLight,
        Augment::TouchOfWeakness,
        Augment::HexOfDomination,
        Augment::SpreadingHex,
        Augment::Nightfall,
        Augment::ShadowBoltVolley,
        Augment::ImprovedShadowBolt,
        Augment::SiphonLife,
        Augment::SeedOfCorruption,
        Augment::EverlastingAffliction,
        Augment::CurseOfDoom,
        Augment::AmplifyCurse,
        Augment::CurseOfExhaustion,
        Augment::FireAndBrimstone,
        Augment::Backdraft,
        Augment::Emberstorm,
        Augment::FirestormRain,
        Augment::Aftermath,
        Augment::Pyroclasm,
        Augment::InfernalPact,
        Augment::RitualOfFire,
        Augment::HellfireWard,
        Augment::Horrify,
        Augment::DeathsEmbrace,
        Augment::DarkBargain,
        Augment::SoulHarvest,
        Augment::DrainAura,
        Augment::ImprovedDrain,
        Augment::Nightmare,
        Augment::Shadowfury,
        Augment::DemonicPresence,
        Augment::BurningImp,
        Augment::DemonicSacrifice,
        Augment::ImprovedImp,
        Augment::Sacrifice,
        Augment::Torment,
        Augment::ConsumeShadows,
        Augment::ImprovedSearingPain,
        Augment::SoulFire,
        Augment::FelFlame,
        Augment::ArcaneBarrage,
        Augment::TrickShot,
        Augment::LockAndLoad,
        Augment::SplinterArrows,
        Augment::SpreadShot,
        Augment::SerpentSpread,
        Augment::CarefulAim,
        Augment::SniperTraining,
        Augment::PiercingShots,
        Augment::SpreadingVenom,
        Augment::NoxiousStings,
        Augment::ScorpidSting,
        Augment::ImprovedConcussion,
        Augment::DazeWave,
        Augment::FrostShot,
        Augment::Arrowstorm,
        Augment::ExplosiveVolley,
        Augment::HailOfArrows,
        Augment::Minefield,
        Augment::ScorchedEarth,
        Augment::BigGameTrap,
        Augment::FreezingTrap,
        Augment::FrozenWake,
        Augment::DeepFreeze,
        Augment::BestialWrath,
        Augment::KillCommand,
        Augment::Intimidation,
        Augment::ImprovedScatter,
        Augment::WyvernSting,
        Augment::ScatterBurst,
        Augment::ImprovedMark,
        Augment::MarkedForTheKill,
        Augment::HuntersPrey,
        Augment::RapidKilling,
        Augment::TrueshotAura,
        Augment::QuickShots,
        Augment::LightningOverload,
        Augment::Stormkeeper,
        Augment::RollingThunder,
        Augment::ChainReaction,
        Augment::StormNexus,
        Augment::Electrified,
        Augment::Fulmination,
        Augment::EarthenGrip,
        Augment::ShockMastery,
        Augment::LavaBurst,
        Augment::VolcanicFury,
        Augment::SearingFlames,
        Augment::FrozenPower,
        Augment::FrostNovaShock,
        Augment::Icefury,
        Augment::SearingBurn,
        Augment::SearingPact,
        Augment::TwinFires,
        Augment::FireNova,
        Augment::LiquidMagma,
        Augment::MoltenEarth,
        Augment::Earthgrab,
        Augment::EarthquakeTotem,
        Augment::Stoneclaw,
        Augment::HealingTide,
        Augment::Cloudburst,
        Augment::Stoneskin,
        Augment::Thunderstorm,
        Augment::StaticShield,
        Augment::StaticCharge,
        Augment::AncestralFortitude,
        Augment::NaturesSwiftness,
        Augment::TidalFocus,
        Augment::Windfury,
        Augment::Stormlash,
        Augment::MaelstromWeapon,
        Augment::HammerOfTheRighteous,
        Augment::WrathOfTheExecutioner,
        Augment::BlessedHammer,
        Augment::JudgementOfLight,
        Augment::DivineStorm,
        Augment::JudgementOfTheCrusader,
        Augment::Exorcist,
        Augment::ArtOfWar,
        Augment::BanishEvil,
        Augment::InfusionOfLight,
        Augment::GlimmerOfLight,
        Augment::Daybreak,
        Augment::SacredGround,
        Augment::HallowedGround,
        Augment::ConsecratedChill,
        Augment::Wrathful,
        Augment::DivinePurpose,
        Augment::StunningWrath,
        Augment::FistOfJustice,
        Augment::ShockwaveHammer,
        Augment::LastingJustice,
        Augment::AvengersShield,
        Augment::HolyRetribution,
        Augment::Reckoning,
        Augment::HolyPower,
        Augment::DivineFavor,
        Augment::LightOfDawn,
        Augment::AuraMastery,
        Augment::Devotion,
        Augment::Sanctity,
        Augment::SealOfLight,
        Augment::SealOfRighteousness,
        Augment::SealOfWisdom,
        Augment::ImprovedLayOnHands,
        Augment::MartyrsHands,
        Augment::HandOfProtection,
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
            Augment::Reversed => "Reversed",
            Augment::FourWinds => "Four Winds",
            Augment::FullCircle => "Full Circle",
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
            Augment::Shadowflame => "Shadowflame",
            Augment::Twilight => "Twilight",
            Augment::SacredFlame => "Sacred Flame",
            Augment::Thunderstrike => "Thunderstrike",
            Augment::Balance => "Balance",
            Augment::Frostbrand => "Frostbrand",
            Augment::Crusader => "Crusader",
            Augment::Shadowblade => "Shadowblade",
            Augment::Lacerate => "Lacerate",
            Augment::Hemorrhage => "Hemorrhage",
            Augment::Crippling => "Crippling",
            Augment::Virulent => "Virulent",
            Augment::Dread => "Dread",
            Augment::Panic => "Panic",
            Augment::Bramble => "Bramble",
            Augment::Grasping => "Grasping",
            Augment::Crushing => "Crushing",
            Augment::Forked => "Forked",
            Augment::Conductive => "Conductive",
            Augment::Pack => "Pack",
            Augment::FeralBond => "Feral Bond",
            Augment::Frenzy => "Frenzy",
            Augment::BoomingVoice => "Booming Voice",
            Augment::Commanding => "Commanding",
            Augment::DeepReserves => "Deep Reserves",
            Augment::Overflowing => "Overflowing",
            Augment::MendingLink => "Mending Link",
            Augment::Renewing => "Renewing",
            Augment::Sanguine => "Sanguine",
            Augment::Bloodthirsty => "Bloodthirsty",
            Augment::Sweeping => "Sweeping",
            Augment::BloodAndThunder => "Blood and Thunder",
            Augment::Rampage => "Rampage",
            Augment::CleavingSwings => "Cleaving Swings",
            Augment::Overpower => "Overpower",
            Augment::HeroicThrow => "Heroic Throw",
            Augment::ChampionsGuard => "Champion's Guard",
            Augment::MortalWounds => "Mortal Wounds",
            Augment::SuddenDeath => "Sudden Death",
            Augment::ColossusSmash => "Colossus Smash",
            Augment::Bladestorm => "Bladestorm",
            Augment::FuriousSpin => "Furious Spin",
            Augment::SecondWind => "Second Wind",
            Augment::Shockwave => "Shockwave",
            Augment::Aftershock => "Aftershock",
            Augment::Thunderstruck => "Thunderstruck",
            Augment::GushingWound => "Gushing Wound",
            Augment::BloodFrenzy => "Blood Frenzy",
            Augment::DeepWounds => "Deep Wounds",
            Augment::ExecutionersChain => "Executioner's Chain",
            Augment::Massacre => "Massacre",
            Augment::VictoryRush => "Victory Rush",
            Augment::BloodCraze => "Blood Craze",
            Augment::UnbridledWrath => "Unbridled Wrath",
            Augment::Bloodbath => "Bloodbath",
            Augment::Revenge => "Revenge",
            Augment::ShieldBash => "Shield Bash",
            Augment::LastStand => "Last Stand",
            Augment::RallyingCry => "Rallying Cry",
            Augment::CripplingPresence => "Crippling Presence",
            Augment::WarCry => "War Cry",
            Augment::ChallengingShout => "Challenging Shout",
            Augment::ShatteringRoar => "Shattering Roar",
            Augment::Fearsome => "Fearsome",
            Augment::ExposeArmor => "Expose Armor",
            Augment::Devastate => "Devastate",
            Augment::SunderingBlows => "Sundering Blows",
            Augment::CombatPotency => "Combat Potency",
            Augment::BladeDance => "Blade Dance",
            Augment::RelentlessStrikes => "Relentless Strikes",
            Augment::Ambush => "Ambush",
            Augment::Opportunity => "Opportunity",
            Augment::SealFate => "Seal Fate",
            Augment::RestlessBlades => "Restless Blades",
            Augment::ColdBlood => "Cold Blood",
            Augment::Ruthlessness => "Ruthlessness",
            Augment::VenomousWounds => "Venomous Wounds",
            Augment::CrimsonTempest => "Crimson Tempest",
            Augment::Bloodletter => "Bloodletter",
            Augment::AdrenalineRush => "Adrenaline Rush",
            Augment::FlowingSteel => "Flowing Steel",
            Augment::SerratedBlades => "Serrated Blades",
            Augment::PoisonedKnives => "Poisoned Knives",
            Augment::LethalFan => "Lethal Fan",
            Augment::FanThePack => "Fan the Pack",
            Augment::Envenom => "Envenom",
            Augment::ToxicCloud => "Toxic Cloud",
            Augment::MindNumbingPoison => "Mind-numbing Poison",
            Augment::ImprovedGouge => "Improved Gouge",
            Augment::DirtyTricks => "Dirty Tricks",
            Augment::GougedEyes => "Gouged Eyes",
            Augment::MarkedForDeath => "Marked for Death",
            Augment::KidneyBurst => "Kidney Burst",
            Augment::QuickRecovery => "Quick Recovery",
            Augment::Adrenaline => "Adrenaline",
            Augment::RestlessTempo => "Restless Tempo",
            Augment::ImprovedSlice => "Improved Slice",
            Augment::Counterattack => "Counterattack",
            Augment::Nimble => "Nimble",
            Augment::CheatDeath => "Cheat Death",
            Augment::BlindingPowder => "Blinding Powder",
            Augment::SmokeBomb => "Smoke Bomb",
            Augment::Sap => "Sap",
            Augment::Atonement => "Atonement",
            Augment::DivineFury => "Divine Fury",
            Augment::SearingLight => "Searing Light",
            Augment::Martyrdom => "Martyrdom",
            Augment::PurgeTheWicked => "Purge the Wicked",
            Augment::HolyFervor => "Holy Fervor",
            Augment::PsychicHorror => "Psychic Horror",
            Augment::ShadowyApparitions => "Shadowy Apparitions",
            Augment::ShadowyInsight => "Shadowy Insight",
            Augment::SpreadingPain => "Spreading Pain",
            Augment::Misery => "Misery",
            Augment::DeathsEcho => "Death's Echo",
            Augment::MindSear => "Mind Sear",
            Augment::PsychicLink => "Psychic Link",
            Augment::MindTrauma => "Mind Trauma",
            Augment::Plaguebearer => "Plaguebearer",
            Augment::VampiricEmbrace => "Vampiric Embrace",
            Augment::Insatiable => "Insatiable",
            Augment::SearingNova => "Searing Nova",
            Augment::DivineAegis => "Divine Aegis",
            Augment::Inspiration => "Inspiration",
            Augment::ShootingStars => "Shooting Stars",
            Augment::Starfall => "Starfall",
            Augment::LunarGuidance => "Lunar Guidance",
            Augment::HolyWordSanctify => "Holy Word: Sanctify",
            Augment::BlessedRecovery => "Blessed Recovery",
            Augment::InnerFocus => "Inner Focus",
            Augment::BodyAndSoul => "Body and Soul",
            Augment::Rapture => "Rapture",
            Augment::BorrowedTime => "Borrowed Time",
            Augment::Sanctuary => "Sanctuary",
            Augment::LightsWrath => "Light's Wrath",
            Augment::SpringOfLight => "Spring of Light",
            Augment::TouchOfWeakness => "Touch of Weakness",
            Augment::HexOfDomination => "Hex of Domination",
            Augment::SpreadingHex => "Spreading Hex",
            Augment::Nightfall => "Nightfall",
            Augment::ShadowBoltVolley => "Shadow Bolt Volley",
            Augment::ImprovedShadowBolt => "Improved Shadow Bolt",
            Augment::SiphonLife => "Siphon Life",
            Augment::SeedOfCorruption => "Seed of Corruption",
            Augment::EverlastingAffliction => "Everlasting Affliction",
            Augment::CurseOfDoom => "Curse of Doom",
            Augment::AmplifyCurse => "Amplify Curse",
            Augment::CurseOfExhaustion => "Curse of Exhaustion",
            Augment::FireAndBrimstone => "Fire and Brimstone",
            Augment::Backdraft => "Backdraft",
            Augment::Emberstorm => "Emberstorm",
            Augment::FirestormRain => "Firestorm Rain",
            Augment::Aftermath => "Aftermath",
            Augment::Pyroclasm => "Pyroclasm",
            Augment::InfernalPact => "Infernal Pact",
            Augment::RitualOfFire => "Ritual of Fire",
            Augment::HellfireWard => "Hellfire Ward",
            Augment::Horrify => "Horrify",
            Augment::DeathsEmbrace => "Death's Embrace",
            Augment::DarkBargain => "Dark Bargain",
            Augment::SoulHarvest => "Soul Harvest",
            Augment::DrainAura => "Drain Aura",
            Augment::ImprovedDrain => "Improved Drain",
            Augment::Nightmare => "Nightmare",
            Augment::Shadowfury => "Shadowfury",
            Augment::DemonicPresence => "Demonic Presence",
            Augment::BurningImp => "Burning Imp",
            Augment::DemonicSacrifice => "Demonic Sacrifice",
            Augment::ImprovedImp => "Improved Imp",
            Augment::Sacrifice => "Sacrifice",
            Augment::Torment => "Torment",
            Augment::ConsumeShadows => "Consume Shadows",
            Augment::ImprovedSearingPain => "Improved Searing Pain",
            Augment::SoulFire => "Soul Fire",
            Augment::FelFlame => "Fel Flame",
            Augment::ArcaneBarrage => "Arcane Barrage",
            Augment::TrickShot => "Trick Shot",
            Augment::LockAndLoad => "Lock and Load",
            Augment::SplinterArrows => "Splinter Arrows",
            Augment::SpreadShot => "Spread Shot",
            Augment::SerpentSpread => "Serpent Spread",
            Augment::CarefulAim => "Careful Aim",
            Augment::SniperTraining => "Sniper Training",
            Augment::PiercingShots => "Piercing Shots",
            Augment::SpreadingVenom => "Spreading Venom",
            Augment::NoxiousStings => "Noxious Stings",
            Augment::ScorpidSting => "Scorpid Sting",
            Augment::ImprovedConcussion => "Improved Concussion",
            Augment::DazeWave => "Daze Wave",
            Augment::FrostShot => "Frost Shot",
            Augment::Arrowstorm => "Arrowstorm",
            Augment::ExplosiveVolley => "Explosive Volley",
            Augment::HailOfArrows => "Hail of Arrows",
            Augment::Minefield => "Minefield",
            Augment::ScorchedEarth => "Scorched Earth",
            Augment::BigGameTrap => "Big Game Trap",
            Augment::FreezingTrap => "Freezing Trap",
            Augment::FrozenWake => "Frozen Wake",
            Augment::DeepFreeze => "Deep Freeze",
            Augment::BestialWrath => "Bestial Wrath",
            Augment::KillCommand => "Kill Command",
            Augment::Intimidation => "Intimidation",
            Augment::ImprovedScatter => "Improved Scatter",
            Augment::WyvernSting => "Wyvern Sting",
            Augment::ScatterBurst => "Scatter Burst",
            Augment::ImprovedMark => "Improved Mark",
            Augment::MarkedForTheKill => "Marked for the Kill",
            Augment::HuntersPrey => "Hunter's Prey",
            Augment::RapidKilling => "Rapid Killing",
            Augment::TrueshotAura => "Trueshot Aura",
            Augment::QuickShots => "Quick Shots",
            Augment::LightningOverload => "Lightning Overload",
            Augment::Stormkeeper => "Stormkeeper",
            Augment::RollingThunder => "Rolling Thunder",
            Augment::ChainReaction => "Chain Reaction",
            Augment::StormNexus => "Storm Nexus",
            Augment::Electrified => "Electrified",
            Augment::Fulmination => "Fulmination",
            Augment::EarthenGrip => "Earthen Grip",
            Augment::ShockMastery => "Shock Mastery",
            Augment::LavaBurst => "Lava Burst",
            Augment::VolcanicFury => "Volcanic Fury",
            Augment::SearingFlames => "Searing Flames",
            Augment::FrozenPower => "Frozen Power",
            Augment::FrostNovaShock => "Frost Nova Shock",
            Augment::Icefury => "Icefury",
            Augment::SearingBurn => "Searing Burn",
            Augment::SearingPact => "Searing Pact",
            Augment::TwinFires => "Twin Fires",
            Augment::FireNova => "Fire Nova",
            Augment::LiquidMagma => "Liquid Magma",
            Augment::MoltenEarth => "Molten Earth",
            Augment::Earthgrab => "Earthgrab",
            Augment::EarthquakeTotem => "Earthquake Totem",
            Augment::Stoneclaw => "Stoneclaw",
            Augment::HealingTide => "Healing Tide",
            Augment::Cloudburst => "Cloudburst",
            Augment::Stoneskin => "Stoneskin",
            Augment::Thunderstorm => "Thunderstorm",
            Augment::StaticShield => "Static Shield",
            Augment::StaticCharge => "Static Charge",
            Augment::AncestralFortitude => "Ancestral Fortitude",
            Augment::NaturesSwiftness => "Nature's Swiftness",
            Augment::TidalFocus => "Tidal Focus",
            Augment::Windfury => "Windfury",
            Augment::Stormlash => "Stormlash",
            Augment::MaelstromWeapon => "Maelstrom Weapon",
            Augment::HammerOfTheRighteous => "Hammer of the Righteous",
            Augment::WrathOfTheExecutioner => "Wrath of the Executioner",
            Augment::BlessedHammer => "Blessed Hammer",
            Augment::JudgementOfLight => "Judgement of Light",
            Augment::DivineStorm => "Divine Storm",
            Augment::JudgementOfTheCrusader => "Judgement of the Crusader",
            Augment::Exorcist => "Exorcist",
            Augment::ArtOfWar => "Art of War",
            Augment::BanishEvil => "Banish Evil",
            Augment::InfusionOfLight => "Infusion of Light",
            Augment::GlimmerOfLight => "Glimmer of Light",
            Augment::Daybreak => "Daybreak",
            Augment::SacredGround => "Sacred Ground",
            Augment::HallowedGround => "Hallowed Ground",
            Augment::ConsecratedChill => "Consecrated Chill",
            Augment::Wrathful => "Wrathful",
            Augment::DivinePurpose => "Divine Purpose",
            Augment::StunningWrath => "Stunning Wrath",
            Augment::FistOfJustice => "Fist of Justice",
            Augment::ShockwaveHammer => "Shockwave Hammer",
            Augment::LastingJustice => "Lasting Justice",
            Augment::AvengersShield => "Avenger's Shield",
            Augment::HolyRetribution => "Holy Retribution",
            Augment::Reckoning => "Reckoning",
            Augment::HolyPower => "Holy Power",
            Augment::DivineFavor => "Divine Favor",
            Augment::LightOfDawn => "Light of Dawn",
            Augment::AuraMastery => "Aura Mastery",
            Augment::Devotion => "Devotion",
            Augment::Sanctity => "Sanctity",
            Augment::SealOfLight => "Seal of Light",
            Augment::SealOfRighteousness => "Seal of Righteousness",
            Augment::SealOfWisdom => "Seal of Wisdom",
            Augment::ImprovedLayOnHands => "Improved Lay on Hands",
            Augment::MartyrsHands => "Martyr's Hands",
            Augment::HandOfProtection => "Hand of Protection",
        }
    }

    /// What it does, for its card.
    pub fn description(self) -> &'static str {
        match self {
            Augment::Frostfire => "Counts as Fire and Frost. A Fire spell also Chills; a Frost spell also leaves Burning.",
            Augment::Spellfire => "Counts as Fire and Arcane. +15% area.",
            Augment::Spellfrost => "Counts as Frost and Arcane. -15% cooldown.",
            Augment::Multishot => "+2 projectiles, -25% damage each.",
            Augment::Piercing => "Passes through 2 more enemies.",
            Augment::Returning => "Flies back, hitting its target again.",
            Augment::Wide => "+40% area, -20% damage.",
            Augment::Concentrated => "-30% area, +40% damage.",
            Augment::Echo => "Repeats at half strength 1 second later.",
            Augment::Ring => "Hits a ring from 5 to 12 yards instead of a circle. +50% damage.",
            Augment::Reversed => "The cone points behind you instead of ahead.",
            Augment::FourWinds => "Four narrower cones, one in each direction.",
            Augment::FullCircle => "Hits all around you instead of a cone, with 30% less reach.",
            Augment::Lingering => "Lasts 50% longer.",
            Augment::Mobile => "Follows you.",
            Augment::Targeted => "Lands on the thickest crowd within 25 yards instead of around you.",
            Augment::Contagion => "When a Burning enemy dies, its burn jumps to a nearby enemy.",
            Augment::Quickened => "Its damage over time runs 30% faster.",
            Augment::Molten => "Its damage over time deals double and lasts twice as long; its direct hit -30%.",
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
            Augment::Twin => "Casts twice, -40% damage each.",
            Augment::Hasted => "-25% cooldown, -15% damage.",
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
            Augment::Firestorm => "2 more Flamestrikes on other crowds, -40% damage each.",
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
            Augment::Shadowflame => "Counts as Fire and Shadow. A Fire spell heals you for 10% of its damage; a Shadow spell also leaves Burning.",
            Augment::Twilight => "Counts as Holy and Shadow. +20% damage to bosses and elites.",
            Augment::SacredFlame => "Counts as Holy and Fire. A Holy spell also leaves Burning; a Fire spell heals you for 10% of its damage.",
            Augment::Thunderstrike => "Counts as Physical and Nature. Each hit arcs to one more enemy within 8 yards for 50%.",
            Augment::Balance => "Counts as Arcane and Nature. +15% critical strike chance.",
            Augment::Frostbrand => "Counts as Frost and Nature. A Nature spell also Chills; a Frost spell also Poisons (30% of the hit over 4 seconds).",
            Augment::Crusader => "Counts as Physical and Holy. Its hits heal you for 2% of your health, at most once a second.",
            Augment::Shadowblade => "Counts as Physical and Shadow. +25% damage to enemies that are Feared, Stunned, Frozen or Rooted.",
            Augment::Lacerate => "Its Bleed stacks up to 3 times.",
            Augment::Hemorrhage => "Enemies Bleeding from your spells take +20% from your Physical hits.",
            Augment::Crippling => "Enemies it Poisons are Chilled.",
            Augment::Virulent => "Its Poison stacks 3 more times.",
            Augment::Dread => "Feared enemies take +25% damage from it.",
            Augment::Panic => "Its Fear lasts 1.5 seconds longer.",
            Augment::Bramble => "Rooted enemies take +25% damage from it.",
            Augment::Grasping => "Its Root lasts 1.5 seconds longer.",
            Augment::Crushing => "+40% damage to Stunned enemies.",
            Augment::Forked => "Jumps 2 more times.",
            Augment::Conductive => "Its jumps lose no damage.",
            Augment::Pack => "Two at once, -35% damage each.",
            Augment::FeralBond => "Its strikes heal you for 5% of their damage.",
            Augment::Frenzy => "Strikes 40% faster.",
            Augment::BoomingVoice => "+50% area.",
            Augment::Commanding => "Its effects last 50% longer.",
            Augment::DeepReserves => "Holds 2 more charges.",
            Augment::Overflowing => "Its overflow strikes 2 enemies for +50%.",
            Augment::MendingLink => "A heal it gives also heals every other hero within 10 yards for half as much.",
            Augment::Renewing => "A heal it gives also heals 30% more over 6 seconds.",
            Augment::Sanguine => "Lifesteal past full health becomes a shield, up to 20% of your health.",
            Augment::Bloodthirsty => "Heals you for 10% more of its damage.",
            Augment::Sweeping => "Its single-target hits also strike one enemy beside the target for 50%.",
            Augment::BloodAndThunder => "Enemies it hits that are not Bleeding get your Rend (yours, with its augments, if you know it).",
            Augment::Rampage => "Each enemy it hits gives you +1% damage for 8 seconds, up to 30%.",
            Augment::CleavingSwings => "Every 3rd Cleave hits every enemy within its reach, not only the closest.",
            Augment::Overpower => "When it crits, it is ready again at once and its next hit deals +50%.",
            Augment::HeroicThrow => "Reaches 20 yards: you hurl your weapon at the elite. Counts as a Projectile.",
            Augment::ChampionsGuard => "Each hit on a boss or elite banks a Shield Block charge (up to its cap, even without Shield Block).",
            Augment::MortalWounds => "Its target is Marked: it takes +20% damage from every source for 6 seconds.",
            Augment::SuddenDeath => "Each kill by your Melee spells has a 10% chance to make Mortal Strike ready.",
            Augment::ColossusSmash => "Also strikes every enemy within 6 yards of its target for 50% and Stuns them for 1 second. +50% cooldown.",
            Augment::Bladestorm => "Becomes a storm of blades that follows you for 4 seconds, striking everything within reach twice a second at 40%.",
            Augment::FuriousSpin => "Each kill it makes takes half a second off its cooldown.",
            Augment::SecondWind => "Each enemy it hits heals you for 1% of your health, up to 10% a spin.",
            Augment::Shockwave => "A 12-yard cone ahead of you instead. +40% damage, and it Stuns for 2 seconds instead of Chilling.",
            Augment::Aftershock => "Leaves the ground shaking for 4 seconds: it hits for 25% and Chills every second.",
            Augment::Thunderstruck => "Enemies it hits are Weakened (25% less damage) for 6 seconds.",
            Augment::GushingWound => "When an enemy Bleeding from your spells dies, its bleeds burst onto every enemy within 5 yards.",
            Augment::BloodFrenzy => "Your Bleeds heal you for 20% of the damage they deal.",
            Augment::DeepWounds => "Your Melee crits make their target Bleed with your Rend.",
            Augment::ExecutionersChain => "A kill casts Execute again at once at the next most wounded enemy.",
            Augment::Massacre => "Its bonus counts below 50% health instead of 30%.",
            Augment::VictoryRush => "Each kill heals you for 5% of your health.",
            Augment::BloodCraze => "Below 50% health, its cooldown is halved and it heals twice as much.",
            Augment::UnbridledWrath => "Its crits cast Whirlwind (yours, if you know it), at most every 4 seconds.",
            Augment::Bloodbath => "Its target also Bleeds for 100% of the hit over 6 seconds.",
            Augment::Revenge => "Every block spent strikes every enemy within 6 yards for 150% of its damage.",
            Augment::ShieldBash => "A blocked attacker is Stunned for 1.5 seconds.",
            Augment::LastStand => "Below 30% health, every charge refills at once (at most every 20 seconds).",
            Augment::RallyingCry => "Each enemy it reaches heals you for 1% of your health, up to 15%.",
            Augment::CripplingPresence => "Enemies it Weakens are also Chilled.",
            Augment::WarCry => "Also gives you and every ally within 20 yards +15% damage for 6 seconds.",
            Augment::ChallengingShout => "Instead of scattering them, it pulls every enemy within 15 yards to you, and you take 40% less damage for 4 seconds.",
            Augment::ShatteringRoar => "Everything it reaches is also Sundered twice.",
            Augment::Fearsome => "Its Fear holds elites as long as normal enemies.",
            Augment::ExposeArmor => "At 5 Sunder stacks the armor shatters: the stacks go, and the target is Marked for +25% for 8 seconds.",
            Augment::Devastate => "Hits every enemy within 6 yards of you instead, at 60%, each taking a stack.",
            Augment::SunderingBlows => "Your Melee crits on a Sundered enemy add a stack.",
            Augment::CombatPotency => "Each hit has a 25% chance to give an extra combo point.",
            Augment::BladeDance => "Bounces from its target to the next closest enemy within 8 yards, up to 4 times; every bounce gives a combo point.",
            Augment::RelentlessStrikes => "Each hit takes 0.3 seconds off your Finishers' cooldowns.",
            Augment::Ambush => "For 4 seconds after your defensive button, it deals triple damage and always crits.",
            Augment::Opportunity => "Its bonus against Stunned, Frozen or Feared enemies is +200%.",
            Augment::SealFate => "Its crits give an extra combo point.",
            Augment::RestlessBlades => "Each combo point it spends takes 2 seconds off your defensive button's cooldown.",
            Augment::ColdBlood => "At 5 combo points it also strikes every enemy within 6 yards of its target for half.",
            Augment::Ruthlessness => "Each Eviscerate gives back 1 combo point.",
            Augment::VenomousWounds => "Its bleed deals +50% to enemies Poisoned by you.",
            Augment::CrimsonTempest => "Its bleed also spreads to every enemy within 8 yards of its target.",
            Augment::Bloodletter => "When a Ruptured enemy dies, you gain 2 combo points.",
            Augment::AdrenalineRush => "Hitting 8 or more enemies makes your spells come back 50% faster for 4 seconds (at most every 12 seconds).",
            Augment::FlowingSteel => "While you keep moving it comes back twice as fast; standing still, half as fast.",
            Augment::SerratedBlades => "Everything it hits Bleeds for 50% of the hit over 4 seconds.",
            Augment::PoisonedKnives => "Everything it hits gets your Deadly Poison (yours, with its augments, if you know it).",
            Augment::LethalFan => "Reaches 20 yards but only hits enemies that are Bleeding, Burning or Poisoned, for +100%.",
            Augment::FanThePack => "Gives a combo point for every 3 enemies it hits.",
            Augment::Envenom => "Your Finishers consume your poison stacks on their target, dealing what they had left at once, doubled.",
            Augment::ToxicCloud => "An enemy Poisoned by you that dies leaves a poison cloud (5 yards, 4 seconds) that poisons what walks in.",
            Augment::MindNumbingPoison => "Enemies with 3 or more of its stacks are Weakened.",
            Augment::ImprovedGouge => "Its Stun lasts 1 second longer.",
            Augment::DirtyTricks => "It seeks bosses and elites first, and its Stun holds elites as long as normal enemies.",
            Augment::GougedEyes => "Stunned enemies take +30% from your Finishers.",
            Augment::MarkedForDeath => "Its target is Marked (+25% damage from everyone) for its stun and 4 seconds more; bosses resist the stun but not the mark.",
            Augment::KidneyBurst => "At 5 combo points it also strikes and stuns every enemy within 5 yards of its target.",
            Augment::QuickRecovery => "Heals you 2% of your health for every combo point it spends.",
            Augment::Adrenaline => "Also +20% move speed while it lasts.",
            Augment::RestlessTempo => "Each kill while it lasts makes it last 1 second longer, with no limit.",
            Augment::ImprovedSlice => "Your spells come back 45% faster instead of 30%.",
            Augment::Counterattack => "Every dodge spent casts Blade Flurry (yours, with its augments, if you know it).",
            Augment::Nimble => "Every dodge gives +10% move speed for 3 seconds, up to 30%.",
            Augment::CheatDeath => "Once every 60 seconds, a killing blow leaves you at 10% health and refills your dodges.",
            Augment::BlindingPowder => "Enemies it Fears stay Chilled for 4 seconds after.",
            Augment::SmokeBomb => "Instead of Fear, a cloud of smoke at your feet for 5 seconds: enemies inside lose you and take +30% damage.",
            Augment::Sap => "A Blinded enemy takes double damage from your next hit on it.",
            Augment::Atonement => "Each Smite heals you, or the most wounded ally within 30 yards, for 50% of its damage.",
            Augment::DivineFury => "Every 4th Smite strikes 3 targets.",
            Augment::SearingLight => "Each Smite on the same target deals +10% more than the last, up to +50%; a new target starts again.",
            Augment::Martyrdom => "Costs 3% of your health each cast, but deals +80%.",
            Augment::PurgeTheWicked => "When its target dies, its burn jumps to the 2 closest enemies.",
            Augment::HolyFervor => "Each of its burn ticks has a 5% chance to make Holy Fire ready.",
            Augment::PsychicHorror => "Its target and every enemy within 5 yards of it flee in Fear for 2 seconds.",
            Augment::ShadowyApparitions => "Its crits send a shadow at the closest other enemy for 50%.",
            Augment::ShadowyInsight => "Each Shadow Word: Pain tick has a 10% chance to make Mind Blast ready.",
            Augment::SpreadingPain => "It goes on enemies that do not have it yet first, anywhere within 30 yards.",
            Augment::Misery => "Enemies in pain are Marked: +10% damage from every source.",
            Augment::DeathsEcho => "When an enemy in pain dies, a shadow flies to the closest enemy for 50% of the pain's total.",
            Augment::MindSear => "Each hit also strikes every enemy within 4 yards of the target for 50%.",
            Augment::PsychicLink => "Each hit also strikes every enemy with your Shadow Word: Pain for 30%.",
            Augment::MindTrauma => "Flayed enemies are Weakened for 4 seconds.",
            Augment::Plaguebearer => "When a plagued enemy dies, the plague jumps to the 2 closest enemies.",
            Augment::VampiricEmbrace => "While any enemy carries your plague, all your Shadow damage heals you and every ally within 20 yards for 15% of it.",
            Augment::Insatiable => "Heals twice as much while you are below 50% health.",
            Augment::SearingNova => "Everything it hits Burns for 40% of the hit over 4 seconds.",
            Augment::DivineAegis => "Its heal on a hero at full health becomes a shield instead, up to 20% of their health.",
            Augment::Inspiration => "Each enemy it hits makes you take 1% less damage for 4 seconds, up to 20%.",
            Augment::ShootingStars => "Its crits drop a star on another crowd for 100%.",
            Augment::Starfall => "Instead of one spot, stars fall on 3 enemies near you every second for 6 seconds. It goes where you go.",
            Augment::LunarGuidance => "Enemies it hits are Marked +10% for 4 seconds.",
            Augment::HolyWordSanctify => "Spending your last charge casts Holy Nova (yours, if you know it) and heals you 25%.",
            Augment::BlessedRecovery => "A charge also heals you when a single blow takes 15% or more of your health.",
            Augment::InnerFocus => "While your bank is full, your other Holy spells deal +15%.",
            Augment::BodyAndSoul => "While the shield holds, +25% move speed.",
            Augment::Rapture => "When the shield breaks, every enemy within 8 yards flees in Fear for 3 seconds.",
            Augment::BorrowedTime => "After it breaks, your spells come back 50% faster for 4 seconds.",
            Augment::Sanctuary => "Heroes inside take 20% less damage.",
            Augment::LightsWrath => "When the well ends, it bursts for holy damage equal to all it healed, within 10 yards.",
            Augment::SpringOfLight => "Enemies inside are Chilled.",
            Augment::TouchOfWeakness => "A hexed enemy that strikes you takes Hex's damage back, tripled.",
            Augment::HexOfDomination => "Its targets also take +30% from everyone and deal 50% less instead of 25%.",
            Augment::SpreadingHex => "When a hexed enemy dies, the hex jumps to the closest enemy.",
            Augment::Nightfall => "Your damage over time ticks have a 5% chance to make your next Shadow Bolt deal triple.",
            Augment::ShadowBoltVolley => "It hits the 5 closest enemies instead, for 60%.",
            Augment::ImprovedShadowBolt => "Each hit makes its target take +4% Shadow damage, up to 20%, from every source.",
            Augment::SiphonLife => "Its ticks heal you for 20% of their damage.",
            Augment::SeedOfCorruption => "A Corrupted enemy that dies bursts for the corruption's full damage within 6 yards and corrupts what it hits.",
            Augment::EverlastingAffliction => "Your Shadow Bolts renew Corruption on their target.",
            Augment::CurseOfDoom => "When the curse ends on a target still alive, it deals twice its total at once.",
            Augment::AmplifyCurse => "+50% damage, and cursed enemies are Weakened.",
            Augment::CurseOfExhaustion => "Cursed enemies are Chilled while it lasts.",
            Augment::FireAndBrimstone => "Also hits every enemy within 5 yards of its targets for half, all of them Burning.",
            Augment::Backdraft => "Its crits make your Fire spells come back 30% faster for 4 seconds.",
            Augment::Emberstorm => "Its burn deals +50% to bosses and elites.",
            Augment::FirestormRain => "It follows the thickest crowd near you.",
            Augment::Aftermath => "Its ticks Chill.",
            Augment::Pyroclasm => "Each tick has a 15% chance to Stun what it hits for 1 second.",
            Augment::InfernalPact => "It burns you for 1% of your health a second, but deals +100%, and every enemy that dies inside it heals you 3%.",
            Augment::RitualOfFire => "Everything it touches Burns for 40% of the hit over 4 seconds.",
            Augment::HellfireWard => "While it burns, you take 20% less damage.",
            Augment::Horrify => "Its Fear spreads to every enemy within 5 yards of its target.",
            Augment::DeathsEmbrace => "Below 35% health, it comes back three times as fast.",
            Augment::DarkBargain => "Its heal also heals every ally within 10 yards.",
            Augment::SoulHarvest => "When a drained enemy dies, +5% damage for 20 seconds, up to 25%.",
            Augment::DrainAura => "Drains every enemy within 6 yards of you at once.",
            Augment::ImprovedDrain => "Heals twice as much from bosses and elites.",
            Augment::Nightmare => "Feared enemies also take shadow damage over time (300% of the hit over 4 seconds).",
            Augment::Shadowfury => "Instead of Fear, it Stuns everything in reach for 2 seconds and deals +200%.",
            Augment::DemonicPresence => "Feared enemies are Marked +15%.",
            Augment::BurningImp => "Its firebolts leave Burning (50% over 4 seconds).",
            Augment::DemonicSacrifice => "The imp never comes: instead your Fire spells deal +25%.",
            Augment::ImprovedImp => "Its firebolts hit 2 enemies.",
            Augment::Sacrifice => "When you drop below 30% health, the voidwalker gives itself up: a shield worth 40% of your health.",
            Augment::Torment => "Enemies it hits are Weakened.",
            Augment::ConsumeShadows => "While it is out, you heal 1% of your health a second.",
            Augment::ImprovedSearingPain => "+50% critical strike chance against enemies within 5 yards.",
            Augment::SoulFire => "Every 5th Searing Pain is a Soul Fire: +300% damage, and it Stuns for 1 second.",
            Augment::FelFlame => "Each hit makes your damage over time on its target last 2 seconds longer.",
            Augment::ArcaneBarrage => "Every 4th shot is three arrows (bosses and elites first).",
            Augment::TrickShot => "It ricochets to a second enemy within 8 yards, then a third.",
            Augment::LockAndLoad => "Its crits make Aimed Shot ready.",
            Augment::SplinterArrows => "Each arrow bursts within 3 yards for half its damage.",
            Augment::SpreadShot => "A fan of arrows at every enemy in a cone ahead of you instead, 70% each. You aim it by facing.",
            Augment::SerpentSpread => "Applies your Serpent Sting (yours, if you know it) to what it hits.",
            Augment::CarefulAim => "+60% critical strike chance against enemies above 80% health.",
            Augment::SniperTraining => "After 2 seconds standing still, +50% range and +100% damage.",
            Augment::PiercingShots => "Its target Bleeds for 50% of the hit over 6 seconds.",
            Augment::SpreadingVenom => "It goes on enemies without it first, anywhere within 30 yards.",
            Augment::NoxiousStings => "Stung enemies take +10% from your Physical spells.",
            Augment::ScorpidSting => "Stung enemies are Weakened while it lasts.",
            Augment::ImprovedConcussion => "20% chance to Stun its target for 2 seconds.",
            Augment::DazeWave => "Its Chill spreads to every enemy within 6 yards of its target.",
            Augment::FrostShot => "Its Chill lasts twice as long.",
            Augment::Arrowstorm => "While you stand still, it keeps falling on the thickest crowd with no end. Moving ends it.",
            Augment::ExplosiveVolley => "What it hits Burns for 30% over 4 seconds.",
            Augment::HailOfArrows => "It Chills.",
            Augment::Minefield => "Lays three traps around you instead of one, at -40% each.",
            Augment::ScorchedEarth => "Its burning ground lasts twice as long and Chills.",
            Augment::BigGameTrap => "+100% damage to bosses and elites.",
            Augment::FreezingTrap => "The enemy that springs it is Frozen for 4 seconds.",
            Augment::FrozenWake => "While you move, a small patch of its ice drops behind you every 3 seconds.",
            Augment::DeepFreeze => "Its Chill slows 70%.",
            Augment::BestialWrath => "Every 20 seconds, the pet bites for triple damage for 6 seconds.",
            Augment::KillCommand => "When your Arcane Shot or Aimed Shot crits, the pet bites its target at once for double damage.",
            Augment::Intimidation => "Every 5th bite Stuns its target for 2 seconds.",
            Augment::ImprovedScatter => "Its target stays Chilled for 4 seconds after the stun.",
            Augment::WyvernSting => "Its target sleeps for 6 seconds (a long Stun), and is Poisoned for 100% of the hit over 6 seconds.",
            Augment::ScatterBurst => "Also knocks back and stuns every enemy within 4 yards of its target.",
            Augment::ImprovedMark => "Marks for +25% instead of +15%.",
            Augment::MarkedForTheKill => "When a Marked enemy dies, the mark jumps to the next boss or elite in reach, or the closest enemy.",
            Augment::HuntersPrey => "Your Projectile spells deal +20% to Marked enemies.",
            Augment::RapidKilling => "Each kill while it lasts makes it last 0.5 seconds longer.",
            Augment::TrueshotAura => "Instead, always on: you and every ally within 20 yards deal +10% damage.",
            Augment::QuickShots => "Also +20% move speed while it lasts.",
            Augment::LightningOverload => "20% chance to strike its target a second time for 50%.",
            Augment::Stormkeeper => "Every 5th bolt chains to 4 more enemies.",
            Augment::RollingThunder => "Its crits bank a Lightning Shield orb (up to its cap, even without it).",
            Augment::ChainReaction => "A kill makes it ready (at most every 4 seconds).",
            Augment::StormNexus => "It jumps without limit between enemies within 3 yards of each other, losing little each jump.",
            Augment::Electrified => "Everything it hits is Chilled.",
            Augment::Fulmination => "Spends your Lightning Shield orbs for +60% damage each.",
            Augment::EarthenGrip => "Also Roots every enemy within 6 yards of its target for 2 seconds.",
            Augment::ShockMastery => "Each Earth Shock makes Flame Shock and Frost Shock ready.",
            Augment::LavaBurst => "Your Lightning Bolts always crit enemies with your Flame Shock.",
            Augment::VolcanicFury => "An enemy that dies with it erupts: burning ground (5 yards, 4 seconds) where it fell.",
            Augment::SearingFlames => "Searing Totem's bolts renew it on their target.",
            Augment::FrozenPower => "Roots for 2 seconds instead of Chilling; rooted enemies take +30% from your Nature spells.",
            Augment::FrostNovaShock => "Also Chills every enemy within 5 yards of its targets.",
            Augment::Icefury => "Its crits Freeze for 2 seconds.",
            Augment::SearingBurn => "Its bolts leave Burning (40% over 4 seconds).",
            Augment::SearingPact => "While you stand within 8 yards of it, your Fire spells deal +20%.",
            Augment::TwinFires => "Its bolts hit 2 enemies.",
            Augment::FireNova => "When it ends, it explodes for 300% within 8 yards.",
            Augment::LiquidMagma => "Each pulse also spits a glob at 3 random enemies within 20 yards.",
            Augment::MoltenEarth => "Its pulses leave Burning (30% over 4 seconds).",
            Augment::Earthgrab => "An enemy entering its reach is Rooted for 2 seconds (once each).",
            Augment::EarthquakeTotem => "Its pulses deal +300% and Stun for half a second instead of Chilling.",
            Augment::Stoneclaw => "Enemies in its reach are Weakened.",
            Augment::HealingTide => "Heals twice as much on heroes below 30% health.",
            Augment::Cloudburst => "It stores everything it heals and releases it as nature damage within 10 yards when it ends.",
            Augment::Stoneskin => "Heroes in its reach take 15% less damage.",
            Augment::Thunderstorm => "Spending your last orb blasts everything within 8 yards for 200% and knocks it back.",
            Augment::StaticShield => "Its zaps also Chill.",
            Augment::StaticCharge => "Each orb spent gives +5% Nature damage for 6 seconds, up to 15%.",
            Augment::AncestralFortitude => "A hero it heals takes 20% less damage for 6 seconds.",
            Augment::NaturesSwiftness => "Below 25% health, every wave spends at once on you and your spells come back twice as fast for 4 seconds (at most every 30 seconds).",
            Augment::TidalFocus => "Its waves bank 30% faster.",
            Augment::Windfury => "20% chance a hit strikes twice more at once; each Windfury hit banks a Lightning Shield orb.",
            Augment::Stormlash => "Its mark is +30%.",
            Augment::MaelstromWeapon => "Every 5th Stormstrike makes your next Lightning Bolt or Chain Lightning deal double.",
            Augment::HammerOfTheRighteous => "Its hit bursts within 3 yards for half its damage.",
            Augment::WrathOfTheExecutioner => "It seeks the most wounded enemy instead, +200% against enemies below 20% health, and a kill makes it ready.",
            Augment::BlessedHammer => "Its crits heal you 3% of your health.",
            Augment::JudgementOfLight => "A judged enemy that strikes you heals you 2% of your health.",
            Augment::DivineStorm => "Judges every enemy within 8 yards of you instead, at 60%.",
            Augment::JudgementOfTheCrusader => "Judged enemies are Marked +10% for 6 seconds.",
            Augment::Exorcist => "+100% damage to bosses and elites.",
            Augment::ArtOfWar => "Your Holy crits have a 20% chance to make Exorcism ready.",
            Augment::BanishEvil => "Its target is Stunned for 2 seconds.",
            Augment::InfusionOfLight => "Its crits bank a Holy Light charge (even without Holy Light).",
            Augment::GlimmerOfLight => "Its target keeps a Glimmer for 10 seconds; each heal you get strikes every Glimmered enemy for 50% of the heal.",
            Augment::Daybreak => "Below 50% health it heals 15% and comes back twice as fast.",
            Augment::SacredGround => "Heroes standing in it heal 1% of their health a second.",
            Augment::HallowedGround => "While you stand in it you take 15% less damage, and enemies in it are Weakened.",
            Augment::ConsecratedChill => "Enemies in it are Chilled.",
            Augment::Wrathful => "+100% damage to bosses and elites.",
            Augment::DivinePurpose => "Each enemy it hits takes 0.2 seconds off your defensive button's cooldown.",
            Augment::StunningWrath => "It Stuns for 1 second.",
            Augment::FistOfJustice => "Your Holy crits take 1 second off its cooldown.",
            Augment::ShockwaveHammer => "Also strikes and stuns every enemy within 5 yards of its targets.",
            Augment::LastingJustice => "Its Stun lasts 1.5 seconds longer.",
            Augment::AvengersShield => "Its overflow is a shield that strikes 3 enemies and Stuns them for 1 second.",
            Augment::HolyRetribution => "Its retaliation strikes every enemy within 5 yards.",
            Augment::Reckoning => "Each block has a 20% chance to cast Judgement (yours, if you know it).",
            Augment::HolyPower => "Each heal it gives makes your next Holy damage spell deal +50%.",
            Augment::DivineFavor => "30% of its heals are doubled.",
            Augment::LightOfDawn => "Its overflow strikes every enemy in a 10-yard cone ahead of you.",
            Augment::AuraMastery => "Each blow it answers also heals the hero struck for 20% of the retaliation.",
            Augment::Devotion => "You and allies in reach also take 10% less damage.",
            Augment::Sanctity => "You and allies in reach deal +10% damage.",
            Augment::SealOfLight => "Each hit heals you 1% of your health.",
            Augment::SealOfRighteousness => "Each hit also strikes every enemy within 4 yards of its target for 40%.",
            Augment::SealOfWisdom => "Each hit takes 0.2 seconds off Holy Light's cooldown.",
            Augment::ImprovedLayOnHands => "For 10 seconds after it heals, that hero takes 30% less damage.",
            Augment::MartyrsHands => "It heals below 40% instead of 20%, and every enemy within 10 yards takes holy damage equal to the heal.",
            Augment::HandOfProtection => "After it heals, that hero takes no damage for 2 seconds.",
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
            Augment::BloodAndThunder | Augment::Rampage | Augment::CleavingSwings => "cleave",
            Augment::Overpower | Augment::HeroicThrow | Augment::ChampionsGuard => "heroic_strike",
            Augment::MortalWounds | Augment::SuddenDeath | Augment::ColossusSmash => {
                "mortal_strike"
            }
            Augment::Bladestorm | Augment::FuriousSpin | Augment::SecondWind => "whirlwind",
            Augment::Shockwave | Augment::Aftershock | Augment::Thunderstruck => "thunder_clap",
            Augment::GushingWound | Augment::BloodFrenzy | Augment::DeepWounds => "rend",
            Augment::ExecutionersChain | Augment::Massacre | Augment::VictoryRush => "execute",
            Augment::BloodCraze | Augment::UnbridledWrath | Augment::Bloodbath => "bloodthirst",
            Augment::Revenge | Augment::ShieldBash | Augment::LastStand => "shield_block",
            Augment::RallyingCry | Augment::CripplingPresence | Augment::WarCry => {
                "demoralizing_shout"
            }
            Augment::ChallengingShout | Augment::ShatteringRoar | Augment::Fearsome => {
                "intimidating_shout"
            }
            Augment::ExposeArmor | Augment::Devastate | Augment::SunderingBlows => "sunder_armor",
            Augment::CombatPotency | Augment::BladeDance | Augment::RelentlessStrikes => {
                "sinister_strike"
            }
            Augment::Ambush | Augment::Opportunity | Augment::SealFate => "backstab",
            Augment::RestlessBlades | Augment::ColdBlood | Augment::Ruthlessness => "eviscerate",
            Augment::VenomousWounds | Augment::CrimsonTempest | Augment::Bloodletter => "rupture",
            Augment::AdrenalineRush | Augment::FlowingSteel | Augment::SerratedBlades => {
                "blade_flurry"
            }
            Augment::PoisonedKnives | Augment::LethalFan | Augment::FanThePack => "fan_of_knives",
            Augment::Envenom | Augment::ToxicCloud | Augment::MindNumbingPoison => "deadly_poison",
            Augment::ImprovedGouge | Augment::DirtyTricks | Augment::GougedEyes => "gouge",
            Augment::MarkedForDeath | Augment::KidneyBurst | Augment::QuickRecovery => {
                "kidney_shot"
            }
            Augment::Adrenaline | Augment::RestlessTempo | Augment::ImprovedSlice => {
                "slice_and_dice"
            }
            Augment::Counterattack | Augment::Nimble | Augment::CheatDeath => "riposte",
            Augment::BlindingPowder | Augment::SmokeBomb | Augment::Sap => "blind",
            Augment::Atonement | Augment::DivineFury | Augment::SearingLight => "smite",
            Augment::Martyrdom | Augment::PurgeTheWicked | Augment::HolyFervor => "holy_fire",
            Augment::PsychicHorror | Augment::ShadowyApparitions | Augment::ShadowyInsight => {
                "mind_blast"
            }
            Augment::SpreadingPain | Augment::Misery | Augment::DeathsEcho => "shadow_word_pain",
            Augment::MindSear | Augment::PsychicLink | Augment::MindTrauma => "mind_flay",
            Augment::Plaguebearer | Augment::VampiricEmbrace | Augment::Insatiable => {
                "devouring_plague"
            }
            Augment::SearingNova | Augment::DivineAegis | Augment::Inspiration => "holy_nova",
            Augment::ShootingStars | Augment::Starfall | Augment::LunarGuidance => "starshards",
            Augment::HolyWordSanctify | Augment::BlessedRecovery | Augment::InnerFocus => "renew",
            Augment::BodyAndSoul | Augment::Rapture | Augment::BorrowedTime => "power_word_shield",
            Augment::Sanctuary | Augment::LightsWrath | Augment::SpringOfLight => "lightwell",
            Augment::TouchOfWeakness | Augment::HexOfDomination | Augment::SpreadingHex => {
                "hex_of_weakness"
            }
            Augment::Nightfall | Augment::ShadowBoltVolley | Augment::ImprovedShadowBolt => {
                "shadow_bolt"
            }
            Augment::SiphonLife | Augment::SeedOfCorruption | Augment::EverlastingAffliction => {
                "corruption"
            }
            Augment::CurseOfDoom | Augment::AmplifyCurse | Augment::CurseOfExhaustion => {
                "curse_of_agony"
            }
            Augment::FireAndBrimstone | Augment::Backdraft | Augment::Emberstorm => "immolate",
            Augment::FirestormRain | Augment::Aftermath | Augment::Pyroclasm => "rain_of_fire",
            Augment::InfernalPact | Augment::RitualOfFire | Augment::HellfireWard => "hellfire",
            Augment::Horrify | Augment::DeathsEmbrace | Augment::DarkBargain => "death_coil",
            Augment::SoulHarvest | Augment::DrainAura | Augment::ImprovedDrain => "drain_life",
            Augment::Nightmare | Augment::Shadowfury | Augment::DemonicPresence => "howl_of_terror",
            Augment::BurningImp | Augment::DemonicSacrifice | Augment::ImprovedImp => "summon_imp",
            Augment::Sacrifice | Augment::Torment | Augment::ConsumeShadows => "summon_voidwalker",
            Augment::ImprovedSearingPain | Augment::SoulFire | Augment::FelFlame => "searing_pain",
            Augment::ArcaneBarrage | Augment::TrickShot | Augment::LockAndLoad => "arcane_shot",
            Augment::SplinterArrows | Augment::SpreadShot | Augment::SerpentSpread => "multi_shot",
            Augment::CarefulAim | Augment::SniperTraining | Augment::PiercingShots => "aimed_shot",
            Augment::SpreadingVenom | Augment::NoxiousStings | Augment::ScorpidSting => {
                "serpent_sting"
            }
            Augment::ImprovedConcussion | Augment::DazeWave | Augment::FrostShot => {
                "concussive_shot"
            }
            Augment::Arrowstorm | Augment::ExplosiveVolley | Augment::HailOfArrows => "volley",
            Augment::Minefield | Augment::ScorchedEarth | Augment::BigGameTrap => "explosive_trap",
            Augment::FreezingTrap | Augment::FrozenWake | Augment::DeepFreeze => "frost_trap",
            Augment::BestialWrath | Augment::KillCommand | Augment::Intimidation => "call_pet",
            Augment::ImprovedScatter | Augment::WyvernSting | Augment::ScatterBurst => {
                "scatter_shot"
            }
            Augment::ImprovedMark | Augment::MarkedForTheKill | Augment::HuntersPrey => {
                "hunters_mark"
            }
            Augment::RapidKilling | Augment::TrueshotAura | Augment::QuickShots => "rapid_fire",
            Augment::LightningOverload | Augment::Stormkeeper | Augment::RollingThunder => {
                "lightning_bolt"
            }
            Augment::ChainReaction | Augment::StormNexus | Augment::Electrified => {
                "chain_lightning"
            }
            Augment::Fulmination | Augment::EarthenGrip | Augment::ShockMastery => "earth_shock",
            Augment::LavaBurst | Augment::VolcanicFury | Augment::SearingFlames => "flame_shock",
            Augment::FrozenPower | Augment::FrostNovaShock | Augment::Icefury => "frost_shock",
            Augment::SearingBurn | Augment::SearingPact | Augment::TwinFires => "searing_totem",
            Augment::FireNova | Augment::LiquidMagma | Augment::MoltenEarth => "magma_totem",
            Augment::Earthgrab | Augment::EarthquakeTotem | Augment::Stoneclaw => "earthbind_totem",
            Augment::HealingTide | Augment::Cloudburst | Augment::Stoneskin => {
                "healing_stream_totem"
            }
            Augment::Thunderstorm | Augment::StaticShield | Augment::StaticCharge => {
                "lightning_shield"
            }
            Augment::AncestralFortitude | Augment::NaturesSwiftness | Augment::TidalFocus => {
                "healing_wave"
            }
            Augment::Windfury | Augment::Stormlash | Augment::MaelstromWeapon => "stormstrike",
            Augment::HammerOfTheRighteous
            | Augment::WrathOfTheExecutioner
            | Augment::BlessedHammer => "hammer_of_wrath",
            Augment::JudgementOfLight | Augment::DivineStorm | Augment::JudgementOfTheCrusader => {
                "judgement"
            }
            Augment::Exorcist | Augment::ArtOfWar | Augment::BanishEvil => "exorcism",
            Augment::InfusionOfLight | Augment::GlimmerOfLight | Augment::Daybreak => "holy_shock",
            Augment::SacredGround | Augment::HallowedGround | Augment::ConsecratedChill => {
                "consecration"
            }
            Augment::Wrathful | Augment::DivinePurpose | Augment::StunningWrath => "holy_wrath",
            Augment::FistOfJustice | Augment::ShockwaveHammer | Augment::LastingJustice => {
                "hammer_of_justice"
            }
            Augment::AvengersShield | Augment::HolyRetribution | Augment::Reckoning => {
                "holy_shield"
            }
            Augment::HolyPower | Augment::DivineFavor | Augment::LightOfDawn => "holy_light",
            Augment::AuraMastery | Augment::Devotion | Augment::Sanctity => "retribution_aura",
            Augment::SealOfLight | Augment::SealOfRighteousness | Augment::SealOfWisdom => {
                "seal_of_command"
            }
            Augment::ImprovedLayOnHands | Augment::MartyrsHands | Augment::HandOfProtection => {
                "lay_on_hands"
            }
            _ => return None,
        })
    }

    /// The augment written `name`, ignoring case, spaces and punctuation ("Hot Streak",
    /// "hotstreak", "HotStreak"); for test loadouts.
    pub fn from_name(name: &str) -> Option<Augment> {
        let squash = |s: &str| {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        };
        let want = squash(name);
        Augment::GENERAL
            .into_iter()
            .chain(Augment::CAPSTONES)
            .find(|a| squash(a.name()) == want)
    }

    /// The three capstones of the spell `key`, in order; empty for a spell without any.
    pub fn capstones_of(key: &str) -> Vec<Augment> {
        Augment::CAPSTONES
            .into_iter()
            .filter(|a| a.capstone_of() == Some(key))
            .collect()
    }

    /// A cone's new shape: a spell takes at most one.
    pub fn is_cone_shape(self) -> bool {
        matches!(
            self,
            Augment::Reversed | Augment::FourWinds | Augment::FullCircle
        )
    }

    /// A conversion: it adds a school, and a spell takes at most one.
    pub fn is_conversion(self) -> bool {
        matches!(
            self,
            Augment::Frostfire
                | Augment::Spellfire
                | Augment::Spellfrost
                | Augment::Shadowflame
                | Augment::Twilight
                | Augment::SacredFlame
                | Augment::Thunderstrike
                | Augment::Balance
                | Augment::Frostbrand
                | Augment::Crusader
                | Augment::Shadowblade
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
            Augment::Reversed | Augment::FourWinds | Augment::FullCircle => {
                Requires::AnyOf(&[Cone])
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
            Augment::Shadowflame => Requires::AnyOf(&[Fire, Shadow]),
            Augment::Twilight => Requires::AnyOf(&[Holy, Shadow]),
            Augment::SacredFlame => Requires::AnyOf(&[Holy, Fire]),
            Augment::Thunderstrike => Requires::AnyOf(&[Physical, Nature]),
            Augment::Balance => Requires::AnyOf(&[Arcane, Nature]),
            Augment::Frostbrand => Requires::AnyOf(&[Frost, Nature]),
            Augment::Crusader => Requires::AnyOf(&[Physical, Holy]),
            Augment::Shadowblade => Requires::AnyOf(&[Physical, Shadow]),
            Augment::Lacerate => Requires::AnyOf(&[Bleed]),
            Augment::Hemorrhage => Requires::AnyOf(&[Bleed]),
            Augment::Crippling => Requires::AnyOf(&[Poison]),
            Augment::Virulent => Requires::AnyOf(&[Poison]),
            Augment::Dread => Requires::AnyOf(&[Fear]),
            Augment::Panic => Requires::AnyOf(&[Fear]),
            Augment::Bramble => Requires::AnyOf(&[Root]),
            Augment::Grasping => Requires::AnyOf(&[Root]),
            Augment::Crushing => Requires::AnyOf(&[Stun]),
            Augment::Forked => Requires::AnyOf(&[Chain]),
            Augment::Conductive => Requires::AnyOf(&[Chain]),
            Augment::Pack => Requires::AnyOf(&[Summon, Totem]),
            Augment::FeralBond => Requires::AnyOf(&[Summon, Totem]),
            Augment::Frenzy => Requires::AnyOf(&[Summon, Totem]),
            Augment::BoomingVoice => Requires::AnyOf(&[Shout]),
            Augment::Commanding => Requires::AnyOf(&[Shout]),
            Augment::DeepReserves => Requires::AnyOf(&[Ward]),
            Augment::Overflowing => Requires::AnyOf(&[Ward]),
            Augment::MendingLink => Requires::AnyOf(&[Heal]),
            Augment::Renewing => Requires::AnyOf(&[Heal]),
            Augment::Sanguine => Requires::AnyOf(&[Leech]),
            Augment::Bloodthirsty => Requires::AnyOf(&[Leech]),
            Augment::Sweeping => Requires::AnyOf(&[Melee]),
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
        if self.is_cone_shape() && taken.iter().any(|a| a.is_cone_shape()) {
            return false;
        }
        // A shield or a buff deals no damage itself: damage trades mean nothing to it.
        if (spec.tags.has(Tag::Shield) || spec.damage <= 0.0)
            && matches!(self, Augment::Heavy | Augment::Twin)
        {
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
            Augment::Reversed => s.cone = Cone::Behind,
            Augment::FourWinds => s.cone = Cone::Four,
            Augment::FullCircle => {
                s.targeting = Targeting::Around;
                s.radius *= 0.7;
                s.tags = s.tags.without(Tag::Cone);
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
                s.fx.fear = 0.0;
                s.fx.root = 0.0;
                s.tags = s
                    .tags
                    .without(Tag::Slow)
                    .without(Tag::Freeze)
                    .without(Tag::Stun)
                    .without(Tag::Fear)
                    .without(Tag::Root);
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
                s.tags = s.tags.with(Tag::Cone);
                s.damage *= 1.6;
                s.stun = s.stun.max(2.0);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::LivingWave => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::FrozenOrb => s.follows = false,
            // The other classes' general augments.
            Augment::Shadowflame => {
                if s.tags.has(Tag::Fire) {
                    s.fx.leech += 0.1;
                }
                if s.tags.has(Tag::Shadow) {
                    s.burn += 0.3;
                    s.tags = s.tags.with(Tag::DamageOverTime);
                }
                s.tags = s.tags.with(Tag::Fire).with(Tag::Shadow);
            }
            Augment::Twilight => {
                s.tags = s.tags.with(Tag::Holy).with(Tag::Shadow);
                s.fx.vs_elite += 0.2;
            }
            Augment::SacredFlame => {
                if s.tags.has(Tag::Holy) {
                    s.burn += 0.3;
                    s.tags = s.tags.with(Tag::DamageOverTime);
                }
                if s.tags.has(Tag::Fire) {
                    s.fx.leech += 0.1;
                }
                s.tags = s.tags.with(Tag::Holy).with(Tag::Fire);
            }
            Augment::Thunderstrike => {
                s.tags = s.tags.with(Tag::Physical).with(Tag::Nature);
                s.fx.arc += 0.5;
            }
            Augment::Balance => {
                s.tags = s.tags.with(Tag::Arcane).with(Tag::Nature);
                s.crit += 0.15;
            }
            Augment::Frostbrand => {
                if s.tags.has(Tag::Nature) && s.chill == 0.0 {
                    s.chill = crate::spell::CHILL_SECS;
                    s.tags = s.tags.with(Tag::Slow);
                }
                if s.tags.has(Tag::Frost) {
                    s.burn += 0.3;
                    s.tags = s.tags.with(Tag::DamageOverTime).with(Tag::Poison);
                }
                s.tags = s.tags.with(Tag::Frost).with(Tag::Nature);
            }
            Augment::Crusader => s.tags = s.tags.with(Tag::Physical).with(Tag::Holy),
            Augment::Shadowblade => {
                s.tags = s.tags.with(Tag::Physical).with(Tag::Shadow);
                s.fx.vs_held += 0.25;
                s.fx.vs_rooted += 0.25;
            }
            Augment::Lacerate => s.fx.stacks = s.fx.stacks.max(3),
            Augment::Crippling => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::Virulent => s.fx.stacks += 3,
            Augment::Dread => s.fx.vs_feared += 0.25,
            Augment::Panic => s.fx.fear += 1.5,
            Augment::Bramble => s.fx.vs_rooted += 0.25,
            Augment::Grasping => s.fx.root += 1.5,
            Augment::Crushing => s.fx.vs_stunned += 0.4,
            Augment::Forked => s.fx.jumps += 2,
            Augment::Conductive => s.fx.jump_keep = 1.0,
            Augment::Pack => {
                s.twin = true;
                s.damage *= 0.65;
            }
            Augment::FeralBond => s.fx.leech += 0.05,
            Augment::Frenzy => s.tick_rate *= 1.4,
            Augment::BoomingVoice => s.radius *= 1.5,
            Augment::Commanding => {
                s.fx.fear *= 1.5;
                s.fx.weaken *= 1.5;
                s.fx.mark_secs *= 1.5;
                s.fx.buff.secs *= 1.5;
            }
            Augment::DeepReserves => s.fx.bank += 2,
            Augment::Overflowing => s.fx.overflow = 2,
            Augment::MendingLink => s.fx.mending = 0.5,
            Augment::Renewing => s.fx.renewing = 0.3,
            Augment::Sanguine => s.fx.sanguine = 0.2,
            Augment::Bloodthirsty => s.fx.leech += 0.1,
            Augment::Sweeping => s.fx.sweep = 0.5,
            // Warrior capstones that change the cast's numbers.
            Augment::HeroicThrow => {
                s.range = s.range.max(20.0);
                s.tags = s.tags.with(Tag::Projectile);
            }
            Augment::MortalWounds => {
                s.fx.mark = s.fx.mark.max(0.2);
                s.fx.mark_secs = s.fx.mark_secs.max(6.0);
            }
            Augment::ColossusSmash => s.cooldown *= 1.5,
            Augment::Bladestorm => {
                s.ground = 4.0;
                s.follows = true;
                s.tick_rate *= 2.0;
                s.fx.ground_share = 0.4;
            }
            Augment::Shockwave => {
                s.targeting = Targeting::Ahead;
                s.radius = s.radius.max(12.0);
                s.damage *= 1.4;
                s.chill = 0.0;
                s.stun = s.stun.max(2.0);
                s.tags = s.tags.with(Tag::Cone).with(Tag::Stun).without(Tag::Slow);
            }
            Augment::Aftershock => {
                s.ground = 4.0;
                s.fx.ground_share = 0.25;
                s.fx.ground_control = true;
            }
            Augment::Thunderstruck => s.fx.weaken = s.fx.weaken.max(6.0),
            Augment::Massacre => s.fx.execute_at = s.fx.execute_at.max(0.5),
            Augment::Bloodbath => {
                s.burn += 1.0;
                s.fx.dot_time = 6.0;
                s.tags = s.tags.with(Tag::Bleed).with(Tag::DamageOverTime);
            }
            Augment::CripplingPresence => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::ChallengingShout => {
                s.fx.fear = 0.0;
                s.fx.pull = 15.0;
                s.tags = s.tags.without(Tag::Fear);
            }
            Augment::ShatteringRoar => s.fx.sunder += 2,
            Augment::Fearsome => s.fx.full_control = true,
            Augment::Devastate => {
                s.targeting = Targeting::Nearest;
                s.count = 99;
                s.range = 6.0;
                s.damage *= 0.6;
            }
            // Rogue capstones that change the cast's numbers.
            Augment::BladeDance => {
                s.fx.jumps = s.fx.jumps.max(4);
                s.tags = s.tags.with(Tag::Chain);
            }
            Augment::Opportunity => s.fx.vs_held += 1.0,
            Augment::SerratedBlades => {
                s.burn += 0.5;
                s.tags = s.tags.with(Tag::Bleed).with(Tag::DamageOverTime);
            }
            Augment::LethalFan => {
                s.radius = s.radius.max(20.0);
                s.damage *= 2.0;
                s.fx.only_dotted = true;
            }
            Augment::ImprovedGouge => s.stun += 1.0,
            Augment::DirtyTricks => {
                s.targeting = Targeting::ElitesFirst;
                s.fx.full_control = true;
            }
            Augment::MarkedForDeath => {
                s.fx.mark = s.fx.mark.max(0.25);
                s.fx.mark_secs = s.fx.mark_secs.max(4.0);
            }
            Augment::Adrenaline => s.fx.buff.haste += 0.2,
            Augment::ImprovedSlice => s.fx.buff.cd_rate = s.fx.buff.cd_rate.max(0.45),
            Augment::BlindingPowder => {
                s.chill = s.chill.max(s.fx.fear + 4.0);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::SmokeBomb => {
                s.fx.fear = 0.0;
                s.tags = s.tags.without(Tag::Fear);
            }
            // Priest capstones that change the cast's numbers.
            Augment::Martyrdom => s.damage *= 1.8,
            Augment::SpreadingPain => {
                s.fx.fresh = true;
                s.range = s.range.max(30.0);
            }
            Augment::Misery => {
                s.fx.mark = s.fx.mark.max(0.1);
                s.fx.mark_secs = s.fx.mark_secs.max(12.0);
            }
            Augment::MindTrauma => s.fx.weaken = s.fx.weaken.max(4.0),
            Augment::SearingNova => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::Starfall => {
                s.follows = true;
                s.fx.strikes = 3;
                s.range = s.range.max(20.0);
                s.ground = s.ground.max(6.0);
            }
            Augment::LunarGuidance => {
                s.fx.mark = s.fx.mark.max(0.1);
                s.fx.mark_secs = s.fx.mark_secs.max(4.0);
            }
            Augment::SpringOfLight => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::HexOfDomination => {
                s.fx.mark = s.fx.mark.max(0.3);
                s.fx.mark_secs = s.fx.mark_secs.max(s.fx.weaken);
                s.fx.weaken_cut = s.fx.weaken_cut.max(0.5);
            }
            // Warlock capstones that change the cast's numbers.
            Augment::ShadowBoltVolley => {
                s.targeting = Targeting::Nearest;
                s.count = s.count.max(5);
                s.damage *= 0.6;
            }
            Augment::SiphonLife => {
                s.fx.leech += 0.2;
                s.tags = s.tags.with(Tag::Leech);
            }
            Augment::AmplifyCurse => {
                s.damage *= 1.5;
                s.fx.weaken = s.fx.weaken.max(s.fx.dot_time);
            }
            Augment::CurseOfExhaustion => {
                s.chill = s.chill.max(s.fx.dot_time);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::FireAndBrimstone => s.radius = s.radius.max(5.0),
            Augment::Aftermath => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::InfernalPact => s.damage *= 2.0,
            Augment::RitualOfFire => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::DrainAura => {
                s.targeting = Targeting::Nearest;
                s.count = 99;
                s.range = 6.0;
            }
            Augment::Nightmare => {
                s.burn += 3.0;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::Shadowfury => {
                s.fx.fear = 0.0;
                s.stun = s.stun.max(2.0);
                s.damage *= 3.0;
                s.tags = s.tags.without(Tag::Fear).with(Tag::Stun);
            }
            Augment::DemonicPresence => {
                s.fx.mark = s.fx.mark.max(0.15);
                s.fx.mark_secs = s.fx.mark_secs.max(s.fx.fear);
            }
            Augment::BurningImp => {
                s.burn += 0.5;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::DemonicSacrifice => s.fx.skip = true,
            Augment::ImprovedImp => s.fx.strikes = s.fx.strikes.max(2),
            Augment::Torment => s.fx.weaken = s.fx.weaken.max(3.0),
            // Hunter capstones that change the cast's numbers.
            Augment::TrickShot => {
                s.fx.jumps = s.fx.jumps.max(2);
                s.tags = s.tags.with(Tag::Chain);
            }
            Augment::SplinterArrows => s.radius = s.radius.max(3.0),
            Augment::SpreadShot => {
                s.targeting = Targeting::Ahead;
                s.count = 99;
                s.range = s.range.min(20.0);
                s.damage *= 0.7;
                s.tags = s.tags.with(Tag::Cone);
            }
            Augment::PiercingShots => {
                s.burn += 0.5;
                s.fx.dot_time = 6.0;
                s.tags = s.tags.with(Tag::Bleed).with(Tag::DamageOverTime);
            }
            Augment::SpreadingVenom => {
                s.fx.fresh = true;
                s.range = s.range.max(30.0);
            }
            Augment::ScorpidSting => s.fx.weaken = s.fx.weaken.max(s.fx.dot_time),
            Augment::FrostShot => s.chill *= 2.0,
            Augment::ExplosiveVolley => {
                s.burn += 0.3;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::HailOfArrows => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::Minefield => s.damage *= 0.6,
            Augment::ScorchedEarth => {
                s.ground *= 2.0;
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::BigGameTrap => s.fx.vs_elite += 1.0,
            Augment::DeepFreeze => s.slow = s.slow.max(0.7),
            Augment::ImprovedScatter => {
                s.chill = s.chill.max(s.stun + 4.0);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::WyvernSting => {
                s.stun = s.stun.max(6.0);
                s.burn += 1.0;
                s.fx.dot_time = 6.0;
                s.tags = s.tags.with(Tag::Poison).with(Tag::DamageOverTime);
            }
            Augment::ScatterBurst => s.radius = s.radius.max(4.0),
            Augment::ImprovedMark => s.fx.mark = s.fx.mark.max(0.25),
            Augment::TrueshotAura => {
                s.cooldown = 5.0;
                s.fx.buff.secs = 6.0;
                s.fx.buff.cd_rate = 0.0;
                s.fx.buff.power = 0.1;
                s.tags = s.tags.with(Tag::Aura);
            }
            Augment::QuickShots => s.fx.buff.haste += 0.2,
            // Shaman capstones that change the cast's numbers.
            Augment::StormNexus => {
                s.fx.jumps = s.fx.jumps.max(60);
                s.fx.jump_reach = 3.0;
                s.fx.jump_keep = s.fx.jump_keep.max(0.95);
            }
            Augment::Electrified => {
                s.chill = s.chill.max(1.0);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::FrozenPower => {
                s.chill = 0.0;
                s.fx.root = s.fx.root.max(2.0);
                s.tags = s.tags.without(Tag::Slow).with(Tag::Root);
            }
            Augment::SearingBurn => {
                s.burn += 0.4;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::TwinFires => s.fx.strikes = s.fx.strikes.max(2),
            Augment::MoltenEarth => {
                s.burn += 0.3;
                s.tags = s.tags.with(Tag::DamageOverTime);
            }
            Augment::EarthquakeTotem => {
                s.chill = 0.0;
                s.damage *= 4.0;
                s.stun = s.stun.max(0.5);
                s.tags = s.tags.without(Tag::Slow).with(Tag::Stun);
            }
            Augment::Stoneclaw => s.fx.weaken = s.fx.weaken.max(1.5),
            Augment::StaticShield => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::TidalFocus => s.cooldown *= 0.7,
            Augment::Stormlash => s.fx.mark = s.fx.mark.max(0.3),
            // Paladin capstones that change the cast's numbers.
            Augment::HammerOfTheRighteous => s.radius = s.radius.max(3.0),
            Augment::WrathOfTheExecutioner => {
                s.targeting = Targeting::Weakest;
                s.execute += 2.0;
                s.fx.execute_at = 0.2;
            }
            Augment::DivineStorm => {
                s.targeting = Targeting::Nearest;
                s.count = 99;
                s.range = 8.0;
                s.damage *= 0.6;
            }
            Augment::JudgementOfTheCrusader => {
                s.fx.mark = s.fx.mark.max(0.1);
                s.fx.mark_secs = s.fx.mark_secs.max(6.0);
            }
            Augment::Exorcist | Augment::Wrathful => s.fx.vs_elite += 1.0,
            Augment::BanishEvil => {
                s.stun = s.stun.max(2.0);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::SacredGround => {
                s.fx.heal = s.fx.heal.max(0.01);
                s.tags = s.tags.with(Tag::Heal);
            }
            Augment::HallowedGround => s.fx.weaken = s.fx.weaken.max(1.5),
            Augment::ConsecratedChill => {
                s.chill = s.chill.max(crate::spell::CHILL_SECS);
                s.tags = s.tags.with(Tag::Slow);
            }
            Augment::StunningWrath => {
                s.stun = s.stun.max(1.0);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::ShockwaveHammer => s.radius = s.radius.max(5.0),
            Augment::LastingJustice => s.stun += 1.5,
            Augment::AvengersShield => {
                s.fx.overflow = s.fx.overflow.max(3);
                s.stun = s.stun.max(1.0);
                s.tags = s.tags.with(Tag::Stun);
            }
            Augment::Devotion => s.fx.buff.guard = s.fx.buff.guard.max(0.1),
            Augment::Sanctity => s.fx.buff.power += 0.1,
            Augment::MartyrsHands => s.fx.heal_below = s.fx.heal_below.max(0.4),
            _ => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
            cone: Cone::Ahead,
            missile_speed: 20.0,
            crit: 0.05,
            crit_mult: 2.0,
            chill: 0.0,
            slow: crate::spell::CHILL_SLOW,
            freeze: 0.0,
            freeze_chance: 0.0,
            permafrost: false,
            stun: 0.0,
            knockback: 0.0,
            burn: 0.0,
            dot_mult: 1.0,
            dot_speed: 1.0,
            dot_secs: 1.0,
            tick_rate: 1.0,
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
            fx: crate::spell::SpellFx::default(),
        }
    }

    #[test]
    fn three_capstones_for_each_spell_that_has_them() {
        let mut spells: Vec<&str> = Augment::CAPSTONES
            .iter()
            .filter_map(|a| a.capstone_of())
            .collect();
        assert_eq!(spells.len(), Augment::CAPSTONES.len());
        spells.dedup();
        assert_eq!(spells.len() * 3, Augment::CAPSTONES.len());
        for key in &spells {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
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
    fn a_cone_takes_one_new_shape() {
        let cone = spec(&[Tag::Frost, Tag::Area, Tag::Cone]);
        assert!(Augment::Reversed.fits(&cone, &[]));
        assert!(Augment::Wide.fits(&cone, &[]), "a cone is still an area");
        assert!(!Augment::FourWinds.fits(&cone, &[Augment::Reversed]));
        assert!(!Augment::Reversed.fits(&spec(&[Tag::Area]), &[]));
        let mut round = cone.clone();
        round.targeting = Targeting::Ahead;
        Augment::FullCircle.shape(&mut round);
        assert_eq!(round.targeting, Targeting::Around);
        assert!((round.radius - 2.1).abs() < 1e-4);
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
