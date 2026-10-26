//! WoW's own interface, for its game menu: Escape opens it as in WoW, Exit Game quits, and its
//! Logout button becomes "Return to Abbey". Everything else FrameXML would show (action bars, unit
//! frames, minimap, chat) is hidden, since this game draws its own HUD, and the panels its key
//! bindings would open are turned off. The world waits while the menu is up.

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::ui_script::PendingEntryUiLoad;

/// Raises (so the next frame tries again) until the in-game UI is loaded.
const TUNE_UI: &str = r#"
if not (UIParent and GameMenuFrame and GameMenuButtonLogout and MainMenuBar and PlayerFrame) then
    error("in-game UI not up yet")
end
local hide = {
    "MainMenuBar", "MultiBarBottomLeft", "MultiBarBottomRight", "MultiBarLeft", "MultiBarRight",
    "ShapeshiftBarFrame", "PetActionBarFrame", "BonusActionBarFrame",
    "PlayerFrame", "TargetFrame", "PartyMemberFrame1", "PartyMemberFrame2", "PartyMemberFrame3",
    "PartyMemberFrame4", "PetFrame", "MinimapCluster", "BuffFrame", "TemporaryEnchantFrame",
    "CastingBarFrame", "DurabilityFrame", "QuestWatchFrame", "ChatFrameMenuButton",
}
for _, name in ipairs(hide) do
    local f = getglobal(name)
    if f then
        f:UnregisterAllEvents()
        f:Hide()
    end
end
for i = 1, NUM_CHAT_WINDOWS do
    local frame = getglobal("ChatFrame" .. i)
    if frame then
        frame:UnregisterAllEvents()
        frame:Hide()
    end
end
-- The keys this game uses are bound to WoW panels too (Enter opens chat, C the character
-- sheet...): those panels have nothing to show here, so their openers do nothing.
local off = function() end
ChatFrame_OpenChat = off
ChatFrame_ReplyTell = off
ChatFrame_ReplyTell2 = off
ToggleCharacter = off
ToggleSpellBook = off
ToggleTalentFrame = off
ToggleBackpack = off
OpenAllBags = off
ToggleWorldMap = off
ToggleQuestLog = off
ToggleFriendsFrame = off
ToggleHelpFrame = off
ToggleMinimap = off
-- Logout has no realm to log out of: it takes the hero home instead.
GameMenuButtonLogout:SetText("Return to Abbey")
GameMenuButtonLogout:SetScript("OnClick", function()
    HideUIPanel(GameMenuFrame)
    BENILLA_ARPG_HOME = 1
end)
"#;

/// Whether the game menu, or a panel it opened, is up; and the Return to Abbey press, taken once.
const POLL_LUA: &str = r#"
local open = 0
if (GameMenuFrame and GameMenuFrame:IsVisible())
    or (OptionsFrame and OptionsFrame:IsVisible())
    or (SoundOptionsFrame and SoundOptionsFrame:IsVisible())
    or (UIOptionsFrame and UIOptionsFrame:IsVisible())
    or (KeyBindingFrame and KeyBindingFrame:IsVisible())
    or (AtmosOptionsFrame and AtmosOptionsFrame:IsVisible()) then
    open = 1
end
local home = BENILLA_ARPG_HOME or 0
BENILLA_ARPG_HOME = nil
return open, home
"#;

/// The game menu is up: the world waits and no key or click reaches the hero.
#[derive(Resource, Default)]
pub(super) struct Menu {
    pub open: bool,
}

/// Runs [`TUNE_UI`] once per UI session (a `/reload` is a new one), then each frame asks whether
/// the menu is up and whether Return to Abbey was pressed.
pub(super) fn menu(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut menu: ResMut<Menu>,
    mut sim: ResMut<super::Sim>,
    mut done: Local<Option<u64>>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) {
        if script.run(TUNE_UI).is_err() {
            return;
        }
        *done = Some(session);
    }
    let Ok((open, home)) = script.eval::<(i64, i64)>(POLL_LUA) else {
        return;
    };
    let open = open != 0;
    if menu.open != open {
        menu.open = open;
    }
    if home != 0 {
        sim.world.restart();
    }
}
