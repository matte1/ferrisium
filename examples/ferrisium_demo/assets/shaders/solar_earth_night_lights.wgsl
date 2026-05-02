#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings as view_bindings,
}

struct SolarEarthNightLightsMaterial {
    sun_direction_strength: vec4<f32>,
    light_color_alpha: vec4<f32>,
    mask_params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> night_lights: SolarEarthNightLightsMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var light_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var light_sampler: sampler;

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_direction = normalize(view_bindings::view.world_position - in.world_position.xyz);
    let sun_direction = normalize(night_lights.sun_direction_strength.xyz);

    let normal_dot_sun = dot(normal, sun_direction);
    let normal_dot_view = saturate(dot(normal, view_direction));
    let terminator_fade = night_lights.mask_params.z;
    let night = 1.0 - smoothstep(-terminator_fade, 0.04, normal_dot_sun);
    let limb_visibility = smoothstep(0.025, 0.16, normal_dot_view);

    let sample = textureSample(light_texture, light_sampler, in.uv).rgb;
    let luminance = dot(sample, vec3<f32>(0.299, 0.587, 0.114));
    let threshold = night_lights.mask_params.x;
    let softness = night_lights.mask_params.y;
    let lit_mask = smoothstep(threshold, threshold + softness, luminance);
    let hotspot = pow(saturate(luminance), 1.25);
    let alpha =
        mix(lit_mask * 0.45, hotspot, 0.55)
        * night_lights.light_color_alpha.w
        * night
        * limb_visibility
        * night_lights.sun_direction_strength.w;

    if alpha < 0.001 {
        discard;
    }

    let warm = night_lights.light_color_alpha.rgb;
    let hot = vec3<f32>(1.0, 0.90, 0.62);
    let color = mix(warm, hot, saturate(luminance * 1.8));

    return vec4<f32>(color * alpha, alpha);
}
