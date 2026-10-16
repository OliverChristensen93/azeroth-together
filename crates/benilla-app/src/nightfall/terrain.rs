//! Duskwood's walls for the host's rules: the baked height map (`nightfall-bake`, spec §4.1),
//! read from `benilla-config/nightfall/` or baked on a thread of its own the first time
//! Nightfall's menu shows, so neither the menu nor a run waits on it. A line at the bottom of the
//! screen, over the loading screen, says how far it has got. Only a host needs the map, but every
//! player bakes it once, so whoever hosts next starts at once. A run that starts before it is
//! ready plays without enemies until it is; a bake that fails plays without them and says why.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use nightfall_bake::{load_or_bake, BakeRequest, Baked};
use nightfall_sim::HeightMap;
use web_time::Instant;

use super::content;
use super::session::Net;
use crate::game_pick::GameMode;

/// What the bake's thread hands back: the map and the seconds it took, or why there is none.
type Outcome = Result<(Baked, f32), String>;

#[derive(Default)]
enum Bake {
    #[default]
    Idle,
    /// On its thread: how far (thousandths) and, once done, what came of it.
    Running {
        progress: Arc<AtomicU32>,
        outcome: Arc<Mutex<Option<Outcome>>>,
    },
    Ready(Arc<HeightMap>),
    Failed(String),
}

/// The height map for the host's world, once there is one.
#[derive(Resource, Default)]
pub(super) struct Terrain {
    bake: Bake,
}

/// How the bake stands, for the line.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Progress {
    Idle,
    Running(f32),
    Ready,
    Failed(String),
}

impl Terrain {
    pub fn ready(&self) -> Option<&Arc<HeightMap>> {
        match &self.bake {
            Bake::Ready(map) => Some(map),
            _ => None,
        }
    }

    fn progress(&self) -> Progress {
        match &self.bake {
            Bake::Idle => Progress::Idle,
            Bake::Running { progress, .. } => {
                Progress::Running(progress.load(Ordering::Relaxed) as f32 / 1000.0)
            }
            Bake::Ready(_) => Progress::Ready,
            Bake::Failed(why) => Progress::Failed(why.clone()),
        }
    }
}

/// The line at the bottom of the screen: while baking, and after a failure; nothing otherwise.
pub(super) fn line(map_name: &str, progress: &Progress) -> Option<String> {
    match progress {
        Progress::Idle | Progress::Ready => None,
        Progress::Running(f) => Some(format!("Learning {map_name}'s walls… {:.0}%", f * 100.0)),
        Progress::Failed(why) => Some(format!(
            "{map_name}'s walls are unknown ({why}): no enemies this run"
        )),
    }
}

/// What the bake does this frame, given whether we have joined someone else's run: only a solo
/// player or a host needs the map, and so only they bake (a solo player may host next, so the
/// menu pre-bakes) and are told how it goes; a joined client uses the host's enemies and its own
/// bake's state is of no interest to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub start: bool,
    pub show_line: bool,
}

pub(super) fn plan(is_client: bool, progress: &Progress) -> Plan {
    Plan {
        start: !is_client && *progress == Progress::Idle,
        show_line: !is_client,
    }
}

/// The first frame Nightfall is up as a solo player or host: read the cache, or bake, on a thread
/// of its own.
pub(super) fn begin(mut terrain: ResMut<Terrain>, net: Res<Net>) {
    if !plan(net.is_client(), &terrain.progress()).start {
        return;
    }
    let Some(data) = benilla_formats::wow_data() else {
        terrain.bake = Bake::Failed("no WoW data folder".into());
        return;
    };
    let cache = crate::local_state::home().map(|h| h.join("nightfall"));
    let progress = Arc::new(AtomicU32::new(0));
    let outcome: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
    let (to_progress, to_outcome) = (progress.clone(), outcome.clone());
    let content = content();
    let spawned = std::thread::Builder::new()
        .name("nightfall-bake".into())
        .spawn(move || {
            let started = Instant::now();
            let map = &content.map;
            let req = BakeRequest {
                data_dir: &data,
                map_id: map.map,
                key: &map.key,
                bounds: &map.bounds,
                props: &[],
            };
            let report =
                |f: f32| to_progress.store((f.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
            // A panic in the bake must still end in an outcome, or the line would stay at its last
            // percentage for good.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                load_or_bake(&req, cache.as_deref(), &report)
            }))
            .unwrap_or_else(|_| Err(anyhow::anyhow!("the bake crashed")))
            .map(|baked| (baked, started.elapsed().as_secs_f32()))
            .map_err(|e| format!("{e:#}"));
            *to_outcome.lock().unwrap() = Some(result);
        });
    terrain.bake = match spawned {
        Ok(_) => Bake::Running { progress, outcome },
        Err(e) => Bake::Failed(format!("no thread for the bake: {e}")),
    };
}

/// The bake's result, once its thread is done.
pub(super) fn poll(mut terrain: ResMut<Terrain>) {
    let Bake::Running { outcome, .. } = &terrain.bake else {
        return;
    };
    let Some(result) = outcome.lock().unwrap().take() else {
        return;
    };
    terrain.bake = match result {
        Ok((baked, secs)) => {
            info!(
                "nightfall: height map {} in {secs:.1} s ({} columns hold more than terrain)",
                if baked.from_cache {
                    "read from the cache"
                } else {
                    "baked"
                },
                baked.map.complex_columns()
            );
            // A map with unreadable files plays but is not cached, so every launch bakes it again:
            // the log says which files and why.
            for failure in &baked.failures {
                warn!("nightfall: the bake could not read {failure}");
            }
            if let Some(why) = &baked.cache_error {
                warn!("nightfall: the height map was not cached: {why}");
            }
            Bake::Ready(Arc::new(baked.map))
        }
        Err(why) => {
            warn!("nightfall: no height map: {why}");
            Bake::Failed(why)
        }
    };
}

/// The line's node, hidden until there is something to say.
#[derive(Component)]
pub(super) struct TerrainLine;

pub(super) fn spawn_line(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Over the loading screen (1000), which covers the start of a run.
            GlobalZIndex(1001),
            Visibility::Hidden,
            TerrainLine,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font,
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
                TextShadow::default(),
            ));
        });
}

/// The line follows the bake while Nightfall is the game picked, and hides otherwise.
pub(super) fn show_line(
    mode: Res<State<GameMode>>,
    terrain: Res<Terrain>,
    net: Res<Net>,
    mut roots: Query<(&mut Visibility, &Children), With<TerrainLine>>,
    mut texts: Query<&mut Text>,
) {
    let progress = terrain.progress();
    let said = if *mode.get() == GameMode::Nightfall && plan(net.is_client(), &progress).show_line
    {
        line(&content().map.name, &progress)
    } else {
        None
    };
    for (mut visibility, children) in &mut roots {
        let want = if said.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        let Some(said) = &said else { continue };
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != *said {
                    text.0 = said.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_shows_while_baking_and_after_a_failure_only() {
        assert_eq!(line("Duskwood", &Progress::Idle), None);
        assert_eq!(
            line("Duskwood", &Progress::Running(0.42)).as_deref(),
            Some("Learning Duskwood's walls… 42%")
        );
        assert_eq!(line("Duskwood", &Progress::Ready), None);
        let failed = line("Duskwood", &Progress::Failed("no WoW data folder".into())).unwrap();
        assert!(failed.contains("no WoW data folder") && failed.contains("no enemies"));
    }

    #[test]
    fn only_a_solo_player_or_host_bakes_and_sees_the_line() {
        let (start, quiet) = (
            Plan {
                start: true,
                show_line: true,
            },
            Plan {
                start: false,
                show_line: true,
            },
        );
        // Solo or hosting: bake once, while nothing has begun.
        assert_eq!(plan(false, &Progress::Idle), start);
        assert_eq!(plan(false, &Progress::Running(0.3)), quiet);
        assert_eq!(plan(false, &Progress::Ready), quiet);
        assert_eq!(plan(false, &Progress::Failed("x".into())), quiet);
        // Joined: never starts one, never shows the line, whatever state an earlier bake is in.
        let none = Plan {
            start: false,
            show_line: false,
        };
        for progress in [
            Progress::Idle,
            Progress::Running(0.3),
            Progress::Ready,
            Progress::Failed("x".into()),
        ] {
            assert_eq!(plan(true, &progress), none);
        }
    }
}
