//! The trader's panel (spec §8.4): what they sell and for how much, and what of the stash they
//! would buy, at their share of its value. Our own panel, opened by a right-click on the trader.
//!
//! A purchase or a sale works on the stash and the purse alone, never the loadout, so it goes
//! through [`act_on_profile`] without touching the loadout: a ready joined player may still trade,
//! a read-only stash may not, and a trade that cannot be saved is undone (gold from nothing, or
//! gold lost, otherwise).

use bevy::prelude::*;
use nightfall_sim::{Content, InnContent, NpcRole};

use super::npcs::InnPanel;
use super::stash::Stash;
use super::stash_panel::{act_on_profile, Lock, StashLock};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, inn, Phase};
use crate::game_pick::GameMode;

#[derive(Component)]
pub(super) struct TraderRoot;

/// The part rebuilt whenever the stash or the panel changes.
#[derive(Component)]
pub(super) struct TraderBody;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TraderButton {
    Buy(usize),
    Sell(usize),
    Close,
}

#[derive(Resource)]
pub(super) struct TraderFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        TraderRoot,
        TraderBody,
        Node {
            position_type: PositionType::Absolute,
            // Below the inn bar (top centre, about 190 px tall), clear of WoW's player frame (top
            // left) and minimap (top right); the stash panel is also centred but is the banker's,
            // and only one NPC's panel is open at a time.
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-360.0)),
            top: Val::Px(200.0),
            column_gap: Val::Px(24.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
        GlobalZIndex(420),
        Visibility::Hidden,
    ));
    commands.insert_resource(TraderFont(font));
}

/// Whether the open panel is the trader's.
fn trading(panel: &InnPanel, inn: &InnContent) -> bool {
    panel
        .0
        .is_some_and(|i| inn.npcs.get(i).is_some_and(|n| n.role == NpcRole::Trader))
}

/// Shown at Nightfall's inn while the trader's panel is open.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    panel: Res<InnPanel>,
    mut roots: Query<&mut Visibility, With<TraderRoot>>,
) {
    let on = *mode.get() == GameMode::Nightfall
        && *phase.get() == Phase::Inn
        && trading(&panel, &inn());
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

/// What the trader pays for `count` of an item worth `value` each, as the profile pays it.
fn sale_price(value: u32, count: u16, inn: &InnContent) -> u32 {
    (value.saturating_mul(u32::from(count)) as f32 * inn.trader.sells_for).floor() as u32
}

/// The goods and the stash's sellable stacks, built again whenever the stash, the lock or the
/// panel changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    lock: Res<StashLock>,
    panel: Res<InnPanel>,
    font: Option<Res<TraderFont>>,
    body: Query<Entity, With<TraderBody>>,
) {
    let inn = inn();
    if !(stash.is_changed() || lock.is_changed() || panel.is_changed()) || !trading(&panel, &inn)
    {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let npc = &inn.npcs[panel.0.expect("trading")];
    let row = || Node {
        column_gap: Val::Px(8.0),
        align_items: AlignItems::Center,
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    let column = || Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        width: Val::Px(330.0),
        ..default()
    };
    commands.entity(body).with_children(|b| {
        b.spawn(column()).with_children(|col| {
            col.spawn(text(font, format!("{}: {}", npc.name, npc.title), 14.0, GOLD));
            col.spawn(text(
                font,
                format!("You have {} gold.", stash.profile.gold),
                12.0,
                GREY,
            ));
            if lock.0 == Some(Lock::ReadOnly) {
                col.spawn(text(
                    font,
                    "Nothing can be traded: your stash is read-only this session.",
                    12.0,
                    widgets::RED,
                ));
            }
            col.spawn(text(font, "For sale", 12.0, GREY));
            for (i, &(item, price)) in inn.trader.stock.iter().enumerate() {
                let def = &content.items[item];
                col.spawn(row()).with_children(|r| {
                    widgets::slot(r, font, TraderButton::Buy(i), Some((def, 1)));
                    r.spawn(text(
                        font,
                        format!("{}  ·  {price} gold", def.name),
                        12.0,
                        Color::WHITE,
                    ));
                    widgets::text_button(r, font, TraderButton::Buy(i), "Buy");
                });
            }
            if let Some(note) = &stash.note {
                col.spawn((
                    text(font, note.clone(), 12.0, GOLD),
                    Node {
                        // Wraps, so a long note never stretches the panel.
                        width: Val::Px(330.0),
                        ..default()
                    },
                ));
            }
            widgets::text_button(col, font, TraderButton::Close, "Close");
        });
        b.spawn(column()).with_children(|col| {
            col.spawn(text(font, "From your stash", 12.0, GREY));
            // Rows wrap into more columns when there are more than fit the window's height.
            col.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_wrap: FlexWrap::Wrap,
                height: Val::Px(360.0),
                row_gap: Val::Px(4.0),
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|list| {
                for (n, s) in stash.profile.stash.iter().enumerate() {
                    let Some(k) = content.item(&s.item) else {
                        continue;
                    };
                    let def = &content.items[k];
                    let gold = sale_price(def.value, s.count, &inn);
                    if gold == 0 {
                        continue;
                    }
                    list.spawn(row()).with_children(|r| {
                        widgets::slot(r, font, TraderButton::Sell(n), Some((def, s.count)));
                        r.spawn(text(
                            font,
                            format!("{}  ·  {gold} gold", def.name),
                            12.0,
                            Color::WHITE,
                        ));
                        widgets::text_button(r, font, TraderButton::Sell(n), "Sell");
                    });
                }
            });
        });
    });
}

/// One purchase or sale, through the shared [`act_on_profile`]: it works on the stash and the
/// purse only, so the lobby's freeze does not stop it, but a read-only stash does. Whether it
/// stood.
fn trade(
    stash: &mut Stash,
    button: TraderButton,
    lock: Option<Lock>,
    inn: &InnContent,
    content: &Content,
) -> bool {
    act_on_profile(stash, lock, false, |p| match button {
        TraderButton::Buy(n) => p
            .buy(n, inn, content)
            .map(|()| String::new())
            .map_err(|e| e.message()),
        TraderButton::Sell(n) => p
            .sell_to_trader(n, inn, content)
            .map(|g| format!("Sold for {g} gold."))
            .map_err(|e| e.message()),
        TraderButton::Close => Ok(String::new()),
    })
}

pub(super) fn clicks(
    buttons: Query<(&Interaction, &TraderButton), Changed<Interaction>>,
    lock: Res<StashLock>,
    mut stash: ResMut<Stash>,
    mut panel: ResMut<InnPanel>,
) {
    let content = content();
    let inn = inn();
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            TraderButton::Close => panel.0 = None,
            _ => {
                trade(&mut stash, *b, lock.0, &inn, &content);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::PathBuf;

    use nightfall_sim::{SavedRaid, SavedStack, StationError};

    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-trader-{name}-{}", std::process::id()));
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

    /// The index of the trader's stock line selling `item`.
    fn stock_of(inn: &InnContent, content: &Content, item: &str) -> usize {
        inn.trader
            .stock
            .iter()
            .position(|&(i, _)| content.items[i].key == item)
            .unwrap()
    }

    /// A stash in a folder of its own: 40 gold and three ruby shards (sellable).
    fn stash_with_shards(name: &str, content: &Content) -> (Stash, PathBuf) {
        let path = dir(name).join("stash.ron");
        let mut stash = Stash::open_at(Some(path.clone()), content);
        stash.profile.gold = 40;
        stash.profile.stash_add(saved("ruby_shard", 3), content);
        assert!(stash.save());
        (stash, path)
    }

    /// A stash whose folder cannot be written: its parent is a file.
    fn stash_that_cannot_write(name: &str, content: &Content) -> Stash {
        let blocker = dir(name).join("not-a-folder");
        std::fs::write(&blocker, "x").unwrap();
        let mut stash = Stash::open_at(Some(blocker.join("stash.ron")), content);
        assert!(!stash.read_only(), "a missing file is a fresh stash that may save");
        stash.profile.gold = 40;
        stash.profile.stash_add(saved("ruby_shard", 3), content);
        assert!(stash.try_save().is_err());
        stash
    }

    #[test]
    fn a_sale_and_a_purchase_are_saved_at_once() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let (mut stash, path) = stash_with_shards("saved", &content);
        // Sell the shards: the gold, said and on disk.
        let before = stash.profile.gold;
        assert!(trade(&mut stash, TraderButton::Sell(0), None, &inn, &content));
        assert!(stash.profile.gold > before, "the shards fetch gold");
        assert!(stash.note.as_deref().unwrap().starts_with("Sold for"));
        assert!(stash.profile.stash.is_empty());
        let on_disk = super::super::stash::load(&path).profile;
        assert_eq!(on_disk.gold, stash.profile.gold);
        assert!(on_disk.stash.is_empty());
        // Buy bread: the price paid, the bread in the stash, on disk.
        let bread = stock_of(&inn, &content, "tough_bread");
        let price = inn.trader.stock[bread].1;
        let gold = stash.profile.gold;
        assert!(trade(&mut stash, TraderButton::Buy(bread), None, &inn, &content));
        assert_eq!(stash.profile.gold, gold - price);
        assert_eq!(stash.profile.stashed("tough_bread"), 1);
        assert_eq!(stash.note, None);
        let on_disk = super::super::stash::load(&path).profile;
        assert_eq!(on_disk.gold, stash.profile.gold);
        assert_eq!(on_disk.stash, stash.profile.stash);
    }

    #[test]
    fn a_refused_trade_says_why_and_changes_nothing() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let (mut stash, path) = stash_with_shards("refused", &content);
        stash.profile.gold = 0;
        let bread = stock_of(&inn, &content, "tough_bread");
        let before = stash.profile.clone();
        // Nothing to pay with, and no such stack.
        assert!(!trade(&mut stash, TraderButton::Buy(bread), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(StationError::NoGold.message()));
        assert_eq!(stash.profile, before);
        assert!(!trade(&mut stash, TraderButton::Sell(9), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(StationError::Unknown.message()));
        assert_eq!(stash.profile, before);
        // In a raid the profile itself refuses both.
        stash.profile.gold = 40;
        stash.profile.in_raid = Some(SavedRaid {
            map: "m".into(),
            brought: Default::default(),
            kept: None,
        });
        let before = stash.profile.clone();
        assert!(!trade(&mut stash, TraderButton::Sell(0), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(StationError::InRaid.message()));
        assert!(!trade(&mut stash, TraderButton::Buy(bread), None, &inn, &content));
        assert_eq!(stash.profile, before);
        assert_eq!(super::super::stash::load(&path).profile.stash, before.stash);
    }

    #[test]
    fn a_full_stash_refuses_a_purchase_whole() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let (mut stash, _) = stash_with_shards("full", &content);
        let bread = stock_of(&inn, &content, "tough_bread");
        // Fill every slot with something that is not bread.
        let mut n = 0;
        while stash.profile.stash.len() < stash.profile.stash_capacity(&content) {
            n += 1;
            stash.profile.stash_add(saved("night_watch_blade", 1), &content);
            assert!(n < 1000, "the stash fills");
        }
        let before = stash.profile.clone();
        assert!(!trade(&mut stash, TraderButton::Buy(bread), None, &inn, &content));
        assert_eq!(stash.note.as_deref(), Some(StationError::StashFull.message()));
        assert_eq!(stash.profile, before, "no gold taken for what did not fit");
    }

    #[test]
    fn a_locked_trader_trades_nothing_and_saves_nothing() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let (mut stash, path) = stash_with_shards("locked", &content);
        let before = stash.profile.clone();
        let file = std::fs::read_to_string(&path).unwrap();
        let bread = stock_of(&inn, &content, "tough_bread");
        // A read-only stash trades nothing.
        for button in [TraderButton::Sell(0), TraderButton::Buy(bread)] {
            assert!(!trade(&mut stash, button, Some(Lock::ReadOnly), &inn, &content));
        }
        assert_eq!(stash.profile, before);
        assert_eq!(stash.note, None, "the panel's red line says why, not the note");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), file);
        // A ready joined player's loadout is out of their hands; the stash and purse are not,
        // so the trader still trades (and saves).
        assert!(trade(&mut stash, TraderButton::Sell(0), Some(Lock::Lobby), &inn, &content));
        assert!(stash.profile.gold > before.gold);
        assert_eq!(super::super::stash::load(&path).profile.gold, stash.profile.gold);
    }

    #[test]
    fn a_trade_that_cannot_be_saved_is_undone() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let mut stash = stash_that_cannot_write("unwritable", &content);
        let before = stash.profile.clone();
        // A sale (gold from nothing, if it stuck).
        assert!(!trade(&mut stash, TraderButton::Sell(0), None, &inn, &content));
        assert_eq!(stash.profile, before);
        assert_eq!(stash.profile.gold, 40);
        let note = stash.note.clone().unwrap();
        assert!(note.contains("could not be saved"), "{note}");
        // A purchase (goods for nothing, if it stuck).
        stash.note = None;
        let bread = stock_of(&inn, &content, "tough_bread");
        assert!(!trade(&mut stash, TraderButton::Buy(bread), None, &inn, &content));
        assert_eq!(stash.profile, before);
        let note = stash.note.unwrap();
        assert!(note.contains("could not be saved"), "{note}");
    }

    #[test]
    fn the_trader_is_found_by_role() {
        let inn = super::super::shipped_inn();
        let at = |role| inn.npcs.iter().position(|n| n.role == role).unwrap();
        assert!(trading(&InnPanel(Some(at(NpcRole::Trader))), &inn));
        assert!(!trading(&InnPanel(Some(at(NpcRole::Banker))), &inn));
        assert!(!trading(&InnPanel(None), &inn));
        assert!(!trading(&InnPanel(Some(inn.npcs.len())), &inn));
    }

    #[test]
    fn a_sale_pays_the_traders_share() {
        let (content, inn) = (super::super::shipped_content(), super::super::shipped_inn());
        let (mut stash, _) = stash_with_shards("price", &content);
        let shown = sale_price(
            content.items[content.item("ruby_shard").unwrap()].value,
            3,
            &inn,
        );
        assert!(shown > 0);
        let before = stash.profile.gold;
        assert!(trade(&mut stash, TraderButton::Sell(0), None, &inn, &content));
        assert_eq!(stash.profile.gold - before, shown, "the panel quotes what is paid");
    }
}
