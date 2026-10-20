//! The chat window's stock look, copied from `together/ui.rs` (Nightfall copies what it needs
//! instead of sharing it, so the two games can change apart), and when WoW's interface is shown.
//! A run without a server or character has no saved chat settings, which would leave the chat
//! window a solid white box.

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
for i = 1, NUM_CHAT_WINDOWS do
    local frame = getglobal("ChatFrame" .. i)
    if frame then
        FCF_SetWindowColor(frame, 0, 0, 0, 1)
        FCF_SetWindowAlpha(frame, 0, 1)
    end
end
"#;

/// What [`tune_ui`] last did to `UIParent`: the UI session and phase it brought it in line with.
/// A resource, so that leaving Nightfall ([`show_on_exit`]) can forget it.
#[derive(Resource, Default)]
pub(super) struct UiShown(Option<(u64, super::Phase)>);

/// The Lua that brings `UIParent` in line with `phase` in UI session `session`, or `None` when
/// `last` says it already is. In a run WoW's interface is up: the player frame, the action bars,
/// chat and the minimap; the inn is a place the hero walks, so it shows the same (the player
/// frame, the chat and the minimap are the hero's there too).
pub(crate) fn ui_chunk(
    session: u64,
    phase: super::Phase,
    last: Option<(u64, super::Phase)>,
) -> Option<&'static str> {
    if last == Some((session, phase)) {
        return None;
    }
    // The same in the inn and in a run: the interface is up.
    Some("UIParent:Show()")
}

/// The Lua that leaves `UIParent` as a launch starts, shown, when Nightfall goes, after `last`.
pub(crate) fn exit_chunk(last: Option<(u64, super::Phase)>) -> Option<&'static str> {
    match last {
        Some((_, super::Phase::Inn | super::Phase::Playing)) | None => None,
    }
}

/// Run [`TUNE_UI`] once per UI session (a `/reload` is a new one), after its load has finished,
/// and show WoW's interface as [`ui_chunk`] says. Nightfall decides this itself rather than
/// relying on what Survivors left behind.
pub(super) fn tune_ui(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    phase: Res<State<super::Phase>>,
    mut tuned: Local<crate::ui_script::VmMemo<bool>>,
    mut shown: ResMut<UiShown>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let tuned = tuned.get(&script);
    if !*tuned && script.run(TUNE_UI).is_ok() {
        *tuned = true;
    }
    if !*tuned {
        return;
    }
    let session = script.session();
    let phase = *phase.get();
    if let Some(chunk) = ui_chunk(session, phase, shown.0) {
        if script.run(chunk).is_ok() {
            shown.0 = Some((session, phase));
        }
    }
}

/// Leaving Nightfall: WoW's interface goes back to shown ([`exit_chunk`]), and Nightfall forgets
/// what it did, so it decides afresh on the way back.
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
    fn the_inn_and_a_run_both_show_the_ui_once_each() {
        assert_eq!(ui_chunk(1, Phase::Playing, None), Some("UIParent:Show()"));
        assert_eq!(ui_chunk(1, Phase::Playing, Some((1, Phase::Playing))), None);
        assert_eq!(ui_chunk(1, Phase::Inn, None), Some("UIParent:Show()"));
        assert_eq!(ui_chunk(1, Phase::Inn, Some((1, Phase::Inn))), None);
        assert_eq!(
            ui_chunk(2, Phase::Playing, Some((1, Phase::Playing))),
            Some("UIParent:Show()")
        );
    }

    #[test]
    fn leaving_has_nothing_to_show_since_nightfall_never_hides_the_ui() {
        assert_eq!(exit_chunk(Some((1, Phase::Inn))), None);
        assert_eq!(exit_chunk(Some((1, Phase::Playing))), None);
        assert_eq!(exit_chunk(None), None);
    }
}
