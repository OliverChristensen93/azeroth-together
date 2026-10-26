//! The top-down camera over the hero, the ground point under the cursor, and the light.
//! The camera and the cursor ray follow the survivors mode's (`survivors/hero.rs`).

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::view::HeroBody;

/// The view's tilt above the horizon.
const PITCH: f32 = 60.0 * std::f32::consts::PI / 180.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 12.0;
const ZOOM_MAX: f32 = 55.0;

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        let dist = std::env::var("ARPG_CAM_DIST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(28.0);
        Self { dist }
    }
}

/// The ground under the mouse cursor, `None` with the cursor off the window: where every ability
/// is aimed.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub Option<Vec3>);

/// Cast the cursor onto the terrain: the camera ray meets the plane at the hero's height, then the
/// terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<HeroBody>, Without<WorldCamera>)>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let point = (|| {
        let cursor = windows.single().ok()?.cursor_position()?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    })();
    if aim.0 != point {
        aim.0 = point;
    }
}

/// Looks down on the hero from the south at a fixed tilt, so up on the screen is north and WASD
/// lines up with it; the wheel zooms. Also keeps the terrain streaming around the hero, and the
/// see-through line from the eye to the hero's chest, along which trees fade.
pub(super) fn place_camera(
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    world: WorldPoint,
    sim: Res<super::Sim>,
    hero: Query<&Transform, (With<HeroBody>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut player: ResMut<crate::player::Player>,
    mut see_through: ResMut<benilla_world::model_fade::SeeThrough>,
) {
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    let focus = match hero.single() {
        Ok(t) => t.translation,
        Err(_) => {
            let s = sim.world.start();
            let (_, _, z) = sim.world.content().zones[sim.world.zone()].start;
            let mut p = super::view::ground(s, z);
            if let Some(h) = world.terrain_height_under(p) {
                p.y = h;
            }
            p
        }
    };
    let look = focus + Vec3::Y * 1.2;
    let (sp, cp) = PITCH.sin_cos();
    // Bevy's +Z is WoW's south: the eye stands south of the hero, looking north.
    let mut eye = look + Vec3::new(0.0, sp, cp) * rig.dist;
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + 0.6);
    }
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
    let line = hero.single().ok().map(|t| (eye, t.translation + Vec3::Y * 1.2));
    if see_through.line != line {
        see_through.line = line;
    }
    // The terrain streams and the loading screen waits on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// Late afternoon over the field, whatever the clock says; `ARPG_HOUR` picks another.
pub(super) fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let Some(mut debug) = debug else { return };
    let minute = std::env::var("ARPG_HOUR")
        .ok()
        .and_then(|h| h.parse::<f32>().ok())
        .map_or(960, |h| (h.rem_euclid(24.0) * 60.0) as u32);
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = minute;
    }
}
