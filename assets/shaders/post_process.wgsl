#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct PostProcessSettings {
    show_depth: f32,
}

@group(0) @binding(0)
var screen_texture: texture_2d<f32>;
@group(0) @binding(1)
var depth_texture: texture_depth_multisampled_2d;
@group(0) @binding(2)
var texture_sampler: sampler;
@group(0) @binding(3)
var<uniform> settings: PostProcessSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);
    
    // Mix between color and depth based on the show_depth setting
    let final_color = mix(
        color.rgb,
        vec3(depth),
        settings.show_depth,
    );
    
    return vec4(final_color, 1.0);
} 