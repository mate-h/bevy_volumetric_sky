use std::borrow::Cow;

use bevy::{
    log,
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
        },
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        globals::{GlobalsBuffer, GlobalsUniform},
        render_asset::{RenderAssetUsages, RenderAssets},
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
        RenderApp,
    },
};

use binding_types::{texture_storage_2d, uniform_buffer};

const SHADER_ASSET_PATH: &str = "shaders/compute_shader.wgsl";
const SIZE: (u32, u32) = (256, 256);
const WORKGROUP_SIZE: u32 = 8;

#[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
pub struct ComputeShaderSettings {
    pub value: f32,
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let initial_data = vec![0u8; (SIZE.0 * SIZE.1 * 16) as usize];
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE.0,
            height: SIZE.1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &initial_data,
        TextureFormat::Rgba32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let image_handle = images.add(image);
    commands.insert_resource(ComputedTexture {
        texture: image_handle,
    });
    commands.spawn(ComputeShaderSettings { value: 1.0 });
}

#[derive(Resource, Clone, ExtractResource)]
pub struct ComputedTexture {
    pub texture: Handle<Image>,
}

pub struct ComputeShaderPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ComputeShaderLabel;

impl Plugin for ComputeShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup).add_plugins((
            ExtractResourcePlugin::<ComputedTexture>::default(),
            ExtractComponentPlugin::<ComputeShaderSettings>::default(),
            UniformComponentPlugin::<ComputeShaderSettings>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);

        // Add node to render graph
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(ComputeShaderLabel, ComputeNode::default());
        render_graph.add_node_edge(ComputeShaderLabel, bevy::render::graph::CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<ComputeShaderPipeline>();
    }
}

#[derive(Resource)]
struct ComputeShaderPipeline {
    bind_group_layout: BindGroupLayout,
    pipeline: CachedComputePipelineId,
}

impl FromWorld for ComputeShaderPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let bind_group_layout = render_device.create_bind_group_layout(
            "compute_shader_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    uniform_buffer::<GlobalsUniform>(false),
                    texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
                    uniform_buffer::<ComputeShaderSettings>(true),
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

        ComputeShaderPipeline {
            bind_group_layout,
            pipeline,
        }
    }
}

enum ComputeState {
    Loading,
    Ready,
}

struct ComputeNode {
    state: ComputeState,
}

impl Default for ComputeNode {
    fn default() -> Self {
        Self {
            state: ComputeState::Loading,
        }
    }
}

impl render_graph::Node for ComputeNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<ComputeShaderPipeline>();
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
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        if let ComputeState::Ready = self.state {
            let pipeline = world.resource::<ComputeShaderPipeline>();
            let pipeline_cache = world.resource::<PipelineCache>();

            // Bind group setup
            let gpu_images = world.resource::<RenderAssets<GpuImage>>();
            let computed_texture = world.resource::<ComputedTexture>();
            let globals_buffer = world.resource::<GlobalsBuffer>();
            let settings_uniforms = world.resource::<ComponentUniforms<ComputeShaderSettings>>();
            let Some(settings_binding) = settings_uniforms.binding() else {
                return Ok(());
            };

            let Some(view) = gpu_images.get(&computed_texture.texture) else {
                log::error!("Computed texture not found");
                return Ok(());
            };

            let bind_group = render_context.render_device().create_bind_group(
                "compute_shader_bind_group",
                &pipeline.bind_group_layout,
                &BindGroupEntries::sequential((
                    &globals_buffer.buffer,
                    &view.texture_view,
                    settings_binding.clone(),
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
            pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
        }
        Ok(())
    }
}
