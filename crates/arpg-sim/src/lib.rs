//! Azeroth ARPG's game rules: a top-down action RPG over WoW's world, as a plain-Rust simulation
//! stepped at a fixed [`TICK_HZ`].
//!
//! Nothing casts by itself and nothing needs a target. A player's input is where they walk, the
//! ground point under their cursor and the skill they hold down; every ability is a shape aimed at
//! that point (a missile that really flies, an area at the cursor, a cone, a nova, a blink).
//! Enemies telegraph their blows, so stepping out of them is the defence.
//!
//! Nothing here knows about Bevy, rendering or WoW's file formats. The world is a flat plane in
//! WoW's own `(x, y)` yards; the client puts every unit on the terrain and turns the [`Event`]s a
//! step emits into spell visuals, swings and numbers. Every hero belongs to a [`PlayerId`], so
//! co-op is one more [`World::add_hero`].

mod content;
mod math;
mod rng;
mod world;

pub use content::{
    AbilityDef, Behaviour, ClassDef, Content, ContentError, Effects, EnemyDef, School, Shape,
    SpawnEntry, ZoneDef,
};
pub use math::V2;
pub use rng::Rng;
pub use world::{
    Action, Area, Enemy, EnemyState, Event, Hero, Owner, PlayerId, PlayerInput, Projectile,
    Target, Telegraph, World, WorldError, CHILL_SLOW, CRIT_MULT, DT, HERO_RADIUS, TICK_HZ,
};
