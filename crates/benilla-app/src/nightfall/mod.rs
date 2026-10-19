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
mod auras;
mod backpack;
mod bar;
mod board;
mod companion;
mod conceal;
mod dodge;
mod dress;
mod end_screen;
mod enemies;
mod exits;
mod fallen;
mod fight;
mod fx;
mod gear;
mod grade;
mod inn;
mod inn_panels;
mod loot_view;
mod marks;
mod menu;
mod npcs;
mod run;
mod senses_hud;
mod session;
mod stance;
mod stash;
mod stash_lock;
mod stash_panel;
mod telegraphs;
mod terrain;
mod trader;
mod ui;
mod units;
mod view;
mod widgets;

/// The UI decisions, for the launcher's test of moving between the games.
#[cfg(test)]
pub(crate) use ui::{exit_chunk as ui_exit_chunk, ui_chunk};

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use nightfall_sim::{Content, ContentSources, InnContent, InnSources};

use crate::game_pick::GameMode;

static CONTENT: OnceLock<Arc<Content>> = OnceLock::new();
/// The content files' text (classes, map, rules, enemies, spawns, abilities, items, loot, then the
/// inn's hub, contracts and stations, then the night's monsters), for the hash a client
/// and host compare.
static CONTENT_TEXTS: OnceLock<[String; 12]> = OnceLock::new();
/// The inn's content, set once by [`prepare`] and read through [`inn`].
static INN: OnceLock<Arc<InnContent>> = OnceLock::new();

pub(super) fn content_texts() -> &'static [String; 12] {
    CONTENT_TEXTS.get().expect("nightfall::prepare ran")
}

/// The game's content, as [`prepare`] loaded it.
pub(super) fn content() -> Arc<Content> {
    CONTENT.get().expect("nightfall::prepare ran").clone()
}

/// The inn's content, as [`prepare`] loaded it.
pub(super) fn inn() -> Arc<InnContent> {
    INN.get().expect("nightfall::prepare ran").clone()
}

/// The class a launch starts on: `NIGHTFALL_CLASS`, else the first.
fn class_key() -> Option<String> {
    std::env::var("NIGHTFALL_CLASS").ok()
}

/// The folder the RON files are read from, if one exists on disk ([`pick_content_dir`]).
fn content_dir(project_dir: &str) -> Result<Option<PathBuf>, String> {
    let fallbacks = [
        Some(Path::new(project_dir).join("content").join("nightfall")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content").join("nightfall"))),
    ];
    pick_content_dir(
        std::env::var_os("NIGHTFALL_CONTENT").map(PathBuf::from),
        fallbacks.into_iter().flatten(),
        |d| d.join("map.ron").is_file(),
    )
}

/// `NIGHTFALL_CONTENT`'s folder (`chosen`), which must hold the content (`has_content`): a
/// folder asked for by name and not there is an error, not a quiet fallback to other content.
/// Unset, the first fallback that holds it, else none (the compiled-in content).
fn pick_content_dir(
    chosen: Option<PathBuf>,
    fallbacks: impl IntoIterator<Item = PathBuf>,
    has_content: impl Fn(&Path) -> bool,
) -> Result<Option<PathBuf>, String> {
    if let Some(dir) = chosen {
        return if has_content(&dir) {
            Ok(Some(dir))
        } else {
            Err(format!(
                "NIGHTFALL_CONTENT: no map.ron in {}",
                dir.display()
            ))
        };
    }
    Ok(fallbacks.into_iter().find(|d| has_content(d)))
}

/// Read the content (from disk when there, so it can be edited without a rebuild); called once
/// by [`crate::run_together`] beside Survivors' own. Bad content is reported and exits.
pub(crate) fn prepare(project_dir: &str) -> Result<(), String> {
    let dir = content_dir(project_dir)?;
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
    let map = read(
        "map.ron",
        include_str!("../../../../content/nightfall/map.ron"),
    )?;
    let rules = read(
        "rules.ron",
        include_str!("../../../../content/nightfall/rules.ron"),
    )?;
    let enemies = read(
        "enemies.ron",
        include_str!("../../../../content/nightfall/enemies.ron"),
    )?;
    let spawns = read(
        "spawns.ron",
        include_str!("../../../../content/nightfall/spawns.ron"),
    )?;
    let abilities = read(
        "abilities.ron",
        include_str!("../../../../content/nightfall/abilities.ron"),
    )?;
    let items = read(
        "items.ron",
        include_str!("../../../../content/nightfall/items.ron"),
    )?;
    let loot = read(
        "loot.ron",
        include_str!("../../../../content/nightfall/loot.ron"),
    )?;
    let monsters = read(
        "monsters.ron",
        include_str!("../../../../content/nightfall/monsters.ron"),
    )?;
    let hub = read(
        "hub.ron",
        include_str!("../../../../content/nightfall/hub.ron"),
    )?;
    let contracts = read(
        "contracts.ron",
        include_str!("../../../../content/nightfall/contracts.ron"),
    )?;
    let stations = read(
        "stations.ron",
        include_str!("../../../../content/nightfall/stations.ron"),
    )?;
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
        enemies: &enemies,
        spawns: &spawns,
        abilities: &abilities,
        items: &items,
        loot: &loot,
        monsters: &monsters,
    })
    .map_err(|e| e.to_string())?;
    let inn = InnContent::from_ron(
        InnSources {
            hub: &hub,
            contracts: &contracts,
            stations: &stations,
        },
        &content,
    )
    .map_err(|e| e.to_string())?;
    if let Some(key) = class_key() {
        if content.class(&key).is_none() {
            return Err(format!(
                "NIGHTFALL_CLASS: no class \"{key}\" in classes.ron"
            ));
        }
    }
    match &dir {
        Some(d) => println!("nightfall: content from {}", d.display()),
        None => println!("nightfall: content compiled in"),
    }
    let _ = CONTENT.set(Arc::new(content));
    let _ = INN.set(Arc::new(inn));
    let _ = CONTENT_TEXTS.set([
        classes, map, rules, enemies, spawns, abilities, items, loot, hub, contracts, stations,
        monsters,
    ]);
    Ok(())
}

/// A rogue whose bar holds Stealth (on both bars), a strike that starts the global cooldown, and
/// a stone: content for the client's tests.
#[cfg(test)]
pub(super) fn bar_test_content() -> nightfall_sim::Content {
    nightfall_sim::Content::from_ron(nightfall_sim::ContentSources {
        classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
            base_hp: 1000.0, speed: 7.0, power_max: 100.0,
            bar: [(1, "stealth"), (2, "strike"), (11, "stone"), (73, "stealth")])]"#,
        map: r#"(key: "t", name: "T", map: 0,
            bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
            entries: [(5.0, 5.0, 0.0)])"#,
        rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
            fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
            hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
        enemies: "[]",
        spawns: "[]",
        abilities: r#"[
            (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0,
             effects: [Stealth]),
            (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0, gcd: 1.0,
             effects: [Weapon(bonus: 22.0)]),
            (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0,
             cooldown: 20.0, effects: []),
        ]"#,
        items: "[]",
        loot: "()",
        monsters: "[]",
    })
    .unwrap()
}

/// The shipped content (`content/nightfall/`), for tests of what the client makes of the real
/// classes and enemies.
#[cfg(test)]
pub(super) fn shipped_content() -> Arc<Content> {
    Arc::new(
        Content::from_ron(ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
        })
        .expect("the shipped content parses"),
    )
}

/// The shipped inn (`hub.ron` and its neighbours), for tests of what the client makes of it.
#[cfg(test)]
pub(super) fn shipped_inn() -> Arc<InnContent> {
    Arc::new(
        InnContent::from_ron(
            InnSources {
                hub: include_str!("../../../../content/nightfall/hub.ron"),
                contracts: include_str!("../../../../content/nightfall/contracts.ron"),
                stations: include_str!("../../../../content/nightfall/stations.ron"),
            },
            &shipped_content(),
        )
        .expect("the shipped inn parses"),
    )
}

/// The screen Nightfall is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    /// Between raids: the inn (spec §8.4), which is also the lobby.
    #[default]
    Inn,
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
        // Read once, here: a raid the last session never ended is settled as missing and saved
        // before anything else can look at the stash.
        let stash = stash::Stash::open();
        app.init_state::<Phase>()
            .configure_sets(Update, NightfallSet.run_if(in_state(GameMode::Nightfall)))
            .insert_resource(menu::Selection::new(&content(), &stash))
            .insert_resource(stash)
            .init_resource::<menu::StartRequest>()
            .init_resource::<session::Net>()
            .init_resource::<session::InnState>()
            .init_resource::<inn::InnViews>()
            .init_resource::<npcs::NpcViews>()
            .init_resource::<npcs::InnPanel>()
            .init_resource::<run::LocalInput>()
            .init_resource::<run::EndRun>()
            .init_resource::<view::Views>()
            .init_resource::<enemies::EnemyViews>()
            .init_resource::<marks::MarkViews>()
            .init_resource::<telegraphs::TelegraphViews>()
            .init_resource::<terrain::Terrain>()
            .init_resource::<ui::UiShown>()
            .init_resource::<conceal::TargetGrace>()
            .init_resource::<stash_panel::StashOpen>()
            .init_resource::<stash_panel::StashLock>()
            .init_resource::<run::Autoplay>()
            .init_resource::<exits::ExitViews>()
            .init_resource::<loot_view::ContainerViews>()
            .init_resource::<loot_view::AutoLoot>()
            .init_resource::<backpack::BackpackOpen>()
            .init_resource::<board::BoardOpen>()
            .add_plugins(grade::plugin)
            .add_systems(
                Startup,
                (
                    menu::spawn_roots,
                    terrain::spawn_line,
                    stash_panel::spawn_root,
                    trader::spawn_root,
                    inn_panels::spawn_root,
                    board::spawn_root,
                    backpack::spawn_root,
                    gear::spawn_line,
                ),
            )
            .add_systems(OnExit(Phase::Playing), (loot_view::forget, exits::forget))
            .add_systems(Startup, (exits::spawn_lines, end_screen::spawn_root))
            // Ungated by the run: they hide when none is up.
            .add_systems(
                Update,
                (exits::lines, end_screen::show).in_set(NightfallSet),
            )
            .add_systems(
                Update,
                (
                    exits::draw,
                    exits::remains_marker,
                    end_screen::continue_click,
                    end_screen::out_of_raid,
                )
                    // The Continue click ends the run in the frame it comes.
                    .before(run::back_to_menu)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                exits::autoexit
                    // After the world is stepped, and before the loot's detour takes the goal over
                    // (and clears only a goal it set), so the two never fight over it.
                    .after(view::sync)
                    .before(loot_view::autoloot)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            // Ungated: hides the load line whenever no run is up.
            .add_systems(Update, gear::load_line)
            // Ungated by the run: hides the panels outside one.
            .add_systems(Update, backpack::show.in_set(NightfallSet))
            // Ungated: hides Nightfall's screens whenever another game is up.
            .add_systems(Update, menu::show_roots)
            // Ungated: hides the stash panel whenever another game is up.
            .add_systems(Update, stash_panel::show)
            // Ungated: hides the trader's panel whenever another game is up.
            .add_systems(Update, trader::show)
            // Ungated: hides the givers' and keepers' panel whenever another game is up.
            .add_systems(Update, inn_panels::show)
            // Ungated: hides the raid board whenever another game is up.
            .add_systems(Update, board::show)
            .add_systems(Update, inn_panels::scroll.in_set(NightfallSet))
            .add_systems(OnEnter(GameMode::Nightfall), npcs::open_test_panel)
            .add_systems(Update, widgets::fill_icons.in_set(NightfallSet))
            .add_systems(
                Update,
                // After the lobby has sent our pick (so a pick sent this frame locks this frame)
                // and before the run can start: a host's edit and its Start are each one whole
                // step on the stash, never torn.
                (
                    stash_panel::refresh_lock,
                    stash_panel::clicks,
                    stash_panel::rebuild,
                    stash_panel::hover,
                )
                    .chain()
                    .after(session::lobby)
                    .before(run::start_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
            .add_systems(
                Update,
                // The raid board: the door, Ready, the start. After our hero is put where the
                // controller has it (so the door sees this frame's place), before the run can
                // start.
                (board::door, board::clicks, board::start, board::rebuild)
                    .chain()
                    .after(inn::feed)
                    .before(run::start_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
            .add_systems(
                Update,
                // The NPCs, their panels, the trader and the givers and keepers. After the lock is
                // read, so a trade this frame sees this frame's; before the run can start, as the
                // stash panel.
                (
                    npcs::spawn,
                    npcs::right_click,
                    npcs::walk_away,
                    trader::clicks,
                    trader::rebuild,
                    inn_panels::clicks,
                    inn_panels::rebuild,
                )
                    .chain()
                    .after(stash_panel::refresh_lock)
                    .before(run::start_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
            // Ungated: hides the bake's line whenever another game is up.
            .add_systems(Update, terrain::show_line)
            .add_systems(
                Update,
                (terrain::begin, terrain::poll).chain().in_set(NightfallSet),
            )
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
                    .run_if(in_state(Phase::Inn)),
            )
            // Back at the menu with a raid still on the books (we left, the host went): it was
            // never settled by an outcome, so it is counted missing, once.
            .add_systems(OnEnter(Phase::Inn), stash::after_raid)
            .add_systems(
                OnExit(Phase::Inn),
                (inn::leave, npcs::forget, board::forget),
            )
            .add_systems(
                OnExit(GameMode::Nightfall),
                (inn::leave, npcs::forget, board::forget),
            )
            .add_systems(Update, session::close_on_exit.in_set(NightfallSet))
            .add_systems(Update, ui::tune_ui.in_set(NightfallSet))
            .add_systems(
                OnExit(GameMode::Nightfall),
                (
                    atmosphere::leave,
                    ui::show_on_exit,
                    backpack::restore_wow_bags,
                ),
            )
            .add_systems(
                Update,
                (atmosphere::enter, atmosphere::night)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing)),
            )
            .add_systems(
                Update,
                atmosphere::inn_light
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
            .add_systems(
                Update,
                (atmosphere::follow_map, atmosphere::hero_lights).in_set(NightfallSet),
            )
            .add_systems(
                Update,
                (inn::sync, inn::dress_new, inn::feed, inn::place_friends)
                    .chain()
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
            .add_systems(
                Update,
                (
                    run::advance_acts,
                    run::feed_pose,
                    run::step_sim,
                    run::take_events,
                    view::sync,
                    view::write_vitals,
                    view::place,
                    enemies::sync,
                    enemies::vitals,
                    enemies::place,
                    enemies::trace_seen,
                )
                    .chain()
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    // The controller has moved the hero this frame before its pose is read.
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            // Every hero's companion, drawn after the heroes are put where they are.
            .init_resource::<companion::PetViews>()
            .add_systems(OnExit(Phase::Playing), companion::forget)
            .add_systems(
                Update,
                companion::sync
                    .after(view::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                fight::show_events
                    // After this frame's events are taken and every body they name is drawn.
                    .after(run::take_events)
                    .after(enemies::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (
                    loot_view::draw,
                    loot_view::right_click,
                    loot_view::autoloot,
                    gear::wear,
                    gear::lanterns,
                    backpack::toggle,
                    backpack::quiet_wow_bags,
                    backpack::clicks,
                    backpack::rebuild,
                    backpack::hover,
                )
                    .chain()
                    // The bodies and the world are drawn and stepped this frame.
                    .after(view::sync)
                    // `toggle` and `quiet_wow_bags` hold the Lua VM, which the engine's schedule
                    // test wants after the UI's input.
                    .after(crate::ui_script::UiInput)
                    // Its commands land before the run can end and despawn the containers.
                    .before(run::leave_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                stance::auto_stealth
                    .before(run::feed_pose)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (bar::presses, dodge::key)
                    // After the UI's key and click handlers queue `UseAction`, before the engine
                    // applies the script's calls, and before the input goes out.
                    .after(crate::ui_script::UiInput)
                    .before(crate::script_calls::apply_script_calls)
                    .before(run::feed_pose)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (bar::fill, bar::autofight, dodge::setup)
                    // They hold the Lua VM, which the engine's schedule test wants after the
                    // UI's input.
                    .after(crate::ui_script::UiInput)
                    // The autofight's `UseAction` must be queued before `presses` takes the frame's
                    // presses, or the engine's own cast path would get it.
                    .before(bar::presses)
                    .before(dodge::key)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                fight::fields
                    // After the vitals and the enemies are written and drawn.
                    .after(view::write_vitals)
                    .after(enemies::place)
                    // Its commands land before the run can end and despawn the bodies.
                    .before(run::leave_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                // Our outcome into the stash and onto disk the frame it comes. Before the run can
                // end and take the `Sim` with it: a client hears `Outcome` and `Ended` in one poll,
                // and an outcome lost there would be counted missing by `after_raid`.
                stash::settle_run
                    .after(run::step_sim)
                    .before(run::leave_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                // What the hero holds, noted in the profile: a raid that ends without an outcome
                // is settled by it, not by what was brought (`Profile::note_carry`). Before the
                // outcome settles, so a hero who just got out is not noted again.
                stash::note_carry
                    .after(run::step_sim)
                    .before(stash::settle_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (
                    conceal::enemies,
                    auras::heroes,
                    auras::enemies,
                    stance::trace,
                    marks::draw,
                    senses_hud::spawn_eye,
                    senses_hud::drive_eye,
                    senses_hud::heartbeat,
                )
                    .chain()
                    .after(enemies::place)
                    // Its commands land before the run can end and despawn the bodies.
                    .before(run::leave_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                // `death_popup` holds the Lua VM, which the engine's schedule test wants after
                // the UI's input.
                fallen::death_popup
                    .after(crate::ui_script::UiInput)
                    .after(enemies::place)
                    .before(run::leave_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (telegraphs::draw, fallen::status_line)
                    .after(enemies::place)
                    // Their commands land before the run can end and despawn what they made.
                    .before(run::leave_run)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(chosen: Option<&str>, there: &[&str]) -> Result<Option<PathBuf>, String> {
        pick_content_dir(
            chosen.map(PathBuf::from),
            ["project", "beside-exe"].map(PathBuf::from),
            |d| there.iter().any(|t| Path::new(t) == d),
        )
    }

    #[test]
    fn a_chosen_folder_with_the_content_is_used() {
        assert_eq!(
            pick(Some("mine"), &["mine", "project"]),
            Ok(Some(PathBuf::from("mine")))
        );
    }

    #[test]
    fn a_chosen_folder_without_the_content_is_an_error_naming_it() {
        let why = pick(Some("typo"), &["project"]).unwrap_err();
        assert!(why.contains("typo") && why.contains("map.ron"), "{why}");
    }

    #[test]
    fn unset_takes_the_first_fallback_with_the_content_else_none() {
        assert_eq!(
            pick(None, &["beside-exe"]),
            Ok(Some(PathBuf::from("beside-exe")))
        );
        assert_eq!(
            pick(None, &["project", "beside-exe"]),
            Ok(Some(PathBuf::from("project")))
        );
        assert_eq!(pick(None, &[]), Ok(None));
    }
}
