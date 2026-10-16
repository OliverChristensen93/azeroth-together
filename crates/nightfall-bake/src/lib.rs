//! Nightfall's height map, baked from the player's own WoW data (spec §4.1): the terrain of every
//! tile under a run's bounds, its buildings' (WMO) walking collision and its doodads' collision
//! hulls (a tree's trunk, never its canopy), rasterized into `nightfall-sim`'s columns. Only the
//! host needs the map; it is baked once and kept in a cache keyed by the data files, the bounds
//! and the props, so nothing Blizzard-derived ever ships.

mod bake;
mod cache;
mod place;
mod raster;

pub use bake::{bake, buildings_near, BakeRequest, PropPlacement};
pub use cache::{cache_file, cache_key, load_or_bake, Baked, BAKE_VERSION};
pub use place::placement_point;
pub use raster::Footprint;
