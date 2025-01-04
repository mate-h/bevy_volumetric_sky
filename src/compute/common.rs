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
    RadianceMaps,
    SunTransmittance,
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

    // Create placeholder texture
    let mut placeholder = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    placeholder.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    let placeholder = images.add(placeholder);

    // Create compute target
    let mut diffuse_compute_target = Image::new(
        Extent3d {
            width: 256,
            height: 256 * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 256 * 256 * 6 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    diffuse_compute_target.texture_descriptor.usage = TextureUsages::COPY_DST
        | TextureUsages::STORAGE_BINDING
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC;

    // Create cubemap target
    let mut diffuse_cubemap = Image::new(
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
    diffuse_cubemap.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;
    diffuse_cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    let diffuse_compute_target_handle = images.add(diffuse_compute_target);
    let diffuse_cubemap_handle = images.add(diffuse_cubemap);

    // create the specular maps
    let mut specular_compute_target = Image::new(
        Extent3d {
            width: 256,
            height: 256 * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 256 * 256 * 6 * 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    specular_compute_target.texture_descriptor.usage = TextureUsages::COPY_DST
        | TextureUsages::STORAGE_BINDING
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC;

    let specular_compute_target_handle = images.add(specular_compute_target);

    let mut specular_cubemap = Image::new(
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
    specular_cubemap.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;
    specular_cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    let specular_cubemap_handle = images.add(specular_cubemap);

    // Create sun transmittance texture
    let mut sun_transmittance = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytemuck::cast_slice(&vec![0f32; 4]).to_vec(),
        TextureFormat::Rgba32Float,
        RenderAssetUsages::all(),
    );

    sun_transmittance.texture_descriptor.usage = TextureUsages::COPY_DST
        | TextureUsages::STORAGE_BINDING
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC;

    let sun_transmittance_handle = images.add(sun_transmittance);

    commands.insert_resource(AtmosphereResources {
        transmittance_texture,
        multiple_scattering_texture,
        cloud_texture,
        placeholder,
        diffuse_irradiance_compute_target: diffuse_compute_target_handle,
        diffuse_irradiance_cubemap: diffuse_cubemap_handle,
        specular_radiance_compute_target: specular_compute_target_handle,
        specular_radiance_cubemap: specular_cubemap_handle,
        sun_transmittance_texture: sun_transmittance_handle,
    });
}
