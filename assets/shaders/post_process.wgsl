#import bevy_render::view::View
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import atmosphere::{
    RenderSkyPS,GetAtmosphereParameters,uniformBuffer,view,transmittanceTexture,transmittanceTextureSampler,
    lights,directional_shadow_texture,directional_shadow_sampler,raySphereIntersect,sample_shadow_map_hardware
};

struct PostProcessSettings {
    show: f32,
};

@group(0) @binding(8)
var screen_texture: texture_2d<f32>;
@group(0) @binding(9)
var depth_texture: texture_depth_multisampled_2d;
@group(0) @binding(10)
var texture_sampler: sampler;

@group(0) @binding(11)
var<uniform> settings: PostProcessSettings;

#define USE_DEPTH_BUFFER
#define USE_SHADOW_MAP

var<private> PI: f32 = 3.1415926535897932384626433832795;
var<private> PI_2: f32 = 6.283185307179586476925286766559;

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

fn GetViewRay(uv: vec2<f32>) -> vec3<f32> {
    // Convert UV to clip space coordinates
    let clip_pos = vec2(uv.x * 2.0 - 1.0, uv.y * 2.0 - 1.0) * vec2(1.0, -1.0);
    
    // Transform to view space
    let view_pos = view.view_from_clip * vec4(clip_pos, 1.0, 1.0);
    let view_ray = normalize(view_pos.xyz / view_pos.w);
    
    // Transform to world space
    let world_ray = (view.world_from_view * vec4(view_ray, 0.0)).xyz;
    
    return normalize(world_ray);
}

fn get_cascade_index(light_id: u32, view_z: f32) -> u32 {
    let light = &lights.directional_lights[light_id];

    for (var i: u32 = 0u; i < (*light).num_cascades; i = i + 1u) {
        if (-view_z < (*light).cascades[i].far_bound) {
            return i;
        }
    }
    return (*light).num_cascades;
}

fn world_to_directional_light_local(
    light_id: u32,
    cascade_index: u32,
    offset_position: vec4<f32>
) -> vec4<f32> {
    let light = &lights.directional_lights[light_id];
    let cascade = &(*light).cascades[cascade_index];

    let offset_position_clip = (*cascade).clip_from_world * offset_position;
    if (offset_position_clip.w <= 0.0) {
        return vec4(0.0);
    }
    let offset_position_ndc = offset_position_clip.xyz / offset_position_clip.w;
    // No shadow outside the orthographic projection volume
    if (any(offset_position_ndc.xy < vec2<f32>(-1.0)) || offset_position_ndc.z < 0.0
            || any(offset_position_ndc > vec3<f32>(1.0))) {
        return vec4(0.0);
    }

    // compute texture coordinates for shadow lookup, compensating for the Y-flip difference
    // between the NDC and texture coordinates
    let flip_correction = vec2<f32>(0.5, -0.5);
    let light_local = offset_position_ndc.xy * flip_correction + vec2<f32>(0.5, 0.5);

    let depth = offset_position_ndc.z;

    return vec4(light_local, depth, 1.0);
}

fn sample_directional_cascade(
    light_id: u32,
    cascade_index: u32,
    frag_position: vec4<f32>,
    surface_normal: vec3<f32>,
) -> f32 {
    let light = &lights.directional_lights[light_id];
    let cascade = &(*light).cascades[cascade_index];

    // The normal bias is scaled to the texel size.
    let normal_offset = (*light).shadow_normal_bias * (*cascade).texel_size * surface_normal.xyz;
    let depth_offset = (*light).shadow_depth_bias * (*light).direction_to_light.xyz;
    let offset_position = vec4<f32>(frag_position.xyz + normal_offset + depth_offset, frag_position.w);

    let light_local = world_to_directional_light_local(light_id, cascade_index, offset_position);
    if (light_local.w == 0.0) {
        return 1.0;
    }

    let array_index = i32((*light).depth_texture_base_index + cascade_index);
    let texel_size = (*cascade).texel_size;

    // If soft shadows are enabled, use the PCSS path.
    // if ((*light).soft_shadow_size > 0.0) {
    //     return sample_shadow_map_pcss(
    //         light_local.xy, light_local.z, array_index, texel_size, (*light).soft_shadow_size);
    // }

    return sample_shadow_map_hardware(light_local.xy, light_local.z, array_index);
}

fn fetch_directional_shadow(light_id: u32, world_pos: vec4<f32>, surface_normal: vec3<f32>, view_z: f32) -> f32 {
    let light = &lights.directional_lights[light_id];
    let cascade_index = get_cascade_index(light_id, view_z);

    if (cascade_index >= (*light).num_cascades) {
        return 1.0;
    }

    var shadow = sample_directional_cascade(light_id, cascade_index, world_pos, surface_normal);

    // Blend with the next cascade, if there is one.
    let next_cascade_index = cascade_index + 1u;
    if (next_cascade_index < (*light).num_cascades) {
        let this_far_bound = (*light).cascades[cascade_index].far_bound;
        let next_near_bound = (1.0 - (*light).cascades_overlap_proportion) * this_far_bound;
        if (-view_z >= next_near_bound) {
            let next_shadow = sample_directional_cascade(light_id, next_cascade_index, world_pos, surface_normal);
            shadow = mix(shadow, next_shadow, (-view_z - next_near_bound) / (this_far_bound - next_near_bound));
        }
    }
    return shadow;
}

fn fetch_directional_shadow2(light_index: u32, world_pos: vec4<f32>, normal: vec3<f32>, view_z: f32) -> f32 {
    let light = &lights.directional_lights[light_index]; // Using first directional light
    
    // Get cascade index based on view_z
    var cascade_index = 0u;
    for (var i = 0u; i < (*light).num_cascades; i++) {
        if (-view_z < (*light).cascades[i].far_bound) {
            cascade_index = i;
            break;
        }
    }
    
    // Get the cascade
    let cascade = &(*light).cascades[cascade_index];
    
    // Calculate position with bias
    let normal_offset = (*light).shadow_normal_bias * (*cascade).texel_size * normal;
    let depth_offset = (*light).shadow_depth_bias * (*light).direction_to_light;
    let offset_position = vec4<f32>(world_pos.xyz + normal_offset + depth_offset, world_pos.w);
    
    // Transform to light space
    let light_local = (*cascade).clip_from_world * offset_position;
    
    // Convert to UV coordinates
    let ndc = light_local.xyz / light_local.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    
    // Early exit if outside shadow map
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    
    let depth = ndc.z;
    let array_index = i32((*light).depth_texture_base_index + cascade_index);
    
    // Sample shadow map
    return textureSampleCompareLevel(
        directional_shadow_texture,
        directional_shadow_sampler,
        uv,
        array_index,
        depth
    );
}

fn getShadow(P: vec3<f32>, ray_dir: vec3<f32>, frag_pos: vec2<f32>) -> f32 {
    // For raymarched volumes, we can use the ray direction as the normal
    // since we don't have surface normals
    let world_normal = -ray_dir; // Point against ray direction
    
    // Get view space Z coordinate for cascade selection
    let view_pos = view.view_from_world * vec4(P, 1.0);
    let view_z = view_pos.z;

    // get local up vector
    let local_up = vec3<f32>(0.0, 1.0, 0.0);

    // Assuming we're using the first directional light (index 0)
    return fetch_directional_shadow2(0u, vec4<f32>(P, 1.0), world_normal, view_z);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let ray_dir = GetViewRay(in.uv);
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    var depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);
    let dimensions = textureDimensions(screen_texture, 0);
    let dimensionsF32 = vec2<f32>(dimensions.xy);

    let atmosphere = GetAtmosphereParameters();
    // Start position at camera, which is at ground level
    let origin = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0);
    let WorldPos = origin + uniformBuffer.eye_position + view.world_position;
    let WorldDir = ray_dir;
    // let earthO = vec3<f32>(0.0, 0.0, 0.0);
    // let bottomIntersect = raySphereIntersect(WorldPos, WorldDir, earthO, atmosphere.BottomRadius);
    // var boundingSphere = atmosphere.BottomRadius + 2.0;
    // let topIntersect = raySphereIntersect(WorldPos, WorldDir, earthO, boundingSphere);

    // // Early exit if no intersection with atmosphere
    // if (topIntersect.far < 0.0) {
    //     return vec4(vec3(1.0), 1.0);
    // }

    // // Early exit if we're looking at the sky (no depth)
    // if (depth <= 0.0) {
    //     // return vec4(vec3(0.0, 0.0, 1.0), 1.0);
    //     // depth = 1.0;
    // }

    // // Get the depth from the depth buffer and convert to world distance
    // let ndc = vec2(in.uv.x * 2.0 - 1.0, in.uv.y * 2.0 - 1.0);
    // let clip_pos = vec4(ndc, depth, 1.0);
    // let view_pos = view.view_from_clip * clip_pos;
    // let terrain_distance = length(view_pos.xyz / view_pos.w);

    // // Determine if camera is inside atmosphere
    // let camera_height = length(WorldPos - earthO);
    // let is_camera_in_atmosphere = camera_height < boundingSphere;

    // // Calculate start and end points of the ray
    // var tStart = select(topIntersect.near, 0.0, is_camera_in_atmosphere);
    // var tEnd = min(
    //     select(bottomIntersect.near, topIntersect.far, is_camera_in_atmosphere),
    //     terrain_distance
    // );
    // if (is_camera_in_atmosphere && bottomIntersect.near > 0.0) {
    //     tEnd = min(tEnd, bottomIntersect.near);
    // }
    // let marchDistance = tEnd - tStart;

    // // Early exit if we have an invalid ray segment
    // // if (tEnd <= tStart) {
    // //     return vec4(vec3(0.0, 1.0, 0.0), 1.0);
    // // }

    
    
    
    // let samples = 64u;
    // var density = 0.0;
    // let stepSize = marchDistance / f32(samples);
    
    // // If we have a valid ray segment through the atmosphere
    // if (tEnd > tStart) {
    //     // Use exponential distribution of samples to get better precision near the camera
    //     // and in areas where shadows change rapidly
    //     for (var i = 0u; i < samples; i++) {
    //         // Exponential distribution of sample points
    //         let fi = f32(i) / f32(samples);
    //         let exp_factor = exp(fi * 2.0) - 1.0; // Adjust the 2.0 to control distribution
    //         let t = tStart + (tEnd - tStart) * exp_factor / (exp(2.0) - 1.0);
            
    //         let P = (WorldPos + WorldDir * t) - origin;
            
    //         // Get shadow at this point
    //         let shadow = getShadow(P, WorldDir, in.uv * dimensionsF32);
            
    //         // Calculate the weight for this sample based on its position
    //         // More weight near camera and shadow boundaries
    //         let weight = exp(-fi * 1.0); // Adjust the 1.0 to control falloff
            
    //         density += shadow * weight * stepSize;
    //     }
        
    //     // Normalize the density based on the weights
    //     density = density / (tEnd - tStart);
    // }

    // // Use Beer-Lambert law for shadow extinction
    // let shadowExtinction = exp(-density * 2.0);

    // Use consistent density scaling
    // let shadowExtinction = exp(-density);
    // Only apply the volumetric shadow, don't modify the original color
    // return select(vec4(vec3(shadowExtinction), 1.0), color, in.uv.x < 0.5);

    // let WorldPos = vec3<f32>(0.0, atmosphere.BottomRadius, 0.0) + uniformBuffer.eye_position + view.world_position;
    // let WorldDir = ray_dir;

    // // discard fragment where no depth is available
    // if (depth <= 0.0 || settings.show == 0.0) {
    //     // return vec4(color.rgb, 1.0);
    // }

    // // Draw the transmittance LUT in upper left corner
    // let padding = 16.0;
    // let lut_size = vec2<f32>(textureDimensions(transmittanceTexture, 0));
    // let scaled_size = lut_size * 2.0; // 2x magnification
    // let screen_pos = in.uv * dimensionsF32;
    // let draw_uv = (screen_pos - vec2<f32>(padding)) / scaled_size;
    // let transmittance = textureSample(transmittanceTexture, transmittanceTextureSampler, draw_uv);
    
    // if (draw_uv.x >= 0.0 && draw_uv.x <= 1.0 && 
    //     draw_uv.y >= 0.0 && draw_uv.y <= 1.0 && 
    //     screen_pos.x >= padding && screen_pos.x <= scaled_size.x + padding &&
    //     screen_pos.y >= padding && screen_pos.y <= scaled_size.y + padding) {
    //     return vec4(transmittance.rgb, 1.0);
    // }

    var result = RenderSkyPS(in.uv, in.uv * dimensionsF32, dimensionsF32, WorldPos, WorldDir, depth);
    
    // // calculate L (inscattering)
    var L = result.L + color.rgb * result.Transmittance / PI_2;

    // let ray_uv = rd2uv(ray_dir);

    // let new_color = vec4(renderTestCheckerboard(ray_uv), 1.0);

    // var S = shadowExtinction;
    return vec4(L * 8.0, 1.0);
}
