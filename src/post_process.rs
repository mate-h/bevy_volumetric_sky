use bevy::{
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    log,
    pbr::{GpuLights, LightMeta, ViewLightsUniformOffset, ViewShadowBindings},
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_asset::RenderAssets,
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{binding_types::*, *},
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
        view::{ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
        RenderApp,
    },
};

use crate::{AtmosphereResources, AtmosphereSettings};

#[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
pub struct PostProcessSettings {
    pub show: f32,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct PostProcessLabel;

pub struct PostProcessPlugin;

impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<PostProcessSettings>::default(),
            UniformComponentPlugin::<PostProcessSettings>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<PostProcessNode>>(Core3d, PostProcessLabel)
            .add_render_graph_edges(
                Core3d,
                (Node3d::EndMainPass, PostProcessLabel, Node3d::Tonemapping),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<PostProcessPipeline>();
    }
}

#[derive(Default)]
struct PostProcessNode;

impl ViewNode for PostProcessNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static PostProcessSettings,
        &'static DynamicUniformIndex<PostProcessSettings>,
        &'static ViewUniformOffset,
        &'static DynamicUniformIndex<AtmosphereSettings>,
        &'static ViewShadowBindings,
        &'static ViewLightsUniformOffset,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (
            view_target,
            depth_texture,
            _post_process_settings,
            settings_index,
            view_uniform_offset,
            atmosphere_settings_index,
            view_shadows,
            lights_uniform_offset,
        ): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let atmosphere = world.resource::<AtmosphereResources>();
        let post_process_pipeline = world.resource::<PostProcessPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();
        let gpu_images = world.resource::<RenderAssets<GpuImage>>();
        let atmosphere_settings_uniforms =
            world.resource::<ComponentUniforms<AtmosphereSettings>>();
        let light_meta = world.resource::<LightMeta>();

        let Some(light_binding) = light_meta.view_gpu_lights.binding() else {
            log::error!("Light binding not found");
            return Ok(());
        };

        let Some(atmosphere_settings_binding) = atmosphere_settings_uniforms.binding() else {
            log::error!("Atmosphere settings binding not found");
            return Ok(());
        };

        let Some(transmittance_texture) = gpu_images.get(&atmosphere.transmittance_texture) else {
            log::error!("Transmittance texture not found");
            return Ok(());
        };

        let Some(multiple_scattering_texture) =
            gpu_images.get(&atmosphere.multiple_scattering_texture)
        else {
            log::error!("Multiple scattering texture not found");
            return Ok(());
        };

        let Some(cloud_texture) = gpu_images.get(&atmosphere.cloud_texture) else {
            log::error!("Cloud texture not found");
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(post_process_pipeline.pipeline_id)
        else {
            // log::error!("Post process pipeline not found");
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<PostProcessSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            log::error!("Settings binding not found");
            return Ok(());
        };

        // Get the view uniform binding
        let Some(view_binding) = view_uniforms.uniforms.binding() else {
            log::error!("View binding not found");
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "post_process_bind_group",
            &post_process_pipeline.layout,
            &BindGroupEntries::sequential((
                // atmosphere bindings
                atmosphere_settings_binding.clone(),
                &transmittance_texture.texture_view,
                &post_process_pipeline.sampler,
                &multiple_scattering_texture.texture_view,
                &post_process_pipeline.sampler,
                &cloud_texture.texture_view,
                &post_process_pipeline.sampler,
                // view binding
                view_binding.clone(),
                // output texture and globals
                post_process.source,
                depth_texture.view(),
                &post_process_pipeline.sampler,
                settings_binding.clone(),
            )),
        );
        let shadow_bind_group = render_context.render_device().create_bind_group(
            "post_process_shadow_bind_group",
            &post_process_pipeline.shadow_layout,
            &BindGroupEntries::sequential((
                &view_shadows.directional_light_depth_texture_view,
                &post_process_pipeline.comparison_sampler,
                light_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(
            0,
            &bind_group,
            &[
                atmosphere_settings_index.index(),
                view_uniform_offset.offset,
                settings_index.index(),
            ],
        );
        render_pass.set_bind_group(1, &shadow_bind_group, &[lights_uniform_offset.offset]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
struct PostProcessPipeline {
    layout: BindGroupLayout,
    shadow_layout: BindGroupLayout,
    sampler: Sampler,
    comparison_sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for PostProcessPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "post_process_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // atmosphere bindings
                    uniform_buffer::<AtmosphereSettings>(true),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    // View uniform
                    uniform_buffer::<ViewUniform>(true),
                    // Color texture
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // Depth texture
                    texture_2d_multisampled(TextureSampleType::Depth),
                    // The sampler
                    sampler(SamplerBindingType::Filtering),
                    // The settings uniform
                    uniform_buffer::<PostProcessSettings>(true),
                ),
            ),
        );

        let shadow_layout = render_device.create_bind_group_layout(
            "post_process_shadow_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d_array(TextureSampleType::Depth),
                    sampler(SamplerBindingType::Comparison),
                    uniform_buffer::<GpuLights>(true),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });

        let comparison_sampler = render_device.create_sampler(&SamplerDescriptor {
            compare: Some(CompareFunction::Less),
            ..default()
        });

        let shader = world
            .resource::<AssetServer>()
            .load("shaders/post_process.wgsl");

        let pipeline_id =
            world
                .resource_mut::<PipelineCache>()
                .queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("post_process_pipeline".into()),
                    layout: vec![layout.clone(), shadow_layout.clone()],
                    vertex: fullscreen_shader_vertex_state(),
                    fragment: Some(FragmentState {
                        shader,
                        shader_defs: vec![],
                        entry_point: "fragment".into(),
                        targets: vec![Some(ColorTargetState {
                            format: TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    push_constant_ranges: vec![],
                    zero_initialize_workgroup_memory: false,
                });

        Self {
            layout,
            shadow_layout,
            sampler,
            comparison_sampler,
            pipeline_id,
        }
    }
}
