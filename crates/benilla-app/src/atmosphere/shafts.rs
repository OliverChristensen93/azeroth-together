//! The atmosphere's post pass (`atmos_post.wgsl`) on the world camera: sun shafts, the storm
//! grade and the lightning flash. Bevy's [`FullscreenMaterial`], in the slot the survivors grade
//! uses, between FFXGlow's filter passes and the end of the world's post-processing; the camera
//! carries it only while one of its effects is on.
//!
//! Screen-space shafts need the sun on or near the screen: they come when the camera looks
//! toward a low sun, which the hour of a zone's sky decides. The moon gives fainter ones.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{
    InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph,
};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use benilla_world::lighting::{LightingConsumeSet, WowLighting};
use benilla_world::view::WorldCamera;
use benilla_world::weather::{storm_blend, WeatherState};

use super::{AtmosphereSettings, Flash, SkyRequest};

/// The pass's uniform: the shader's three `vec4`s.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType, Default, Debug, PartialEq)]
pub(super) struct AtmosPost {
    /// xy the light in uv, z the shafts' strength, w the bright-sky threshold.
    light: Vec4,
    /// rgb the shafts' colour, w the aspect.
    color: Vec4,
    /// x flash, y storm grade, z reach, w decay per tap.
    mood: Vec4,
}

impl FullscreenMaterial for AtmosPost {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/atmosphere/atmos_post.wgsl".into()
    }

    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }

    /// After the glow's filter passes, before the world's post-processing ends.
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

/// The base strength of the shafts at `WOW_RAYS=1`.
const SHAFTS: f32 = 0.7;
/// The moon's shafts against the sun's.
const MOON_SHAFTS: f32 = 0.35;
/// How much of the way to the light a pixel's taps reach, and each tap's decay.
const REACH: f32 = 0.85;
const DECAY: f32 = 0.965;
/// The luminance (gamma) that counts as bright sky: by day, and for the moon by night.
const SUN_THRESHOLD: f32 = 0.5;
const MOON_THRESHOLD: f32 = 0.35;
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
    s.rays || s.grade || s.lightning
}

fn attach(
    mut commands: Commands,
    settings: Res<AtmosphereSettings>,
    with: Query<Entity, (With<WorldCamera>, With<Camera3d>, With<AtmosPost>)>,
    without: Query<Entity, (With<WorldCamera>, With<Camera3d>, Without<AtmosPost>)>,
) {
    if wanted(&settings) {
        for e in &without {
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
    let (wet, storm) = weather
        .map(|w| (w.effect_density, storm_blend(w.sky_density)))
        .unwrap_or((0.0, 0.0));
    for (camera, cam_gt, mut post) in &mut cams {
        let aspect = camera
            .logical_viewport_size()
            .map(|s| s.x / s.y.max(1.0))
            .unwrap_or(16.0 / 9.0);
        let mut next = AtmosPost {
            light: Vec4::new(0.5, 0.5, 0.0, 1.0),
            color: Vec4::new(1.0, 1.0, 1.0, aspect),
            mood: Vec4::new(0.0, 0.0, REACH, DECAY),
        };
        if settings.rays {
            if let Some(light) = light_of(&lighting) {
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
    app.add_plugins(FullscreenMaterialPlugin::<AtmosPost>::default())
        .add_systems(Update, (attach, drive.in_set(LightingConsumeSet)).chain());
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