//! Settings WoW's interface normally gets from a character's saved configuration, which a run
//! without a server or character has none of: applied with Lua once the in-game UI is up.

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::ui_script::PendingEntryUiLoad;

/// Raises (so the next frame tries again) until the in-game UI is loaded.
const TUNE_UI: &str = r#"
if not (PlayerFrame and ChatFrame1 and FCF_SetWindowColor and FCF_SetWindowAlpha) then
    error("in-game UI not up yet")
end
-- The chat windows' stock look: black, see-through until the mouse is over them. Their colour
-- and alpha come from the saved chat settings, which would otherwise leave a solid white box.
-- The action bar's spells have our own tooltip (together::tooltip); WoW's would describe the real
-- 1.12 spell behind each icon.
if ActionButton_SetTooltip then
    ActionButton_SetTooltip = function() GameTooltip:Hide() end
end
for i = 1, NUM_CHAT_WINDOWS do
    local frame = getglobal("ChatFrame" .. i)
    if frame then
        FCF_SetWindowColor(frame, 0, 0, 0, 1)
        FCF_SetWindowAlpha(frame, 0, 1)
    end
end
"#;

/// What [`tune_ui`] last did to `UIParent`: the UI session and phase it brought it in line with.
/// A resource, not a `Local`, so that leaving Survivors can forget it ([`show_on_exit`]) and the
/// menu hides the UI again on the way back.
#[derive(Resource, Default)]
pub(super) struct UiShown(Option<(u64, super::Phase)>);

/// The Lua that brings `UIParent` in line with `phase` in UI session `session`, or `None` when
/// `last` says it already is: hidden at the menu, behind our own screen, and shown in a run.
pub(crate) fn ui_chunk(
    session: u64,
    phase: super::Phase,
    last: Option<(u64, super::Phase)>,
) -> Option<&'static str> {
    if last == Some((session, phase)) {
        return None;
    }
    Some(match phase {
        super::Phase::Menu => "UIParent:Hide()",
        super::Phase::Playing => "UIParent:Show()",
    })
}

/// Run [`TUNE_UI`] once per UI session (a `/reload` is a new one), after its load has finished,
/// and show WoW's interface only in a run ([`ui_chunk`]).
pub(super) fn tune_ui(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    phase: Res<State<super::Phase>>,
    mut done: Local<Option<u64>>,
    mut shown: ResMut<UiShown>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) && script.run(TUNE_UI).is_ok() {
        *done = Some(session);
    }
    if *done != Some(session) {
        return;
    }
    let phase = *phase.get();
    if let Some(chunk) = ui_chunk(session, phase, shown.0) {
        if script.run(chunk).is_ok() {
            shown.0 = Some((session, phase));
        }
    }
}

/// The Lua that leaves `UIParent` as a launch starts, shown, when Survivors goes, after `last`.
pub(crate) fn exit_chunk(last: Option<(u64, super::Phase)>) -> Option<&'static str> {
    match last {
        Some((_, super::Phase::Menu)) => Some("UIParent:Show()"),
        Some((_, super::Phase::Playing)) | None => None,
    }
}

/// Leaving Survivors: the next game decides about WoW's interface from where a launch starts,
/// shown ([`exit_chunk`]), and Survivors forgets what it did, so its menu hides the UI again on
/// the way back.
pub(super) fn show_on_exit(script: Option<NonSend<UiScript>>, mut shown: ResMut<UiShown>) {
    if let (Some(script), Some(chunk)) = (script, exit_chunk(shown.0)) {
        let _ = script.run(chunk);
    }
    shown.0 = None;
}

#[cfg(test)]
mod tests {
    use super::super::Phase;
    use super::*;

    #[test]
    fn the_menu_hides_the_ui_and_a_run_shows_it_once_each() {
        assert_eq!(ui_chunk(1, Phase::Menu, None), Some("UIParent:Hide()"));
        assert_eq!(ui_chunk(1, Phase::Menu, Some((1, Phase::Menu))), None);
        assert_eq!(
            ui_chunk(1, Phase::Playing, Some((1, Phase::Menu))),
            Some("UIParent:Show()")
        );
        assert_eq!(ui_chunk(1, Phase::Playing, Some((1, Phase::Playing))), None);
    }

    #[test]
    fn a_new_ui_session_is_brought_in_line_again() {
        assert_eq!(
            ui_chunk(2, Phase::Menu, Some((1, Phase::Menu))),
            Some("UIParent:Hide()")
        );
    }

    #[test]
    fn leaving_shows_the_ui_only_if_survivors_hid_it() {
        assert_eq!(exit_chunk(Some((1, Phase::Menu))), Some("UIParent:Show()"));
        assert_eq!(exit_chunk(Some((1, Phase::Playing))), None);
        assert_eq!(exit_chunk(None), None);
    }
}
