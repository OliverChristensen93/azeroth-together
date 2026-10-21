//! The floor under a spot, as WoW's character controller stands on it: the walk colliders
//! (terrain, building floors and stairs) cast straight down from near a height, nearest face
//! first. Outdoors that is the terrain; inside a building it is the floor a unit is on, not the
//! terrain heightfield above or below it (Ironforge sits under its mountain, with the valley
//! floor beneath).
//!
//! Each spot is asked twice: first near the local hero's level ([`HeroLevel`]), so enemies,
//! pickups and spell marks come to the floor the hero fights on, then near the unit's own height,
//! so one on stairs above the hero keeps them. A miss on both (colliders not loaded yet, or the
//! spot far above both) takes the terrain height.

use bevy::prelude::*;

use benilla_world::collision::WorldCollision;
use benilla_world::world_point::WorldPoint;
use together_sim::V2;

use super::units;

/// How far above the height asked a cast starts (yd): one step up a stair or slope per frame.
const UP: f32 = 4.0;
/// How far below it a cast reaches (yd).
const DOWN: f32 = 60.0;

/// The local hero's height (Bevy `y`) this frame, written where the controller's pose is read.
#[derive(Resource, Default)]
pub(super) struct HeroLevel(pub Option<f32>);

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Floor<'w, 's> {
    collision: WorldCollision<'w, 's>,
    pub point: WorldPoint<'w, 's>,
    level: Res<'w, HeroLevel>,
}

/// The walk-collider floor under `p`, cast from just above `near_y` (Bevy `y`).
pub(super) fn near(collision: &WorldCollision, p: V2, near_y: f32) -> Option<f32> {
    let origin = units::ground_to_bevy(p, near_y + UP);
    collision.ray_body(origin, Dir3::NEG_Y, UP + DOWN).map(|hit| origin.y - hit.distance)
}

impl Floor<'_, '_> {
    fn near(&self, p: V2, near_y: f32) -> Option<f32> {
        near(&self.collision, p, near_y)
    }

    /// The ground under `p` for a body last at `own_y` (`None` for a new one): near the hero's
    /// level, else near its own, else the terrain, else `fallback_y`.
    pub fn under(&self, p: V2, own_y: Option<f32>, fallback_y: f32) -> Vec3 {
        let y = self
            .level
            .0
            .and_then(|hero| self.near(p, hero))
            .or_else(|| own_y.and_then(|own| self.near(p, own)))
            .or_else(|| self.point.terrain_height_under(units::ground_to_bevy(p, fallback_y)))
            .unwrap_or(own_y.unwrap_or(fallback_y));
        units::ground_to_bevy(p, y)
    }
}