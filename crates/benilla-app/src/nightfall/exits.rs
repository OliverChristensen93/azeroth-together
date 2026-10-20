//! The ways out (spec §8.1): each open exit's ring on the ground (WoW's green rune), the exits
//! listed under the run bar with how long a late one has to go, the count while we stand in a
//! ring, our remains from the last raid marked on the minimap and the world map, and
//! `NIGHTFALL_AUTOPLAY_EXTRACT`'s walk to the nearest way out.

use std::collections::HashMap;
use std::sync::OnceLock;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{Content, ExitMark, Holder, V2};

use super::run::{Autoplay, Sim};
use super::widgets::{text, GOLD, GREY};
use super::{content, units, Phase};
use crate::net::GuidIndex;

/// Exits' GameObject serials start here.
const EXIT_SERIAL_BASE: u64 = 0x30_0000;
/// `G_RuneGroundGreen01.mdx`, and its radius at scale 1 in yards (the survivors' circles).
const RUNE: (u32, f32) = (674, 2.8);
/// How often the remains' marker is set again (WoW's own clears after 8 minutes), seconds.
const MARK_EVERY: f32 = 60.0;
/// The name the remains' marker carries, so it can be told from any other.
const REMAINS_NAME: &str = "Your remains";

/// `secs` as `m:ss`.
pub(super) fn clock(secs: f32) -> String {
    let s = secs.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The exits as lines: open ones, and the late ones with their wait; the shut ones not at all.
pub(super) fn exit_list(exits: &[ExitMark], content: &Content) -> String {
    exits
        .iter()
        .filter_map(|e| {
            let def = content.map.exits.get(usize::from(e.exit))?;
            match (e.open, e.opens_in) {
                (true, _) => Some(format!("{}: open", def.name)),
                (false, Some(t)) => Some(format!("{}: opens in {}", def.name, clock(t))),
                (false, None) => None,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Which entity draws which exit's ring.
#[derive(Resource, Default)]
pub(super) struct ExitViews {
    drawn: HashMap<u8, Entity>,
    /// Real seconds at which the remains' marker was last set this run.
    marked: Option<f32>,
}

/// Every open exit's ring on the ground, sized to it; one that is not open (yet) is not drawn.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<ExitViews>,
    mut index: ResMut<GuidIndex>,
) {
    let content = content();
    let loot = sim.world.loot_view(Some(sim.me));
    for e in &loot.exits {
        let Some(def) = content.map.exits.get(usize::from(e.exit)) else {
            continue;
        };
        let drawn = views.drawn.get(&e.exit).copied();
        match (e.open, drawn) {
            (true, None) => {
                let mut at = units::ground_to_bevy(def.at, 0.0);
                // Without ground under it (not streamed in yet) the ring waits.
                let Some(h) = world_point.terrain_height_under(at) else {
                    continue;
                };
                at.y = h + 0.05;
                let serial = EXIT_SERIAL_BASE + u64::from(e.exit);
                let scale = def.radius / RUNE.1;
                let id = units::spawn_object(
                    &mut commands,
                    serial,
                    RUNE.0,
                    scale,
                    at,
                    units::GO_TYPE_GENERIC,
                );
                index.0.insert(units::object_guid(serial), id);
                views.drawn.insert(e.exit, id);
            }
            (false, Some(id)) => {
                index.0.retain(|_, v| *v != id);
                commands.entity(id).despawn();
                views.drawn.remove(&e.exit);
            }
            _ => {}
        }
    }
}

/// The exits' list under the run bar ([`lines`] sets its text).
#[derive(Component)]
pub(super) struct ExitList;

/// The count in the middle of the screen while we stand in a ring ([`lines`] sets its text).
#[derive(Component)]
pub(super) struct ExtractLine;

/// Spawns the two lines, hidden: the exits' list under the minimap and the extract count.
pub(super) fn spawn_lines(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        ExitList,
        text(&font, "", 12.0, GREY),
        Node {
            position_type: PositionType::Absolute,
            // Under the minimap (which fills the corner down to about 150 px), not over it.
            right: Val::Px(16.0),
            top: Val::Px(150.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
    commands.spawn((
        ExtractLine,
        text(&font, "", 18.0, GOLD),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(38.0),
            top: Val::Percent(30.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
}

/// "The bridge to Elwynn: open …", and "Extracting… 6 / 10 s" while we count.
#[allow(clippy::type_complexity)]
pub(super) fn lines(
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    mut list: Query<(&mut Text, &mut Visibility), (With<ExitList>, Without<ExtractLine>)>,
    mut count: Query<(&mut Text, &mut Visibility), (With<ExtractLine>, Without<ExitList>)>,
) {
    let content = content();
    // Once the raid is over there is nothing left to get out by.
    let sim = sim.filter(|s| *phase.get() == Phase::Playing && s.over.is_none());
    let exits = sim
        .as_ref()
        .map(|s| exit_list(&s.world.loot_view(Some(s.me)).exits, &content));
    let counting = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .map(|h| h.carry.extracting)
        .filter(|t| *t > 0.0)
        .map(|t| {
            format!(
                "Extracting… {:.0} / {:.0} s  ·  stay in the ring",
                t.floor(),
                content.rules.run.extract_for
            )
        });
    let set = |text: &mut Text, vis: &mut Visibility, line: Option<String>| {
        let want = if line.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if let Some(line) = line {
            if text.0 != line {
                text.0 = line;
            }
        }
    };
    for (mut t, mut v) in &mut list {
        set(&mut t, &mut v, exits.clone());
    }
    for (mut t, mut v) in &mut count {
        set(&mut t, &mut v, counting.clone());
    }
}

/// Our remains from the last raid, marked on the minimap and the world map as WoW marks a guard's
/// directions; set again every minute while they are within the world's reach of us (the sim lists
/// only those).
pub(super) fn remains_marker(
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut marker: ResMut<crate::poi_marker::PoiMarker>,
    mut views: ResMut<ExitViews>,
) {
    let now = real.elapsed_secs();
    // The timer first: the loot view is worked out only on the frames the marker is due.
    if views.marked.is_some_and(|t| now - t < MARK_EVERY) {
        return;
    }
    let loot = sim.world.loot_view(Some(sim.me));
    let Some(m) = loot
        .marks
        .iter()
        .find(|m| m.holder == Holder::Remains(sim.me))
    else {
        return;
    };
    views.marked = Some(now);
    let poi = benilla_protocol::messages::GossipPoi {
        flags: 99,
        pos: [m.at.x, m.at.y],
        icon: 6,
        data: 0,
        name: REMAINS_NAME.into(),
    };
    marker.set(&poi, content().map.map, f64::from(now));
}

/// `NIGHTFALL_AUTOPLAY_EXTRACT`: set, the autoplay walks to the nearest open exit.
fn autoextract() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_AUTOPLAY_EXTRACT").is_some())
}

/// The middle of the open exit nearest `from`; none when every exit is shut (or late).
fn nearest_open_exit(exits: &[ExitMark], content: &Content, from: V2) -> Option<V2> {
    exits
        .iter()
        .filter(|e| e.open)
        .filter_map(|e| content.map.exits.get(usize::from(e.exit)))
        .map(|d| d.at)
        .min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)))
}

/// `NIGHTFALL_AUTOPLAY_EXTRACT`: the autoplay walks to the nearest open exit (without it, once the
/// plan says it has had enough, [`super::auto_brain`]). It runs just before
/// [`super::loot_view::autoloot`], which takes the goal over for a detour to a container and
/// clears only a goal it set itself, so the two never fight over it.
pub(super) fn autoexit(
    sim: Res<Sim>,
    plan: Res<super::auto_brain::AutoPlan>,
    mut auto: ResMut<Autoplay>,
) {
    // The setting always walks out; without it the hero does once it has had enough.
    if !autoextract() {
        if !super::run::autoplay() {
            return;
        }
        if let Some(exit) = plan.0.leave {
            if auto.goal != Some(exit) {
                auto.goal = Some(exit);
            }
        }
        return;
    }
    let content = content();
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let goal = nearest_open_exit(&sim.world.loot_view(Some(sim.me)).exits, &content, hero.pos);
    if auto.goal != goal {
        auto.goal = goal;
    }
}

/// A run ended: nothing of its exits is drawn any more (their entities went with the run), and
/// the flag on our remains is taken down.
pub(super) fn forget(
    mut views: ResMut<ExitViews>,
    mut marker: ResMut<crate::poi_marker::PoiMarker>,
) {
    *views = ExitViews::default();
    if marker.poi.as_ref().is_some_and(|p| p.name == REMAINS_NAME) {
        marker.poi = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ContentSources;

    /// A map with three exits: the bridge and the road (up from the start), the cart (late).
    fn three_exits() -> Content {
        nightfall_sim::Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (99.0, 0.0), (99.0, 99.0), (0.0, 99.0)],
                entries: [(1.0, 1.0, 0.0)],
                exits: [(key: "a", name: "The Bridge", at: (5.0, 5.0)),
                        (key: "b", name: "The Cart", at: (9.0, 9.0), late: true),
                        (key: "c", name: "The Road", at: (20.0, 9.0))])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: "[]",
            loot: "()",
            monsters: "[]",
            rivals: "",
        })
        .unwrap()
    }

    fn mark(exit: u8, open: bool, opens_in: Option<f32>) -> ExitMark {
        ExitMark {
            exit,
            open,
            opens_in,
        }
    }

    #[test]
    fn the_exits_say_when_they_open() {
        let c = three_exits();
        let marks = [
            mark(0, true, Some(0.0)),
            mark(1, false, Some(754.4)),
            mark(2, false, None),
        ];
        assert_eq!(
            exit_list(&marks, &c),
            "The Bridge: open\nThe Cart: opens in 12:34"
        );
        assert_eq!(clock(9.6), "0:10");
        assert_eq!(clock(-3.0), "0:00");
    }

    #[test]
    fn the_autoplay_walks_to_the_nearest_open_exit_and_none_when_all_are_shut() {
        let c = three_exits();
        let open_all = [
            mark(0, true, Some(0.0)),
            mark(1, true, Some(0.0)),
            mark(2, true, Some(0.0)),
        ];
        // Nearest to (19, 9): the road, not the cart or the bridge.
        assert_eq!(
            nearest_open_exit(&open_all, &c, V2::new(19.0, 9.0)),
            Some(V2::new(20.0, 9.0))
        );
        // The cart is nearest to (10, 10) but shut: the bridge it is.
        let cart_shut = [
            mark(0, true, Some(0.0)),
            mark(1, false, Some(100.0)),
            mark(2, true, Some(0.0)),
        ];
        assert_eq!(
            nearest_open_exit(&cart_shut, &c, V2::new(10.0, 10.0)),
            Some(V2::new(5.0, 5.0))
        );
        let shut = [mark(0, false, Some(5.0)), mark(2, false, None)];
        assert_eq!(nearest_open_exit(&shut, &c, V2::new(0.0, 0.0)), None);
    }
}
