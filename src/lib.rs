use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::{core_3d::Camera3dDepthTextureUsage, tonemapping::Tonemapping, Skybox},
    gltf::GltfMaterialName,
    log,
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
            TextureViewDimension,
        },
    },
};
use bevy_egui::EguiPlugin;

mod atmosphere;
mod compute;
mod gui;
mod post_process;

pub struct VolumetricSkyPlugin;

#[derive(Event)]
struct TransmittanceUpdate(Vec3);

#[derive(Component)]
pub struct Ground;

impl Plugin for VolumetricSkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            compute::ComputeShaderPlugin,
            EguiPlugin,
            post_process::PostProcessPlugin,
            gui::GuiPlugin,
        ))
        .add_event::<TransmittanceUpdate>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                update_sky_environment,
                update_sun_direction,
                handle_readback_events,
                find_plane_and_remove_shadow,
            ),
        );
    }
}

fn update_sky_environment(
    atmosphere_res: Res<AtmosphereResources>,
    mut query: Query<(&mut Skybox, &mut EnvironmentMapLight)>,
) {
    for (mut skybox, mut env_map) in query.iter_mut() {
        skybox.image = atmosphere_res.specular_radiance_cubemap.clone();
        env_map.diffuse_map = atmosphere_res.diffuse_irradiance_cubemap.clone();
        env_map.specular_map = atmosphere_res.specular_radiance_cubemap.clone();
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

fn create_ground_plane_mesh(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    let plane_mesh = Plane3d::new(Vec3::new(0.0, 1.0, 0.0), Vec2::new(1000.0, 1000.0))
        .mesh()
        .build();
    meshes.add(plane_mesh)
}

fn find_plane_and_remove_shadow(mut commands: Commands, query: Query<(Entity, &GltfMaterialName)>) {
    for (entity, name) in query.iter() {
        if name.0 == "Material" {
            commands.entity(entity).insert(NotShadowCaster);
            commands.entity(entity).insert(NotShadowReceiver);
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    images: ResMut<Assets<Image>>,
    atmosphere_res: Res<AtmosphereResources>,
) {
    let skybox_handle = create_placeholder_skybox_texture(images);

    // Spawn the GLTF scene
    commands.spawn((
        SceneRoot(
            asset_server
                // .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf")),
                .load(
                    GltfAssetLabel::Scene(0).from_asset("models/porsche_911_carrera_4s/scene.gltf"),
                ),
        ),
        Transform::from_xyz(0.0, 0.6667, 0.0),
    ));

    // spawn ground plane
    let ground_plane_mesh = create_ground_plane_mesh(&mut meshes);
    commands.spawn((
        Mesh3d(ground_plane_mesh),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, -0.001, 0.0),
        Visibility::Visible,
        Ground,
    ));

    // Spawn the directional light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI / 2., -PI / 4.)),
        CascadeShadowConfigBuilder::default().build(),
    ));

    // Readback component with an observer
    commands
        .spawn(Readback::texture(
            atmosphere_res.sun_transmittance_texture.clone(),
        ))
        .observe(
            |trigger: Trigger<ReadbackComplete>, mut events: EventWriter<TransmittanceUpdate>| {
                let transmittance: Vec<f32> = trigger.event().to_shader_type();
                let transmittance = Vec3::new(transmittance[0], transmittance[1], transmittance[2]);
                events.send(TransmittanceUpdate(transmittance));
            },
        );

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
            radius: Some(6.0),
            pitch: Some(6.0 * PI / 180.0),
            yaw: Some(22.0 * PI / 180.0),
            focus: Vec3::new(0.0, 0.5, 0.0),
            ..default()
        },
        AtmosphereSettings::default(),
        PostProcessSettings {
            show: 1.0,
            ..default()
        },
        Skybox {
            // not sure why 5000 multiplier is needed here but seems to result in the correct exposure
            brightness: 5000.0,
            image: skybox_handle.clone(),
            ..default()
        },
        EnvironmentMapLight {
            intensity: 5000.0,
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

// Update the directional light direction
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

// Update the directional light color
fn handle_readback_events(
    mut light_query: Query<&mut DirectionalLight>,
    mut transmittance_events: EventReader<TransmittanceUpdate>,
) {
    if let Ok(mut light) = light_query.get_single_mut() {
        for event in transmittance_events.read() {
            let transmittance = event.0;
            light.color = Color::srgb(transmittance.x, transmittance.y, transmittance.z);
        }
    }
}
