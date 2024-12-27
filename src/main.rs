use bevy::{
    asset::AssetMetaCheck,
    core_pipeline::core_3d::Camera3dDepthTextureUsage,
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderRef, TextureUsages},
    render::view::Visibility,
};
use bevy_debug_grid::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use wasm_bindgen::prelude::*;
mod shader_reload;
use shader_reload::ShaderReloadPlugin;
mod compute;
use bevy_egui::EguiPlugin;
use compute::{ComputeShaderPlugin, ComputedTexture};
mod gui;
use gui::GuiPlugin;
mod post_process;
use post_process::{PostProcessPlugin, PostProcessSettings};

#[wasm_bindgen]
pub fn run() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..Default::default()
            }),
            DebugGridPlugin::without_floor_grid(), 
            MaterialPlugin::<CustomMaterial>::default(),
            PanOrbitCameraPlugin,
            FrameTimeDiagnosticsPlugin::default(),
            ShaderReloadPlugin,
            ComputeShaderPlugin,
            EguiPlugin,
            GuiPlugin,
            PostProcessPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    computed_texture: Res<ComputedTexture>,
) {
    // Floor grid
    commands.spawn((
        Grid {
            spacing: 10.0_f32,
            count: 16,
            ..default()
        },
        SubGrid::default(),
        GridAxis::new_rgb(),
        Transform::default(),
        Visibility::Visible,
    ));
    
    commands.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(CustomMaterial {
            computed_texture: computed_texture.texture.clone(),
        })),
    ));

    commands.spawn((
        Transform::from_translation(Vec3::new(0.0, 1.5, 5.0)),
        Camera3d {
            depth_texture_usages: Camera3dDepthTextureUsage::from(
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            ),
            ..default()
        },
        PanOrbitCamera::default(),
        PostProcessSettings::default(),
    ));
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CustomMaterial {
    #[texture(1)]
    #[sampler(2)]
    computed_texture: Handle<Image>,
}

impl Material for CustomMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/animate_shader.wgsl".into()
    }
}
