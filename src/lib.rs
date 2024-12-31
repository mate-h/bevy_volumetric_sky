use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::{core_3d::Camera3dDepthTextureUsage, Skybox},
    log,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
        TextureViewDimension,
    },
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
            // post_process::PostProcessPlugin,
            gui::GuiPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, update_sky_environment);
    }
}

fn update_sky_environment(
    atmosphere_res: Res<AtmosphereResources>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&AtmosphereSettings, &mut Skybox, &mut EnvironmentMapLight)>,
) {
    for (atmosphere, mut skybox, mut env_map) in query.iter_mut() {
        skybox.image = atmosphere_res.diffuse_irradiance_cubemap.clone();
        env_map.diffuse_map = atmosphere_res.diffuse_irradiance_cubemap.clone();
        env_map.specular_map = atmosphere_res.diffuse_irradiance_cubemap.clone();
    }
}

fn create_placeholder_skybox_texture(mut images: ResMut<Assets<Image>>) -> Handle<Image> {
    // Create a 1x1x6 cubemap texture
    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6, // 6 faces for cubemap
        },
        TextureDimension::D2,
        &[0, 0, 0, 255], // Black color with full alpha
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::all(),
    );

    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    images.add(image)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    images: ResMut<Assets<Image>>,
) {
    let skybox_handle = create_placeholder_skybox_texture(images);

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
        AtmosphereSettings::default(),
        PostProcessSettings {
            show_depth: 1.0,
            ..default()
        },
        Skybox {
            brightness: 1000.0,
            image: skybox_handle.clone(),
            ..default()
        },
        EnvironmentMapLight {
            intensity: 1000.0,
            diffuse_map: skybox_handle.clone(),
            specular_map: skybox_handle,
            ..default()
        },
    ));
}

// Re-export main components and types
pub use atmosphere::{AtmosphereResources, AtmosphereSettings};
use bevy_panorbit_camera::PanOrbitCamera;
pub use post_process::PostProcessSettings;
