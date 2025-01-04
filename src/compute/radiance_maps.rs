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

#[derive(Resource)]
pub struct RadianceMapPipeline {
    bind_group_layout: BindGroupLayout,
    specular_radiance_pipeline: CachedComputePipelineId,
    diffuse_radiance_pipeline: CachedComputePipelineId,
    sampler: Sampler,
}

impl FromWorld for RadianceMapPipeline {
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
                    // specular texture for diffuse computation
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    // output texture and globals
                    uniform_buffer::<GlobalsUniform>(false),
                    texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
                ),
            ),
        );

        let shader = world.load_asset("shaders/cubemap.wgsl");

        let pipeline_cache = world.resource::<PipelineCache>();

        let specular_radiance_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("radiance_map_pipeline".into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: Vec::new(),
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Cow::from("specular_radiance"),
                zero_initialize_workgroup_memory: false,
            });

        let diffuse_radiance_pipeline =
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some("diffuse_radiance_pipeline".into()),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: Vec::new(),
                shader,
                shader_defs: vec![],
                entry_point: Cow::from("diffuse_radiance"),
                zero_initialize_workgroup_memory: false,
            });

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });

        RadianceMapPipeline {
            bind_group_layout,
            specular_radiance_pipeline,
            diffuse_radiance_pipeline,
            sampler,
        }
    }
}

enum ComputeState {
    Loading,
    Ready,
}

pub struct RadianceMapNode {
    state: ComputeState,
}

impl Default for RadianceMapNode {
    fn default() -> Self {
        Self {
            state: ComputeState::Loading,
        }
    }
}

impl Node for RadianceMapNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<RadianceMapPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        if let ComputeState::Loading = self.state {
            if let CachedPipelineState::Ok(_) =
                pipeline_cache.get_compute_pipeline_state(pipeline.specular_radiance_pipeline)
            {
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
            let pipeline = world.resource::<RadianceMapPipeline>();
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

            let Some(placeholder_texture) = gpu_images.get(&atmosphere.placeholder) else {
                log::error!("Placeholder texture not found");
                return Ok(());
            };

            let Some(transmittance_texture) = gpu_images.get(&atmosphere.transmittance_texture)
            else {
                log::error!("Transmittance texture not found");
                return Ok(());
            };

            let Some(diffuse_irradiance_compute_target) =
                gpu_images.get(&atmosphere.diffuse_irradiance_compute_target)
            else {
                log::error!("Diffuse irradiance map not found");
                return Ok(());
            };

            let Some(specular_radiance_compute_target) =
                gpu_images.get(&atmosphere.specular_radiance_compute_target)
            else {
                log::error!("Specular radiance map not found");
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

            // First compute specular radiance
            {
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
                        // specular texture
                        &placeholder_texture.texture_view,
                        &pipeline.sampler,
                        // output texture and globals
                        &globals_buffer.buffer,
                        &specular_radiance_compute_target.texture_view,
                    )),
                );

                let compute_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.specular_radiance_pipeline)
                    .unwrap();

                let mut pass = render_context
                    .command_encoder()
                    .begin_compute_pass(&ComputePassDescriptor::default());

                pass.set_pipeline(compute_pipeline);
                pass.set_bind_group(0, &bind_group, &[0]);
                pass.dispatch_workgroups(
                    specular_radiance_compute_target.size.x / 8,
                    specular_radiance_compute_target.size.y / 8,
                    1,
                );
            }

            // Then compute diffuse radiance
            {
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
                        // specular texture
                        &specular_radiance_compute_target.texture_view,
                        &pipeline.sampler,
                        // output texture and globals
                        &globals_buffer.buffer,
                        &diffuse_irradiance_compute_target.texture_view,
                    )),
                );

                let compute_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.diffuse_radiance_pipeline)
                    .unwrap();

                let mut pass = render_context
                    .command_encoder()
                    .begin_compute_pass(&ComputePassDescriptor::default());

                pass.set_pipeline(compute_pipeline);
                pass.set_bind_group(0, &bind_group, &[0]);
                pass.dispatch_workgroups(
                    diffuse_irradiance_compute_target.size.x / 8,
                    diffuse_irradiance_compute_target.size.y / 8,
                    1,
                );
            }

            let diffuse_cubemap = gpu_images
                .get(&atmosphere.diffuse_irradiance_cubemap)
                .unwrap();
            let specular_cubemap = gpu_images
                .get(&atmosphere.specular_radiance_cubemap)
                .unwrap();

            // Copy each face
            for face in 0..6 {
                render_context.command_encoder().copy_texture_to_texture(
                    ImageCopyTexture {
                        texture: &diffuse_irradiance_compute_target.texture,
                        mip_level: 0,
                        origin: Origin3d {
                            x: 0,
                            y: face * 256, // Offset for each face in the 2D texture
                            z: 0,
                        },
                        aspect: TextureAspect::All,
                    },
                    ImageCopyTexture {
                        texture: &diffuse_cubemap.texture,
                        mip_level: 0,
                        origin: Origin3d {
                            x: 0,
                            y: 0,
                            z: face,
                        }, // Each array layer is a face
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );

                render_context.command_encoder().copy_texture_to_texture(
                    ImageCopyTexture {
                        texture: &specular_radiance_compute_target.texture,
                        mip_level: 0,
                        origin: Origin3d {
                            x: 0,
                            y: face * 256, // Offset for each face in the 2D texture
                            z: 0,
                        },
                        aspect: TextureAspect::All,
                    },
                    ImageCopyTexture {
                        texture: &specular_cubemap.texture,
                        mip_level: 0,
                        origin: Origin3d {
                            x: 0,
                            y: 0,
                            z: face,
                        }, // Each array layer is a face
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        Ok(())
    }
}
