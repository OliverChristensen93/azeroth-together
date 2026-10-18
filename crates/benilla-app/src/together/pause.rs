//! Escape in a run: WoW's own game menu, which now pauses the run while it is open (in co-op,
//! for everyone, as the cards do), and whose Logout button is "Main Menu": it asks first, then
//! ends the run and goes back to our menu. What the run earned (renown, the gear it found) is
//! kept, as for a run that ended by itself (`talents::award_on_leave`, `armory::collect_found`).

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::ui_script::PendingEntryUiLoad;

use super::{session, LocalInput};

/// Raises (so the next frame tries again) until the game menu and its popups are loaded.
const MENU_LUA: &str = r#"
if not (GameMenuFrame and GameMenuButtonLogout and StaticPopupDialogs and StaticPopup_Show) then
    error("game menu not up yet")
end
StaticPopupDialogs["TOGETHER_LEAVE_RUN"] = {
    text = "Exiting to the main menu will end the run, but you keep the rewards for your progress so far.",
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
    or (KeyBindingFrame and KeyBindingFrame:IsVisible()) then
    open = 1
end
local leave = BENILLA_TOGETHER_LEAVE or 0
BENILLA_TOGETHER_LEAVE = nil
return open, leave
"#;

/// The player chose Main Menu: the run ends at the next [`super::back_to_menu`].
#[derive(Resource, Default)]
pub(super) struct LeaveRun(pub bool);

pub(super) fn game_menu(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut input: ResMut<LocalInput>,
    mut leave: ResMut<LeaveRun>,
    mut done: Local<Option<u64>>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) && script.run(MENU_LUA).is_ok() {
        *done = Some(session);
    }
    if *done != Some(session) {
        return;
    }
    let Ok((open, chose)) = script.eval::<(i64, i64)>(POLL_LUA) else {
        return;
    };
    input.menu = open != 0;
    if chose != 0 {
        leave.0 = true;
        input.menu = false;
    }
}

/// Leaving by choice: a host takes everyone back to the lobby; a joined player leaves the
/// session.
pub(super) fn leave_session(net: &mut session::Net) {
    if net.is_client() {
        net.leave("You left the run");
    } else {
        net.broadcast_ended();
    }
}
