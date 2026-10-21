//! Atmosphere: weather, lightning and thunder, sun shafts and the time of day, as one plugin a
//! game adds and steers through two resources, [`SkyRequest`] (the weather and time of day it
//! wants now) and [`AtmosphereSettings`] (which effects are on). It knows bevy and benilla-world
//! only, not any one game, so Azeroth Together and Nightfall add it the same way;
//! `docs/ATMOSPHERE.md` is the port guide.
//!
//! - Weather: the engine's own 1.12 rain, snow and sandstorm ([`benilla_world::weather`]), fed a
//!   `WeatherMessage` whenever the request changes, as a server's `SMSG_WEATHER` would.
//! - Lightning ([`lightning`]): in a storm, flashes over the lighting and the screen, and thunder
//!   a moment later (the Blasted Lands bolt kit, the only thunder in 1.12's data).
//! - Shafts ([`shafts`]): screen-space sun (and faint moon) rays, a radial blur of the bright sky
//!   toward the sun, plus the storm's cool, drained grade and the flash, in one post pass.
//! - Clock ([`clock`]): the request's minute of the day into the engine's `WorldTime`.
//! - Bolts ([`bolt`]): each strike also draws a jagged bolt in the sky, mostly where you look.
//! - Fog: a low ground mist that pools and drifts (a zone's own, more in rain) and a light
//!   aerial haze, from the scene's depth in the post pass ([`post_node`]).
//! - Occlusion and polish: soft contact shadows from depth, a light sharpen and colour polish.
//! - Lens drops (opt-in): raindrops on the camera in rain, in the post pass.
//! - Wind ([`wind`]): gusts that lean the engine's rain and mist, harder in a storm.
//!
//! Techniques after Corfirean/modern-wow-renderer's ideas (no code taken; it carries no
//! licence): rays only from bright sky near the sun, the moon's rays at about a third of the
//! sun's, weather chosen per zone.

use bevy::prelude::*;

use benilla_world::weather::WeatherKind;

mod bolt;
mod clock;
mod lightning;
mod options;
mod post_node;
pub(crate) mod schedule;
mod shafts;
mod weather;
mod wind;

pub(crate) use options::toggle_panel;
pub(crate) use schedule::ZoneSky;

/// Which effects are on. A game may change it at any time; `WOW_ATMOS` sets where it starts:
/// `off` for none, or a comma list of effects to leave out (`-rays,-lightning`); `WOW_RAYS=<n>`,
/// `WOW_MIST=<n>` and `WOW_AO=<n>` scale the shafts, the fog and the occlusion (1 the default).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct AtmosphereSettings {
    pub weather: bool,
    pub lightning: bool,
    pub rays: bool,
    /// The storm grade (cooler, a little drained) in the post pass.
    pub grade: bool,
    /// Whether a game's schedule gives each zone its own, moving hour; off, it is midday.
    pub clock: bool,
    /// Height fog: low ground mist and aerial haze.
    pub mist: bool,
    /// Ambient occlusion from depth.
    pub ao: bool,
    /// The sharpen and colour polish.
    pub polish: bool,
    /// Raindrops on the camera. Off unless asked for (`+lens`): in playtest they covered the
    /// screen and cleared too slowly.
    pub lens: bool,
    /// Wind gusts in the weather.
    pub wind: bool,
    pub ray_strength: f32,
    /// Scales the fog's density (`WOW_MIST`, 1 the default).
    pub mist_strength: f32,
    /// Scales the occlusion (`WOW_AO`, 1 the default).
    pub ao_strength: f32,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            weather: true,
            lightning: true,
            rays: true,
            grade: true,
            clock: true,
            mist: true,
            ao: true,
            polish: true,
            lens: false,
            wind: true,
            ray_strength: 1.0,
            mist_strength: 1.0,
            ao_strength: 1.0,
        }
    }
}

impl AtmosphereSettings {
    /// Settings from a `WOW_ATMOS` value and a `WOW_RAYS` value.
    pub fn parse(atmos: Option<&str>, rays: Option<&str>) -> Self {
        let mut s = Self::default();
        if let Some(v) = atmos {
            for part in v.split(',').map(str::trim) {
                let (on, name) = match part.strip_prefix('-') {
                    Some(n) => (false, n),
                    None => (true, part.strip_prefix('+').unwrap_or(part)),
                };
                match name {
                    "off" => {
                        s.weather = false;
                        s.lightning = false;
                        s.rays = false;
                        s.grade = false;
                        s.clock = false;
                        s.mist = false;
                        s.ao = false;
                        s.polish = false;
                        s.lens = false;
                        s.wind = false;
                    }
                    "weather" => s.weather = on,
                    "lightning" => s.lightning = on,
                    "rays" => s.rays = on,
                    "grade" => s.grade = on,
                    "clock" => s.clock = on,
                    "mist" => s.mist = on,
                    "ao" => s.ao = on,
                    "polish" => s.polish = on,
                    "lens" => s.lens = on,
                    "wind" => s.wind = on,
                    _ => {}
                }
            }
        }
        if let Some(n) = rays.and_then(|r| r.trim().parse::<f32>().ok()) {
            s.ray_strength = n.max(0.0);
        }
        s
    }

    fn from_env() -> Self {
        let scale = |name: &str| {
            crate::webenv::var(name)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|n| n.max(0.0))
        };
        let mut s = Self::parse(
            crate::webenv::var("WOW_ATMOS").as_deref(),
            crate::webenv::var("WOW_RAYS").as_deref(),
        );
        s.mist_strength = scale("WOW_MIST").unwrap_or(1.0);
        s.ao_strength = scale("WOW_AO").unwrap_or(1.0);
        s
    }
}

/// What a game wants the sky to be now. The default is no request: the plugin leaves the
/// weather and the clock to whoever else drives them (a server, the debug panel).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SkyRequest {
    /// Rain, snow, sand or fine.
    pub weather: WeatherKind,
    /// How hard it falls, 0..1 (ignored when fine).
    pub intensity: f32,
    /// How stormy, 0..1: lightning comes only in rain with a storm above 0.
    pub storm: f32,
    /// The zone's own ground mist, 0..1 (rain adds to it).
    pub mist: f32,
    /// The minute of the day (0..1440) to draw, or `None` to leave the clock alone.
    pub minute: Option<f32>,
    /// Whether a game is steering at all; when it stops, the plugin clears its weather.
    pub active: bool,
}

/// The atmosphere's frame state the post pass draws: the lightning flash now, 0..1.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct Flash(pub f32);

/// The plugin. Add it once; it does nothing until a game writes an active [`SkyRequest`].
pub(crate) struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AtmosphereSettings::from_env())
            .init_resource::<SkyRequest>()
            .init_resource::<Flash>();
        weather::plugin(app);
        lightning::plugin(app);
        clock::plugin(app);
        shafts::plugin(app);
        bolt::plugin(app);
        wind::plugin(app);
        options::plugin(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_parse_off_minus_and_strength() {
        assert_eq!(AtmosphereSettings::parse(None, None), AtmosphereSettings::default());
        let off = AtmosphereSettings::parse(Some("off"), None);
        assert!(!off.weather && !off.lightning && !off.rays && !off.grade && !off.clock);
        assert!(!off.mist && !off.ao && !off.polish && !off.lens && !off.wind);
        let no_mist = AtmosphereSettings::parse(Some("-mist,-ao"), None);
        assert!(!no_mist.mist && !no_mist.ao && no_mist.polish && !no_mist.lens && no_mist.wind);
        let opt_in = AtmosphereSettings::parse(Some("+lens"), None);
        assert!(opt_in.lens);
        let some = AtmosphereSettings::parse(Some("-rays, -lightning"), Some("1.5"));
        assert!(some.weather && !some.lightning && !some.rays && some.grade);
        assert_eq!(some.ray_strength, 1.5);
        let back = AtmosphereSettings::parse(Some("off,rays"), None);
        assert!(back.rays && !back.weather);
    }
}
