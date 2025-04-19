use std::borrow::Cow;

use bevy::{
    log,
    prelude::*,
    render::{
        extract_component::ComponentUniforms,
        globals::{GlobalsBuffer, GlobalsUniform},
        render_asset::RenderAssets,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
    },
};

use binding_types::*;

use crate::atmosphere::{AtmosphereResources, AtmosphereSettings};

use super::common::ComputeLabel;

#[derive(Resource)]
pub struct AtmosphereLutPipeline {
    bind_group_layout: BindGroupLayout,
    transmittance_lut_pipeline: CachedComputePipelineId,
    multiple_scattering_lut_pipeline: CachedComputePipelineId,
    sun_transmittance_lut_pipeline: CachedComputePipelineId,
    sampler: Sampler,
}

impl FromWorld for AtmosphereLutPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let bind_group_layout = render_device.create_bind_group_layout(
            "compute_shader_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // atmosphere bindings
                    uniform_buffer::<AtmosphereSettings>(true),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    // output texture and globals
                    uniform_buffer::<GlobalsUniform>(false),
                    texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
                ),
            ),
        );

        let shader = world.load_asset("shaders/compute_luts.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();
        let transmittance_lut_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("transmittance_lut_pipeline".into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: Vec::new(),
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Cow::from("transmittance"),
                zero_initialize_workgroup_memory: false,
            });

        let multiple_scattering_lut_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("multiple_scattering_lut_pipeline".into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: Vec::new(),
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Cow::from("multiple_scattering"),
                zero_initialize_workgroup_memory: false,
            });

        let sun_transmittance_lut_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("sun_transmittance_lut_pipeline".into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: Vec::new(),
                shader,
                shader_defs: vec![],
                entry_point: Cow::from("sun_transmittance"),
                zero_initialize_workgroup_memory: false,
            });

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });

        AtmosphereLutPipeline {
            bind_group_layout,
            transmittance_lut_pipeline,
            multiple_scattering_lut_pipeline,
            sun_transmittance_lut_pipeline,
            sampler,
        }
    }
}

enum ComputeState {
    Loading,
    Ready,
}

pub struct AtmosphereLutNode {
    state: ComputeState,
    pub label: ComputeLabel,
}

impl AtmosphereLutNode {
    pub fn new(label: ComputeLabel) -> Self {
        Self {
            label,
            state: ComputeState::Loading,
        }
    }
}

impl Node for AtmosphereLutNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<AtmosphereLutPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        if let ComputeState::Loading = self.state {
            let transmittance_ready = matches!(
                pipeline_cache.get_compute_pipeline_state(pipeline.transmittance_lut_pipeline),
                CachedPipelineState::Ok(_)
            );
            let multiple_scattering_ready = matches!(
                pipeline_cache
                    .get_compute_pipeline_state(pipeline.multiple_scattering_lut_pipeline),
                CachedPipelineState::Ok(_)
            );

            if transmittance_ready && multiple_scattering_ready {
                self.state = ComputeState::Ready;
            }
        }
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if let ComputeState::Ready = self.state {
            let pipeline = world.resource::<AtmosphereLutPipeline>();
            let pipeline_cache = world.resource::<PipelineCache>();

            // Bind group setup
            let gpu_images = world.resource::<RenderAssets<GpuImage>>();
            let atmosphere = world.resource::<AtmosphereResources>();
            let globals_buffer = world.resource::<GlobalsBuffer>();
            let settings_uniforms = world.resource::<ComponentUniforms<AtmosphereSettings>>();
            let Some(settings_binding) = settings_uniforms.binding() else {
                log::error!("Settings binding not found");
                return Ok(());
            };

            let Some(transmittance_texture) = gpu_images.get(&atmosphere.transmittance_texture)
            else {
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

            let Some(placeholder_texture) = gpu_images.get(&atmosphere.placeholder) else {
                log::error!("Placeholder texture not found");
                return Ok(());
            };

            let Some(sun_transmittance_texture) =
                gpu_images.get(&atmosphere.sun_transmittance_texture)
            else {
                log::error!("Sun transmittance texture not found");
                return Ok(());
            };

            // Select pipeline based on current state
            let (compute_pipeline, bind_group, workgroups) = match self.label {
                ComputeLabel::TransmittanceLUT => {
                    let compute_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.transmittance_lut_pipeline)
                        .unwrap();
                    let bind_group = render_context.render_device().create_bind_group(
                        "compute_shader_bind_group",
                        &pipeline.bind_group_layout,
                        &BindGroupEntries::sequential((
                            // atmosphere bindings
                            settings_binding.clone(),
                            &placeholder_texture.texture_view,
                            &pipeline.sampler,
                            &multiple_scattering_texture.texture_view,
                            &pipeline.sampler,
                            &cloud_texture.texture_view,
                            &pipeline.sampler,
                            // output texture and globals
                            &globals_buffer.buffer,
                            &transmittance_texture.texture_view,
                        )),
                    );
                    (compute_pipeline, bind_group, (256 / 8, 64 / 8, 1))
                }
                ComputeLabel::MultipleScatteringLUT => {
                    let compute_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.multiple_scattering_lut_pipeline)
                        .unwrap();
                    let bind_group = render_context.render_device().create_bind_group(
                        "compute_shader_bind_group",
                        &pipeline.bind_group_layout,
                        &BindGroupEntries::sequential((
                            // atmosphere bindings
                            settings_binding.clone(),
                            &transmittance_texture.texture_view,
                            &pipeline.sampler,
                            &placeholder_texture.texture_view,
                            &pipeline.sampler,
                            &cloud_texture.texture_view,
                            &pipeline.sampler,
                            // output texture and globals
                            &globals_buffer.buffer,
                            &multiple_scattering_texture.texture_view,
                        )),
                    );
                    (compute_pipeline, bind_group, (32, 32, 1))
                }
                ComputeLabel::SunTransmittance => {
                    let compute_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.sun_transmittance_lut_pipeline)
                        .unwrap();
                    let bind_group = render_context.render_device().create_bind_group(
                        "compute_shader_bind_group",
                        &pipeline.bind_group_layout,
                        &BindGroupEntries::sequential((
                            // atmosphere bindings
                            settings_binding.clone(),
                            &transmittance_texture.texture_view,
                            &pipeline.sampler,
                            &multiple_scattering_texture.texture_view,
                            &pipeline.sampler,
                            &cloud_texture.texture_view,
                            &pipeline.sampler,
                            // output texture and globals
                            &globals_buffer.buffer,
                            &sun_transmittance_texture.texture_view,
                        )),
                    );
                    (compute_pipeline, bind_group, (1, 1, 1))
                }
                _ => return Ok(()),
            };

            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor::default());

            pass.set_pipeline(compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[0]);
            pass.dispatch_workgroups(workgroups.0, workgroups.1, workgroups.2);
        } else {
            // log::warn!("ComputeNode::run - Not in ready state");
        }
        Ok(())
    }
}
