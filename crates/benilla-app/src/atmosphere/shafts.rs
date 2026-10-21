//! The atmosphere's post pass (`atmos_post.wgsl`, run by [`super::post_node`]) on the world
//! camera: polish, ambient occlusion, height fog and ground mist, sun shafts, the storm grade,
//! the lightning flash and the lens drops. This fills its uniform each frame; the camera carries
//! it only while one of its effects is on, and then keeps a depth buffer the pass can read.
//!
//! Screen-space shafts need the sun on or near the screen: they come when the camera looks
//! toward a low sun, which the hour of a zone's sky decides. The moon gives fainter ones.

use bevy::camera::Camera3dDepthTextureUsage;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::{ShaderType, TextureUsages};

use benilla_world::lighting::{LightingConsumeSet, WowLighting};
use benilla_world::view::WorldCamera;
use benilla_world::weather::{storm_blend, WeatherGust, WeatherKind, WeatherState};
use benilla_world::world_unit::ViewerUnit;

use super::{AtmosphereSettings, Flash, SkyRequest};

/// The pass's uniform: the shader's `Params`, nine `vec4`s.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType, Default, Debug, PartialEq)]
pub(super) struct AtmosPost {
    /// xy the light in uv, z the shafts' strength, w the bright-sky threshold.
    light: Vec4,
    /// rgb the shafts' colour, w the aspect.
    color: Vec4,
    /// x flash, y storm grade, z reach, w decay per tap.
    mood: Vec4,
    /// x rain on the lens, y seconds.
    lens: Vec4,
    /// rgb the fog's colour, w the ground mist's density at its floor.
    fog_color: Vec4,
    /// x the mist's floor, y its falloff, z the haze's density, w seconds.
    fog: Vec4,
    /// x occlusion strength, y its radius, zw the mist's drift.
    ao: Vec4,
    /// xyz camera to sun, w the fog's glow toward it.
    sun: Vec4,
    /// x polish amount, y sharpness, z vibrance, w contrast.
    polish: Vec4,
}

/// The ground mist's density per yard at its floor at full mist, and how fast it thins upward.
const MIST_DENSITY: f32 = 0.05;
const MIST_FALLOFF: f32 = 0.45;
/// The aerial haze's density per yard, clear and in full rain.
const HAZE: f32 = 0.0008;
const RAIN_HAZE: f32 = 0.004;
/// How much rain adds to the ground mist.
const RAIN_MIST: f32 = 0.35;
/// Occlusion strength and radius (yards) at `WOW_AO=1`.
const AO_STRENGTH: f32 = 0.55;
const AO_RADIUS: f32 = 1.6;
/// The polish: sharpness, vibrance, S-curve.
const SHARPNESS: f32 = 0.4;
const VIBRANCE: f32 = 0.15;
const CONTRAST: f32 = 0.1;

/// The base strength of the shafts at `WOW_RAYS=1`.
const SHAFTS: f32 = 0.7;
/// The moon's shafts against the sun's.
const MOON_SHAFTS: f32 = 0.35;
/// How much of the way to the light a pixel's taps reach, and each tap's decay.
const REACH: f32 = 0.85;
const DECAY: f32 = 0.965;
/// The luminance (gamma) that counts as bright sky: by day, and for the moon by night.
const SUN_THRESHOLD: f32 = 0.35;
const MOON_THRESHOLD: f32 = 0.25;
const MOON_COLOR: [f32; 3] = [0.62, 0.72, 0.92];

/// The light the shafts come from, as the frame sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Light {
    /// Camera to light, unit.
    dir: Vec3,
    /// 0..1 before the camera and weather are counted.
    strength: f32,
    color: [f32; 3],
    threshold: f32,
}

/// The sun by day, the moon by night (whichever is up and stronger).
fn light_of(l: &WowLighting) -> Option<Light> {
    let above = |d: Vec3| {
        let t = ((d.y + 0.02) / 0.1).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let sun = l.sun_day() * above(l.celestial_dir());
    let moon = l.moon_night() * above(l.moon_dir()) * MOON_SHAFTS;
    if sun.max(moon) <= 0.001 {
        return None;
    }
    Some(if sun >= moon {
        let d = l.diffuse;
        let top = d[0].max(d[1]).max(d[2]).max(1e-3);
        // The sunlight's hue at full brightness, warmed a touch.
        let color = [
            (d[0] / top).min(1.0),
            (d[1] / top * 0.95).min(1.0),
            (d[2] / top * 0.82).min(1.0),
        ];
        Light {
            dir: l.celestial_dir().normalize_or_zero(),
            strength: sun,
            color,
            threshold: SUN_THRESHOLD,
        }
    } else {
        Light {
            dir: l.moon_dir().normalize_or_zero(),
            strength: moon,
            color: MOON_COLOR,
            threshold: MOON_THRESHOLD,
        }
    })
}

/// How much of the shafts' strength survives the light's place on screen: `ndc` its projected
/// position, `facing` the cosine between the view and the light.
fn screen_fade(ndc: Vec2, facing: f32) -> f32 {
    let front = ((facing - 0.05) / 0.35).clamp(0.0, 1.0);
    let edge = (ndc.x.abs().max(ndc.y.abs()) - 1.0).max(0.0);
    let on = 1.0 - (edge / 0.8).clamp(0.0, 1.0);
    front * on
}

/// Whether any of the pass's effects is on.
fn wanted(s: &AtmosphereSettings) -> bool {
    s.rays || s.grade || s.lightning || s.lens || s.mist || s.ao || s.polish
}

/// The depth usages the pass needs: drawn into, and sampled.
fn sampled_depth() -> TextureUsages {
    TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING
}

fn attach(
    mut commands: Commands,
    settings: Res<AtmosphereSettings>,
    with: Query<Entity, (With<WorldCamera>, With<Camera3d>, With<AtmosPost>)>,
    mut without: Query<(Entity, &mut Camera3d), (With<WorldCamera>, Without<AtmosPost>)>,
) {
    if wanted(&settings) {
        for (e, mut cam) in &mut without {
            let usage = TextureUsages::from_bits_truncate(cam.depth_texture_usages.0);
            if !usage.contains(TextureUsages::TEXTURE_BINDING) {
                cam.depth_texture_usages = Camera3dDepthTextureUsage::from(usage | sampled_depth());
            }
            commands.entity(e).insert(AtmosPost::default());
        }
    } else {
        for e in &with {
            commands.entity(e).remove::<AtmosPost>();
        }
    }
}

fn drive(
    settings: Res<AtmosphereSettings>,
    req: Res<SkyRequest>,
    flash: Res<Flash>,
    lighting: Res<WowLighting>,
    weather: Option<Res<WeatherState>>,
    gust: Option<Res<WeatherGust>>,
    focus: Query<&GlobalTransform, (With<ViewerUnit>, Without<WorldCamera>)>,
    mut cams: Query<(&Camera, &GlobalTransform, &mut AtmosPost), With<WorldCamera>>,
    time: Res<Time<Real>>,
    mut next_dump: Local<f64>,
) {
    // `WOW_ATMOS_DUMP=1`: the pass's inputs once a second, to aim a test camera at the sun.
    let dump = {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("WOW_ATMOS_DUMP").is_some())
            && time.elapsed_secs_f64() >= *next_dump
    };
    if dump {
        *next_dump = time.elapsed_secs_f64() + 1.0;
    }
    let (wet, storm, rain) = weather
        .map(|w| {
            let rain = if w.effect_kind == WeatherKind::Rain { w.effect_density } else { 0.0 };
            (w.effect_density, storm_blend(w.sky_density), rain)
        })
        .unwrap_or((0.0, 0.0, 0.0));
    for (camera, cam_gt, mut post) in &mut cams {
        let aspect = camera
            .logical_viewport_size()
            .map(|s| s.x / s.y.max(1.0))
            .unwrap_or(16.0 / 9.0);
        let mut next = AtmosPost {
            light: Vec4::new(0.5, 0.5, 0.0, 1.0),
            color: Vec4::new(1.0, 1.0, 1.0, aspect),
            mood: Vec4::new(0.0, 0.0, REACH, DECAY),
            lens: Vec4::ZERO,
            ..default()
        };
        let secs = (time.elapsed_secs_f64() % 1000.0) as f32;
        // The light the shafts and the fog's glow take.
        let light = light_of(&lighting);
        if let Some(l) = light {
            let [r, g, b] = l.color;
            next.color = Vec4::new(r, g, b, aspect);
        }
        if settings.polish {
            next.polish = Vec4::new(1.0, SHARPNESS, VIBRANCE, CONTRAST);
        }
        if settings.ao {
            next.ao.x = (AO_STRENGTH * settings.ao_strength).min(1.0);
            next.ao.y = AO_RADIUS;
        }
        if settings.mist && req.active {
            // The mist lies on the ground the player stands on.
            let floor = focus
                .iter()
                .next()
                .map_or(cam_gt.translation().y - 2.0, |t| t.translation().y);
            let thick = (req.mist + RAIN_MIST * rain).clamp(0.0, 1.0);
            let fog = Vec3::from_array(lighting.fog_color).lerp(Vec3::new(0.78, 0.8, 0.84), 0.3);
            next.fog_color = fog.extend(MIST_DENSITY * thick * settings.mist_strength);
            next.fog = Vec4::new(
                floor - 0.5,
                MIST_FALLOFF,
                (HAZE + RAIN_HAZE * wet) * settings.mist_strength,
                secs,
            );
            let drift = gust.as_ref().map_or(Vec3::ZERO, |g| g.wind) * 0.01
                + Vec3::new(0.02, 0.0, 0.01);
            next.ao.z = drift.x;
            next.ao.w = drift.z;
            let sun = lighting.celestial_dir().normalize_or_zero();
            let glow =
                lighting.sun_day() * (1.0 - storm) * ((sun.y + 0.1) * 5.0).clamp(0.0, 1.0);
            next.sun = sun.extend(glow);
        }
        if settings.rays {
            if let Some(light) = light {
                let eye = cam_gt.translation();
                let facing = cam_gt.forward().dot(light.dir);
                if let Some(ndc) = camera.world_to_ndc(cam_gt, eye + light.dir * 500.0) {
                    let fade = screen_fade(ndc.truncate(), facing);
                    // Rain and an overcast sky hide the sun.
                    let clear = (1.0 - 0.9 * wet) * (1.0 - storm);
                    let strength = SHAFTS * settings.ray_strength * light.strength * fade * clear;
                    if strength > 0.002 {
                        let uv = Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
                        next.light = Vec4::new(uv.x, uv.y, strength, light.threshold);
                        let [r, g, b] = light.color;
                        next.color = Vec4::new(r, g, b, aspect);
                    }
                }
            }
        }
        if settings.grade && req.active {
            next.mood.y = storm * (0.35 + 0.4 * req.storm);
        }
        if settings.lightning {
            next.mood.x = flash.0;
        }
        if settings.lens && req.active && rain > 0.0 {
            next.lens = Vec4::new(rain, secs, 0.0, 0.0);
        }
        if dump {
            let sun = lighting.celestial_dir();
            let fwd = cam_gt.forward();
            info!(
                "atmos: sun dir {:.2},{:.2},{:.2} (yaw {:.0} deg, up {:.0} deg) day {:.2} | cam fwd yaw {:.0} pitch {:.0} | shafts uv {:.2},{:.2} strength {:.3} | wet {:.2} storm {:.2} flash {:.2}",
                sun.x, sun.y, sun.z,
                sun.x.atan2(-sun.z).to_degrees(), sun.y.asin().to_degrees(),
                lighting.sun_day(),
                fwd.x.atan2(-fwd.z).to_degrees(), fwd.y.asin().to_degrees(),
                next.light.x, next.light.y, next.light.z, wet, storm, next.mood.x,
            );
        }
        if *post != next {
            *post = next;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "atmos_post.wgsl");
    super::post_node::plugin(app);
    app.add_systems(Update, (attach, drive.in_set(LightingConsumeSet)).chain());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shafts_fade_off_screen_and_behind() {
        assert_eq!(screen_fade(Vec2::ZERO, 1.0), 1.0);
        assert!(screen_fade(Vec2::new(1.4, 0.0), 1.0) > 0.0);
        assert_eq!(screen_fade(Vec2::new(2.0, 0.0), 1.0), 0.0);
        assert_eq!(screen_fade(Vec2::ZERO, 0.0), 0.0);
    }

    #[test]
    fn no_light_when_neither_sun_nor_moon_is_up() {
        let mut l = WowLighting::default();
        l.diffuse = [1.0, 0.8, 0.6];
        assert!(light_of(&l).is_none());
    }
}