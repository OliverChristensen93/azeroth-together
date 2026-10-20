//! Wind for the weather: while it rains, snows or blows sand, a wind that veers slowly and comes
//! in gusts, harder the heavier the weather and hardest in a storm, fed to the engine's
//! `WeatherGust` so the rain slants and the mist streams with it. Fine weather is still air.

use bevy::prelude::*;

use benilla_world::weather::{WeatherGust, WeatherKind};

use super::{AtmosphereSettings, SkyRequest};

/// The steady wind for a weather, yards per second: a breeze in light rain, a gale in a storm.
fn base_speed(req: &SkyRequest) -> f32 {
    match req.weather {
        WeatherKind::Fine => 0.0,
        WeatherKind::Sand => 6.0 + 8.0 * req.intensity,
        _ => 1.5 + 4.0 * req.intensity + 9.0 * req.storm,
    }
}

/// Gustiness at `t` seconds, about 0.4..1.6: two slow waves and a quicker one.
fn gust(t: f32) -> f32 {
    let w = (t * 0.37).sin() * 0.5 + (t * 0.91 + 1.3).sin() * 0.3 + (t * 2.3 + 0.7).sin() * 0.2;
    1.0 + 0.6 * w
}

/// The wind's heading at `t` seconds, radians: it veers through about a right angle each way.
fn heading(t: f32) -> f32 {
    0.8 + (t * 0.021).sin() * 0.9 + (t * 0.053 + 2.0).sin() * 0.4
}

/// The wind wanted at `t`, Bevy space.
fn wind_at(req: &SkyRequest, t: f32) -> Vec3 {
    let speed = base_speed(req) * gust(t);
    let h = heading(t);
    Vec3::new(h.cos(), 0.0, h.sin()) * speed
}

fn drive(
    time: Res<Time>,
    settings: Res<AtmosphereSettings>,
    req: Res<SkyRequest>,
    mut gust_res: ResMut<WeatherGust>,
) {
    let want = if settings.wind && req.active {
        wind_at(&req, time.elapsed_secs())
    } else {
        Vec3::ZERO
    };
    // Eased over about two seconds, so a new weather's wind rises rather than snaps.
    let k = 1.0 - (-time.delta_secs() * 0.5).exp();
    let next = gust_res.wind + (want - gust_res.wind) * k;
    if (next - gust_res.wind).length_squared() > 1e-8 {
        gust_res.wind = next;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<WeatherGust>().add_systems(Update, drive);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(weather: WeatherKind, intensity: f32, storm: f32) -> SkyRequest {
        SkyRequest { weather, intensity, storm, active: true, ..Default::default() }
    }

    #[test]
    fn still_air_when_fine_and_a_gale_in_a_storm() {
        assert_eq!(wind_at(&req(WeatherKind::Fine, 0.0, 0.0), 10.0), Vec3::ZERO);
        let light = base_speed(&req(WeatherKind::Rain, 0.3, 0.0));
        let storm = base_speed(&req(WeatherKind::Rain, 0.9, 1.0));
        assert!(storm > light * 3.0, "{storm} vs {light}");
    }

    #[test]
    fn gusts_stay_in_range_and_the_wind_stays_flat() {
        for i in 0..2000 {
            let t = i as f32 * 0.37;
            let g = gust(t);
            assert!((0.39..=1.61).contains(&g), "{g}");
            assert_eq!(wind_at(&req(WeatherKind::Rain, 1.0, 1.0), t).y, 0.0);
        }
    }
}
