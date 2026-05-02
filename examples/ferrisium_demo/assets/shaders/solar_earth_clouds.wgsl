#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings as view_bindings,
}

struct SolarEarthCloudMaterial {
    sun_direction_strength: vec4<f32>,
    cloud_color_alpha: vec4<f32>,
    mask_params: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> clouds: SolarEarthCloudMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var cloud_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var cloud_sampler: sampler;

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_direction = normalize(view_bindings::view.world_position - in.world_position.xyz);
    let sun_direction = normalize(clouds.sun_direction_strength.xyz);

    let normal_dot_sun = dot(normal, sun_direction);
    let normal_dot_view = saturate(dot(normal, view_direction));
    let day = smoothstep(-0.18, 0.34, normal_dot_sun);
    let terminator =
        smoothstep(-0.20, 0.02, normal_dot_sun) * (1.0 - smoothstep(0.18, 0.38, normal_dot_sun));
    let limb_visibility = smoothstep(0.025, 0.18, normal_dot_view);

    let texture_color = textureSample(cloud_texture, cloud_sampler, in.uv).rgb;
    let luminance = dot(texture_color, vec3<f32>(0.299, 0.587, 0.114));
    let threshold = clouds.mask_params.x;
    let softness = clouds.mask_params.y;
    let cloud_mask = smoothstep(threshold, threshold + softness, luminance);
    let dense_cloud = smoothstep(0.62, 0.92, luminance);

    let night_factor = clouds.mask_params.z;
    let light_factor = mix(night_factor, 1.0, day);
    let alpha =
        cloud_mask
        * clouds.cloud_color_alpha.w
        * mix(0.78, 1.0, dense_cloud)
        * mix(1.0, 0.62, terminator)
        * light_factor
        * limb_visibility
        * clouds.sun_direction_strength.w;

    if alpha < 0.001 {
        discard;
    }

    let shade = mix(0.32, 0.94, day) + terminator * 0.06;
    let color = clouds.cloud_color_alpha.rgb * shade;

    return vec4<f32>(color * alpha, alpha);
}
