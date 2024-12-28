use std::borrow::Cow;

use bevy::{
    asset::load_internal_asset,
    log,
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, ExtractComponentPlugin, UniformComponentPlugin},
        extract_resource::ExtractResourcePlugin,
        globals::{GlobalsBuffer, GlobalsUniform},
        render_asset::{RenderAssetUsages, RenderAssets},
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
        RenderApp,
    },
};

use binding_types::*;

use crate::atmosphere::{AtmosphereResources, AtmosphereSettings};

const SHADER_ASSET_PATH: &str = "shaders/compute_shader.wgsl";
const SIZE: (u32, u32) = (256, 64);
const WORKGROUP_SIZE: u32 = 8;
pub const ATMOSPHERE_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(13871298374012);

pub struct ComputeShaderPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ComputeShaderLabel;

impl Plugin for ComputeShaderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            ATMOSPHERE_SHADER_HANDLE,
            "../assets/shaders/atmosphere.wgsl",
            Shader::from_wgsl
        );

        app.add_systems(PreStartup, setup_compute_shader)
            .add_plugins((
                ExtractResourcePlugin::<AtmosphereResources>::default(),
                ExtractComponentPlugin::<AtmosphereSettings>::default(),
                UniformComponentPlugin::<AtmosphereSettings>::default(),
            ));

        let render_app = app.sub_app_mut(RenderApp);

        // Add nodes to render graph
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(ComputeShaderLabel, TransmittanceNode::default());
        // render_graph.add_node(
        //     ComputeShaderLabel::MultipleScattering,
        //     MultipleScatteringNode::default(),
        // );

        // Add dependencies
        // render_graph.add_node_edge(
        //     ComputeShaderLabel::Transmittance,
        //     ComputeShaderLabel::MultipleScattering,
        // );
        render_graph.add_node_edge(ComputeShaderLabel, bevy::render::graph::CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<TransmittancePipeline>();
        // render_app.init_resource::<MultipleScatteringPipeline>();
    }
}

fn setup_compute_shader(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Create transmittance texture
    let mut image = Image::new(
        Extent3d {
            width: 256,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 256 * 64 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    let transmittance_texture = images.add(image);

    // Create multiple scattering texture
    let multiple_scattering_texture = images.add(Image::new(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 32 * 32 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    ));

    let cloud_texture = images.add(Image::new(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 32,
        },
        TextureDimension::D3,
        bytemuck::cast_slice(&vec![0f32; 32 * 32 * 32 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    ));

    let placeholder = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 1 * 1 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    ));

    commands.insert_resource(AtmosphereResources {
        transmittance_texture,
        multiple_scattering_texture,
        cloud_texture,
        placeholder,
    });

    commands.spawn(AtmosphereSettings::default());
}

// Create separate pipeline resources for each pass
#[derive(Resource)]
struct TransmittancePipeline {
    bind_group_layout: BindGroupLayout,
    pipeline: CachedComputePipelineId,
    sampler: Sampler,
}

impl FromWorld for TransmittancePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let bind_group_layout = render_device.create_bind_group_layout(
            "compute_shader_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    uniform_buffer::<GlobalsUniform>(false),
                    texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
                    uniform_buffer::<AtmosphereSettings>(true),
                    // Add dummy bindings for later stages
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );

        let shader = world.load_asset(SHADER_ASSET_PATH);

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("simple_pipeline".into()),
            layout: vec![bind_group_layout.clone()],
            push_constant_ranges: Vec::new(),
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("main"),
            zero_initialize_workgroup_memory: false,
        });

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        TransmittancePipeline {
            bind_group_layout,
            pipeline,
            sampler,
        }
    }
}

// #[derive(Resource)]
// struct MultipleScatteringPipeline {
//     bind_group_layout: BindGroupLayout,
//     pipeline: CachedComputePipelineId,
// }

enum ComputeState {
    Loading,
    Ready,
}

struct TransmittanceNode {
    state: ComputeState,
}

impl Default for TransmittanceNode {
    fn default() -> Self {
        Self {
            state: ComputeState::Loading,
        }
    }
}

impl Node for TransmittanceNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<TransmittancePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        match self.state {
            ComputeState::Loading => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.pipeline)
                {
                    self.state = ComputeState::Ready;
                }
            }
            ComputeState::Ready => {}
        }
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        if let ComputeState::Ready = self.state {
            let pipeline = world.resource::<TransmittancePipeline>();
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

            let bind_group = render_context.render_device().create_bind_group(
                "compute_shader_bind_group",
                &pipeline.bind_group_layout,
                &BindGroupEntries::sequential((
                    &globals_buffer.buffer,
                    &transmittance_texture.texture_view,
                    settings_binding.clone(),
                    &placeholder_texture.texture_view,
                    &pipeline.sampler,
                    &multiple_scattering_texture.texture_view,
                    &pipeline.sampler,
                    &cloud_texture.texture_view,
                    &pipeline.sampler,
                )),
            );

            let compute_pipeline = pipeline_cache
                .get_compute_pipeline(pipeline.pipeline)
                .unwrap();

            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor::default());

            pass.set_pipeline(compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[0]);

            // Add validation for workgroup calculations
            let workgroup_x = SIZE.0 / WORKGROUP_SIZE;
            let workgroup_y = SIZE.1 / WORKGROUP_SIZE;
            pass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        } else {
            // log::warn!("TransmittanceNode::run - Not in ready state");
        }
        Ok(())
    }
}
