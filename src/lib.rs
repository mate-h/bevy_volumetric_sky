use bevy::{
    core_pipeline::core_3d::Camera3dDepthTextureUsage, prelude::*,
    render::render_resource::TextureUsages,
};
use bevy_egui::EguiPlugin;

mod atmosphere;
mod compute;
mod gui;
mod post_process;

pub struct VolumetricSkyPlugin;

impl Plugin for VolumetricSkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            compute::ComputeShaderPlugin,
            EguiPlugin,
            post_process::PostProcessPlugin,
            gui::GuiPlugin,
        ))
        .add_systems(Startup, setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(Color::WHITE)),
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
        PostProcessSettings {
            show_depth: 1.0,
            ..default()
        },
    ));
}

// Re-export main components and types
pub use atmosphere::{AtmosphereResources, AtmosphereSettings};
use bevy_panorbit_camera::PanOrbitCamera;
pub use post_process::PostProcessSettings;
