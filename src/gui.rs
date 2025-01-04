use std::borrow::BorrowMut;
use std::f32::consts::PI;

use crate::atmosphere::{AtmosphereResources, AtmosphereSettings};
use crate::post_process::PostProcessSettings;
use crate::Ground;
use bevy::color::palettes::tailwind;
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseMotion,
    prelude::*,
};
use bevy_debug_grid::Grid;
use bevy_egui::egui::Color32;
use bevy_egui::{egui, EguiContexts, EguiSet};
use bevy_panorbit_camera::PanOrbitCamera;

#[derive(Resource, Default)]
pub struct EguiInteractionState {
    pub wants_focus: bool,
    pub is_dragging: bool,
    pub camera_interaction_active: bool,
}

#[derive(Resource)]
pub struct SunPositionState {
    pub target_theta: f32, // altitude angle
    pub target_phi: f32,   // azimuth angle
    pub current_theta: f32,
    pub current_phi: f32,
}

impl Default for SunPositionState {
    fn default() -> Self {
        Self {
            target_theta: 1.82,
            target_phi: 0.0,
            current_theta: 1.82,
            current_phi: 0.0,
        }
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PanOrbitCameraSystemSet;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EguiInteractionState>()
            .init_resource::<SunPositionState>()
            .add_systems(
                PreUpdate,
                check_egui_wants_focus
                    .after(EguiSet::BeginPass)
                    .before(PanOrbitCameraSystemSet),
            )
            .add_systems(
                Update,
                (
                    ui_system,
                    handle_camera_block.after(ui_system),
                    update_sun_position.after(ui_system),
                ),
            );
    }
}

fn check_egui_wants_focus(mut state: ResMut<EguiInteractionState>, mut contexts: EguiContexts) {
    let ctx = contexts.ctx_mut();
    let pointer_state = ctx.input(|i| i.pointer.clone());

    // Start dragging when mouse is pressed over egui
    if ctx.is_pointer_over_area() && pointer_state.any_pressed() {
        state.is_dragging = true;
    }

    // Stop dragging when mouse is released
    if !pointer_state.any_down() {
        state.is_dragging = false;
    }

    // Update wants_focus based on dragging state
    state.wants_focus =
        state.is_dragging || (ctx.is_pointer_over_area() && !state.camera_interaction_active);
}

fn ui_system(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mut post_process_settings: Query<&mut PostProcessSettings>,
    mut grid_query: Query<&mut Visibility, (With<Grid>, Without<Ground>)>,
    mut ground_query: Query<&mut Visibility, (With<Ground>, Without<Grid>)>,
    mut atmosphere_settings: Query<&mut AtmosphereSettings>,
    atmosphere_res: Res<AtmosphereResources>,
    mut sun_position_state: ResMut<SunPositionState>,
) {
    let ms_texture_id = contexts.add_image(atmosphere_res.multiple_scattering_texture.clone_weak());
    let transmittance_texture_id =
        contexts.add_image(atmosphere_res.transmittance_texture.clone_weak());
    let diffuse_texture_id = contexts.add_image(
        atmosphere_res
            .diffuse_irradiance_compute_target
            .clone_weak(),
    );
    let specular_texture_id =
        contexts.add_image(atmosphere_res.specular_radiance_compute_target.clone_weak());
    let sun_texture_id = contexts.add_image(atmosphere_res.sun_transmittance_texture.clone_weak());
    let ctx = contexts.ctx_mut();

    egui::Window::new("")
        .title_bar(false)
        .default_width(196.0)
        .show(ctx, |ui| {
            if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
                if let Some(fps_value) = fps.smoothed() {
                    ui.label(format!("FPS: {:.1}", fps_value));
                }
            }

            if let Ok(camera) = camera_query.get_single() {
                ui.separator();
                ui.label(format!(
                    "Focus: ({:.2}, {:.2}, {:.2})",
                    camera.focus.x, camera.focus.y, camera.focus.z
                ));
                ui.label(format!("Radius: {:.2}", camera.radius.unwrap_or(0.0)));
                ui.label(format!(
                    "Yaw: {:.1}°",
                    camera.yaw.unwrap_or(0.0).to_degrees()
                ));
                ui.label(format!(
                    "Pitch: {:.1}°",
                    camera.pitch.unwrap_or(0.0).to_degrees()
                ));
            }

            ui.horizontal(|ui| {
                if ui.button("Reset Camera").clicked() {
                    if let Ok(mut camera) = camera_query.get_single_mut() {
                        *camera = PanOrbitCamera {
                            focus: Vec3::ZERO,
                            radius: Some(6.0),
                            yaw: Some(0.0),
                            pitch: Some(std::f32::consts::PI * 0.1),
                            ..Default::default()
                        };
                    }
                }
            });

            ui.separator();
            if let Ok(mut grid_visibility) = grid_query.get_single_mut() {
                let mut show_grid = *grid_visibility != Visibility::Hidden;
                if ui.checkbox(&mut show_grid, "Show Grid").clicked() {
                    *grid_visibility = if show_grid {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }

            if let Ok(mut ground_visibility) = ground_query.get_single_mut() {
                let mut show_ground = *ground_visibility != Visibility::Hidden;
                if ui.checkbox(&mut show_ground, "Show Ground").clicked() {
                    *ground_visibility = if show_ground {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }

            ui.separator();
            let blue_400 = Color32::from_hex(tailwind::BLUE_400.to_hex().as_str()).unwrap();
            ui.colored_label(blue_400, "Atmosphere");

            // Sun position controls
            ui.add(
                egui::Slider::new(&mut sun_position_state.target_theta, 0.0..=PI)
                    .text("Sun Altitude"),
            );

            if sun_position_state.target_theta != 0.0 && sun_position_state.target_theta != PI {
                ui.add(
                    egui::Slider::new(&mut sun_position_state.target_phi, -PI..=PI)
                        .text("Sun Azimuth"),
                );
            }

            if let Ok(mut settings) = atmosphere_settings.get_single_mut() {
                // add text for the sun position vec3
                ui.label(format!(
                    "Sun Position: ({:.2}, {:.2}, {:.2})",
                    settings.sun_position.x, settings.sun_position.y, settings.sun_position.z
                ));

                // Add slider for the eye position
                ui.add(
                    egui::Slider::new(&mut settings.eye_position.y, 0.01..=50.0)
                        .text("Eye Position"),
                );

                let mut show = settings.multiple_scattering_factor != 0.0;
                if ui.checkbox(&mut show, "Multiple Scattering").clicked() {
                    settings.multiple_scattering_factor = show as u32 as f32;
                }
            }

            // Post process
            if let Ok(mut settings) = post_process_settings.get_single_mut() {
                let mut show = settings.show != 0.0;
                if ui.checkbox(&mut show, "Aerial Perspective").clicked() {
                    settings.show = show as u32 as f32;
                }
            }

            let s = 8.0;
            ui.horizontal_top(|ui| {
                ui.image(egui::load::SizedTexture::new(
                    diffuse_texture_id,
                    egui::vec2(256.0 / s, 256.0 * 6.0 / s),
                ));

                ui.image(egui::load::SizedTexture::new(
                    specular_texture_id,
                    egui::vec2(256.0 / s, 256.0 * 6.0 / s),
                ));
                ui.image(egui::load::SizedTexture::new(
                    ms_texture_id,
                    egui::vec2(32.0, 32.0),
                ));

                ui.image(egui::load::SizedTexture::new(
                    sun_texture_id,
                    egui::vec2(32.0, 32.0),
                ));
            });

            ui.image(egui::load::SizedTexture::new(
                transmittance_texture_id,
                egui::vec2(256.0 / 2.0, 64.0 / 2.0),
            ));
        });
}

fn handle_camera_block(
    mut state: ResMut<EguiInteractionState>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut camera_query: Query<&mut PanOrbitCamera>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    // Check if camera interaction is starting
    if mouse_buttons.just_pressed(MouseButton::Left) && !state.wants_focus {
        state.camera_interaction_active = true;
    }

    // Check if camera interaction is ending
    if mouse_buttons.just_released(MouseButton::Left) {
        state.camera_interaction_active = false;
    }

    if state.wants_focus && !state.camera_interaction_active {
        mouse_motion.clear();

        if let Ok(mut camera) = camera_query.get_single_mut() {
            camera.enabled = false;
        }
    } else if let Ok(mut camera) = camera_query.get_single_mut() {
        camera.enabled = true;
    }
}

fn update_sun_position(
    mut sun_state: ResMut<SunPositionState>,
    mut atmosphere_settings: Query<&mut AtmosphereSettings>,
    time: Res<Time>,
) {
    const LERP_SPEED: f32 = 2.0;

    // Lerp the current angles towards target
    sun_state.current_theta = lerp(
        sun_state.current_theta,
        sun_state.target_theta,
        LERP_SPEED * time.delta_secs(),
    );

    sun_state.current_phi = lerp(
        sun_state.current_phi,
        sun_state.target_phi,
        LERP_SPEED * time.delta_secs(),
    );

    // Update the actual sun position
    if let Ok(mut settings) = atmosphere_settings.get_single_mut() {
        settings.sun_position = Vec3::new(
            sun_state.current_phi.sin() * sun_state.current_theta.sin(),
            -sun_state.current_theta.cos(),
            sun_state.current_phi.cos() * sun_state.current_theta.sin(),
        );
    }
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}
