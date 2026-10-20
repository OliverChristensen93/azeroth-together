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
//!
//! Techniques after Corfirean/modern-wow-renderer's ideas (no code taken; it carries no
//! licence): rays only from bright sky near the sun, the moon's rays at about a third of the
//! sun's, weather chosen per zone.

use bevy::prelude::*;

use benilla_world::weather::WeatherKind;

mod clock;
mod lightning;
pub(crate) mod schedule;
mod shafts;
mod weather;

pub(crate) use schedule::ZoneSky;

/// Which effects are on. A game may change it at any time; `WOW_ATMOS` sets where it starts:
/// `off` for none, or a comma list of effects to leave out (`-rays,-lightning`); `WOW_RAYS=<n>`
/// scales the shafts (1 is the default strength).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct AtmosphereSettings {
    pub weather: bool,
    pub lightning: bool,
    pub rays: bool,
    /// The storm grade (cooler, a little drained) in the post pass.
    pub grade: bool,
    /// Whether a game's schedule moves the time of day; off, it keeps its fixed hour.
    pub clock: bool,
    pub ray_strength: f32,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            weather: true,
            lightning: true,
            rays: true,
            grade: true,
            clock: true,
            ray_strength: 1.0,
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
                    }
                    "weather" => s.weather = on,
                    "lightning" => s.lightning = on,
                    "rays" => s.rays = on,
                    "grade" => s.grade = on,
                    "clock" => s.clock = on,
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
        Self::parse(
            crate::webenv::var("WOW_ATMOS").as_deref(),
            crate::webenv::var("WOW_RAYS").as_deref(),
        )
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
        let some = AtmosphereSettings::parse(Some("-rays, -lightning"), Some("1.5"));
        assert!(some.weather && !some.lightning && !some.rays && some.grade);
        assert_eq!(some.ray_strength, 1.5);
        let back = AtmosphereSettings::parse(Some("off,rays"), None);
        assert!(back.rays && !back.weather);
    }
}
