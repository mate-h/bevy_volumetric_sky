#import bevy_render::globals::Globals;
#import atmosphere::{RenderTransmittanceLutPS,RenderSkyPS,GetAtmosphereParameters,uniformBuffer,PI,PI_1_2};

@group(0) @binding(7) var<uniform> globals: Globals;
@group(0) @binding(8) var texture: texture_storage_2d<rgba32float, write>;

fn compute_cubemap_direction(coords: vec2<f32>, dimensions: vec2<f32>) -> vec3<f32> {
    let w = dimensions.x;
    
    // Get face index and local UV coordinates [0,1]
    let index = i32(coords.y) / i32(w);
    let local_uv = vec2<f32>(
        coords.x / w,
        (coords.y - f32(index) * w) / w
    );
    
    // Convert to [-1,1] range
    let uv = 2.0 * local_uv - 1.0;
    
    // Generate direction based on face
    var dir: vec3<f32>;
    switch index {
        case 0: { // +X
            dir = vec3<f32>(1.0, -uv.y, -uv.x);
        }
        case 1: { // -X
            dir = vec3<f32>(-1.0, -uv.y, uv.x);
        }
        case 2: { // +Y
            dir = vec3<f32>(uv.x, 1.0, uv.y);
        }
        case 3: { // -Y
            dir = vec3<f32>(uv.x, -1.0, -uv.y);
        }
        case 4: { // +Z
            dir = vec3<f32>(uv.x, -uv.y, 1.0);
        }
        default: { // -Z
            dir = vec3<f32>(-uv.x, -uv.y, -1.0);
        }
    }
    
    return normalize(dir);
}

fn rd2uv(rd: vec3<f32>) -> vec2<f32> {
    // Use spherical coordinates relative to the view direction
    let u = 0.5 + atan2(rd.z, rd.x) / (2.0 * PI);  // Note the negative rd.z
    let v = 0.5 - asin(rd.y) / PI;
    return vec2(u, v);
}

fn renderTestCheckerboard(uv: vec2<f32>) -> vec3<f32> {
    const checkerSize = 32.0;
    let x = floor(uv.x * checkerSize);
    let y = floor(uv.y * checkerSize);
    let checker = (x + y) % 2.0;
    var color = vec3<f32>(checker, checker, checker);
    return color;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let coords = vec2<i32>(invocation_id.xy);
    
    let dir = compute_cubemap_direction(vec2<f32>(coords), dimensions);
    
    let atmosphere = GetAtmosphereParameters();
    // Convert position to kilometers as used in the atmosphere calculations
    let WorldPos = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0) + uniformBuffer.eye_position * 0.001;
    
    // Use the same sky rendering function as in post_process.wgsl
    let result = RenderSkyPS(vec2(0.0), vec2(0.0), dimensions, WorldPos, dir, 1.0);
    let color = vec4(result.L * 20.0, 1.0);
    
    textureStore(texture, coords, color);
}
