use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::*;

#[derive(Clone, Resource, ExtractResource)]
pub struct AtmosphereResources {
    pub transmittance_texture: Handle<Image>,
    pub multiple_scattering_texture: Handle<Image>,
    pub cloud_texture: Handle<Image>,
    pub placeholder: Handle<Image>,
}

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct AtmosphereSettings {
    pub sun_position: Vec3,
    pub eye_position: Vec3,
    pub sun_intensity: f32,
    pub rayleigh_scattering: Vec3,
    pub mie_scattering: Vec3,
    pub mie_g: f32,
    pub atmosphere_height: f32,
    pub cloud_coverage: f32,
    pub enable_clouds: f32,
    pub exposure: f32,
    pub multiple_scattering_factor: f32,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            // sunset towards the horizon
            sun_position: Vec3::new(0.0, 0.0, -1.0),
            eye_position: Vec3::new(0.0, 1000.0, 0.0),
            sun_intensity: 22.0,
            rayleigh_scattering: Vec3::new(5.802, 13.558, 33.1),
            mie_scattering: Vec3::new(3.996, 3.996, 3.996),
            mie_g: 0.8,
            atmosphere_height: 100000.0,
            cloud_coverage: 0.5,
            enable_clouds: 0.0,
            exposure: 1.0,
            multiple_scattering_factor: 1.0,
        }
    }
}
