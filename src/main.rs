use bevy::{
    asset::AssetMetaCheck,
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderRef},
};
use bevy_debug_grid::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_volumetric_sky::VolumetricSkyPlugin;
use wasm_bindgen::prelude::*;
mod shader_reload;
use shader_reload::ShaderReloadPlugin;
mod atmosphere;
mod compute;

#[wasm_bindgen]
pub fn run() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..Default::default()
            }),
            DebugGridPlugin::with_floor_grid(),
            PanOrbitCameraPlugin,
            FrameTimeDiagnosticsPlugin::default(),
            VolumetricSkyPlugin,
            ShaderReloadPlugin,
        ))
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

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
