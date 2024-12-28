#import bevy_render::view::View
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct PostProcessSettings {
    show_depth: f32,
};

@group(0) @binding(0)
var screen_texture: texture_2d<f32>;
@group(0) @binding(1)
var depth_texture: texture_depth_multisampled_2d;
@group(0) @binding(2)
var texture_sampler: sampler;
@group(0) @binding(3)
var<uniform> view: View;
@group(0) @binding(4)
var<uniform> settings: PostProcessSettings;


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

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let ray_dir = GetViewRay(in.uv);
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);

    let ray_uv = rd2uv(ray_dir);

    let new_color = vec4(renderTestCheckerboard(ray_uv), 1.0);
    return mix(color, new_color, 0.01);
}
