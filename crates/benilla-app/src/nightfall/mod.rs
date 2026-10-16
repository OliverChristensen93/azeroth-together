//! Nightfall: the co-op stealth extraction game, picked on the launcher's first screen
//! ([`crate::game_pick`]). Its rules are [`nightfall_sim::World`], a headless simulation; this
//! module steps it (alone or hosting) or mirrors a host's (joined) and draws it with the real WoW
//! client: its interface, its character controller, Duskwood's terrain. It decides nothing about
//! the game.
//!
//! It shares no code with the survivors games (`crate::together`): what both need (units, dress,
//! sessions) is copied here, so the two can change apart. Configuration: `NIGHTFALL_CLASS`
//! (`rogue` or `mage`), `NIGHTFALL_NAME`, `NIGHTFALL_CONTENT` (a folder with the RON files; default
//! the repository's `content/nightfall/`, then the copies compiled in).

mod atmosphere;
mod dress;
mod grade;
mod menu;
mod run;
mod session;
mod ui;
mod units;
mod view;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use nightfall_sim::{Content, ContentSources};

use crate::game_pick::GameMode;

static CONTENT: OnceLock<Arc<Content>> = OnceLock::new();
/// The content files' text (classes, map, rules), for the hash a client and host compare.
static CONTENT_TEXTS: OnceLock<(String, String, String)> = OnceLock::new();

pub(super) fn content_texts() -> &'static (String, String, String) {
    CONTENT_TEXTS.get().expect("nightfall::prepare ran")
}

/// The game's content, as [`prepare`] loaded it.
pub(super) fn content() -> Arc<Content> {
    CONTENT.get().expect("nightfall::prepare ran").clone()
}

/// The class a launch starts on: `NIGHTFALL_CLASS`, else the first.
fn class_key() -> Option<String> {
    std::env::var("NIGHTFALL_CLASS").ok()
}

/// The folder the RON files are read from, if one exists on disk.
fn content_dir(project_dir: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("NIGHTFALL_CONTENT").map(PathBuf::from),
        Some(Path::new(project_dir).join("content").join("nightfall")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content").join("nightfall"))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("map.ron").is_file())
}

/// Read the content (from disk when there, so it can be edited without a rebuild); called once
/// by [`crate::run_together`] beside Survivors' own. Bad content is reported and exits.
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
        include_str!("../../../../content/nightfall/classes.ron"),
    )?;
    let map = read("map.ron", include_str!("../../../../content/nightfall/map.ron"))?;
    let rules = read(
        "rules.ron",
        include_str!("../../../../content/nightfall/rules.ron"),
    )?;
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
    })
    .map_err(|e| e.to_string())?;
    if let Some(key) = class_key() {
        if content.class(&key).is_none() {
            return Err(format!("NIGHTFALL_CLASS: no class \"{key}\" in classes.ron"));
        }
    }
    match &dir {
        Some(d) => println!("nightfall: content from {}", d.display()),
        None => println!("nightfall: content compiled in"),
    }
    let _ = CONTENT.set(Arc::new(content));
    let _ = CONTENT_TEXTS.set((classes, map, rules));
    Ok(())
}

/// The screen Nightfall is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    /// The main menu: pick a class, play alone, host or join.
    #[default]
    Menu,
    /// A run.
    Playing,
}

/// Every Nightfall system that runs each frame; they run only while Nightfall is the game picked.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct NightfallSet;

/// Everything a run spawned, despawned when it ends.
#[derive(Component)]
pub(super) struct NightfallEntity;

pub(crate) struct NightfallPlugin;

impl Plugin for NightfallPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Phase>()
            .configure_sets(Update, NightfallSet.run_if(in_state(GameMode::Nightfall)))
            .insert_resource(menu::Selection::new(&content()))
            .init_resource::<menu::StartRequest>()
            .init_resource::<session::Net>()
            .init_resource::<run::LocalInput>()
            .init_resource::<run::EndRun>()
            .init_resource::<view::Views>()
            .add_plugins(grade::plugin)
            .add_systems(Startup, menu::spawn_roots)
            // Ungated: hides Nightfall's screens whenever another game is up.
            .add_systems(Update, menu::show_roots)
            .add_systems(
                Update,
                (
                    menu::build_menu,
                    menu::menu_clicks,
                    session::lobby,
                    menu::refresh_menu,
                    run::start_run,
                )
                    .chain()
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Menu)),
            )
            .add_systems(Update, session::close_on_exit.in_set(NightfallSet))
            .add_systems(Update, ui::tune_ui.in_set(NightfallSet))
            .add_systems(OnExit(GameMode::Nightfall), atmosphere::leave)
            .add_systems(
                Update,
                (
                    atmosphere::enter,
                    atmosphere::night,
                    atmosphere::follow_map,
                    atmosphere::hero_lights,
                )
                    .in_set(NightfallSet),
            )
            .add_systems(
                Update,
                atmosphere::menu_camera
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Menu)),
            )
            .add_systems(
                Update,
                (
                    run::feed_pose,
                    run::step_sim,
                    view::sync,
                    view::write_vitals,
                    view::place,
                )
                    .chain()
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    // The controller has moved the hero this frame before its pose is read.
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (run::leave_run, run::back_to_menu)
                    .chain()
                    .after(benilla_world::schedule::WorldStage::Input)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            );
    }
}
