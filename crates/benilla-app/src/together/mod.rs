//! Azeroth Together: the co-op survivors mode, started by [`crate::run_together`].
//!
//! The game itself is [`together_sim::World`], a headless simulation that knows nothing of Bevy.
//! This module only feeds it the local player's keys, steps it at its fixed rate, and draws it:
//! every hero, enemy and gem in the world gets a WoW unit or object ([`view`]), and the events a
//! step emits become spell visuals, swings and numbers. A co-op client will draw a host's world
//! the same way, which is why nothing here decides anything about the game.
//!
//! Configuration, for now: `TOGETHER_CLASS` (default `mage`), `TOGETHER_ZONE` (default `elwynn`),
//! `TOGETHER_CONTENT` (a folder with the four RON files; default the repository's `content/`, and
//! the copies compiled in when that is missing).

mod hud;
mod units;
mod view;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use benilla_world::schedule::WorldStage;
use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use together_sim::{Content, ContentSources, PlayerId, PlayerInput, World, DT, V2};

/// Set by [`crate::run_together`] before the app builds.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The zone's `Map.dbc` id, read before the world map plugin starts.
static START_MAP: AtomicU32 = AtomicU32::new(0);
static CONTENT: OnceLock<Arc<Content>> = OnceLock::new();

pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub(crate) fn start_map() -> u32 {
    START_MAP.load(Ordering::Relaxed)
}

/// The player at this keyboard. Until co-op joins, always the host.
const LOCAL_PLAYER: PlayerId = 0;

fn class_key() -> String {
    std::env::var("TOGETHER_CLASS").unwrap_or_else(|_| "mage".into())
}

fn zone_key() -> String {
    std::env::var("TOGETHER_ZONE").unwrap_or_else(|_| "elwynn".into())
}

/// The folder the RON files are read from, if one exists on disk.
fn content_dir(project_dir: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("TOGETHER_CONTENT").map(PathBuf::from),
        Some(Path::new(project_dir).join("content")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content"))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("zones.ron").is_file())
}

/// Read the content (from disk when there, so it can be edited without a rebuild) and remember
/// the zone's map; called once by [`crate::run_together`]. Bad content is reported and exits.
pub(crate) fn prepare(project_dir: &str) -> Result<(), String> {
    let dir = content_dir(project_dir);
    let read = |name: &str, built_in: &'static str| -> Result<String, String> {
        match &dir {
            Some(d) => std::fs::read_to_string(d.join(name))
                .map_err(|e| format!("{}: {e}", d.join(name).display())),
            None => Ok(built_in.to_owned()),
        }
    };
    let classes = read(
        "classes.ron",
        include_str!("../../../../content/classes.ron"),
    )?;
    let abilities = read(
        "abilities.ron",
        include_str!("../../../../content/abilities.ron"),
    )?;
    let enemies = read(
        "enemies.ron",
        include_str!("../../../../content/enemies.ron"),
    )?;
    let zones = read("zones.ron", include_str!("../../../../content/zones.ron"))?;
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        abilities: &abilities,
        enemies: &enemies,
        zones: &zones,
    })
    .map_err(|e| e.to_string())?;
    let zone = content
        .zone(&zone_key())
        .ok_or_else(|| format!("TOGETHER_ZONE: no zone \"{}\" in zones.ron", zone_key()))?;
    if content.class(&class_key()).is_none() {
        return Err(format!(
            "TOGETHER_CLASS: no class \"{}\" in classes.ron",
            class_key()
        ));
    }
    match &dir {
        Some(d) => println!("together: content from {}", d.display()),
        None => println!("together: content compiled in"),
    }
    START_MAP.store(content.zones[zone].map, Ordering::Relaxed);
    let _ = CONTENT.set(Arc::new(content));
    ACTIVE.store(true, Ordering::Relaxed);
    Ok(())
}

/// The run being played, and the clock that steps it.
#[derive(Resource)]
pub(super) struct Sim {
    pub world: World,
    /// Real seconds not yet simulated; always under one [`DT`] after a frame's steps.
    acc: f32,
    /// Steps taken this frame, so the views know to roll their interpolation forward.
    pub stepped: bool,
}

impl Sim {
    fn new() -> Self {
        let content = CONTENT.get().expect("together::prepare ran").clone();
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut world = World::new(content, &zone_key(), seed).expect("zone checked in prepare");
        world
            .add_hero(LOCAL_PLAYER, &class_key())
            .expect("class checked in prepare");
        Self {
            world,
            acc: 0.0,
            stepped: false,
        }
    }

    /// How far between the last two steps the present lies, 0..1, for smooth movement.
    pub fn alpha(&self) -> f32 {
        (self.acc / DT).clamp(0.0, 1.0)
    }
}

/// Everything this mode spawned, despawned when a run restarts.
#[derive(Component)]
pub(super) struct TogetherEntity;

/// What the local player's keys say: the run direction, held, and a card pick and a jump, each
/// used by the next step.
#[derive(Resource, Default)]
struct LocalInput {
    move_dir: V2,
    pick: Option<u8>,
    jump: bool,
}

pub(crate) struct TogetherPlugin;

impl Plugin for TogetherPlugin {
    fn build(&self, app: &mut App) {
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML.
        app.insert_resource(crate::run_mode::CaptureMode)
            .insert_resource(Sim::new())
            .init_resource::<LocalInput>()
            .init_resource::<CamRig>()
            .init_resource::<view::Views>()
            .add_systems(Startup, hud::setup)
            .add_systems(Update, daylight)
            .add_systems(
                Update,
                (
                    read_keys,
                    step_sim,
                    view::apply_events,
                    view::sync,
                    view::place,
                    place_camera,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_systems(
                Update,
                (hud::refresh, restart).chain().after(WorldStage::Input),
            );
    }
}

/// Midday, whatever the clock says.
fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    const MINUTE_OF_DAY: u32 = 780;
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != MINUTE_OF_DAY {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = MINUTE_OF_DAY;
    }
}

/// `TOGETHER_AUTOPLAY=1`: the local hero walks a slow circle and takes the first card, so a test
/// run shows play without anyone at the keys.
fn autoplay() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TOGETHER_AUTOPLAY").is_some())
}

/// WASD (or the arrows) to run, Space to jump, 1/2/3 to pick a card. W runs away from the camera:
/// north while it is locked, wherever it looks once `C` frees it.
fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    rig: Res<CamRig>,
    mut input: ResMut<LocalInput>,
) {
    if autoplay() {
        let a = sim.world.elapsed * 0.35;
        input.move_dir = V2::new(a.cos(), a.sin());
        // Hop now and then, so test shots catch a jump.
        input.jump = (sim.world.elapsed % 4.0) < 0.1;
        if sim
            .world
            .hero(LOCAL_PLAYER)
            .is_some_and(|h| !h.offers.is_empty())
        {
            input.pick = Some(0);
        }
        return;
    }
    let mut dir = V2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.x += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.y -= 1.0;
    }
    input.move_dir = rig.relative(dir);
    if keys.just_pressed(KeyCode::Space) {
        input.jump = true;
    }
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            input.pick = Some(i as u8);
        }
    }
}

/// Run the simulation's fixed steps for the real time that passed. It waits while the loading
/// screen is up and until the hero has a body, so a run never starts unseen.
fn step_sim(
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    views: Res<view::Views>,
    mut sim: ResMut<Sim>,
    mut input: ResMut<LocalInput>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    sim.stepped = false;
    if loading.covering() || !views.heroes.contains_key(&LOCAL_PLAYER) {
        return;
    }
    // A long hitch would otherwise replay seconds of play in one frame.
    sim.acc = (sim.acc + time.delta_secs()).min(0.25);
    while sim.acc >= DT {
        sim.acc -= DT;
        // A key press picks (or jumps) once: the first step takes it.
        let held = PlayerInput {
            move_dir: input.move_dir,
            pick: input.pick.take(),
            jump: std::mem::take(&mut input.jump),
        };
        sim.world.set_input(LOCAL_PLAYER, held);
        sim.world.step();
        sim.stepped = true;
    }
    // Cards up (or the run over): the world holds still, animations included.
    let hold = sim.world.paused() || sim.world.game_over();
    if hold && !virtual_time.is_paused() {
        virtual_time.pause();
    } else if !hold && virtual_time.is_paused() {
        virtual_time.unpause();
    }
}

/// Enter after the run ends: a fresh run on the same zone.
fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut sim: ResMut<Sim>,
    mut views: ResMut<view::Views>,
    spawned: Query<Entity, With<TogetherEntity>>,
) {
    if !sim.world.game_over() || !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    for e in &spawned {
        commands.entity(e).despawn();
    }
    *views = view::Views::default();
    *sim = Sim::new();
}

/// The camera over the local hero. Locked (the default) it looks down from the south, like the
/// survivors games; `C` frees it to swing around the hero and tilt as in WoW, by dragging with
/// either mouse button. The wheel zooms in both.
#[derive(Resource)]
struct CamRig {
    dist: f32,
    free: bool,
    /// Around the hero, radians; 0 is south of it, looking north.
    yaw: f32,
    /// Above the horizon, radians.
    pitch: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        // `TOGETHER_CAM_DIST=<yards>`: the starting zoom, for close-up test shots.
        let dist = std::env::var("TOGETHER_CAM_DIST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30.0);
        Self {
            dist,
            free: false,
            yaw: 0.0,
            pitch: LOCKED_PITCH,
        }
    }
}

impl CamRig {
    /// A screen-relative run direction (x ahead, y left) as a direction in the world: the same
    /// while locked, turned with the camera while free.
    fn relative(&self, dir: V2) -> V2 {
        if !self.free {
            return dir;
        }
        // The camera's yaw turns the view anticlockwise seen from above, as the world's x→y does.
        let (s, c) = self.yaw.sin_cos();
        V2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c)
    }
}

/// The locked view's tilt above the horizon.
const LOCKED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// The free view's tilt range: from just above the ground to nearly straight down.
const PITCH_RANGE: (f32, f32) = (0.18, 1.45);
/// Radians of swing per pixel of mouse drag.
const ORBIT_RATE: f32 = 0.008;

fn place_camera(
    mut rig: ResMut<CamRig>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    views: Res<view::Views>,
    sim: Res<Sim>,
    world: WorldPoint,
    tfs: Query<&Transform, Without<WorldCamera>>,
    mut player: ResMut<crate::player::Player>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(3.0, 70.0);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        rig.free = !rig.free;
        if !rig.free {
            rig.yaw = 0.0;
            rig.pitch = LOCKED_PITCH;
        }
    }
    if rig.free && mouse.any_pressed([MouseButton::Left, MouseButton::Right]) {
        rig.yaw -= motion.delta.x * ORBIT_RATE;
        rig.pitch = (rig.pitch + motion.delta.y * ORBIT_RATE).clamp(PITCH_RANGE.0, PITCH_RANGE.1);
    }
    // The ground under the hero (before it has a body, the arena's centre): not the hero itself,
    // or a jump would lift the whole view with it and never show.
    let mut focus = views
        .heroes
        .get(&LOCAL_PLAYER)
        .and_then(|&e| tfs.get(e).ok())
        .map(|t| t.translation)
        .unwrap_or_else(|| units::wow_to_bevy(sim.world.zone().center));
    if let Some(h) = world.terrain_height_under(focus) {
        focus.y = h;
    }
    // The terrain streams, and the loading screen waits, on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;

    let look = focus + Vec3::Y * 1.4;
    let (sp, cp) = rig.pitch.sin_cos();
    let (sy, cy) = rig.yaw.sin_cos();
    let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + 0.6);
    }
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}
