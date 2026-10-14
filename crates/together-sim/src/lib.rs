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

mod content;
mod math;
mod rng;
mod world;

pub use content::{
    AbilityDef, AbilityKind, ClassDef, Content, ContentError, ContentSources, EnemyDef, WeaponDef,
    ZoneDef,
};
pub use math::V2;
pub use rng::Rng;
pub use world::{
    xp_to_next, Card, Enemy, Event, Gem, Hero, HeroPose, Offer, Owned, PlayerId, PlayerInput,
    Target, World, WorldError, DT, TICK_HZ,
};
