//! Escape in a run: WoW's own game menu, which now pauses the run for everyone while it is open.
//! Once it closes, the run goes on when every player has pressed Ready on the pause panel (alone,
//! at once). Its Logout button is "Main Menu": it asks first, then the run is over for us (for a
//! host, for the whole party) and the results come up, with the renown earned, before Enter goes
//! back to the menu. What the run earned is kept, as for a run that ended by itself
//! (`talents::track_run`, `armory::collect_found`).

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::ui_script::PendingEntryUiLoad;

use super::bossintro::BossIntro;
use super::{session, LocalInput, Phase, Sim};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);

/// Raises (so the next frame tries again) until the game menu and its popups are loaded.
const MENU_LUA: &str = r#"
if not (GameMenuFrame and GameMenuButtonLogout and StaticPopupDialogs and StaticPopup_Show) then
    error("game menu not up yet")
end
StaticPopupDialogs["TOGETHER_LEAVE_RUN"] = {
    text = "Exiting to the main menu will end the run, but reward you for your progress so far.",
    button1 = "Main Menu",
    button2 = CANCEL,
    OnAccept = function() BENILLA_TOGETHER_LEAVE = 1 end,
    timeout = 0,
    whileDead = 1,
    hideOnEscape = 1,
}
GameMenuButtonLogout:SetText("Main Menu")
GameMenuButtonLogout:SetScript("OnClick", function()
    HideUIPanel(GameMenuFrame)
    StaticPopup_Show("TOGETHER_LEAVE_RUN")
end)
"#;

/// Whether the game menu, its confirmation, or a panel it opened is up; and the confirmation's
/// answer, taken once.
const POLL_LUA: &str = r#"
local open = 0
if (GameMenuFrame and GameMenuFrame:IsVisible())
    or (StaticPopup_Visible and StaticPopup_Visible("TOGETHER_LEAVE_RUN"))
    or (OptionsFrame and OptionsFrame:IsVisible())
    or (SoundOptionsFrame and SoundOptionsFrame:IsVisible())
    or (UIOptionsFrame and UIOptionsFrame:IsVisible())
    or (KeyBindingFrame and KeyBindingFrame:IsVisible())
    or (AtmosOptionsFrame and AtmosOptionsFrame:IsVisible()) then
    open = 1
end
local leave = BENILLA_TOGETHER_LEAVE or 0
BENILLA_TOGETHER_LEAVE = nil
return open, leave
"#;

/// We chose Main Menu: the run is over for us, its results are up, and Enter leaves
/// ([`super::back_to_menu`]).
#[derive(Resource, Default)]
pub(super) struct GaveUp(pub bool);

#[allow(clippy::too_many_arguments)]
pub(super) fn game_menu(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut sim: ResMut<Sim>,
    net: Res<session::Net>,
    mut input: ResMut<LocalInput>,
    mut gave_up: ResMut<GaveUp>,
    mut done: Local<Option<u64>>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) && script.run(MENU_LUA).is_ok() {
        *done = Some(session);
        info!("together: the game menu's Logout is Main Menu, and the menu pauses the run");
    }
    if *done != Some(session) {
        return;
    }
    let Ok((open, chose)) = script.eval::<(i64, i64)>(POLL_LUA) else {
        return;
    };
    input.menu = open != 0 && !gave_up.0;
    if chose != 0 && !gave_up.0 {
        gave_up.0 = true;
        input.menu = false;
        if net.is_client() {
            // Our hero falls for good at the host; the others play on.
            input.give_up = true;
        } else {
            // Alone or hosting: the run is over, for everyone.
            sim.world.give_up(None);
        }
    }
}

#[derive(Component)]
pub(super) struct PausePanel;

#[derive(Component)]
pub(super) struct PauseText;

#[derive(Component)]
pub(super) struct ReadyButton;

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let morpheus: Handle<Font> = asset_server.load("mpq://Fonts/MORPHEUS.TTF");
    commands
        .spawn((
            PausePanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(30.0),
                width: Val::Px(300.0),
                margin: UiRect::left(Val::Px(-150.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.03, 0.06, 0.94)),
            BorderColor::all(Color::srgb(0.75, 0.62, 0.3)),
            GlobalZIndex(330),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Paused"),
                TextFont {
                    font: morpheus,
                    font_size: 28.0,
                    ..default()
                },
                TextColor(GOLD),
                TextShadow::default(),
            ));
            p.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow::default(),
                TextLayout::new_with_justify(Justify::Center),
                PauseText,
            ));
            p.spawn((
                Button,
                ReadyButton,
                Node {
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.3, 0.08, 0.95)),
                BorderColor::all(Color::srgb(0.85, 0.7, 0.35)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Ready"),
                    TextFont {
                        font,
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(GOLD),
                    TextShadow::default(),
                ));
            });
        });
}

/// While the run is paused and our own menu is closed: alone, go on at once; in co-op, the
/// panel with a Ready button (Enter or Space too) and who the party is still waiting for.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn ready_panel(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    net: Res<session::Net>,
    keys: Res<ButtonInput<KeyCode>>,
    intro: Res<BossIntro>,
    gave_up: Res<GaveUp>,
    mut input: ResMut<LocalInput>,
    mut panel: Query<&mut Visibility, With<PausePanel>>,
    mut text: Query<&mut Text, With<PauseText>>,
    mut button: Query<(&Interaction, &mut Node), With<ReadyButton>>,
) {
    let mut show = false;
    if let Some(sim) = sim.filter(|_| *phase.get() == Phase::Playing) {
        let world = &sim.world;
        let alive = world.hero(sim.me).is_some_and(|h| h.alive);
        let waiting =
            world.menu_pause && alive && !input.menu && !intro.open() && !gave_up.0;
        if waiting && net.is_solo() {
            input.ready = true;
        } else if waiting {
            show = true;
            let ready = world.pause_ready.contains(&sim.me);
            let clicked = button
                .iter()
                .any(|(i, _)| *i == Interaction::Pressed);
            if !ready
                && (clicked
                    || keys.just_pressed(KeyCode::Enter)
                    || keys.just_pressed(KeyCode::Space))
            {
                input.ready = true;
            }
            let others: Vec<&str> = world
                .heroes
                .iter()
                .filter(|h| h.alive && h.player != sim.me && !world.pause_ready.contains(&h.player))
                .map(|h| h.name.as_str())
                .collect();
            let waiting_for = if others.is_empty() {
                String::new()
            } else {
                format!("\nWaiting for {}", others.join(", "))
            };
            let line = if ready {
                format!("You are ready.{waiting_for}")
            } else {
                format!("The run goes on when everyone is ready.{waiting_for}")
            };
            if let Ok(mut t) = text.single_mut() {
                if t.0 != line {
                    t.0 = line;
                }
            }
            for (_, mut node) in &mut button {
                let display = if ready { Display::None } else { Display::Flex };
                if node.display != display {
                    node.display = display;
                }
            }
        }
    }
    let want = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut panel {
        if *v != want {
            *v = want;
        }
    }
}

/// Leaving by choice, once the results have been read: a host takes everyone back to the lobby;
/// a joined player leaves the session.
pub(super) fn leave_session(net: &mut session::Net) {
    if net.is_client() {
        net.leave("You left the run");
    } else {
        net.broadcast_ended();
    }
}
