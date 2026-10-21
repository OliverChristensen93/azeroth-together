//! Lightning in a thunderstorm: now and then a strike flashes the world's light, sky and fog
//! toward a cold white in two or three quick pulses, and the screen with them (the post pass
//! reads [`Flash`]); its thunder follows after the light has had time to travel, from that side
//! of the sky, quieter the farther off. 1.12 has no thunder of its own, so it is the Blasted Lands
//! lightning bolt kit (SoundEntries 4936: full volume to 50 yards, gone at 100). Strikes are each
//! player's own: the weather is shared, its lightning is not.

use bevy::prelude::*;

use benilla_world::lighting::{LightingConsumeSet, LightingResolveSet, WowLighting};
use benilla_world::view::WorldCamera;
use benilla_world::weather::{WeatherKind, WeatherState};

use crate::sound::{Cue, SoundCue};

use super::bolt::BoltSpawn;
use super::{AtmosphereSettings, Flash, SkyRequest};

/// The Blasted Lands lightning bolt kit: three cracks with their rumble.
const THUNDER_KIT: u32 = 4936;
/// The cold white a flash pulls the light toward.
const FLASH_TINT: [f32; 3] = [0.82, 0.87, 1.0];
/// How far a full flash pulls the light toward it.
const FLASH_LIGHT: f32 = 0.6;
/// A pulse's decay time, seconds.
const PULSE_DECAY: f32 = 0.07;
/// The rain must be at least this dense before the first strike.
const MIN_RAIN: f32 = 0.3;

/// One strike: its pulses (offset seconds, strength) and its thunder.
#[derive(Clone, Debug, PartialEq)]
struct Strike {
    at: f64,
    pulses: Vec<(f32, f32)>,
    /// Seconds from the flash to the thunder: a near strike cracks at once.
    delay: f32,
    /// Bearing of the strike, radians about the vertical.
    bearing: f32,
    thundered: bool,
}

/// The storm's running state.
#[derive(Resource)]
struct Storm {
    /// Seconds until the next strike.
    next: f32,
    strikes: Vec<Strike>,
    rng: u64,
}

impl Default for Storm {
    fn default() -> Self {
        Self {
            next: 4.0,
            strikes: Vec::new(),
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }
}

impl Storm {
    /// A uniform 0..1 (xorshift64*).
    fn roll(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Seconds to the next strike in a storm of `storm` (0..1): about every 16 s in a light one,
    /// every 6 s in the worst.
    fn gap(&mut self, storm: f32) -> f32 {
        let mean = 16.0 - 10.0 * storm.clamp(0.0, 1.0);
        mean * (0.4 + 1.2 * self.roll())
    }

    /// A new strike at `now`: two or three pulses, a near one brighter. Three in four fall
    /// within a third of a turn of `facing` (the camera's bearing), so most bolts are seen.
    fn strike(&mut self, now: f64, facing: Option<f32>) -> Strike {
        let delay = 0.3 + 4.2 * self.roll();
        let near = 1.0 - delay / 4.5;
        let peak = 0.45 + 0.55 * near;
        let mut pulses = vec![(0.0, peak * 0.7), (0.08 + 0.05 * self.roll(), peak)];
        if self.roll() < 0.5 {
            pulses.push((0.25 + 0.15 * self.roll(), peak * 0.6));
        }
        Strike {
            at: now,
            pulses,
            delay,
            bearing: match facing {
                Some(f) if self.roll() < 0.75 => f + (self.roll() - 0.5) * 2.1,
                _ => self.roll() * std::f32::consts::TAU,
            },
            thundered: false,
        }
    }
}

/// A strike's flash `t` seconds after it, 0..1.
fn flash_of(strike: &Strike, t: f32) -> f32 {
    flash_of_pulses(&strike.pulses, t)
}

/// The flash of `pulses` (offset seconds, strength) `t` seconds in, 0..1.
pub(super) fn flash_of_pulses(pulses: &[(f32, f32)], t: f32) -> f32 {
    pulses
        .iter()
        .filter(|&&(o, _)| t >= o)
        .map(|&(o, s)| s * (-(t - o) / PULSE_DECAY).exp())
        .fold(0.0, f32::max)
}

/// Where the thunder of a strike `delay` seconds off comes from, from the camera at `eye`.
fn thunder_at(eye: Vec3, bearing: f32, delay: f32) -> Vec3 {
    let dist = 20.0 + 15.0 * delay;
    eye + Vec3::new(bearing.cos() * dist, 10.0, bearing.sin() * dist)
}

fn strike_and_flash(
    time: Res<Time>,
    settings: Res<AtmosphereSettings>,
    req: Res<SkyRequest>,
    weather: Option<Res<WeatherState>>,
    cam: Query<&GlobalTransform, With<WorldCamera>>,
    mut storm: ResMut<Storm>,
    mut flash: ResMut<Flash>,
    mut lighting: ResMut<WowLighting>,
    mut cues: MessageWriter<SoundCue>,
    mut bolts: MessageWriter<BoltSpawn>,
) {
    let now = time.elapsed_secs_f64();
    let raining = weather
        .as_ref()
        .is_some_and(|w| w.effect_kind == WeatherKind::Rain && w.effect_density >= MIN_RAIN);
    let stormy = settings.lightning
        && req.active
        && req.weather == WeatherKind::Rain
        && req.storm > 0.0
        && raining;
    if stormy {
        storm.next -= time.delta_secs();
        if storm.next <= 0.0 {
            let facing = cam.iter().next().map(|t| {
                let f = t.forward();
                f.z.atan2(f.x)
            });
            let s = storm.strike(now, facing);
            bolts.write(BoltSpawn {
                bearing: s.bearing,
                delay: s.delay,
                pulses: s.pulses.clone(),
                seed: storm.rng ^ (now.to_bits()),
            });
            storm.strikes.push(s);
            storm.next = storm.gap(req.storm);
        }
    }
    let eye = cam.iter().next().map(GlobalTransform::translation);
    let mut k: f32 = 0.0;
    for s in &mut storm.strikes {
        let t = (now - s.at) as f32;
        k = k.max(flash_of(s, t));
        if !s.thundered && t >= s.delay {
            s.thundered = true;
            if let Some(eye) = eye {
                cues.write(SoundCue {
                    sound: Cue::Kit(THUNDER_KIT),
                    at: Some(thunder_at(eye, s.bearing, s.delay)),
                });
            }
        }
    }
    storm.strikes.retain(|s| !s.thundered || ((now - s.at) as f32) < 1.0);
    flash.0 = k;
    if k > 0.001 {
        lighting.flash(k * FLASH_LIGHT, FLASH_TINT);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Storm>().add_systems(
        Update,
        strike_and_flash
            .after(LightingResolveSet)
            .before(LightingConsumeSet),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strike_flashes_in_pulses_and_fades() {
        let mut storm = Storm::default();
        let s = storm.strike(0.0, None);
        assert!(s.pulses.len() >= 2);
        let peak = s.pulses.iter().map(|p| p.1).fold(0.0, f32::max);
        let brightest = s
            .pulses
            .iter()
            .map(|&(o, _)| flash_of(&s, o))
            .fold(0.0, f32::max);
        assert!((brightest - peak).abs() < 1e-3);
        assert!(flash_of(&s, 1.5) < 0.01);
        assert_eq!(flash_of(&s, -0.1), 0.0);
    }

    #[test]
    fn strikes_come_sooner_in_a_worse_storm_and_thunder_stays_in_earshot() {
        let mut storm = Storm::default();
        let n = 400;
        let light: f32 = (0..n).map(|_| storm.gap(0.0)).sum::<f32>() / n as f32;
        let heavy: f32 = (0..n).map(|_| storm.gap(1.0)).sum::<f32>() / n as f32;
        assert!(heavy < light * 0.5, "{heavy} vs {light}");
        for _ in 0..200 {
            let s = storm.strike(0.0, Some(1.0));
            let d = thunder_at(Vec3::ZERO, s.bearing, s.delay).xz().length();
            assert!((20.0..100.0).contains(&d), "{d}");
        }
    }
}
