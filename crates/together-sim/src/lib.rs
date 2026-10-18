//! Azeroth Together's game rules: heroes, enemies, abilities, experience and level-up cards, as a
//! plain-Rust simulation stepped at a fixed [`TICK_HZ`].
//!
//! Nothing here knows about Bevy, rendering or WoW's file formats. The world is a flat plane in
//! WoW's own `(x, y)` yards; the client puts every unit on the terrain and turns the [`Event`]s a
//! step emits into spell visuals, swings and numbers. That split is what lets one machine (the
//! co-op host) run the real game while every player's client only draws it.
//!
//! Every hero belongs to a [`PlayerId`] from the start, so a second player is one more
//! [`World::add_hero`], not a rewrite.

mod armory;
mod augments;
mod cards;
mod content;
mod gear;
mod interact;
mod items;
mod math;
pub mod obstacles;
mod mechanics;
mod powerups;
mod rng;
mod spell;
mod stats;
mod talents;
mod world;

pub use armory::{Armory, Locker, STASH_MAX};
pub use augments::{milestone_rank, Augment, Milestone, Requires};
pub use cards::{
    ability_stats, boost_lines, fallback_gold, AbilityStats, Boost, Card, CardKind, Rarity,
    SpellMods, MAX_RANK,
};
pub use content::{
    AbilityDef, AbilityKind, ClassDef, Content, ContentError, ContentSources, EnemyDef, RaceDef,
    Racial, Rules,
    WardKind, WeaponDef, ZoneDef,
};
pub use gear::{
    describe_line, item_level, level_share, loadout, Armor, GearPiece, HeroGear, Slot, Unique,
    WeaponKind, Worn, BOSS_DROP, ELITE_DROP, FULL_SET_TOMES, MAX_ITEM_LEVEL,
};
pub use interact::{
    chest_price, escort_time, InteractState, Interactable, InteractableKind, Placement, ESCORT_GOAL,
};
pub use items::{ward_cooldown, Item};
pub use math::V2;
pub use obstacles::Obstacles;
pub use mechanics::{Affix, Mechanic, Shape};
pub use powerups::{Pickup, PickupKind, PotLoot, PowerUp};
pub use rng::Rng;
pub use spell::{
    describe, BuffFx, CastSpec, Cone, SpellFx, Tag, Tags, Targeting, BASE_CRIT, BURN_SECS,
    PLACEHOLDERS,
};
pub use stats::{Bane, Difficulty, Pact, Stat, Stats, PACT_BOON};
pub use talents::{
    renown_for_run, ClassTalents, HeroTalents, Keystone, NodeKind, PointRules, SpellBonus,
    TalentBonuses, TalentEffect, TalentNode, TalentProfile, TalentWeb, LOADOUT_SLOTS,
};
pub use world::{
    shield_size, status_bits, xp_to_next, ActiveBuff, Bank, Boon, Enemy, EnemyState, Event, Gem,
    Hero, HeroFx, HeroPose, HeroSpells, Offer, Owned, PlayerId, PlayerInput, Snapshot, Source,
    Status, StatusFx, Target, World, WorldError, BOSS_EVERY, CHAMPION_SCALE, DT, ELITE_SCALE,
    MAX_ABILITIES, REVIVE_RANGE,
    REVIVE_SECS, TICK_HZ,
};
