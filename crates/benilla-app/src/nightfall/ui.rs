//! The chat window's stock look, copied from `together/ui.rs` (Nightfall copies what it needs
//! instead of sharing it, so the two games can change apart). A run without a server or
//! character has no saved chat settings, which would leave the chat window a solid white box.

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

/// Run [`TUNE_UI`] once per UI session (a `/reload` is a new one), after its load has finished.
pub(super) fn tune_ui(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut done: Local<Option<u64>>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) && script.run(TUNE_UI).is_ok() {
        *done = Some(session);
    }
}
