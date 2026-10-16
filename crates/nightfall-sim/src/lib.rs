//! Nightfall's game rules: heroes in Duskwood at night, as a plain-Rust simulation stepped at a
//! fixed [`TICK_HZ`].
//!
//! Nothing here knows about Bevy, rendering, WoW's file formats or the survivors games
//! (`together-sim`). The world is WoW's own `(x, y)` ground plane in yards; the client puts every
//! unit on the terrain. That split lets one machine (the co-op host) run the game while every
//! player's client only draws it. Sub-project 1 has heroes, their poses and the map's edge;
//! sub-project 2 adds the baked height map (walls, floors, sight and paths) and enemies that walk it; senses,
//! combat and loot come in later sub-projects (`docs/superpowers/specs/2026-10-16-nightfall-design.md`).

mod bounds;
mod content;
mod enemies;
mod heightmap;
mod math;
pub mod messages;
mod sight;
mod world;

pub use bounds::Bounds;
pub use content::{
    Atmosphere, Behaviour, ClassDef, Content, ContentError, ContentSources, EnemyDef, HeroLight,
    MapDef, Rules, SpawnDef, WeaponDef, WorldRules,
};
pub use heightmap::{
    Floors, Grid, HeightMap, HeightMapBuilder, Navigator, PathPlan, Span, SpanList, CELL, HEADROOM,
    MAX_FLOORS, MAX_STEP, MERGE_GAP, TERRAIN_DEPTH,
};
pub use enemies::{Enemy, PATROL_BUDGET, STEP_FLOORS, WANDER_BUDGET};
pub use math::{V2, V3};
pub use sight::{CHEST_HEIGHT, EYE_HEIGHT};
pub use world::{Hero, HeroPose, PlayerId, PlayerInput, Snapshot, World, WorldError, DT, TICK_HZ};
