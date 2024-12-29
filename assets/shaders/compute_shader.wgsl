#import bevy_render::globals::Globals;
#import atmosphere::RenderTransmittanceLutPS;

@group(0) @binding(7) var<uniform> globals: Globals;
@group(0) @binding(8) var texture: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = textureDimensions(texture);
    let coords = vec2<i32>(invocation_id.xy);
    var uv = (vec2<f32>(coords) + 0.5) / vec2<f32>(dimensions);
    let color = RenderTransmittanceLutPS(vec2<f32>(coords), uv, vec2<f32>(dimensions));
    textureStore(texture, coords, color);
}
