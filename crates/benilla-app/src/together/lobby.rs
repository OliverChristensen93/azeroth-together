//! The co-op panel on the menu: hosting (the address to share, who has joined, Start Run),
//! joining (the host's address, typed in, then the host's lobby), and what went wrong.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use super::menu::Selection;
use super::session::{Net, Role, MAX_PLAYERS};
use super::{content, StartRequest};
use crate::char_select::race_name;
use crate::glue::widgets::GlueDisabled;
use crate::glue::GlueClicks;

/// The Join Co-op dialog: open while the player types the host's address.
#[derive(Resource)]
pub(super) struct JoinDialog {
    pub open: bool,
    pub address: String,
}

impl Default for JoinDialog {
    fn default() -> Self {
        Self {
            open: false,
            address: saved_address().unwrap_or_else(|| "127.0.0.1".into()),
        }
    }
}

/// The last address joined, `benilla-config/together-join.txt`.
fn address_path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("together-join.txt"))
}

fn saved_address() -> Option<String> {
    let text = crate::local_state::read_to_string(&address_path()?).ok()?;
    Some(text.trim().to_owned()).filter(|a| !a.is_empty())
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum LobbyAction {
    StartRun,
    Stop,
    Connect,
    Cancel,
}

/// The panel's root, shown while hosting, joining or typing an address.
#[derive(Component)]
pub(super) struct LobbyPanel;

/// The panel's text: the address, the players, the status.
#[derive(Component)]
pub(super) struct LobbyText;

/// Typed characters into the address while the dialog is open; Enter connects, Escape closes.
pub(super) fn type_address(
    mut keyboard: MessageReader<KeyboardInput>,
    mut dialog: ResMut<JoinDialog>,
    mut net: ResMut<Net>,
) {
    if !dialog.open {
        keyboard.clear();
        return;
    }
    for key in keyboard.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        match &key.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    let ok = c.is_ascii_alphanumeric() || ".:-[]".contains(c);
                    if ok && dialog.address.len() < 64 {
                        dialog.address.push(c);
                    }
                }
            }
            Key::Backspace => {
                dialog.address.pop();
            }
            Key::Enter => connect(&mut dialog, &mut net),
            Key::Escape => dialog.open = false,
            _ => {}
        }
    }
}

fn connect(dialog: &mut JoinDialog, net: &mut Net) {
    let addr = dialog.address.trim().to_owned();
    if addr.is_empty() {
        return;
    }
    if let Some(path) = address_path() {
        let _ = crate::local_state::write_atomic(&path, &addr);
    }
    dialog.open = false;
    net.join(&addr);
}

/// The panel's buttons.
pub(super) fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &LobbyAction)>,
    mut dialog: ResMut<JoinDialog>,
    mut net: ResMut<Net>,
    mut start: ResMut<StartRequest>,
) {
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    match action {
        LobbyAction::StartRun => start.0 = true,
        LobbyAction::Stop => {
            let why = if net.is_client() {
                "Left the party"
            } else {
                "Stopped hosting"
            };
            net.leave(why);
        }
        LobbyAction::Connect => connect(&mut dialog, &mut net),
        LobbyAction::Cancel => dialog.open = false,
    }
}

/// One line per player: `1. Name, Race Class`.
fn roster_lines(players: impl Iterator<Item = (u8, String, String, u8)>, me: Option<u8>) -> String {
    let content = content();
    let mut out = String::new();
    for (player, name, class, race) in players {
        let class_name = content
            .class(&class)
            .map_or(class.clone(), |c| content.classes[c].name.clone());
        let you = if Some(player) == me { "  (you)" } else { "" };
        out.push_str(&format!(
            "{}. {name}, {} {class_name}{you}\n",
            player + 1,
            race_name(race)
        ));
    }
    out
}

/// The panel's text and which of its buttons show.
#[allow(clippy::type_complexity)]
pub(super) fn refresh(
    net: Res<Net>,
    dialog: Res<JoinDialog>,
    selection: Res<Selection>,
    mut panel: Query<&mut Visibility, With<LobbyPanel>>,
    mut text: Query<&mut Text, With<LobbyText>>,
    mut buttons: Query<(&LobbyAction, &mut Node, Option<&mut GlueDisabled>)>,
) {
    let content = content();
    let show = dialog.open || !net.is_solo() || !net.status.is_empty();
    for mut v in &mut panel {
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    let (body, visible): (String, &[LobbyAction]) = if dialog.open {
        (
            format!(
                "Join a friend's run\n\nHost address:  {}_\n\nType the address your friend's Host Co-op panel shows, then Connect (or Enter).",
                dialog.address
            ),
            &[LobbyAction::Connect, LobbyAction::Cancel],
        )
    } else {
        match &net.role {
            Role::Solo => (net.status.clone(), &[]),
            Role::Host(h) => {
                let mut players = vec![(
                    0u8,
                    super::session::hero_name(),
                    content.classes[selection.class].key.clone(),
                    selection.hero_look().map_or(1, |l| l.1),
                )];
                players.extend(h.peers.iter().map(|p| {
                    (
                        p.player,
                        p.hero.name.clone(),
                        p.hero.class.clone(),
                        p.hero.race,
                    )
                }));
                (
                    format!(
                        "Hosting co-op in {}\nFriends join with:  {}:{}\n\n{}\n{}   ·   {}/{MAX_PLAYERS} players\nAllow the game through Windows Firewall if it asks.",
                        content.zones[selection.zone].name,
                        h.lan,
                        h.port,
                        roster_lines(players.into_iter(), Some(0)),
                        net.status,
                        h.peers.len() + 1,
                    ),
                    &[LobbyAction::StartRun, LobbyAction::Stop],
                )
            }
            Role::Client(c) => {
                let zone = c
                    .zone
                    .as_deref()
                    .and_then(|z| content.zone(z))
                    .map_or("…".to_string(), |z| content.zones[z].name.clone());
                (
                    format!(
                        "Joined {}  ·  {zone}\n\n{}\n{}",
                        c.addr,
                        roster_lines(
                            c.lobby.iter().map(|p| (
                                p.player,
                                p.hero.name.clone(),
                                p.hero.class.clone(),
                                p.hero.race
                            )),
                            c.me
                        ),
                        net.status
                    ),
                    &[LobbyAction::Stop],
                )
            }
        }
    };
    if let Ok(mut t) = text.single_mut() {
        if t.0 != body {
            t.0 = body;
        }
    }
    for (action, mut node, disabled) in &mut buttons {
        let display = if visible.contains(action) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        if let Some(mut d) = disabled {
            let off = *action == LobbyAction::StartRun && selection.hero_look().is_none();
            if d.0 != off {
                d.0 = off;
            }
        }
    }
}
