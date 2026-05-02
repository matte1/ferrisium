#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings as view_bindings,
}

struct SolarEarthAtmosphereMaterial {
    sun_direction_strength: vec4<f32>,
    rayleigh_color_alpha: vec4<f32>,
    terminator_color_alpha: vec4<f32>,
    night_color_alpha: vec4<f32>,
    falloff: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> atmosphere: SolarEarthAtmosphereMaterial;

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_direction = normalize(view_bindings::view.world_position - in.world_position.xyz);
    let sun_direction = normalize(atmosphere.sun_direction_strength.xyz);

    let normal_dot_view = saturate(dot(normal, view_direction));
    let normal_dot_sun = dot(normal, sun_direction);
    let day = smoothstep(-0.08, 0.42, normal_dot_sun);
    let night = 1.0 - smoothstep(-0.18, 0.04, normal_dot_sun);
    let terminator =
        smoothstep(-0.16, 0.02, normal_dot_sun) * (1.0 - smoothstep(0.14, 0.34, normal_dot_sun));
    let fresnel = pow(saturate(1.0 - normal_dot_view), atmosphere.falloff.x);
    let rim_gate = 1.0 - smoothstep(0.02, 0.24, normal_dot_view);
    let rim = fresnel * rim_gate * rim_gate;
    let disk_haze = atmosphere.falloff.y * day * pow(normal_dot_view, 2.2);

    let rim_alpha =
        rim
        * (
            atmosphere.rayleigh_color_alpha.w * mix(0.10, 1.0, day)
            + atmosphere.terminator_color_alpha.w * terminator
            + atmosphere.night_color_alpha.w * night
        );
    let alpha = min((rim_alpha + disk_haze) * atmosphere.sun_direction_strength.w, atmosphere.falloff.z);

    if alpha < 0.001 {
        discard;
    }

    let rayleigh_color = atmosphere.rayleigh_color_alpha.rgb * mix(0.35, 1.0, day);
    let terminator_color = atmosphere.terminator_color_alpha.rgb * terminator * 0.95;
    let night_color = atmosphere.night_color_alpha.rgb * night * 0.24;
    let color = rayleigh_color + terminator_color + night_color;

    return vec4<f32>(color * alpha, alpha);
}
