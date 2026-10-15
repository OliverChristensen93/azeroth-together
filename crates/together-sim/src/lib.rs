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

mod augments;
mod cards;
mod content;
mod interact;
mod items;
mod math;
mod powerups;
mod rng;
mod spell;
mod stats;
mod world;

pub use augments::{milestone_rank, Augment, Milestone, Requires};
pub use cards::{ability_stats, fallback_gold, AbilityStats, Card, CardKind, Rarity, MAX_RANK};
pub use content::{
    AbilityDef, AbilityKind, ClassDef, Content, ContentError, ContentSources, EnemyDef, Rules,
    WeaponDef, ZoneDef,
};
pub use interact::{
    chest_price, escort_time, InteractState, Interactable, InteractableKind, Placement, ESCORT_GOAL,
};
pub use items::{ward_cooldown, Item};
pub use math::V2;
pub use powerups::{Pickup, PickupKind, PotLoot, PowerUp};
pub use rng::Rng;
pub use spell::{CastSpec, Tag, Tags, Targeting};
pub use stats::{Bane, Difficulty, Pact, Stat, Stats, PACT_BOON};
pub use world::{
    status_bits, xp_to_next, Enemy, EnemyState, Event, Gem, Hero, HeroPose, HeroSpells, Offer,
    Owned, PlayerId, PlayerInput, Snapshot, Source, Status, Target, World, WorldError, DT,
    ELITE_SCALE, MAX_ABILITIES, REVIVE_RANGE, REVIVE_SECS, TICK_HZ,
};
