//! The players' graphics options for the atmosphere: a WoW options panel (`options.lua`) in the
//! Game Menu, in Video Options and behind [`toggle_panel`], whose switches are CVars of their own,
//! saved with the client's settings. Every frame they change they become the plugin's
//! [`AtmosphereSettings`]; a test run that sets `WOW_ATMOS`, `WOW_RAYS`, `WOW_MIST` or `WOW_AO`
//! keeps its own settings instead.

use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::cvars::Cvars;
use crate::ui_script::PendingEntryUiLoad;

use super::AtmosphereSettings;

const PANEL: &str = include_str!("options.lua");

/// The settings the CVars hold, over `base` (whatever they do not name stays as it is).
fn from_cvars(cvars: &Cvars, base: AtmosphereSettings) -> AtmosphereSettings {
    let mut s = base;
    let flag = |name: &str, into: &mut bool| {
        if let Some(v) = cvars.flag(name) {
            *into = v;
        }
    };
    flag("atmosWeather", &mut s.weather);
    flag("atmosStorms", &mut s.lightning);
    flag("atmosStorms", &mut s.grade);
    flag("atmosFog", &mut s.mist);
    flag("atmosShafts", &mut s.rays);
    flag("atmosOcclusion", &mut s.ao);
    flag("atmosSharpen", &mut s.polish);
    flag("atmosWind", &mut s.wind);
    flag("atmosDayCycle", &mut s.clock);
    flag("atmosRaindrops", &mut s.lens);
    let scale = |name: &str, into: &mut f32| {
        if let Some(v) = cvars.num(name) {
            *into = v.clamp(0.0, 2.0);
        }
    };
    scale("atmosFogStrength", &mut s.mist_strength);
    scale("atmosShaftStrength", &mut s.ray_strength);
    scale("atmosOcclusionStrength", &mut s.ao_strength);
    s
}

/// Whether a test run set the effects from the environment.
fn env_owned() -> bool {
    ["WOW_ATMOS", "WOW_RAYS", "WOW_MIST", "WOW_AO"]
        .iter()
        .any(|n| crate::webenv::var(n).is_some())
}

/// Builds the panel once per UI session (a `/reload` is a new one), once the UI has loaded.
fn install(
    script: Option<NonSend<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut done: Local<Option<u64>>,
) {
    let Some(script) = script else { return };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done != Some(session) && script.run(PANEL).is_ok() {
        *done = Some(session);
    }
}

fn sync(
    cvars: Option<Res<Cvars>>,
    mut settings: ResMut<AtmosphereSettings>,
    mut owned: Local<Option<bool>>,
) {
    if *owned.get_or_insert_with(env_owned) {
        return;
    }
    let Some(cvars) = cvars else { return };
    if !cvars.is_changed() {
        return;
    }
    let next = from_cvars(&cvars, *settings);
    if next != *settings {
        *settings = next;
    }
}

/// Opens or closes the panel (a game's own menu button calls this).
pub(crate) fn toggle_panel(script: &UiScript) {
    if let Err(e) = script.run("if AtmosOptions_Toggle then AtmosOptions_Toggle() end") {
        warn!("atmosphere: options panel: {e}");
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (install, sync));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_names_every_cvar_the_rust_side_reads() {
        for name in [
            "atmosWeather", "atmosStorms", "atmosFog", "atmosShafts", "atmosOcclusion",
            "atmosSharpen", "atmosWind", "atmosDayCycle", "atmosRaindrops",
            "atmosFogStrength", "atmosShaftStrength", "atmosOcclusionStrength",
        ] {
            assert!(PANEL.contains(&format!("{name} = \"")), "{name} has no default in options.lua");
            assert!(PANEL.contains(&format!("\"{name}\"")), "{name} has no widget in options.lua");
        }
    }
}