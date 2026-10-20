//! The run's sky through the shared atmosphere plugin (`crate::atmosphere`): each zone's weather
//! and hour from `content/atmosphere.ron`, scheduled by the run's `sky_seed` and clock, so every
//! player of a co-op run sees the same rain at the same time. Outside a run the hour is 13:00, as
//! it always was.
//!
//! Test switches: `TOGETHER_WEATHER=fine|rain|snow|sand|storm` forces the weather for the whole
//! run, `TOGETHER_HOUR=<0..24>` the hour it starts at. `WOW_ATMOS` (`off`, `-rays`, ...) and
//! `WOW_RAYS=<n>` are the plugin's own.

use std::collections::HashMap;
use std::sync::OnceLock;

use bevy::prelude::*;

use benilla_world::weather::WeatherKind;

use crate::atmosphere::{schedule::sky_at, AtmosphereSettings, SkyRequest, ZoneSky};

use super::{Phase, Sim};

/// The hour outside a run (and of a zone with no sky of its own), as minutes.
const MENU_MINUTE: f32 = 780.0;

static SKIES: OnceLock<HashMap<String, ZoneSky>> = OnceLock::new();

/// Read `atmosphere.ron`'s zone skies; a bad file is reported and leaves every zone at noon.
pub(super) fn load(text: &str) {
    let skies = match ron::from_str::<HashMap<String, ZoneSky>>(text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("together: atmosphere.ron: {e}");
            HashMap::new()
        }
    };
    let _ = SKIES.set(skies);
}

/// A zone's sky, or the plain one.
fn zone_sky(key: &str) -> ZoneSky {
    SKIES
        .get()
        .and_then(|s| s.get(key))
        .cloned()
        .unwrap_or(ZoneSky {
            minute: MENU_MINUTE,
            ..Default::default()
        })
}

/// The test switches, read once.
struct Overrides {
    weather: Option<(WeatherKind, f32, f32)>,
    minute: Option<f32>,
}

fn overrides() -> &'static Overrides {
    static O: OnceLock<Overrides> = OnceLock::new();
    O.get_or_init(|| Overrides {
        weather: std::env::var("TOGETHER_WEATHER")
            .ok()
            .and_then(|v| parse_weather(&v)),
        minute: std::env::var("TOGETHER_HOUR")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(|h| (h * 60.0).rem_euclid(1440.0)),
    })
}

/// `TOGETHER_WEATHER`'s value as (kind, intensity, storm).
fn parse_weather(v: &str) -> Option<(WeatherKind, f32, f32)> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "fine" | "clear" => (WeatherKind::Fine, 0.0, 0.0),
        "rain" => (WeatherKind::Rain, 0.6, 0.0),
        "snow" => (WeatherKind::Snow, 0.6, 0.0),
        "sand" => (WeatherKind::Sand, 0.7, 0.0),
        "storm" => (WeatherKind::Rain, 0.9, 0.9),
        _ => return None,
    })
}

/// The sky this frame: the zone's schedule during a run, the plain hour outside one.
pub(super) fn drive(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    settings: Res<AtmosphereSettings>,
    mut req: ResMut<SkyRequest>,
) {
    let o = overrides();
    let mut next = match (phase.get(), sim) {
        (Phase::Playing, Some(sim)) => {
            let mut sky = zone_sky(&sim.world.zone().key);
            if let Some(m) = o.minute {
                sky.minute = m;
            }
            let mut r = sky_at(&sky, sim.world.sky_seed, sim.world.elapsed, &settings);
            if let (Some((kind, intensity, storm)), true) = (o.weather, settings.weather) {
                r.weather = kind;
                r.intensity = intensity;
                r.storm = storm;
            }
            r
        }
        _ => SkyRequest {
            minute: Some(o.minute.unwrap_or(MENU_MINUTE)),
            active: true,
            ..Default::default()
        },
    };
    // Whole minutes are enough for the light and keep the request from changing every frame.
    next.minute = next.minute.map(|m| (m * 4.0).round() / 4.0);
    if *req != next {
        *req = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_atmosphere_parses_and_names_real_zones() {
        let skies: HashMap<String, ZoneSky> =
            ron::from_str(include_str!("../../../../content/atmosphere.ron")).unwrap();
        let zones = include_str!("../../../../content/zones.ron");
        for (key, sky) in &skies {
            assert!(zones.contains(&format!("key: \"{key}\"")), "{key} is not a zone");
            assert!((0.0..1440.0).contains(&sky.minute), "{key}");
            assert!((0.0..=1.0).contains(&sky.wet), "{key}");
        }
        assert!(skies.contains_key("elwynn") && skies.contains_key("duskwood"));
    }

    #[test]
    fn weather_switch_values() {
        assert_eq!(parse_weather("Storm"), Some((WeatherKind::Rain, 0.9, 0.9)));
        assert_eq!(parse_weather("snow").unwrap().0, WeatherKind::Snow);
        assert_eq!(parse_weather("hail"), None);
    }
}