use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css::GOLD,
    core_pipeline::{core_3d::Camera3dDepthTextureUsage, tonemapping::Tonemapping, Skybox},
    image::ImageLoaderSettings,
    log,
    pbr::{CascadeShadowConfigBuilder, ShadowFilteringMethod},
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
        .add_systems(Update, (update_sky_environment, update_sun_direction));
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

/// Generates a sphere.
fn create_sphere_mesh(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    // We're going to use normal maps, so make sure we've generated tangents, or
    // else the normal maps won't show up.

    let mut sphere_mesh = Sphere::new(1.0).mesh().build();
    sphere_mesh
        .generate_tangents()
        .expect("Failed to generate tangents");
    meshes.add(sphere_mesh)
}

fn create_ground_plane_mesh(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    let plane_mesh = Plane3d::new(Vec3::new(0.0, 1.0, 0.0), Vec2::new(10.0, 10.0))
        .mesh()
        .build();
    meshes.add(plane_mesh)
}

fn spawn_sphere(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    sphere_mesh: &Handle<Mesh>,
) {
    commands.spawn((
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            clearcoat: 1.0,
            clearcoat_perceptual_roughness: 0.3,
            clearcoat_normal_texture: Some(asset_server.load_with_settings(
                "ScratchedGold-Normal.png",
                |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
            )),
            metallic: 0.9,
            perceptual_roughness: 0.1,
            base_color: GOLD.into(),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(1.25)),
    ));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    images: ResMut<Assets<Image>>,
) {
    // let sphere_mesh = create_sphere_mesh(&mut meshes);
    // spawn_sphere(&mut commands, &mut materials, &asset_server, &sphere_mesh);

    let skybox_handle = create_placeholder_skybox_texture(images);
    // commands.spawn((
    //     Transform::from_xyz(0.0, 0.0, 0.0),
    //     Mesh3d(meshes.add(Cuboid::default())),
    //     MeshMaterial3d(materials.add(Color::WHITE)),
    // ));

    commands.spawn((
        SceneRoot(
            asset_server
                .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf")),
        ),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // spawn ground plane
    let ground_plane_mesh = create_ground_plane_mesh(&mut meshes);
    commands.spawn((
        Mesh3d(ground_plane_mesh),
        MeshMaterial3d(materials.add(Color::WHITE)),
    ));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI / 2., -PI / 4.)),
        CascadeShadowConfigBuilder::default().build(),
    ));

    commands.spawn((
        Transform::from_translation(Vec3::new(0.0, 1.5, 5.0)),
        Camera3d {
            depth_texture_usages: Camera3dDepthTextureUsage::from(
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            ),
            ..default()
        },
        Camera {
            hdr: true,
            ..default()
        },
        Tonemapping::AcesFitted,
        PanOrbitCamera {
            radius: Some(2.0),
            pitch: Some(-8.0 * PI / 180.0),
            yaw: Some(-22.0 * PI / 180.0),
            focus: Vec3::new(0.0, 0.5, 0.0),
            ..default()
        },
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

// Add this new system to update the directional light
fn update_sun_direction(
    atmosphere_query: Query<&AtmosphereSettings>,
    mut light_query: Query<&mut Transform, With<DirectionalLight>>,
) {
    if let Ok(atmosphere) = atmosphere_query.get_single() {
        if let Ok(mut light_transform) = light_query.get_single_mut() {
            let up = Vec3::Z;
            let sun_dir = Vec3::new(
                atmosphere.sun_position.x,
                atmosphere.sun_position.y,
                -atmosphere.sun_position.z,
            )
            .normalize();
            let rotation = Quat::from_rotation_arc(up, sun_dir);
            *light_transform = Transform::from_rotation(rotation);
        }
    }
}
