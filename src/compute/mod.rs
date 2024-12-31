use bevy::{
    asset::load_internal_asset,
    prelude::*,
    render::{
        extract_component::{ExtractComponentPlugin, UniformComponentPlugin},
        extract_resource::ExtractResourcePlugin,
        render_graph::RenderGraph,
        RenderApp,
    },
};

mod atmosphere_luts;
mod clouds;
mod common;
mod radiance_maps;

use atmosphere_luts::{AtmosphereLutNode, AtmosphereLutPipeline};
use common::{setup_atmosphere_resources, ComputeLabel};
use radiance_maps::{RadianceMapNode, RadianceMapPipeline};

use crate::atmosphere::{AtmosphereResources, AtmosphereSettings};

pub struct ComputeShaderPlugin;

pub const ATMOSPHERE_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(13871298374012);

impl Plugin for ComputeShaderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            ATMOSPHERE_SHADER_HANDLE,
            "../../assets/shaders/atmosphere.wgsl",
            Shader::from_wgsl
        );

        app.add_systems(PreStartup, setup_atmosphere_resources)
            .add_plugins((
                ExtractResourcePlugin::<AtmosphereResources>::default(),
                ExtractComponentPlugin::<AtmosphereSettings>::default(),
                UniformComponentPlugin::<AtmosphereSettings>::default(),
            ));

        let render_app = app.sub_app_mut(RenderApp);
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();

        render_graph.add_node(
            ComputeLabel::TransmittanceLUT,
            AtmosphereLutNode::new(ComputeLabel::TransmittanceLUT),
        );
        render_graph.add_node(
            ComputeLabel::MultipleScatteringLUT,
            AtmosphereLutNode::new(ComputeLabel::MultipleScatteringLUT),
        );

        render_graph.add_node(ComputeLabel::DiffuseRadiance, RadianceMapNode::default());

        // Add dependencies
        render_graph.add_node_edge(
            ComputeLabel::TransmittanceLUT,
            ComputeLabel::MultipleScatteringLUT,
        );

        render_graph.add_node_edge(
            ComputeLabel::MultipleScatteringLUT,
            ComputeLabel::DiffuseRadiance,
        );
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<AtmosphereLutPipeline>();
        render_app.init_resource::<RadianceMapPipeline>();
    }
}
