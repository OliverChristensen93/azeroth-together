//! The inn's own panels for contract givers and station keepers (spec §8.4, §8.5): a giver's
//! contracts with their objectives and what is done of them (Take, Drop, Turn In), and a keeper's
//! stations with their level, the next level's price and gifts (Upgrade), and the recipes known
//! there (Craft). Our own panels; the spike says whether WoW's gossip and quest windows could
//! show them instead.
//!
//! Every click that changes the profile goes through [`act_on_profile`], as the stash panel's and
//! the trader's do: refused under a [`Lock`] that covers it, saved at once, and undone when the
//! save fails. What touches the loadout (a turn-in that takes from the backpack or the safe
//! slots, a companion's arrival) is refused while a joined player is ready; the rest works on the
//! stash and the purse alone and goes on. Materials for a station or a recipe come from the stash
//! only, so every price shows the stash's count beside it.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use nightfall_sim::{Content, Gift, InnContent, NpcRole, Profile};

use super::npcs::InnPanel;
use super::stash::Stash;
use super::stash_panel::{act_on_profile, lock_line, refused_by, Lock, StashLock};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL, RED};
use super::{content, inn, Phase};
use crate::game_pick::GameMode;

/// What a station level's gift is, in words.
pub(super) fn gift_line(g: &Gift, inn: &InnContent, content: &Content) -> String {
    match g {
        Gift::StashSlots(n) => format!("{n} more stash slots"),
        Gift::Recipe(r) => {
            let (item, n) = inn.recipes[*r].makes;
            let name = &content.items[item].name;
            if n > 1 {
                format!("Learn to make {name} ×{n}")
            } else {
                format!("Learn to make {name}")
            }
        }
        Gift::Intel => "The map table shows the next raid's exits".into(),
        Gift::Companion(c) => format!("{} joins you on raids", inn.companions[*c].name),
    }
}

/// A price in words, each material with what the stash holds of it: "40 gold, Iron Scrap 3/5".
pub(super) fn cost_line(
    gold: u32,
    items: &[(usize, u32)],
    p: &Profile,
    content: &Content,
) -> String {
    let mut parts = vec![format!("{gold} gold (you have {})", p.gold)];
    for &(i, n) in items {
        let def = &content.items[i];
        parts.push(format!("{} {}/{n}", def.name, p.stashed(&def.key).min(n)));
    }
    parts.join(", ")
}

#[derive(Component)]
pub(super) struct InnPanelRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelButton {
    Take(usize),
    Drop(usize),
    TurnIn(usize),
    Upgrade(usize),
    Craft(usize),
    Close,
}

#[derive(Resource)]
pub(super) struct PanelFont(Handle<Font>);

/// The open panel's NPC when it is one of these: a giver, or a keeper (the banker keeps the
/// stash's station). The trader has a panel of their own.
fn ours(panel: &InnPanel, inn: &InnContent) -> Option<usize> {
    panel
        .0
        .filter(|&i| inn.npcs.get(i).is_some_and(|n| n.role != NpcRole::Trader))
}

/// Whether `button` is one of NPC `who`'s: a contract of theirs, a station they keep, a recipe
/// of one of those. A press from a panel that has just changed hands acts on nothing.
fn belongs(button: PanelButton, who: usize, inn: &InnContent) -> bool {
    match button {
        PanelButton::Take(c) | PanelButton::Drop(c) | PanelButton::TurnIn(c) => {
            inn.contracts.get(c).is_some_and(|d| d.giver == who)
        }
        PanelButton::Upgrade(s) => inn.stations.get(s).is_some_and(|d| d.keeper == who),
        PanelButton::Craft(r) => inn
            .recipes
            .get(r)
            .and_then(|d| inn.stations.get(d.station))
            .is_some_and(|d| d.keeper == who),
        PanelButton::Close => true,
    }
}

/// Whether the next level of station `s` brings a companion. The companion's slot is part of the
/// loadout a hero is picked to bring (`Profile::raid_loadout`), so the level changes the pick.
fn upgrade_touches_loadout(p: &Profile, s: usize, inn: &InnContent) -> bool {
    let Some(level) = p.station_level(s, inn) else {
        return false;
    };
    inn.stations
        .get(s)
        .and_then(|d| d.levels.get(usize::from(level)))
        .is_some_and(|l| l.gives.iter().any(|g| matches!(g, Gift::Companion(_))))
}

/// Whether `button` changes what the hero is picked to bring, so the lobby's freeze refuses it.
fn touches_loadout(p: &Profile, button: PanelButton, inn: &InnContent, content: &Content) -> bool {
    match button {
        PanelButton::TurnIn(c) => p.turn_in_touches_loadout(c, inn, content),
        PanelButton::Upgrade(s) => upgrade_touches_loadout(p, s, inn),
        _ => false,
    }
}

/// One press, through [`act_on_profile`]: what it did is in the stash's note, and the profile is
/// as it was whenever it was refused or could not be saved. Whether the change stands.
fn press(
    stash: &mut Stash,
    button: PanelButton,
    lock: Option<Lock>,
    inn: &InnContent,
    content: &Content,
) -> bool {
    use nightfall_sim::ContractError;
    let touches = touches_loadout(&stash.profile, button, inn, content);
    act_on_profile(stash, lock, touches, |p| match button {
        PanelButton::Take(c) => p
            .take_contract(c, inn)
            .map(|()| String::new())
            .map_err(ContractError::message),
        PanelButton::Drop(c) => {
            if p.in_raid.is_some() {
                Err(ContractError::InRaid.message())
            } else if p.contract(c, inn).is_none_or(|s| !s.taken) {
                Err(ContractError::NotTaken.message())
            } else {
                p.drop_contract(c, inn);
                Ok(String::new())
            }
        }
        PanelButton::TurnIn(c) => p
            .turn_in(c, inn, content)
            .map(|g| format!("Done: {g} gold."))
            .map_err(ContractError::message),
        PanelButton::Upgrade(s) => p
            .upgrade_station(s, inn, content)
            .map(|n| format!("{} is level {n}.", inn.stations[s].name))
            .map_err(nightfall_sim::StationError::message),
        PanelButton::Craft(r) => p
            .craft(r, inn, content)
            .map(|()| {
                let (item, n) = inn.recipes[r].makes;
                format!("Made {} ×{n}: it is in your stash.", content.items[item].name)
            })
            .map_err(nightfall_sim::StationError::message),
        PanelButton::Close => Ok(String::new()),
    })
}

/// How wide the panel of an NPC with this role is. The banker's is a strip at the left edge
/// beside the stash panel (centred and about 740 px wide, so 240 px clears it at 1280 px); the
/// others are 360 px, which ends before the inn bar (top centre, from 388 px at 1280) begins.
fn panel_width(role: NpcRole) -> f32 {
    if role == NpcRole::Banker {
        240.0
    } else {
        360.0
    }
}

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        InnPanelRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            // Below the inn bar (top centre, about 190 px tall) and WoW's player frame.
            top: Val::Px(200.0),
            width: Val::Px(360.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            // A tall keeper's panel scrolls (the wheel, over it) instead of running off the
            // window: from 200 px down, 72% of the height leaves a margin at 720 px.
            max_height: Val::Vh(72.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ScrollPosition::default(),
        RelativeCursorPosition::default(),
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
        // Above the stash panel, which the banker's opens with.
        GlobalZIndex(430),
        Visibility::Hidden,
    ));
    commands.insert_resource(PanelFont(font));
}

/// Shown at Nightfall's inn while a giver's or a keeper's panel is open.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    panel: Res<InnPanel>,
    mut roots: Query<&mut Visibility, With<InnPanelRoot>>,
) {
    let on = *mode.get() == GameMode::Nightfall
        && *phase.get() == Phase::Inn
        && ours(&panel, &inn()).is_some();
    let want = if on {
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

/// The wheel over a panel taller than its window scrolls it.
pub(super) fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&RelativeCursorPosition, &ComputedNode, &mut ScrollPosition), With<InnPanelRoot>>,
) {
    for ev in wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => ev.y * 24.0,
            MouseScrollUnit::Pixel => ev.y,
        };
        for (cursor, node, mut pos) in &mut roots {
            if cursor.cursor_over() {
                // `ComputedNode` sizes are physical pixels.
                let most = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
                pos.0.y = (pos.0.y - dy).clamp(0.0, most);
            }
        }
    }
}

/// An action button, or in its place why there is none: nothing under a read-only stash (the
/// panel's red line says so once), the lobby's line when the freeze refuses it.
fn action(
    r: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    lock: Option<Lock>,
    touches: bool,
    button: PanelButton,
    caption: &str,
) {
    if !refused_by(lock, touches) {
        widgets::text_button(r, font, button, caption);
    } else if lock == Some(Lock::Lobby) {
        r.spawn(text(font, lock_line(Lock::Lobby), 11.0, RED));
    }
}

/// The open NPC's contracts, or the stations they keep, built again whenever the stash, the lock
/// or the panel changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    lock: Res<StashLock>,
    panel: Res<InnPanel>,
    font: Option<Res<PanelFont>>,
    mut body: Query<(Entity, &mut Node), With<InnPanelRoot>>,
) {
    let inn = inn();
    if !(stash.is_changed() || lock.is_changed() || panel.is_changed()) {
        return;
    }
    let Some(who) = ours(&panel, &inn) else {
        return;
    };
    let (Ok((body, mut node)), Some(font)) = (body.single_mut(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let p = &stash.profile;
    let lock = lock.0;
    let npc = &inn.npcs[who];
    let width = panel_width(npc.role);
    if node.width != Val::Px(width) {
        node.width = Val::Px(width);
    }
    let row = || Node {
        column_gap: Val::Px(6.0),
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        b.spawn(text(font, format!("{}: {}", npc.name, npc.title), 14.0, GOLD));
        if lock == Some(Lock::ReadOnly) {
            b.spawn(text(font, lock_line(Lock::ReadOnly), 12.0, RED));
        }
        // A giver's contracts.
        for (c, def) in inn.contracts.iter().enumerate().filter(|(_, d)| d.giver == who) {
            let taken = p.contract(c, &inn).is_some_and(|s| s.taken);
            b.spawn(text(font, &def.name, 13.0, if taken { GOLD } else { Color::WHITE }));
            if !def.text.is_empty() {
                b.spawn(text(font, &def.text, 11.0, GREY));
            }
            for (line, met) in p.objective_lines(c, &inn, &content) {
                let color = if met { Color::srgb(0.3, 1.0, 0.3) } else { GREY };
                b.spawn(text(font, format!("  {line}"), 11.0, color));
            }
            let mut reward = format!("  Reward: {} gold", def.reward.gold);
            for &s in &def.reward.unlocks {
                if let Some(station) = inn.stations.get(s) {
                    reward.push_str(&format!(", opens {}", station.name));
                }
            }
            b.spawn(text(font, reward, 11.0, GREY));
            b.spawn(row()).with_children(|r| {
                if taken {
                    if p.contract_ready(c, &inn, &content) {
                        let touches = p.turn_in_touches_loadout(c, &inn, &content);
                        action(r, font, lock, touches, PanelButton::TurnIn(c), "Turn In");
                    }
                    action(r, font, lock, false, PanelButton::Drop(c), "Drop");
                } else {
                    // Gated on the real answer, a raid under way included.
                    match p.can_take(c, &inn) {
                        Ok(()) => action(r, font, lock, false, PanelButton::Take(c), "Take"),
                        Err(e) => {
                            r.spawn(text(font, e.message(), 11.0, GREY));
                        }
                    }
                }
            });
        }
        // The stations this NPC keeps.
        for (s, def) in inn.stations.iter().enumerate().filter(|(_, d)| d.keeper == who) {
            let level = p.station_level(s, &inn);
            let status = match level {
                None => "Locked: a contract opens it".to_string(),
                Some(n) => format!("Level {n} of {}", def.levels.len()),
            };
            b.spawn(text(font, format!("{}  ·  {status}", def.name), 13.0, Color::WHITE));
            if let Some(next) = level.and_then(|n| def.levels.get(usize::from(n))) {
                b.spawn(text(
                    font,
                    format!(
                        "  Next, from the stash: {}",
                        cost_line(next.gold, &next.items, p, &content)
                    ),
                    11.0,
                    GREY,
                ));
                for g in &next.gives {
                    b.spawn(text(font, format!("  · {}", gift_line(g, &inn, &content)), 11.0, GREY));
                }
                b.spawn(row()).with_children(|r| {
                    let touches = upgrade_touches_loadout(p, s, &inn);
                    action(r, font, lock, touches, PanelButton::Upgrade(s), "Upgrade");
                });
            }
            for (r, recipe) in inn.recipes.iter().enumerate().filter(|(_, d)| d.station == s) {
                if !p.knows(r, &inn) {
                    continue;
                }
                let (item, n) = recipe.makes;
                b.spawn(text(
                    font,
                    format!(
                        "  Make {} ×{n}, from the stash: {}",
                        content.items[item].name,
                        cost_line(recipe.gold, &recipe.needs, p, &content)
                    ),
                    11.0,
                    GREY,
                ));
                b.spawn(row()).with_children(|row| {
                    action(row, font, lock, false, PanelButton::Craft(r), "Craft");
                });
            }
        }
        // The banker's note is the stash panel's, beside this one; the other panels show it
        // here, and the inn bar then leaves it out (`menu::refresh_menu`).
        if let (Some(note), false) = (&stash.note, npc.role == NpcRole::Banker) {
            b.spawn((
                text(font, note.clone(), 12.0, GOLD),
                // Wraps, so a long note never stretches the panel.
                Node {
                    width: Val::Px(width - 26.0),
                    ..default()
                },
            ));
        }
        widgets::text_button(b, font, PanelButton::Close, "Close");
    });
}

pub(super) fn clicks(
    buttons: Query<(&Interaction, &PanelButton), Changed<Interaction>>,
    lock: Res<StashLock>,
    mut stash: ResMut<Stash>,
    mut panel: ResMut<InnPanel>,
) {
    let inn = inn();
    // Only the open NPC's own panel acts: the frame `walk_away` closes it, its buttons may still
    // report a press.
    let Some(who) = ours(&panel, &inn) else {
        return;
    };
    let content = content();
    for (i, b) in &buttons {
        if *i != Interaction::Pressed || !belongs(*b, who, &inn) {
            continue;
        }
        match b {
            PanelButton::Close => panel.0 = None,
            _ => {
                press(&mut stash, *b, lock.0, &inn, &content);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::PathBuf;

    use nightfall_sim::{
        ContentSources, ContractError, InnSources, SavedRaid, SavedStack, StationError,
    };

    use super::*;

    #[test]
    fn gifts_and_prices_read_as_lines() {
        let c = Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (99.0, 0.0), (99.0, 99.0), (0.0, 99.0)],
                entries: [(1.0, 1.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "scrap", name: "Iron Scrap", kind: Material, icon: "a", weight: 1.0, stack: 20),
                (key: "potion", name: "Potion", kind: Potion, icon: "a", weight: 0.2, stack: 5,
                 on_use: Some(Heal(100.0))),
                (key: "conjured_water", name: "Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
            ]"#,
            loot: "()",
            monsters: "[]",
            rivals: "",
        })
        .unwrap();
        let inn = InnContent::from_ron(
            InnSources {
                hub: r#"(place: (name: "Inn", map: 0, spawn: (5.0, 5.0, 0.0),
                    bounds: [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                    door: (at: (9.0, 5.0), radius: 1.0), minute: 1170, view_distance: 100.0),
                    npcs: [(key: "b", name: "B", role: Banker, body: (race: 1, sex: 0, class_id: 1),
                            at: (2.0, 2.0, 0.0)),
                           (key: "a", name: "A", role: Station("lab"), body: (race: 1, sex: 0, class_id: 1),
                            at: (3.0, 3.0, 0.0))],
                    contracts_at_once: 2, trader: (stock: [], sells_for: 1.0),
                    companions: [(key: "dog", name: "Watch Hound", display: 1)])"#,
                contracts: "[]",
                stations: r#"(stations: [(key: "lab", name: "Lab", keeper: "a",
                    levels: [(gold: 40, items: [("scrap", 5)], gives: [Recipe("p"), Intel, Companion("dog"), StashSlots(20)])])],
                    recipes: [(key: "p", station: "lab", makes: ("potion", 2), needs: [("scrap", 1)])])"#,
            },
            &c,
        )
        .unwrap();
        let gives: Vec<String> = inn.stations[0].levels[0]
            .gives
            .iter()
            .map(|g| gift_line(g, &inn, &c))
            .collect();
        assert_eq!(
            gives,
            vec![
                "Learn to make Potion ×2",
                "The map table shows the next raid's exits",
                "Watch Hound joins you on raids",
                "20 more stash slots"
            ]
        );
        let mut p = Profile::default();
        p.gold = 12;
        p.stash.push(SavedStack {
            item: "scrap".into(),
            count: 3,
        });
        assert_eq!(cost_line(40, &[(0, 5)], &p, &c), "40 gold (you have 12), Iron Scrap 3/5");
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-innpanels-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn saved(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    fn setup() -> (std::sync::Arc<Content>, std::sync::Arc<InnContent>) {
        (super::super::shipped_content(), super::super::shipped_inn())
    }

    fn contract(inn: &InnContent, key: &str) -> usize {
        inn.contract(key).unwrap()
    }

    /// A stash in a folder of its own, with 12 bone fragments in it and 100 gold.
    fn stash_with_bones(name: &str, content: &Content) -> (Stash, PathBuf) {
        let path = dir(name).join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), content);
        stash.profile.gold = 100;
        stash.profile.stash_add(saved("bone_fragments", 12), content);
        assert!(stash.save());
        (stash, path)
    }

    /// A stash whose folder cannot be written: its parent is a file.
    fn stash_that_cannot_write(name: &str, content: &Content) -> Stash {
        let blocker = dir(name).join("not-a-folder");
        std::fs::write(&blocker, "x").unwrap();
        let mut stash = Stash::open_at(Some(blocker.join("stash.ron")), content);
        assert!(!stash.read_only(), "a missing file is a fresh stash that may save");
        stash.profile.gold = 100;
        stash.profile.stash_add(saved("bone_fragments", 12), content);
        assert!(stash.try_save().is_err());
        stash
    }

    fn in_raid() -> Option<SavedRaid> {
        Some(SavedRaid {
            map: "m".into(),
            brought: Default::default(),
            kept: None,
        })
    }

    #[test]
    fn a_button_belongs_to_the_npc_whose_it_is() {
        let (_, inn) = setup();
        let giver = inn.npcs.iter().position(|n| n.role == NpcRole::Contracts).unwrap();
        let c = inn.contracts.iter().position(|d| d.giver == giver).unwrap();
        let other = inn.contracts.iter().position(|d| d.giver != giver).unwrap();
        for b in [PanelButton::Take(c), PanelButton::Drop(c), PanelButton::TurnIn(c)] {
            assert!(belongs(b, giver, &inn), "{b:?}");
        }
        assert!(!belongs(PanelButton::Take(other), giver, &inn));
        assert!(!belongs(PanelButton::Take(inn.contracts.len()), giver, &inn));
        // A keeper's station and its recipes are theirs, and nobody else's.
        let workbench = inn.station("workbench").unwrap();
        let smith = inn.stations[workbench].keeper;
        let recipe = inn.recipes.iter().position(|r| r.station == workbench).unwrap();
        assert!(belongs(PanelButton::Upgrade(workbench), smith, &inn));
        assert!(belongs(PanelButton::Craft(recipe), smith, &inn));
        assert!(!belongs(PanelButton::Upgrade(workbench), giver, &inn));
        assert!(!belongs(PanelButton::Craft(recipe), giver, &inn));
        assert!(!belongs(PanelButton::Craft(inn.recipes.len()), smith, &inn));
        assert!(belongs(PanelButton::Close, giver, &inn));
    }

    #[test]
    fn a_panel_is_a_givers_or_a_keepers_not_the_traders() {
        let (_, inn) = setup();
        let at = |role| inn.npcs.iter().position(|n| n.role == role).unwrap();
        assert_eq!(ours(&InnPanel(Some(at(NpcRole::Contracts))), &inn), Some(at(NpcRole::Contracts)));
        // The banker keeps the stash's station.
        assert_eq!(ours(&InnPanel(Some(at(NpcRole::Banker))), &inn), Some(at(NpcRole::Banker)));
        assert_eq!(ours(&InnPanel(Some(at(NpcRole::Trader))), &inn), None);
        assert_eq!(ours(&InnPanel(None), &inn), None);
        assert_eq!(ours(&InnPanel(Some(inn.npcs.len())), &inn), None);
    }

    #[test]
    fn a_contract_is_taken_turned_in_and_saved_at_each_step() {
        let (content, inn) = setup();
        let (mut stash, path) = stash_with_bones("flow", &content);
        let c = contract(&inn, "raven_hill_remains");
        // The contract asks ten bones and the stash holds twelve: take it, then turn it in.
        assert!(press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        assert!(super::super::stash::load(&path).profile.contract(c, &inn).unwrap().taken);
        assert!(stash.profile.contract_ready(c, &inn, &content));
        let gold = stash.profile.gold;
        assert!(press(&mut stash, PanelButton::TurnIn(c), None, &inn, &content));
        assert_eq!(stash.profile.gold, gold + inn.contracts[c].reward.gold);
        assert_eq!(stash.profile.stashed("bone_fragments"), 2);
        assert_eq!(stash.note.as_deref(), Some("Done: 40 gold."));
        let on_disk = super::super::stash::load(&path).profile;
        assert_eq!(on_disk.gold, stash.profile.gold);
        assert_eq!(on_disk.stash, stash.profile.stash);
        // Two stations opened by it; repeatable, so it can be taken again, and dropped.
        assert!(stash.profile.station_level(inn.station("alchemy").unwrap(), &inn).is_some());
        assert!(press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        assert!(press(&mut stash, PanelButton::Drop(c), None, &inn, &content));
        assert!(!stash.profile.contract(c, &inn).unwrap().taken);
        assert!(!press(&mut stash, PanelButton::Drop(c), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(ContractError::NotTaken.message()));
    }

    #[test]
    fn a_turn_in_that_cannot_be_saved_is_undone() {
        let (content, inn) = setup();
        let mut stash = stash_that_cannot_write("unwritable-turn-in", &content);
        let c = contract(&inn, "raven_hill_remains");
        // Taking cannot be saved either, so make the contract taken in memory directly.
        stash.profile.take_contract(c, &inn).unwrap();
        let before = stash.profile.clone();
        assert!(stash.profile.contract_ready(c, &inn, &content));
        assert!(!press(&mut stash, PanelButton::TurnIn(c), None, &inn, &content));
        assert_eq!(stash.profile, before, "no gold paid, no bones taken, still taken");
        assert_eq!(stash.profile.gold, 100);
        assert_eq!(stash.profile.stashed("bone_fragments"), 12);
        let note = stash.note.clone().unwrap();
        assert!(note.contains("could not be saved"), "{note}");
        // The same for taking one.
        let mut stash = stash_that_cannot_write("unwritable-take", &content);
        let before = stash.profile.clone();
        assert!(!press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        assert_eq!(stash.profile, before);
        // And an upgrade.
        let mut stash = stash_that_cannot_write("unwritable-up", &content);
        stash.profile.stash_add(saved("linen_cloth", 10), &content);
        let cooking = inn.station("cooking").unwrap();
        stash.profile.inn.stations.push(("cooking".into(), 0));
        let before = stash.profile.clone();
        assert!(!press(&mut stash, PanelButton::Upgrade(cooking), None, &inn, &content));
        assert_eq!(stash.profile, before);
        assert_eq!(stash.profile.station_level(cooking, &inn), Some(0));
    }

    #[test]
    fn a_locked_panel_does_nothing_and_saves_nothing() {
        let (content, inn) = setup();
        let (mut stash, path) = stash_with_bones("locked", &content);
        let c = contract(&inn, "raven_hill_remains");
        assert!(press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        let before = stash.profile.clone();
        let file = std::fs::read_to_string(&path).unwrap();
        let buttons = [
            PanelButton::Drop(c),
            PanelButton::TurnIn(c),
            PanelButton::Take(contract(&inn, "brightwood_pack")),
            PanelButton::Upgrade(inn.station("stash").unwrap()),
        ];
        // Read-only refuses every one.
        for b in buttons {
            assert!(!press(&mut stash, b, Some(Lock::ReadOnly), &inn, &content), "{b:?}");
        }
        assert_eq!(stash.profile, before);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), file);
        // Ready in a lobby: this turn-in is paid from the stash alone, so it stands (and saves).
        assert!(!stash.profile.turn_in_touches_loadout(c, &inn, &content));
        assert!(press(&mut stash, PanelButton::TurnIn(c), Some(Lock::Lobby), &inn, &content));
        assert_eq!(super::super::stash::load(&path).profile, stash.profile);
    }

    #[test]
    fn the_lobbys_freeze_refuses_what_changes_the_pick() {
        let (content, inn) = setup();
        let path = dir("freeze").join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        let c = contract(&inn, "raven_hill_remains");
        // Ten bones, three of them in the backpack: the turn-in would take from the loadout.
        stash.profile.gold = 500;
        stash.profile.stash_add(saved("bone_fragments", 7), &content);
        stash.profile.loadout.pack.push(saved("bone_fragments", 3));
        stash.profile.take_contract(c, &inn).unwrap();
        assert!(stash.save());
        let before = stash.profile.clone();
        let file = std::fs::read_to_string(&path).unwrap();
        assert!(touches_loadout(&stash.profile, PanelButton::TurnIn(c), &inn, &content));
        assert!(!press(&mut stash, PanelButton::TurnIn(c), Some(Lock::Lobby), &inn, &content));
        assert_eq!(stash.profile, before, "the backpack keeps its bones");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), file);
        // A companion's arrival changes the pick too; its station is the kennel's.
        let kennel = inn.station("kennel").unwrap();
        stash.profile.inn.stations.push(("kennel".into(), 0));
        stash.profile.stash_add(saved("worgen_pelt", 2), &content);
        assert!(touches_loadout(&stash.profile, PanelButton::Upgrade(kennel), &inn, &content));
        let before = stash.profile.clone();
        assert!(!press(&mut stash, PanelButton::Upgrade(kennel), Some(Lock::Lobby), &inn, &content));
        assert_eq!(stash.profile, before);
        // The same two, with no freeze, go through; a stash-only upgrade passes the freeze.
        assert!(press(&mut stash, PanelButton::TurnIn(c), None, &inn, &content));
        assert!(stash.profile.loadout.pack.is_empty(), "taken from the backpack");
        let stash_station = inn.station("stash").unwrap();
        stash.profile.stash_add(saved("linen_cloth", 10), &content);
        assert!(!touches_loadout(&stash.profile, PanelButton::Upgrade(stash_station), &inn, &content));
        assert!(press(&mut stash, PanelButton::Upgrade(stash_station), Some(Lock::Lobby), &inn, &content));
        assert_eq!(stash.profile.station_level(stash_station, &inn), Some(1));
        assert!(press(&mut stash, PanelButton::Upgrade(kennel), None, &inn, &content));
        assert!(stash.profile.inn.companion.is_some());
    }

    #[test]
    fn nothing_changes_during_a_raid() {
        let (content, inn) = setup();
        let (mut stash, path) = stash_with_bones("raid", &content);
        let c = contract(&inn, "raven_hill_remains");
        assert!(press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        stash.profile.in_raid = in_raid();
        let before = stash.profile.clone();
        let file = std::fs::read_to_string(&path).unwrap();
        // The Take button is gated on the real answer, a raid under way included.
        assert_eq!(
            stash.profile.can_take(contract(&inn, "brightwood_pack"), &inn),
            Err(ContractError::InRaid)
        );
        for (b, why) in [
            (PanelButton::TurnIn(c), ContractError::InRaid.message()),
            (PanelButton::Drop(c), ContractError::InRaid.message()),
            (PanelButton::Take(contract(&inn, "brightwood_pack")), ContractError::InRaid.message()),
            (PanelButton::Upgrade(inn.station("stash").unwrap()), StationError::InRaid.message()),
        ] {
            assert!(!press(&mut stash, b, None, &inn, &content), "{b:?}");
            assert_eq!(stash.note.as_deref(), Some(why));
            assert_eq!(stash.profile, before);
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), file);
    }

    #[test]
    fn a_craft_and_an_upgrade_pay_from_the_stash_and_say_why_when_short() {
        let (content, inn) = setup();
        let path = dir("craft").join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        let workbench = inn.station("workbench").unwrap();
        stash.profile.inn.stations.push(("workbench".into(), 0));
        stash.profile.gold = 100;
        // Short of scrap: refused, nothing changed.
        let before = stash.profile.clone();
        assert!(!press(&mut stash, PanelButton::Upgrade(workbench), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(StationError::Missing.message()));
        assert_eq!(stash.profile, before);
        // Scrap in the loadout's backpack does not count: stash only.
        stash.profile.loadout.pack.push(saved("iron_scrap", 5));
        assert!(!press(&mut stash, PanelButton::Upgrade(workbench), None, &inn, &content));
        stash.profile.stash_add(saved("iron_scrap", 5), &content);
        assert!(press(&mut stash, PanelButton::Upgrade(workbench), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some("Workbench is level 1."));
        assert_eq!(stash.profile.stashed("iron_scrap"), 0);
        assert_eq!(stash.profile.loadout.pack, vec![saved("iron_scrap", 5)]);
        // Now the pouch: cloth, then made.
        let pouch = inn.recipe("linen_pouch").unwrap();
        assert!(belongs(PanelButton::Craft(pouch), inn.stations[workbench].keeper, &inn));
        assert!(!press(&mut stash, PanelButton::Craft(pouch), None, &inn, &content));
        stash.profile.stash_add(saved("linen_cloth", 6), &content);
        assert!(press(&mut stash, PanelButton::Craft(pouch), None, &inn, &content));
        assert_eq!(stash.profile.stashed("linen_pouch"), 1);
        assert_eq!(stash.note.as_deref(), Some("Made Mourner's Pouch ×1: it is in your stash."));
        assert_eq!(super::super::stash::load(&path).profile, stash.profile);
    }

    #[test]
    fn contracts_stations_and_recipes_the_content_dropped_are_left_alone() {
        let (content, inn) = setup();
        let path = dir("dropped").join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        let p = &mut stash.profile;
        p.gold = 5;
        p.inn.contracts.push(nightfall_sim::ContractState {
            key: "gone_contract".into(),
            taken: true,
            progress: vec![2],
            finished: 1,
        });
        p.inn.stations.push(("gone_station".into(), 3));
        p.inn.recipes.push("gone_recipe".into());
        let before = p.clone();
        assert!(stash.save());
        let p = &stash.profile;
        // Everything the panel reads, for every contract, station and recipe there is.
        for c in 0..inn.contracts.len() {
            let _ = p.objective_lines(c, &inn, &content);
            let _ = p.can_take(c, &inn);
            let _ = p.contract_ready(c, &inn, &content);
            let _ = p.turn_in_touches_loadout(c, &inn, &content);
        }
        for s in 0..inn.stations.len() {
            let _ = p.station_level(s, &inn);
            let _ = upgrade_touches_loadout(p, s, &inn);
        }
        for r in 0..inn.recipes.len() {
            let _ = p.knows(r, &inn);
        }
        assert_eq!(*p, before, "reading changes nothing");
        // Presses that are refused or taken lose nothing of the unknown keys.
        let c = contract(&inn, "brightwood_pack");
        assert!(press(&mut stash, PanelButton::Take(c), None, &inn, &content));
        assert!(press(&mut stash, PanelButton::Drop(c), None, &inn, &content));
        let p = &stash.profile;
        assert_eq!(p.inn.contracts.iter().filter(|s| s.key == "gone_contract").count(), 1);
        assert!(p.inn.contracts.iter().any(|s| s.key == "gone_contract" && s.taken && s.finished == 1));
        assert!(p.inn.stations.contains(&("gone_station".into(), 3)));
        assert!(p.inn.recipes.contains(&"gone_recipe".to_string()));
        // Saved and read again from disk, nothing is lost.
        let again = Stash::open_at(Some(path.clone()), &content).profile;
        assert_eq!(again.inn.contracts.iter().filter(|s| s.key == "gone_contract").count(), 1);
        assert!(again.inn.contracts.iter().any(|s| s.key == "gone_contract" && s.taken && s.finished == 1));
        assert!(again.inn.stations.contains(&("gone_station".into(), 3)));
        assert!(again.inn.recipes.contains(&"gone_recipe".to_string()));
        // And a raid's outcome moves the known ones on without touching the unknown.
        let mut p = stash.profile.clone();
        p.take_contract(c, &inn).unwrap();
        p.raids = 1;
        p.apply_contracts(
            &nightfall_sim::HeroOutcome {
                player: 0,
                name: "A".into(),
                fate: nightfall_sim::Fate::Died,
                time: 1.0,
                kills: 0,
                carried: Default::default(),
                overflow: Vec::new(),
                recovered: Vec::new(),
                recovery: None,
                slain: Vec::new(),
            },
            &inn,
            &content,
        );
        assert!(p.inn.contracts.iter().any(|s| s.key == "gone_contract" && s.progress == vec![2]));
    }
}
