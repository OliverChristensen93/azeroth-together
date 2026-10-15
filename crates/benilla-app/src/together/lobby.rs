//! The co-op panel on the menu: hosting (the invite code to share, who has joined, Start Run),
//! joining (an invite code pasted in, or an address typed, then the host's lobby), and what went
//! wrong.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use together_net::Invite;

use super::menu::Selection;
use super::session::{Net, Role, MAX_PLAYERS};
use super::{content, StartRequest};
use crate::char_select::race_name;
use crate::glue::widgets::GlueDisabled;
use crate::glue::GlueClicks;
use crate::textinput::{wayland_display, HostClipboard};

/// The longest code or address the join box takes.
const MAX_TYPED: usize = 128;

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
    /// The host's invite code onto the clipboard.
    CopyCode,
    /// The clipboard into the join box.
    Paste,
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

/// The clipboard's text, through the engine's pasteboard.
fn clipboard_text(
    clipboard: &mut HostClipboard,
    window: &Query<&RawHandleWrapper, With<PrimaryWindow>>,
) -> Option<String> {
    clipboard.read(wayland_display(window.iter().next()))
}

/// Pasted text into the join box: the code or address in it, whatever surrounds it.
fn paste_into(dialog: &mut JoinDialog, text: &str) {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || ".:-[]".contains(*c))
        .take(MAX_TYPED)
        .collect();
    if !cleaned.is_empty() {
        dialog.address = cleaned;
    }
}

/// Typed characters into the address while the dialog is open; Ctrl+V pastes, Enter connects,
/// Escape closes.
pub(super) fn type_address(
    mut keyboard: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog: ResMut<JoinDialog>,
    mut net: ResMut<Net>,
    mut clipboard: NonSendMut<HostClipboard>,
    window: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    if !dialog.open {
        keyboard.clear();
        return;
    }
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    for key in keyboard.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        if ctrl && key.key_code == KeyCode::KeyV {
            if let Some(text) = clipboard_text(&mut clipboard, &window) {
                paste_into(&mut dialog, &text);
            }
            continue;
        }
        match &key.logical_key {
            Key::Character(_) if ctrl => {}
            Key::Character(s) => {
                for c in s.chars() {
                    let ok = c.is_ascii_alphanumeric() || ".:-[]".contains(c);
                    if ok && dialog.address.len() < MAX_TYPED {
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
#[allow(clippy::too_many_arguments)]
pub(super) fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &LobbyAction)>,
    mut dialog: ResMut<JoinDialog>,
    mut net: ResMut<Net>,
    mut start: ResMut<StartRequest>,
    mut clipboard: NonSendMut<HostClipboard>,
    window: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    match action {
        LobbyAction::CopyCode => {
            if let Role::Host(h) = &net.role {
                if let Invite::Ready(code) = h.invite() {
                    clipboard.write(wayland_display(window.iter().next()), &code);
                    net.status = "Invite code copied: paste it to your friends".into();
                }
            }
        }
        LobbyAction::Paste => {
            if let Some(text) = clipboard_text(&mut clipboard, &window) {
                paste_into(&mut dialog, &text);
            }
        }
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
                "Join a friend's run\n\n{}_\n\nPaste the invite code your friend sent you (Ctrl+V or Paste), then Connect. On the same network, their address works too.",
                dialog.address
            ),
            &[LobbyAction::Paste, LobbyAction::Connect, LobbyAction::Cancel],
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
                let (invite, ready) = match h.invite() {
                    Invite::Ready(code) => (
                        format!("Invite code: send it to your friends\n{code}"),
                        true,
                    ),
                    Invite::Failed(why) => (format!("No invite code ({why})"), false),
                    Invite::Opening | Invite::Off => ("Getting your invite code…".into(), false),
                };
                let buttons: &[LobbyAction] = if ready {
                    &[
                        LobbyAction::CopyCode,
                        LobbyAction::StartRun,
                        LobbyAction::Stop,
                    ]
                } else {
                    &[LobbyAction::StartRun, LobbyAction::Stop]
                };
                (
                    format!(
                        "Hosting co-op in {}\n\n{invite}\nOn the same network: {}:{}\n\n{}\n{}   ·   {}/{MAX_PLAYERS} players",
                        content.zones[selection.zone].name,
                        h.lan,
                        h.port,
                        roster_lines(players.into_iter(), Some(0)),
                        net.status,
                        h.peers.len() + 1,
                    ),
                    buttons,
                )
            }
            Role::Client(c) => {
                let zone = c
                    .zone
                    .as_deref()
                    .and_then(|z| content.zone(z))
                    .map_or("…".to_string(), |z| content.zones[z].name.clone());
                let host = if together_net::parse_code(&c.addr).is_some() {
                    "by invite code".to_string()
                } else {
                    c.addr.clone()
                };
                (
                    format!(
                        "Joined {host}  ·  {zone}\n\n{}\n{}",
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
