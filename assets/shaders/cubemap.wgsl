#import bevy_render::globals::Globals;
#import atmosphere::{RenderTransmittanceLutPS,RenderSkyPS,GetAtmosphereParameters,uniformBuffer,PI,PI_1_2};

@group(0) @binding(7) var specular_texture: texture_2d<f32>;
@group(0) @binding(8) var specular_sampler: sampler;
@group(0) @binding(9) var<uniform> globals: Globals;
@group(0) @binding(10) var texture: texture_storage_2d<rgba32float, write>;

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

fn radical_inverse_vdc(bits: u32) -> f32 {
    var bits_local = bits;
    bits_local = (bits_local << 16u) | (bits_local >> 16u);
    bits_local = ((bits_local & 0x55555555u) << 1u) | ((bits_local & 0xAAAAAAAAu) >> 1u);
    bits_local = ((bits_local & 0x33333333u) << 2u) | ((bits_local & 0xCCCCCCCCu) >> 2u);
    bits_local = ((bits_local & 0x0F0F0F0Fu) << 4u) | ((bits_local & 0xF0F0F0F0u) >> 4u);
    bits_local = ((bits_local & 0x00FF00FFu) << 8u) | ((bits_local & 0xFF00FF00u) >> 8u);
    return f32(bits_local) * 2.3283064365386963e-10;
}

fn hammersley_2d(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

fn sample_hemisphere_uniform(xi: vec2<f32>) -> vec3<f32> {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = xi.y;
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    
    return vec3<f32>(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta
    );
}

fn sample_hemisphere_cosine(xi: vec2<f32>) -> vec3<f32> {
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt(1.0 - xi.y);
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    
    return vec3<f32>(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta
    );
}

fn direction_to_uv(dir: vec3<f32>) -> vec2<f32> {
    let abs_dir = abs(dir);
    var face_uv: vec2<f32>;
    var face_index: i32;
    
    // Find which face to use based on largest component
    if (abs_dir.x >= abs_dir.y && abs_dir.x >= abs_dir.z) {
        // X axis
        if (dir.x > 0.0) {
            face_uv = vec2(-dir.z, -dir.y) / abs_dir.x;
            face_index = 0;
        } else {
            face_uv = vec2(dir.z, -dir.y) / abs_dir.x;
            face_index = 1;
        }
    } else if (abs_dir.y >= abs_dir.z) {
        // Y axis
        if (dir.y > 0.0) {
            face_uv = vec2(dir.x, dir.z) / abs_dir.y;
            face_index = 2;
        } else {
            face_uv = vec2(dir.x, -dir.z) / abs_dir.y;
            face_index = 3;
        }
    } else {
        // Z axis
        if (dir.z > 0.0) {
            face_uv = vec2(dir.x, -dir.y) / abs_dir.z;
            face_index = 4;
        } else {
            face_uv = vec2(-dir.x, -dir.y) / abs_dir.z;
            face_index = 5;
        }
    }
    
    // Convert from [-1,1] to [0,1] range
    face_uv = face_uv * 0.5 + 0.5;
    
    // Calculate final UV coordinates
    let face_size = f32(textureDimensions(specular_texture).x);
    return vec2(
        face_uv.x * face_size,
        face_uv.y * face_size + f32(face_index) * face_size
    );
}

@compute @workgroup_size(8, 8, 1)
fn specular_radiance(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let coords = vec2<i32>(invocation_id.xy);
    
    let dir = compute_cubemap_direction(vec2<f32>(coords), dimensions);
    
    let atmosphere = GetAtmosphereParameters();
    // Convert position to kilometers as used in the atmosphere calculations
    let WorldPos = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0) + uniformBuffer.eye_position;
    
    // Use the same sky rendering function as in post_process.wgsl
    let result = RenderSkyPS(vec2(0.0), vec2(0.0), dimensions, WorldPos, dir, 1.0);
    let color = vec4(result.L, 1.0);
    
    textureStore(texture, coords, color);
}

@compute @workgroup_size(8, 8, 1)
fn diffuse_radiance(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let coords = vec2<i32>(invocation_id.xy);
    
    let dir = compute_cubemap_direction(vec2<f32>(coords), dimensions);
    
    let atmosphere = GetAtmosphereParameters();
    let WorldPos = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0) + uniformBuffer.eye_position;
    
    // Integrate over hemisphere for diffuse radiance
    var diffuse_radiance = vec3<f32>(0.0);
    let samples = 128u;
    
    // Create a basis where 'dir' is the up vector
    let up = dir;
    let right = normalize(cross(up, vec3<f32>(0.0, 1.0, 0.0)));
    let forward = normalize(cross(right, up));
    
    // Sample hemisphere using Hammersley sequence with cosine-weighted distribution
    for(var i = 0u; i < samples; i = i + 1u) {
        let xi = hammersley_2d(i, samples);
        let sample_dir = sample_hemisphere_cosine(xi);
        
        // Transform sample direction to world space
        let world_sample_dir = normalize(
            sample_dir.x * right +
            sample_dir.y * forward +
            sample_dir.z * up
        );
        
        // Sample from specular texture instead of computing sky radiance
        let uv = direction_to_uv(world_sample_dir);
        let specular = textureSampleLevel(specular_texture, specular_sampler, uv / dimensions, 0.0).rgb;
        // let specular = vec3(1.0);
        // Note: cos(theta) is already included in the sampling probability
        diffuse_radiance += specular;
    }
    
    // Normalize and apply final scaling
    diffuse_radiance = diffuse_radiance * (PI / f32(samples));
    
    let color = vec4(diffuse_radiance, 1.0);
    textureStore(texture, coords, color);
}