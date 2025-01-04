#import bevy_render::globals::Globals;
#import atmosphere::{
    RenderTransmittanceLutPS,
    RenderMultipleScatteringLutPS,
    GetAtmosphereParameters,
    GetTransmittanceToSun,
    uniformBuffer
};

@group(0) @binding(7) var<uniform> globals: Globals;
@group(0) @binding(8) var texture: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn transmittance(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let coords = vec2<i32>(invocation_id.xy);
    var uv = (vec2<f32>(coords) + 0.5) / dimensions;
    let color = RenderTransmittanceLutPS(vec2<f32>(coords), uv, dimensions);
    textureStore(texture, coords, color);
}


@compute @workgroup_size(1, 1, 64)
fn multiple_scattering(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let coords = vec2<i32>(invocation_id.xy);
    var uv = (vec2<f32>(coords) + 0.5) / dimensions;
    let color = RenderMultipleScatteringLutPS(vec2<f32>(coords), dimensions, invocation_id, uv);
    // store the result in the texture at the last invocation
    if invocation_id.z == 0 {
        textureStore(texture, coords, color);
    }
}

@compute @workgroup_size(1, 1, 1)
fn sun_transmittance(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let atmosphere = GetAtmosphereParameters();
    let WorldPos = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0) + uniformBuffer.eye_position;
    let transmittance = GetTransmittanceToSun(atmosphere, WorldPos, uniformBuffer.sun_position);
    textureStore(texture, vec2<i32>(0, 0), vec4<f32>(transmittance, 1.0));
} 
