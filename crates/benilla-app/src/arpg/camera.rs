//! The action camera: over the hero's right shoulder, steered by the mouse at all times (the
//! cursor is locked and hidden while playing, a crosshair in its place), the wheel bringing it
//! closer or farther. What is under the crosshair is the aim: the first enemy along the view, else
//! the ground, else a point far ahead. And the light.

use benilla_ui::script::UiScript;
use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::view::HeroBody;

/// Radians of turn per pixel of mouse motion.
const LOOK_RATE: f32 = 0.0032;
/// How far the view tilts: up to the sky a little, down to the feet.
const PITCH_MIN: f32 = -0.55;
const PITCH_MAX: f32 = 1.2;
/// Zoom range, yards from the shoulder.
const ZOOM_MIN: f32 = 2.5;
const ZOOM_MAX: f32 = 14.0;
/// The shoulder the camera looks over: up from the feet, and to the hero's right.
const SHOULDER_UP: f32 = 1.75;
const SHOULDER_RIGHT: f32 = 0.75;
/// How far the crosshair reaches.
const AIM_REACH: f32 = 45.0;
/// How tall an enemy is to the crosshair, from its feet.
const BODY_HEIGHT: f32 = 2.2;

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
    /// Bevy yaw: 0 looks along −Z (WoW's north), growing to the left.
    pub yaw: f32,
    /// Above the horizon, looking down.
    pub pitch: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        let dist = std::env::var("ARPG_CAM_DIST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(6.0);
        Self {
            dist,
            yaw: 0.0,
            pitch: 0.22,
        }
    }
}

impl CamRig {
    /// Where the camera looks, length 1.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, -sp, -cy * cp)
    }

    /// The view's forward and right in the ground plane, for camera-relative walking.
    pub fn ground_axes(&self) -> (Vec3, Vec3) {
        let (sy, cy) = self.yaw.sin_cos();
        let fwd = Vec3::new(-sy, 0.0, -cy);
        let right = Vec3::new(cy, 0.0, -sy);
        (fwd, right)
    }
}

/// The point under the crosshair: where every ability is aimed.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub Option<Vec3>);

/// The mouse turns the view while the cursor is held; the wheel zooms.
pub(super) fn steer(
    motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    menu: Res<super::ui::Menu>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut rig: ResMut<CamRig>,
) {
    let focused = windows.single().is_ok_and(|w| w.focused);
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    if menu.open || !focused {
        return;
    }
    rig.yaw -= motion.delta.x * LOOK_RATE;
    rig.pitch = (rig.pitch + motion.delta.y * LOOK_RATE).clamp(PITCH_MIN, PITCH_MAX);
}

/// The cursor is the game's while playing (locked, hidden) and the player's with the game menu up
/// or the window in the background.
pub(super) fn hold_cursor(
    menu: Res<super::ui::Menu>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    script: Option<NonSend<UiScript>>,
) {
    let Ok((window, mut opts)) = windows.single_mut() else {
        return;
    };
    // Held only once the interface is up, so the loading screen and a failed UI load keep a
    // cursor to close the window with.
    let held = !menu.open && window.focused && script.is_some();
    let (grab, visible) = if held {
        (CursorGrabMode::Locked, false)
    } else {
        (CursorGrabMode::None, true)
    };
    if opts.grab_mode != grab || opts.visible != visible {
        opts.grab_mode = grab;
        opts.visible = visible;
    }
}

/// The camera over the shoulder, kept above the ground; the see-through line from it to the
/// hero's chest; the terrain streaming around the hero.
pub(super) fn place_camera(
    rig: Res<CamRig>,
    world: WorldPoint,
    sim: Res<super::Sim>,
    hero: Query<&Transform, (With<HeroBody>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut player: ResMut<crate::player::Player>,
    mut see_through: ResMut<benilla_world::model_fade::SeeThrough>,
) {
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
    let (_, right) = rig.ground_axes();
    let pivot = focus + Vec3::Y * SHOULDER_UP + right * SHOULDER_RIGHT;
    let fwd = rig.forward();
    let mut eye = pivot - fwd * rig.dist;
    // Never under the ground: looking up from a slope would bury the lens.
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + 0.5);
    }
    let placed = Transform::from_translation(eye).looking_to(fwd, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
    let line = hero.single().ok().map(|t| (eye, t.translation + Vec3::Y * 1.2));
    if see_through.line != line {
        see_through.line = line;
    }
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// The crosshair's aim: along the view from the camera, the first enemy it passes through, else
/// where it meets the ground, else as far as it reaches. Marched a half yard at a time, which
/// no body or slope slips between.
pub(super) fn aim_crosshair(
    cams: Query<&Transform, With<WorldCamera>>,
    rig: Res<CamRig>,
    sim: Res<super::Sim>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let Ok(cam) = cams.single() else {
        return;
    };
    let origin = cam.translation;
    let dir = rig.forward();
    let enemies: Vec<(Vec3, f32)> = sim
        .world
        .enemies()
        .iter()
        .map(|e| {
            let def = &sim.world.content().enemies[e.kind];
            let mut feet = super::view::ground(e.pos, origin.y);
            if let Some(h) = world.terrain_height_under(feet + Vec3::Y * 50.0) {
                feet.y = h;
            }
            (feet, def.radius * def.scale.max(1.0) + 0.3)
        })
        .collect();
    // Start past the hero, so the crosshair never stops on the shoulder it looks over.
    let start = rig.dist;
    let mut t = start;
    let mut hit = None;
    while t <= start + AIM_REACH {
        let p = origin + dir * t;
        if let Some(&(feet, _)) = enemies.iter().find(|(feet, r)| {
            let flat = Vec2::new(p.x - feet.x, p.z - feet.z).length();
            flat <= *r && p.y >= feet.y && p.y <= feet.y + BODY_HEIGHT
        }) {
            hit = Some(Vec3::new(feet.x, feet.y, feet.z));
            break;
        }
        if world.terrain_height_under(p).is_some_and(|h| p.y <= h) {
            hit = Some(p);
            break;
        }
        t += 0.5;
    }
    let point = hit.unwrap_or(origin + dir * (start + AIM_REACH));
    if aim.0 != Some(point) {
        aim.0 = Some(point);
    }
}

/// Late afternoon over the field, whatever the clock says; `ARPG_HOUR` picks another.
pub(super) fn daylight(
    debug: Option<ResMut<benilla_world::dev_state::DebugState>>,
    mut minute: Local<Option<u32>>,
) {
    let Some(mut debug) = debug else { return };
    let minute = *minute.get_or_insert_with(|| {
        std::env::var("ARPG_HOUR")
            .ok()
            .and_then(|h| h.parse::<f32>().ok())
            .map_or(960, |h| (h.rem_euclid(24.0) * 60.0) as u32)
    });
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = minute;
    }
}
