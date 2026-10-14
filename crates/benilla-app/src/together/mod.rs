//! Azeroth Together: the co-op survivors mode, started by [`crate::run_together`].
//!
//! The game itself is [`together_sim::World`], a headless simulation that knows nothing of Bevy.
//! This module steps it at its fixed rate and draws it: every hero, enemy and gem gets a WoW unit
//! or object ([`view`]), and the events a step emits become spell visuals, swings and numbers.
//!
//! The rest is the real WoW 1.12 client: its interface (player and target frames, minimap, buffs,
//! key bindings) and its character controller (mouselook, strafing, jumping, the camera). The
//! controller moves the local hero, and [`feed_pose`] reports where it went, the way a WoW client
//! reports its moves to the server. A co-op client will draw a host's world the same way, which is
//! why nothing here decides anything about the game.
//!
//! Configuration, for now: `TOGETHER_CLASS` (default `mage`), `TOGETHER_ZONE` (default `elwynn`),
//! `TOGETHER_NAME` (the hero's name), `TOGETHER_CONTENT` (a folder with the four RON files; default
//! the repository's `content/`, and the copies compiled in when that is missing).

mod dress;
mod hud;
mod ui;
mod units;
mod view;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use together_sim::{Content, ContentSources, HeroPose, PlayerId, PlayerInput, World, DT, V2};

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

/// What the local player sends the next step: the hero's pose as the controller left it, and a
/// card pick, used once.
#[derive(Resource, Default)]
struct LocalInput {
    pose: Option<HeroPose>,
    pick: Option<u8>,
}

pub(crate) struct TogetherPlugin;

impl Plugin for TogetherPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Sim::new())
            .init_resource::<LocalInput>()
            .init_resource::<view::Views>()
            .add_systems(Startup, hud::setup)
            .add_systems(Update, daylight)
            .add_systems(
                Update,
                (
                    read_keys,
                    feed_pose,
                    step_sim,
                    view::apply_events,
                    view::sync,
                    view::write_vitals,
                    view::place,
                )
                    .chain()
                    .in_set(WorldStage::Input)
                    // The controller has moved the hero this frame before its pose is read.
                    .after(crate::player::PlayerControlSet),
            )
            .add_systems(
                Update,
                (hud::refresh, restart, ui::tune_ui)
                    .chain()
                    .after(WorldStage::Input),
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

/// `TOGETHER_AUTOPLAY=1`: the local hero takes the first card, so a test run plays on without
/// anyone at the keys.
fn autoplay() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TOGETHER_AUTOPLAY").is_some())
}

/// 1, 2 or 3 picks a card while the cards are up. Everything else (movement, jumping, the camera,
/// the panels) is WoW's own key bindings.
fn read_keys(keys: Res<ButtonInput<KeyCode>>, sim: Res<Sim>, mut input: ResMut<LocalInput>) {
    let choosing = sim
        .world
        .hero(LOCAL_PLAYER)
        .is_some_and(|h| !h.offers.is_empty());
    if !choosing {
        return;
    }
    if autoplay() {
        input.pick = Some(0);
        return;
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

/// Bevy's position to WoW's ground plane.
fn bevy_to_ground(p: Vec3) -> V2 {
    V2::new(-p.z, -p.x)
}

/// Where WoW's controller left the local hero, as the pose the next step takes. The arena's edge
/// is a wall: a hero past the leash is put back on it, so nobody runs off the end of the loaded
/// world. Until the controller has the hero, the terrain streams around the arena's centre.
fn feed_pose(
    views: Res<view::Views>,
    sim: Res<Sim>,
    mut player: ResMut<crate::player::Player>,
    mut input: ResMut<LocalInput>,
    mut bodies: Query<&mut Transform>,
) {
    let center = sim.world.center();
    let Some(mut tf) = views
        .heroes
        .get(&LOCAL_PLAYER)
        .and_then(|&e| bodies.get_mut(e).ok())
        .filter(|_| player.active)
    else {
        if !player.active {
            player.pos = units::wow_to_bevy(sim.world.zone().center);
        }
        input.pose = None;
        return;
    };
    let mut pos = bevy_to_ground(tf.translation);
    let leash = sim.world.zone().leash;
    let off = pos - center;
    if off.length() > leash {
        pos = center + off.normalize_or_zero() * leash;
        let back = units::ground_to_bevy(pos, tf.translation.y);
        tf.translation.x = back.x;
        tf.translation.z = back.z;
        player.pos.x = back.x;
        player.pos.z = back.z;
    }
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    input.pose = Some(HeroPose {
        pos,
        facing: V2::new(yaw.cos(), yaw.sin()),
    });
}

/// Run the simulation's fixed steps for the real time that passed. It waits while the loading
/// screen is up and until the controller has the hero, so a run never starts unseen.
fn step_sim(
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut sim: ResMut<Sim>,
    mut input: ResMut<LocalInput>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    sim.stepped = false;
    let Some(pose) = input.pose else {
        return;
    };
    if loading.covering() {
        return;
    }
    // A long hitch would otherwise replay seconds of play in one frame.
    sim.acc = (sim.acc + time.delta_secs()).min(0.25);
    while sim.acc >= DT {
        sim.acc -= DT;
        // A key press picks once: the first step takes it.
        let held = PlayerInput {
            pick: input.pick.take(),
            pose: Some(pose),
            ..Default::default()
        };
        sim.world.set_input(LOCAL_PLAYER, held);
        sim.world.step();
        sim.stepped = true;
    }
    // Cards up (or the run over): the world holds still, animations and the hero included.
    let hold = sim.world.paused() || sim.world.game_over();
    if hold && !virtual_time.is_paused() {
        virtual_time.pause();
    } else if !hold && virtual_time.is_paused() {
        virtual_time.unpause();
    }
}

/// Enter after the run ends: a fresh run on the same zone, the controller taking the new hero.
fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut sim: ResMut<Sim>,
    mut views: ResMut<view::Views>,
    mut index: ResMut<crate::net::GuidIndex>,
    mut player: ResMut<crate::player::Player>,
    spawned: Query<Entity, With<TogetherEntity>>,
) {
    if !sim.world.game_over() || !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    for e in &spawned {
        commands.entity(e).despawn();
    }
    let gone: std::collections::HashSet<Entity> = spawned.iter().collect();
    index.0.retain(|_, e| !gone.contains(e));
    *views = view::Views::default();
    *sim = Sim::new();
    player.active = false;
}
