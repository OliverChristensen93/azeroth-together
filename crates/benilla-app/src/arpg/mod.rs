//! Azeroth ARPG: a top-down action RPG on the benilla world, started by [`crate::run_together`]
//! when `ARPG=1` is set. Its game rules are `arpg-sim`; this module only reads the keys and the
//! mouse into a [`PlayerInput`], steps the world and draws it with the engine's unit, spell-visual
//! and combat-text systems. There is no server and no FrameXML: the player controller is off
//! ([`crate::run_mode::CaptureMode`]), the hero is driven from the sim, the camera looks down.
//!
//! Content is read from `ARPG_CONTENT`, else `content/arpg` in the project, else the copy compiled
//! in. `ARPG_CLASS` and `ARPG_ZONE` pick the class and zone.

mod camera;
mod hud;
mod input;
mod telegraph;
mod units;
mod view;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use arpg_sim::{Content, PlayerId, World, DT};
use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The zone's `Map.dbc` id, read before the world map plugin starts.
static START_MAP: AtomicU32 = AtomicU32::new(0);
static CONTENT: OnceLock<Content> = OnceLock::new();

/// The local player.
pub(super) const ME: PlayerId = 0;

pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Whether the launcher was asked for this game: `ARPG` set to anything but `0`.
pub(crate) fn wanted() -> bool {
    std::env::var("ARPG").is_ok_and(|v| v != "0")
}

pub(crate) fn start_map() -> u32 {
    START_MAP.load(Ordering::Relaxed)
}

fn class_key() -> String {
    std::env::var("ARPG_CLASS").unwrap_or_else(|_| "mage".into())
}

fn zone_key() -> String {
    std::env::var("ARPG_ZONE").unwrap_or_else(|_| "northshire".into())
}

/// The folder the RON files are read from, if one exists on disk.
fn content_dir(project_dir: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("ARPG_CONTENT").map(PathBuf::from),
        Some(Path::new(project_dir).join("content/arpg")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content/arpg"))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("zones.ron").is_file())
}

/// Read the content and remember the zone's map; called once by [`crate::run_together`]. Bad
/// content is reported and exits.
pub(crate) fn prepare(project_dir: &str) -> Result<(), String> {
    let dir = content_dir(project_dir);
    let content = match &dir {
        Some(d) => Content::from_dir(d),
        None => Content::from_ron(
            include_str!("../../../../content/arpg/abilities.ron"),
            include_str!("../../../../content/arpg/classes.ron"),
            include_str!("../../../../content/arpg/enemies.ron"),
            include_str!("../../../../content/arpg/zones.ron"),
        ),
    }
    .map_err(|e| e.to_string())?;
    let zone = content
        .zone(&zone_key())
        .ok_or_else(|| format!("ARPG_ZONE: no zone \"{}\" in zones.ron", zone_key()))?;
    if content.class(&class_key()).is_none() {
        return Err(format!(
            "ARPG_CLASS: no class \"{}\" in classes.ron",
            class_key()
        ));
    }
    match &dir {
        Some(d) => println!("arpg: content from {}", d.display()),
        None => println!("arpg: content compiled in"),
    }
    START_MAP.store(content.zones[zone].map, Ordering::Relaxed);
    let _ = CONTENT.set(content);
    ACTIVE.store(true, Ordering::Relaxed);
    Ok(())
}

/// The running world, and how far into the next tick the frame is.
#[derive(Resource)]
pub(super) struct Sim {
    pub world: World,
    /// Seconds of real time not yet stepped.
    acc: f32,
}

impl Sim {
    /// How far between the last two ticks to draw, 0..1.
    pub fn alpha(&self) -> f32 {
        (self.acc / DT).clamp(0.0, 1.0)
    }
}

/// The events of the ticks stepped this frame, for the view systems.
#[derive(Resource, Default)]
pub(super) struct Pending(pub Vec<arpg_sim::Event>);

/// Everything spawned for the run, despawned on a restart.
#[derive(Component)]
pub(super) struct RunEntity;

pub(crate) struct ArpgPlugin;

impl Plugin for ArpgPlugin {
    fn build(&self, app: &mut App) {
        let content = CONTENT.get().expect("arpg::prepare ran").clone();
        let seed = std::env::var("ARPG_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0x5eed);
        let mut world = World::new(content, &zone_key(), seed).expect("checked in prepare");
        world.add_hero(ME, &class_key()).expect("checked in prepare");
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML.
        app.insert_resource(crate::run_mode::CaptureMode)
            .insert_resource(Sim { world, acc: 0.0 })
            .init_resource::<Pending>()
            .init_resource::<camera::CamRig>()
            .init_resource::<camera::AimPoint>()
            .init_resource::<input::Held>()
            .init_resource::<view::Views>()
            .init_resource::<telegraph::Shown>()
            .add_systems(Startup, hud::setup)
            .add_systems(Update, camera::daylight)
            .add_systems(
                Update,
                (
                    camera::aim_cursor,
                    input::read_input,
                    step_sim,
                    view::sync,
                    view::apply_events,
                    view::place,
                    view::statuses,
                    camera::place_camera,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_systems(
                Update,
                (telegraph::show, hud::refresh, view::reap)
                    .chain()
                    .after(WorldStage::Input),
            );
    }
}

/// Steps the world in fixed ticks for the real time that passed; nothing moves while the loading
/// screen covers the world.
fn step_sim(
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut sim: ResMut<Sim>,
    mut pending: ResMut<Pending>,
    mut views: ResMut<view::Views>,
) {
    if loading.covering() {
        return;
    }
    sim.acc += time.delta_secs().min(0.25);
    let mut steps = 0;
    while sim.acc >= DT && steps < 8 {
        views.remember(&sim.world);
        sim.world.step();
        let events = sim.world.drain_events();
        pending.0.extend(events);
        sim.acc -= DT;
        steps += 1;
    }
    if steps == 8 {
        sim.acc = 0.0;
    }
}
