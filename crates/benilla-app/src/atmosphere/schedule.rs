//! A zone's sky over a run: its weather in spells and its time of day, as a pure function of a
//! shared seed and the run's clock, so every player of a co-op run sees the same sky with no
//! message of its own. A game reads a [`ZoneSky`] from its content and calls [`sky_at`] each frame.

use serde::Deserialize;

use benilla_world::weather::WeatherKind;

use super::{AtmosphereSettings, SkyRequest};

/// What falls in a zone's wet spells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
pub(crate) enum Precip {
    #[default]
    Rain,
    Snow,
    Sand,
}

/// One zone's sky, as content gives it. Every field has a default, so `()` is noon, always fine.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct ZoneSky {
    /// What falls.
    pub precip: Precip,
    /// The share of the run with something falling, 0..1.
    pub wet: f32,
    /// The chance a wet spell is a thunderstorm (rain only), 0..1.
    pub storms: f32,
    /// How long one spell of weather (wet or dry) lasts, seconds.
    pub spell: f32,
    /// How hard a wet spell falls, the range rolled from (0..1).
    pub intensity: (f32, f32),
    /// Seconds into the run before the first wet spell may come.
    pub dry_start: f32,
    /// The minute of the day (0..1440) the run starts at.
    pub minute: f32,
    /// Game minutes the clock moves per second of run; 0 holds the hour.
    pub day_speed: f32,
}

impl Default for ZoneSky {
    fn default() -> Self {
        Self {
            precip: Precip::Rain,
            wet: 0.0,
            storms: 0.0,
            spell: 120.0,
            intensity: (0.35, 0.9),
            dry_start: 0.0,
            minute: 780.0,
            day_speed: 0.0,
        }
    }
}

/// A 64-bit mix (splitmix64's finaliser).
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A uniform 0..1 from `seed`, a spell index and a stream.
fn unit(seed: u64, spell: u64, stream: u64) -> f32 {
    let h = mix(seed ^ mix(spell.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ stream));
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// The sky `t` seconds into a run seeded `seed`, under `settings`. The weather is fine while
/// `settings.weather` is off, and the hour holds while `settings.clock` is off.
pub(crate) fn sky_at(sky: &ZoneSky, seed: u64, t: f32, settings: &AtmosphereSettings) -> SkyRequest {
    let t = t.max(0.0);
    let minute = if settings.clock {
        (sky.minute + sky.day_speed * t).rem_euclid(1440.0)
    } else {
        sky.minute.rem_euclid(1440.0)
    };
    let mut req = SkyRequest {
        minute: Some(minute),
        active: true,
        ..Default::default()
    };
    if !settings.weather || sky.wet <= 0.0 || t < sky.dry_start {
        return req;
    }
    let index = (t / sky.spell.max(10.0)) as u64;
    if unit(seed, index, 1) >= sky.wet {
        return req;
    }
    let (lo, hi) = sky.intensity;
    req.intensity = (lo + (hi - lo) * unit(seed, index, 2)).clamp(0.0, 1.0);
    req.weather = match sky.precip {
        Precip::Rain => WeatherKind::Rain,
        Precip::Snow => WeatherKind::Snow,
        Precip::Sand => WeatherKind::Sand,
    };
    if sky.precip == Precip::Rain && unit(seed, index, 3) < sky.storms {
        // A storm rains hard.
        req.storm = 0.5 + 0.5 * unit(seed, index, 4);
        req.intensity = req.intensity.max(0.75);
    }
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wet(share: f32) -> ZoneSky {
        ZoneSky {
            wet: share,
            ..Default::default()
        }
    }

    #[test]
    fn the_same_seed_and_time_give_the_same_sky() {
        let s = AtmosphereSettings::default();
        let sky = ZoneSky { storms: 0.5, ..wet(0.5) };
        for t in [0.0, 50.0, 500.0, 1300.0] {
            assert_eq!(sky_at(&sky, 7, t, &s), sky_at(&sky, 7, t, &s));
        }
    }

    #[test]
    fn the_wet_share_is_about_what_the_zone_asks() {
        let s = AtmosphereSettings::default();
        for share in [0.0, 0.3, 0.6, 1.0] {
            let sky = wet(share);
            let n = 2000;
            let rainy = (0..n)
                .filter(|&i| sky_at(&sky, 99, i as f32 * sky.spell, &s).weather != WeatherKind::Fine)
                .count() as f32
                / n as f32;
            assert!((rainy - share).abs() < 0.05, "{share}: {rainy}");
        }
    }

    #[test]
    fn a_spell_holds_its_weather_through() {
        let s = AtmosphereSettings::default();
        let sky = ZoneSky { storms: 0.5, ..wet(0.5) };
        for k in 0..20 {
            let a = sky_at(&sky, 3, k as f32 * 120.0 + 1.0, &s);
            let b = sky_at(&sky, 3, k as f32 * 120.0 + 119.0, &s);
            assert_eq!((a.weather, a.intensity, a.storm), (b.weather, b.intensity, b.storm));
        }
    }

    #[test]
    fn storms_are_rain_and_hard() {
        let s = AtmosphereSettings::default();
        let rain = ZoneSky { storms: 1.0, ..wet(1.0) };
        let r = sky_at(&rain, 1, 10.0, &s);
        assert_eq!(r.weather, WeatherKind::Rain);
        assert!(r.storm >= 0.5 && r.intensity >= 0.75);
        let snow = ZoneSky { precip: Precip::Snow, ..rain };
        assert_eq!(sky_at(&snow, 1, 10.0, &s).storm, 0.0);
    }

    #[test]
    fn settings_off_keep_it_fine_and_the_hour_fixed() {
        let off = AtmosphereSettings { weather: false, clock: false, ..Default::default() };
        let sky = ZoneSky { minute: 1000.0, day_speed: 0.5, ..wet(1.0) };
        let r = sky_at(&sky, 1, 600.0, &off);
        assert_eq!(r.weather, WeatherKind::Fine);
        assert_eq!(r.minute, Some(1000.0));
        let on = sky_at(&sky, 1, 600.0, &AtmosphereSettings::default());
        assert_eq!(on.minute, Some(1300.0));
    }

    #[test]
    fn the_clock_wraps_past_midnight_and_dry_start_holds() {
        let sky = ZoneSky { minute: 1430.0, day_speed: 1.0, dry_start: 60.0, ..wet(1.0) };
        let s = AtmosphereSettings::default();
        assert_eq!(sky_at(&sky, 1, 20.0, &s).minute, Some(10.0));
        assert_eq!(sky_at(&sky, 1, 30.0, &s).weather, WeatherKind::Fine);
        assert_ne!(sky_at(&sky, 1, 61.0, &s).weather, WeatherKind::Fine);
    }

    #[test]
    fn an_empty_ron_is_the_default() {
        let parsed: ZoneSky = ron::from_str("()").unwrap();
        assert_eq!(parsed, ZoneSky::default());
    }
}
