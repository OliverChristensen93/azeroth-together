//! The launcher's first screen: which game to play. **Survivors** is the arena and its extraction
//! mode ([`crate::together`]); **Nightfall** is the stealth extraction game
//! ([`crate::nightfall`]). Each has its own main menu, whose Back button returns here.
//!
//! Test runs skip the screen: any `TOGETHER_*` variable starts Survivors and any `NIGHTFALL*` one
//! starts Nightfall, so the existing co-op and screenshot hooks keep working.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::glue::art::GlueArt;
use crate::glue::widgets::{glue_button, GlueBtnKind};
use crate::glue::GlueClicks;

/// Which game the launcher is in.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum GameMode {
    /// The pick screen.
    #[default]
    Pick,
    Survivors,
    Nightfall,
}

/// The game a launch starts in, from the names of the environment's variables.
pub(crate) fn initial_mode<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> GameMode {
    let mut mode = GameMode::Pick;
    for name in names {
        let name = name.as_ref();
        if name == "NIGHTFALL" || name.starts_with("NIGHTFALL_") {
            return GameMode::Nightfall;
        }
        if name.starts_with("TOGETHER_") {
            mode = GameMode::Survivors;
        }
    }
    mode
}

/// The screen's root; [`build_screen`] fills it once the glue art is in.
#[derive(Component)]
struct PickRoot;

#[derive(Component)]
struct PickBuilt;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PickAction {
    Survivors,
    Nightfall,
    Exit,
}

pub(crate) struct GamePickPlugin;

impl Plugin for GamePickPlugin {
    fn build(&self, app: &mut App) {
        let names = std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned());
        app.insert_state(initial_mode(names))
            .add_systems(Startup, spawn_root)
            .add_systems(Update, (build_screen, clicks, show_screen, apply_ui_scale));
    }
}

fn spawn_root(mut commands: Commands) {
    commands.spawn((
        PickRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        // Nearly opaque: nothing drives the camera on this screen.
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
        GlobalZIndex(430),
        Visibility::Hidden,
    ));
}

/// Build the screen once the archives are readable, so the glue art is there.
fn build_screen(
    mut commands: Commands,
    roots: Query<Entity, (With<PickRoot>, Without<PickBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    asset_server: Res<AssetServer>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let font = crate::char_select::wow_font(&asset_server);
    commands.entity(root).insert(PickBuilt).with_children(|ui| {
        if let Some(logo) = &art.logo {
            ui.spawn((
                ImageNode::new(logo.clone()),
                Node {
                    width: Val::Px(256.0),
                    height: Val::Px(128.0),
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
        }
        for (action, caption, line) in [
            (
                PickAction::Survivors,
                "Survivors",
                "The arena and its extraction mode: hordes, cards and broken builds.",
            ),
            (
                PickAction::Nightfall,
                "Nightfall",
                "Duskwood at night: sneak in, take what you can, get out alive.",
            ),
        ] {
            glue_button(
                ui,
                &art,
                &font,
                action,
                caption,
                240.0,
                45.0,
                GlueBtnKind::Normal,
                1.0,
            );
            ui.spawn((
                Text::new(line),
                TextFont {
                    font: font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.85, 0.7)),
                Node {
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
            ));
        }
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                &font,
                PickAction::Exit,
                "Exit",
                120.0,
                32.0,
                GlueBtnKind::Small,
                1.0,
            );
        });
    });
}

fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &PickAction)>,
    mode: Res<State<GameMode>>,
    mut next: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *mode.get() != GameMode::Pick {
        return;
    }
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    match action {
        PickAction::Survivors => next.set(GameMode::Survivors),
        PickAction::Nightfall => next.set(GameMode::Nightfall),
        PickAction::Exit => {
            exit.write(AppExit::Success);
        }
    }
}

/// The screen shows only while picking.
fn show_screen(mode: Res<State<GameMode>>, mut roots: Query<&mut Visibility, With<PickRoot>>) {
    let want = if *mode.get() == GameMode::Pick {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The one owner of `UiScale`: glue screens, the HUD and the level-up screen lay out on a screen
/// 768 units tall at any resolution, in every game, so it runs ungated.
fn apply_ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let want = windows.single().map(|w| w.height() / 768.0).unwrap_or(1.0);
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_launch_shows_the_pick_screen() {
        assert_eq!(initial_mode(["PATH", "USERNAME", "WOW_BG"]), GameMode::Pick);
    }

    #[test]
    fn together_variables_start_survivors() {
        assert_eq!(
            initial_mode(["PATH", "TOGETHER_AUTOPLAY"]),
            GameMode::Survivors
        );
    }

    #[test]
    fn nightfall_variables_start_nightfall_even_beside_together_ones() {
        assert_eq!(initial_mode(["NIGHTFALL"]), GameMode::Nightfall);
        assert_eq!(
            initial_mode(["TOGETHER_NAME", "NIGHTFALL_AUTOPLAY"]),
            GameMode::Nightfall
        );
        assert_eq!(initial_mode(["NIGHTFALLEN"]), GameMode::Pick);
    }

    /// Whether `UIParent` is shown after `chunk` runs.
    fn apply(chunk: Option<&str>, shown: &mut bool) {
        match chunk {
            Some("UIParent:Show()") => *shown = true,
            Some("UIParent:Hide()") => *shown = false,
            Some(other) => panic!("unexpected chunk {other}"),
            None => {}
        }
    }

    /// Pick → Survivors' menu → Back → Nightfall's inn → a run → Back → Survivors' menu → a
    /// run: each game owns WoW's interface while it is up and leaves it shown when it goes.
    #[test]
    fn each_game_owns_the_ui_across_switches() {
        use crate::{nightfall as nf, together as sv};
        let session = 1;
        let mut shown = true;
        let (mut sv_last, mut nf_last) = (None, None);

        apply(sv::ui_chunk(session, sv::Phase::Menu, sv_last), &mut shown);
        sv_last = Some((session, sv::Phase::Menu));
        assert!(!shown, "Survivors' menu hides the UI");
        apply(sv::ui_exit_chunk(sv_last), &mut shown);
        sv_last = None;
        assert!(shown, "leaving Survivors shows it again");

        apply(nf::ui_chunk(session, nf::Phase::Inn, nf_last), &mut shown);
        nf_last = Some((session, nf::Phase::Inn));
        assert!(shown, "the inn shows WoW's frames: the hero walks it");
        apply(
            nf::ui_chunk(session, nf::Phase::Playing, nf_last),
            &mut shown,
        );
        nf_last = Some((session, nf::Phase::Playing));
        assert!(
            shown,
            "a Nightfall run has the player frame, bars, chat and minimap"
        );
        apply(nf::ui_exit_chunk(nf_last), &mut shown);
        assert!(shown);

        apply(sv::ui_chunk(session, sv::Phase::Menu, sv_last), &mut shown);
        sv_last = Some((session, sv::Phase::Menu));
        assert!(!shown, "coming back, Survivors' menu hides the UI again");
        apply(
            sv::ui_chunk(session, sv::Phase::Playing, sv_last),
            &mut shown,
        );
        assert!(shown, "and a Survivors run shows it");
    }
}

/// Both games share one `Update` schedule, so a game's system that is not in its game's set runs
/// while the other game is up. Built from the real plugins and read from the schedule itself.
#[cfg(test)]
mod set_guard_tests {
    use bevy::ecs::schedule::SystemSet;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;

    use super::GameMode;
    use crate::nightfall::{NightfallPlugin, NightfallSet};
    use crate::together::{SurvivorsSet, TogetherPlugin};

    /// Systems that run outside their game's set on purpose: name, reason.
    const UNGATED: &[(&str, &str)] = &[
        (
            "benilla_app::together::menu::show_menu",
            "hides Survivors' menu whenever another game is up",
        ),
        (
            "benilla_app::nightfall::menu::show_roots",
            "hides Nightfall's screens whenever another game is up",
        ),
        (
            "benilla_app::nightfall::terrain::show_line",
            "hides the bake's line whenever another game is up",
        ),
        (
            "benilla_app::nightfall::stash_panel::show",
            "hides the stash panel whenever another game is up",
        ),
        (
            "benilla_app::nightfall::trader::show",
            "hides the trader's panel whenever another game is up",
        ),
        (
            "benilla_app::nightfall::inn_panels::show",
            "hides the givers' and keepers' panel whenever another game is up",
        ),
        (
            "benilla_app::nightfall::board::show",
            "hides the raid board whenever another game is up",
        ),
        (
            "benilla_app::nightfall::gear::load_line",
            "hides the load line whenever no run is up",
        ),
    ];

    #[test]
    fn every_game_system_runs_in_its_games_set() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        crate::together::prepare(root).expect("Survivors' content");
        crate::nightfall::prepare(root).expect("Nightfall's content");
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<GameMode>()
            .add_plugins((TogetherPlugin, NightfallPlugin));
        app.world_mut()
            .resource_scope(|world, mut schedules: Mut<Schedules>| {
                schedules
                    .get_mut(Update)
                    .expect("Update schedule")
                    .initialize(world)
                    .expect("the Update schedule builds");
            });
        let schedule = app.get_schedule(Update).expect("Update schedule");
        let graph = schedule.graph();
        let survivors = graph
            .systems_in_set(SurvivorsSet.intern())
            .expect("SurvivorsSet is in the schedule");
        let nightfall = graph
            .systems_in_set(NightfallSet.intern())
            .expect("NightfallSet is in the schedule");

        let (mut checked, mut offenders) = ([0usize; 2], Vec::new());
        for (key, system) in schedule.systems().expect("initialized") {
            let name = system.name().as_string();
            let (game, set) = if name.starts_with("benilla_app::together::") {
                (0, survivors)
            } else if name.starts_with("benilla_app::nightfall::") {
                (1, nightfall)
            } else {
                continue;
            };
            checked[game] += 1;
            if !set.contains(&key) && !UNGATED.iter().any(|(n, _)| *n == name) {
                offenders.push(name);
            }
        }
        // Without system names (Bevy's `debug` feature) nothing would be checked at all.
        assert!(
            checked[0] > 10 && checked[1] > 10,
            "too few game systems found by name: {checked:?}"
        );
        assert!(
            offenders.is_empty(),
            "these run in every game, outside their game's set: {offenders:?}\n\
             Add them `.in_set(SurvivorsSet)` / `.in_set(NightfallSet)`, or, if they must run \
             while the other game is up, to UNGATED here with the reason."
        );
    }
}
