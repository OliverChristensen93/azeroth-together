//! The menu's stash panel (spec §8.3): the stash on the left; on the right the loadout the next
//! raid brings (worn, bags, backpack, safe slots), its weight, and what the free kit would give
//! when it is empty. A click moves a stack between them; Shift and a click sells a stash stack.
//! Our own panel; in sub-project 5b the inn's banker opens it.
//!
//! Two things turn the moves off, and the panel says which ([`Lock`]): a joined player's pick is
//! with the host (a change now could be copied or lost, depending on whether the host starts
//! before or after hearing of it), and a stash this session may not save (every move would be
//! undone at the next launch).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use nightfall_sim::{inventory_of, Content, EquipSlot, Place, Stack, StashError};

use super::menu::Selection;
use super::session::{self, Net};
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL, RED, SLOT};
use super::{content, Phase};
use crate::game_pick::GameMode;

/// Whether the panel is open. `NIGHTFALL_STASH` opens it, for a test shot.
#[derive(Resource)]
pub(super) struct StashOpen(pub bool);

impl Default for StashOpen {
    fn default() -> Self {
        Self(std::env::var_os("NIGHTFALL_STASH").is_some())
    }
}

#[derive(Component)]
pub(super) struct StashRoot;

/// The part rebuilt whenever the stash changes.
#[derive(Component)]
pub(super) struct StashBody;

/// The line under the panel: what the mouse is over, or what just happened.
#[derive(Component)]
pub(super) struct StashInfo;

#[derive(Component)]
pub(super) struct StashClose;

/// Which stack a slot shows.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StashSlot {
    Stash(usize),
    Loadout(Place),
}

#[derive(Resource)]
pub(super) struct StashFont(Handle<Font>);

/// Why the panel's moves and sales are off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lock {
    /// Joined, and our pick is with the host.
    Lobby,
    /// This session may not save the stash.
    ReadOnly,
}

/// The panel's lock now, kept by [`refresh_lock`] so the others read one answer.
#[derive(Resource, Default)]
pub(super) struct StashLock(pub Option<Lock>);

/// Whether a joined player's loadout is out of their hands: they are joined, and their pick has
/// gone to the host or the host's lobby already lists it. Alone, or hosting (before Start, or
/// after: [`super::run::start_run`] reads the stash in one step), nothing is out.
pub(super) fn lobby_frozen(joined: bool, pick_sent: bool, host_holds: bool) -> bool {
    joined && (pick_sent || host_holds)
}

/// The lock the session's state puts on the panel; read-only is the more basic of the two.
pub(super) fn lock_of(read_only: bool, frozen: bool) -> Option<Lock> {
    if read_only {
        Some(Lock::ReadOnly)
    } else if frozen {
        Some(Lock::Lobby)
    } else {
        None
    }
}

/// The line that tells why moves are off. A read-only stash's own note says why.
pub(super) fn lock_line(lock: Lock, note: Option<&str>) -> String {
    match lock {
        Lock::Lobby => "Your loadout is with the host: leave the lobby to change it.".into(),
        Lock::ReadOnly => match note {
            Some(n) => format!("Nothing can be moved: your stash is read-only this session.\n{n}"),
            None => "Nothing can be moved: your stash is read-only this session.".into(),
        },
    }
}

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            StashRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                // From the top: the menu's own title and buttons sit at the bottom, and stay clear.
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::top(Val::Px(18.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            // The root covers the window: `ui_focus_system` treats no policy as `Block`, and it
            // would swallow every click meant for the menu under it.
            FocusPolicy::Pass,
            GlobalZIndex(420),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                StashBody,
                Node {
                    column_gap: Val::Px(24.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
            root.spawn((StashInfo, text(&font, "", 12.0, GREY)));
            widgets::text_button(root, &font, StashClose, "Close");
        });
    commands.insert_resource(StashFont(font));
}

/// Shown at Nightfall's menu while open.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    open: Res<StashOpen>,
    mut roots: Query<&mut Visibility, With<StashRoot>>,
) {
    let on = *mode.get() == GameMode::Nightfall && *phase.get() == Phase::Menu && open.0;
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

/// What turns the moves off now, if anything. Run after the lobby has told the host our pick, so
/// a pick sent this frame locks this frame.
pub(super) fn refresh_lock(
    net: Res<Net>,
    selection: Res<Selection>,
    stash: Res<Stash>,
    mut lock: ResMut<StashLock>,
) {
    let frozen = net.is_client() && {
        let own = session::own_pick(&selection, session::own_loadout(&selection, &stash));
        lobby_frozen(true, net.pick_sent(), net.host_holds(&own))
    };
    let now = lock_of(stash.read_only(), frozen);
    if lock.0 != now {
        lock.0 = now;
    }
}

/// The stash and the loadout, built again whenever either (or the class, or the lock) changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    open: Res<StashOpen>,
    selection: Res<Selection>,
    lock: Res<StashLock>,
    font: Option<Res<StashFont>>,
    body: Query<Entity, With<StashBody>>,
) {
    if !(stash.is_changed() || open.is_changed() || selection.is_changed() || lock.is_changed())
        || !open.0
    {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let p = &stash.profile;
    let (inv, _) = inventory_of(&p.loadout, &content);
    let item_of = |s: Option<Stack>| s.map(|s| (&content.items[usize::from(s.item)], s.count));
    let grid = |cols: usize| Node {
        flex_wrap: FlexWrap::Wrap,
        width: Val::Px(cols as f32 * (SLOT + 3.0)),
        column_gap: Val::Px(3.0),
        row_gap: Val::Px(3.0),
        ..default()
    };
    let column = || Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        b.spawn(column()).with_children(|col| {
            col.spawn(text(
                font,
                format!("Stash  {}/{}", p.stash.len(), p.stash_capacity(&content)),
                14.0,
                GOLD,
            ));
            if let Some(l) = lock.0 {
                col.spawn((
                    text(font, lock_line(l, stash.note.as_deref()), 12.0, RED),
                    Node {
                        width: Val::Px(10.0 * (SLOT + 3.0)),
                        ..default()
                    },
                ));
            }
            col.spawn(grid(10)).with_children(|g| {
                for (i, s) in p.stash.iter().enumerate() {
                    let def = content.item(&s.item).map(|k| (&content.items[k], s.count));
                    widgets::slot(g, font, StashSlot::Stash(i), def);
                }
            });
            col.spawn(text(font, format!("{} gold", p.gold), 13.0, GOLD));
            col.spawn(text(font, "Click: take  ·  Shift-click: sell", 11.0, GREY));
        });
        b.spawn(column()).with_children(|col| {
            let class = &content.classes[selection.class];
            col.spawn(text(
                font,
                format!("Next raid  ·  {}", class.name),
                14.0,
                GOLD,
            ));
            col.spawn(text(font, "Worn", 11.0, GREY));
            col.spawn(grid(EquipSlot::COUNT)).with_children(|g| {
                for s in EquipSlot::ALL {
                    let at = Place::Equip(s);
                    widgets::slot(
                        g,
                        font,
                        StashSlot::Loadout(at),
                        item_of(inv.get(at).copied()),
                    );
                }
            });
            col.spawn(text(font, "Bags", 11.0, GREY));
            col.spawn(grid(nightfall_sim::MAX_BAGS)).with_children(|g| {
                for i in 0..nightfall_sim::MAX_BAGS {
                    let at = Place::Bag(i as u8);
                    widgets::slot(
                        g,
                        font,
                        StashSlot::Loadout(at),
                        item_of(inv.get(at).copied()),
                    );
                }
            });
            col.spawn(text(
                font,
                format!("Backpack  ({} slots)", inv.pack.len()),
                11.0,
                GREY,
            ));
            col.spawn(grid(8)).with_children(|g| {
                for i in 0..inv.pack.len() {
                    let at = Place::Pack(i as u16);
                    widgets::slot(
                        g,
                        font,
                        StashSlot::Loadout(at),
                        item_of(inv.get(at).copied()),
                    );
                }
            });
            col.spawn(text(font, "Safe slots: always kept", 11.0, GREY));
            col.spawn(grid(inv.safe.len().max(1))).with_children(|g| {
                for i in 0..inv.safe.len() {
                    let at = Place::Safe(i as u8);
                    widgets::slot(
                        g,
                        font,
                        StashSlot::Loadout(at),
                        item_of(inv.get(at).copied()),
                    );
                }
            });
            let r = &content.rules.run;
            // An empty sum is negative zero, which would print as "-0.0".
            let weight = match inv.weight(&content) {
                w if w == 0.0 => 0.0,
                w => w,
            };
            col.spawn(text(
                font,
                format!(
                    "Weight {weight:.1}  (no slowing up to {:.0})",
                    r.weight_free
                ),
                11.0,
                if weight > r.weight_free { RED } else { GREY },
            ));
            if p.loadout.is_empty() {
                let kit: Vec<String> = class
                    .free_items
                    .iter()
                    .map(|&(i, n)| format!("{} ×{n}", content.items[i].name))
                    .collect();
                col.spawn((
                    text(
                        font,
                        format!(
                            "Nothing chosen: you go in with the free kit ({}).",
                            kit.join(", ")
                        ),
                        11.0,
                        GREY,
                    ),
                    // Wraps, so the long line never stretches the panel across the window.
                    Node {
                        width: Val::Px(10.0 * (SLOT + 3.0)),
                        ..default()
                    },
                ));
            }
        });
    });
}

/// One click on `slot`, and what it did: Shift sells a stash stack, or sends a loadout stack
/// between the backpack and the safe slots as it does in a raid; without Shift a stack goes to
/// the loadout, or back to the stash. A move that went is saved at once; one that did not says why
/// in the stash's note. With the panel [`Lock`]ed nothing happens at all (the panel already says
/// why), so a locked click never touches the profile or the disk.
fn act(stash: &mut Stash, slot: StashSlot, shift: bool, lock: Option<Lock>, content: &Content) {
    if lock.is_some() {
        return;
    }
    let done = match slot {
        StashSlot::Stash(n) if shift => stash
            .profile
            .sell(n, content)
            .map(|g| format!("Sold for {g} gold.")),
        StashSlot::Stash(n) => stash
            .profile
            .take_from_stash(n, content)
            .map(|_| String::new()),
        StashSlot::Loadout(at) if shift => {
            // Between the backpack and the safe slots, as Shift does in a raid.
            let (inv, _) = inventory_of(&stash.profile.loadout, content);
            let to = match at {
                Place::Safe(_) => inv.first_empty(false),
                _ => inv.first_empty(true),
            };
            match to {
                Some(to) => stash
                    .profile
                    .loadout_move(at, to, content)
                    .map(|_| String::new()),
                None => Err(StashError::LoadoutFull),
            }
        }
        StashSlot::Loadout(at) => stash
            .profile
            .put_in_stash(at, content)
            .map(|_| String::new()),
    };
    match done {
        Ok(line) => {
            stash.note = (!line.is_empty()).then_some(line);
            stash.save();
        }
        Err(e) => stash.note = Some(e.message().into()),
    }
}

/// A click moves a stack, Shift and a click sells it; Close closes. What went wrong (or the gold
/// a sale made) goes in the stash's note.
pub(super) fn clicks(
    slots: Query<(&Interaction, &StashSlot), Changed<Interaction>>,
    close: Query<&Interaction, (Changed<Interaction>, With<StashClose>)>,
    keys: Res<ButtonInput<KeyCode>>,
    lock: Res<StashLock>,
    mut stash: ResMut<Stash>,
    mut open: ResMut<StashOpen>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = false;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let content = content();
    for (i, slot) in &slots {
        if *i == Interaction::Pressed {
            act(&mut stash, *slot, shift, lock.0, &content);
        }
    }
}

/// The line under the panel: the stack under the mouse, else the stash's note.
pub(super) fn hover(
    slots: Query<(&Interaction, &StashSlot)>,
    stash: Res<Stash>,
    lock: Res<StashLock>,
    mut info: Query<&mut Text, With<StashInfo>>,
) {
    let content = content();
    let hovered = slots
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .map(|(_, s)| *s);
    // Under a lock there is nothing to click, so no hint says there is.
    let hint = |h: &str| {
        if lock.0.is_some() {
            String::new()
        } else {
            h.to_owned()
        }
    };
    let line = match hovered {
        Some(StashSlot::Stash(n)) => {
            stash
                .profile
                .stash
                .get(n)
                .map(|s| match content.item(&s.item) {
                    Some(k) => format!(
                        "{}{}",
                        widgets::describe(&content.items[k], s.count),
                        hint("  (click: take · shift-click: sell)")
                    ),
                    None => format!(
                        "{} ×{}: this game no longer knows it; it is kept",
                        s.item, s.count
                    ),
                })
        }
        Some(StashSlot::Loadout(at)) => {
            let (inv, _) = inventory_of(&stash.profile.loadout, &content);
            inv.get(at).map(|s| {
                format!(
                    "{}{}",
                    widgets::describe(&content.items[usize::from(s.item)], s.count),
                    hint("  (click: back to the stash · shift-click: safe slot)")
                )
            })
        }
        None => None,
    }
    .or_else(|| stash.note.clone())
    .unwrap_or_default();
    for mut t in &mut info {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::path::PathBuf;

    use nightfall_sim::SavedStack;

    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-panel-{name}-{}", std::process::id()));
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

    /// A stash file in a folder of its own, holding the blade and three shards.
    fn stash_with_things(name: &str) -> (Stash, PathBuf) {
        let path = dir(name).join("stash.ron");
        let content = super::super::shipped_content();
        let mut stash = Stash::open_at(Some(path.clone()), &content);
        stash
            .profile
            .stash_add(saved("night_watch_blade", 1), &content);
        stash.profile.stash_add(saved("ruby_shard", 3), &content);
        assert!(stash.save());
        (stash, path)
    }

    #[test]
    fn only_a_joined_player_whose_pick_is_out_is_frozen() {
        // Alone: nothing is out, whatever the lobby says (a solo `host_holds` is always true).
        assert!(!lobby_frozen(false, false, true));
        assert!(!lobby_frozen(false, true, true));
        // A host before Start (or after: `start_run` reads the stash in one step) is not joined.
        assert!(!lobby_frozen(false, false, false));
        // Joined, nothing sent and nothing held yet: free to edit, the first pick will carry it.
        assert!(!lobby_frozen(true, false, false));
        // Joined with a pick sent, or the host's lobby already listing it.
        assert!(lobby_frozen(true, true, false));
        assert!(lobby_frozen(true, false, true));
        assert!(lobby_frozen(true, true, true));
    }

    #[test]
    fn read_only_comes_before_the_lobby_and_each_says_why() {
        assert_eq!(lock_of(false, false), None);
        assert_eq!(lock_of(false, true), Some(Lock::Lobby));
        assert_eq!(lock_of(true, false), Some(Lock::ReadOnly));
        assert_eq!(lock_of(true, true), Some(Lock::ReadOnly));
        assert!(lock_line(Lock::Lobby, None).contains("with the host"));
        let line = lock_line(
            Lock::ReadOnly,
            Some("Your stash is from a newer Nightfall."),
        );
        assert!(
            line.contains("read-only") && line.contains("newer Nightfall"),
            "{line}"
        );
    }

    #[test]
    fn a_move_and_a_sale_are_saved_at_once() {
        let content = super::super::shipped_content();
        let (mut stash, path) = stash_with_things("saved");
        // Take the blade: worn, and on disk.
        act(&mut stash, StashSlot::Stash(0), false, None, &content);
        assert_eq!(stash.profile.stash.len(), 1);
        assert_eq!(stash.profile.loadout.equip.len(), 1);
        let on_disk = super::super::stash::load(&path).profile;
        assert_eq!(on_disk.loadout, stash.profile.loadout);
        assert_eq!(on_disk.stash, stash.profile.stash);
        // Sell the shards: the gold, said and on disk.
        let before = stash.profile.gold;
        act(&mut stash, StashSlot::Stash(0), true, None, &content);
        assert!(stash.profile.gold > before, "the shards fetch gold");
        assert!(stash.note.as_deref().unwrap().starts_with("Sold for"));
        let on_disk = super::super::stash::load(&path).profile;
        assert_eq!(on_disk.gold, stash.profile.gold);
        assert!(on_disk.stash.is_empty());
        // Put the blade back.
        act(
            &mut stash,
            StashSlot::Loadout(Place::Equip(EquipSlot::MainHand)),
            false,
            None,
            &content,
        );
        assert!(stash.profile.loadout.is_empty());
        assert_eq!(super::super::stash::load(&path).profile.stash.len(), 1);
    }

    #[test]
    fn a_refused_move_says_why_and_changes_nothing() {
        let content = super::super::shipped_content();
        let (mut stash, path) = stash_with_things("refused");
        let before = stash.profile.clone();
        act(&mut stash, StashSlot::Stash(9), false, None, &content);
        assert_eq!(
            stash.note.as_deref(),
            Some(StashError::NoSuchSlot.message())
        );
        assert_eq!(stash.profile, before);
        // In a raid every move is refused by the profile itself.
        stash.profile.in_raid = Some(nightfall_sim::SavedRaid {
            map: "m".into(),
            brought: Default::default(),
        });
        act(&mut stash, StashSlot::Stash(0), true, None, &content);
        assert_eq!(stash.note.as_deref(), Some(StashError::InRaid.message()));
        assert_eq!(stash.profile.gold, 0);
        assert_eq!(super::super::stash::load(&path).profile.gold, 0);
    }

    #[test]
    fn a_locked_panel_moves_and_sells_nothing_and_saves_nothing() {
        let content = super::super::shipped_content();
        let (mut stash, path) = stash_with_things("locked");
        let before = stash.profile.clone();
        let file = std::fs::read_to_string(&path).unwrap();
        for lock in [Lock::Lobby, Lock::ReadOnly] {
            for shift in [false, true] {
                act(&mut stash, StashSlot::Stash(0), shift, Some(lock), &content);
                act(
                    &mut stash,
                    StashSlot::Loadout(Place::Pack(0)),
                    shift,
                    Some(lock),
                    &content,
                );
            }
        }
        assert_eq!(stash.profile, before);
        assert_eq!(
            stash.note, None,
            "the panel's red line says why, not the note"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), file);
    }

    #[test]
    fn a_stash_this_session_may_not_save_is_read_only() {
        let content = super::super::shipped_content();
        let path = dir("newer").join("stash.ron");
        std::fs::write(&path, "(version: 999, gold: 5)").unwrap();
        let stash = Stash::open_at(Some(path), &content);
        assert!(stash.read_only());
        assert!(stash.note.as_deref().unwrap().contains("newer Nightfall"));
        // A fresh stash, and one with no folder at all, may be edited.
        let fresh = Stash::open_at(Some(dir("fresh").join("stash.ron")), &content);
        assert!(!fresh.read_only());
        assert!(!Stash::open_at(None, &content).read_only());
    }
}
