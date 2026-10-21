//! The atmosphere's render pass: Bevy's `FullscreenMaterial` node (bevy_core_pipeline,
//! MIT/Apache-2.0) grown to read the view's depth and its `View` uniform, which the fog, the
//! occlusion and the depth-masked shafts need. It runs in the slot the survivors grade uses,
//! after FFXGlow's filter passes and before the world's post-processing ends, on every camera
//! that carries an [`AtmosPost`]. A camera whose depth cannot be sampled is skipped for that frame.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::FullscreenShader;
use bevy::ecs::query::QueryItem;
use bevy::image::BevyDefault;
use bevy::prelude::*;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponentPlugin, UniformComponentPlugin,
};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, texture_depth_2d_multisampled, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::{
    Msaa, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{RenderApp, RenderStartup};

use super::shafts::AtmosPost;

/// The pass's label in the 3D graph.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(super) struct AtmosPassLabel;

/// The shader, embedded by [`super::shafts::plugin`].
const SHADER: &str = "embedded://benilla_app/atmosphere/atmos_post.wgsl";

/// The pipelines: by multisampled depth (index 1) and by HDR target (second index 1).
#[derive(Resource)]
struct AtmosPipelines {
    layouts: [BindGroupLayoutDescriptor; 2],
    sampler: Sampler,
    ids: [[CachedRenderPipelineId; 2]; 2],
}

fn init(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = |multisampled: bool| {
        let depth = if multisampled {
            texture_depth_2d_multisampled()
        } else {
            texture_depth_2d()
        };
        BindGroupLayoutDescriptor::new(
            "atmos_post_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<AtmosPost>(true),
                    depth,
                    uniform_buffer::<ViewUniform>(true),
                ),
            ),
        )
    };
    let layouts = [layout(false), layout(true)];
    let shader = asset_server.load(SHADER);
    let mut ids = [[CachedRenderPipelineId::INVALID; 2]; 2];
    for (ms, layout) in layouts.iter().enumerate() {
        for (hdr, format) in [TextureFormat::bevy_default(), ViewTarget::TEXTURE_FORMAT_HDR]
            .into_iter()
            .enumerate()
        {
            let shader_defs = if ms == 1 {
                vec!["MULTISAMPLED".into()]
            } else {
                vec![]
            };
            ids[ms][hdr] = cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("atmos_post_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shader.clone(),
                    shader_defs,
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            });
        }
    }
    commands.insert_resource(AtmosPipelines {
        layouts,
        sampler: render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        }),
        ids,
    });
}

#[derive(Default)]
struct AtmosNode;

impl ViewNode for AtmosNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static DynamicUniformIndex<AtmosPost>,
        &'static ViewDepthTexture,
        &'static ViewUniformOffset,
        &'static Msaa,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (target, settings, depth, view_offset, msaa): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // The camera asks for a sampleable depth; until that texture exists, skip the pass.
        if !depth.texture.usage().contains(TextureUsages::TEXTURE_BINDING) {
            return Ok(());
        }
        let pipelines = world.resource::<AtmosPipelines>();
        let cache = world.resource::<PipelineCache>();
        let ms = usize::from(*msaa != Msaa::Off);
        let Some(pipeline) = cache.get_render_pipeline(pipelines.ids[ms][usize::from(target.is_hdr())])
        else {
            return Ok(());
        };
        let Some(params) = world.resource::<ComponentUniforms<AtmosPost>>().uniforms().binding()
        else {
            return Ok(());
        };
        let Some(view) = world.resource::<ViewUniforms>().uniforms.binding() else {
            return Ok(());
        };
        let post = target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "atmos_post_bind_group",
            &cache.get_bind_group_layout(&pipelines.layouts[ms]),
            &BindGroupEntries::sequential((
                post.source,
                &pipelines.sampler,
                params,
                depth.view(),
                view,
            )),
        );
        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("atmos_post_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[settings.index(), view_offset.offset]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

/// Registers the uniform's extraction, the pipelines and the graph node.
pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        ExtractComponentPlugin::<AtmosPost>::default(),
        UniformComponentPlugin::<AtmosPost>::default(),
    ));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .add_systems(RenderStartup, init)
        .add_render_graph_node::<ViewNodeRunner<AtmosNode>>(Core3d, AtmosPassLabel)
        .add_render_graph_edges(
            Core3d,
            (
                Node3d::Tonemapping,
                AtmosPassLabel,
                Node3d::EndMainPassPostProcessing,
            ),
        );
}