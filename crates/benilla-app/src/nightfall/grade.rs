//! Nightfall's grade on the world camera (`shaders/nightfall_grade.wgsl`): Bevy's
//! [`FullscreenMaterial`], placed where the survivors grade sits, between FFXGlow and the end of
//! the world's post-processing. It is on the camera only while Nightfall is up.

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
    pub unused: f32,
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
            unused: 0.0,
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

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(FullscreenMaterialPlugin::<NightGrade>::default())
        .add_systems(Update, attach.in_set(super::NightfallSet))
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
