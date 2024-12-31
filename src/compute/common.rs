use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_graph::RenderLabel,
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
            TextureViewDimension,
        },
    },
};

use crate::atmosphere::AtmosphereResources;

// Shared traits and enums
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub enum ComputeLabel {
    TransmittanceLUT,
    MultipleScatteringLUT,
    SkyViewLUT,
    CloudVolume,
    SpecularRadiance,
    DiffuseRadiance,
}

pub fn setup_atmosphere_resources(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
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
    let mut image = Image::new(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 32 * 32 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    let multiple_scattering_texture = images.add(image);

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

    // Create compute target (2D storage texture)
    let mut compute_target = Image::new(
        Extent3d {
            width: 256,
            height: 256 * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![1f32; 256 * 256 * 6 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );
    compute_target.texture_descriptor.usage =
        TextureUsages::COPY_SRC | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    // Create cubemap target
    let mut cubemap = Image::new(
        Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 256 * 256 * 6 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );
    cubemap.texture_descriptor.usage = TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;
    cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    let compute_target_handle = images.add(compute_target);
    let cubemap_handle = images.add(cubemap);

    commands.insert_resource(AtmosphereResources {
        transmittance_texture,
        multiple_scattering_texture,
        cloud_texture,
        placeholder,
        diffuse_irradiance_compute_target: compute_target_handle,
        diffuse_irradiance_cubemap: cubemap_handle,
        // environment_diffuse_cubemap,
        // specular_irradiance_map,
    });
}
