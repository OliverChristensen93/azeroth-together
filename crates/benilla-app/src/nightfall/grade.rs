//! Nightfall's grade on the world camera (`shaders/nightfall_grade.wgsl`): Bevy's
//! [`FullscreenMaterial`], placed where the survivors grade sits, between FFXGlow and the end of
//! the world's post-processing. It is on the camera only while Nightfall is up.
//! Its vignette tightens as the local hero's visibility drops (spec §10).

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{
    InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph,
};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use benilla_world::view::WorldCamera;

use crate::game_pick::GameMode;

/// The grade's uniform: eight floats, the shader's two `vec4`s.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub(super) struct NightGrade {
    pub amount: f32,
    pub desaturate: f32,
    pub vignette: f32,
    pub darken: f32,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    /// Where the vignette starts darkening, from the centre (0) to the corners (1).
    pub vignette_from: f32,
}

impl Default for NightGrade {
    fn default() -> Self {
        Self {
            amount: 1.0,
            desaturate: 0.35,
            vignette: 0.45,
            darken: 0.1,
            tint_r: 0.85,
            tint_g: 0.95,
            tint_b: 1.12,
            vignette_from: 0.35,
        }
    }
}

impl FullscreenMaterial for NightGrade {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/nightfall_grade.wgsl".into()
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

/// How fast the vignette follows the hero's visibility: this share of the gap a second.
const VIGNETTE_EASE: f32 = 3.0;

/// The vignette's (strength, inner edge) for a hero at `visibility` 0..100: `seen` at 100,
/// `hidden` at 0, straight between (spec §10: it tightens as the hero hides).
pub(super) fn vignette_for(visibility: f32, seen: (f32, f32), hidden: (f32, f32)) -> (f32, f32) {
    let t = (visibility / 100.0).clamp(0.0, 1.0);
    (
        hidden.0 + (seen.0 - hidden.0) * t,
        hidden.1 + (seen.1 - hidden.1) * t,
    )
}

/// `from` moved toward `to` by `rate` of the gap a second over `dt` seconds, never past it.
pub(super) fn ease(from: f32, to: f32, rate: f32, dt: f32) -> f32 {
    from + (to - from) * (rate * dt).min(1.0)
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(FullscreenMaterialPlugin::<NightGrade>::default())
        .add_systems(
            Update,
            (attach, follow_visibility).in_set(super::NightfallSet),
        )
        .add_systems(OnExit(GameMode::Nightfall), detach);
}

fn attach(
    mut commands: Commands,
    cams: Query<Entity, (With<WorldCamera>, With<Camera3d>, Without<NightGrade>)>,
) {
    for e in &cams {
        commands.entity(e).insert(NightGrade::default());
    }
}

fn detach(mut commands: Commands, cams: Query<Entity, With<NightGrade>>) {
    for e in &cams {
        commands.entity(e).remove::<NightGrade>();
    }
}

/// The vignette follows the local hero's visibility, eased so a change of stance never pops; at
/// the menu it stands as for a hero in plain sight.
fn follow_visibility(
    sim: Option<Res<super::run::Sim>>,
    real: Res<Time<Real>>,
    mut grades: Query<&mut NightGrade>,
) {
    let r = &super::content().rules.senses;
    let visibility = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .map_or(100.0, |h| h.visibility);
    let (strength, from) = vignette_for(visibility, r.vignette_seen, r.vignette_hidden);
    let dt = real.delta_secs();
    for mut g in &mut grades {
        g.vignette = ease(g.vignette, strength, VIGNETTE_EASE, dt);
        // An inner edge at the corners would leave the shader's smoothstep with no width.
        g.vignette_from = ease(g.vignette_from, from, VIGNETTE_EASE, dt).min(0.95);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vignette_tightens_as_the_hero_hides() {
        let seen = (0.45, 0.35);
        let hidden = (0.85, 0.1);
        assert_eq!(vignette_for(100.0, seen, hidden), seen);
        assert_eq!(vignette_for(0.0, seen, hidden), hidden);
        let (s, from) = vignette_for(50.0, seen, hidden);
        assert!((s - 0.65).abs() < 1e-6 && (from - 0.225).abs() < 1e-6);
        assert_eq!(vignette_for(150.0, seen, hidden), seen);
    }

    #[test]
    fn easing_closes_the_gap_without_passing_it() {
        assert!((ease(0.0, 1.0, 3.0, 0.1) - 0.3).abs() < 1e-6);
        assert_eq!(ease(0.0, 1.0, 3.0, 1.0), 1.0);
        assert_eq!(ease(0.5, 0.5, 3.0, 0.1), 0.5);
    }
}
