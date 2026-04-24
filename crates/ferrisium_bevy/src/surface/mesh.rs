//! Shared mesh helpers for spherical surface overlays.

use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::prelude::LonLat;

const DEFAULT_GLOBE_MAX_EDGE_SEGMENT_RADIANS: f64 = 0.02;
const DEFAULT_GLOBE_MAX_EDGE_SEGMENTS: usize = 24;

pub(crate) fn triangle_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    mesh_with_topology(
        PrimitiveTopology::TriangleList,
        positions,
        normals,
        colors,
        indices,
    )
}

pub(crate) fn line_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
) -> Mesh {
    mesh_with_topology(
        PrimitiveTopology::LineList,
        positions,
        normals,
        Vec::new(),
        indices,
    )
}

fn mesh_with_topology(
    topology: PrimitiveTopology,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        topology,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if !normals.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
    if !colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub(crate) fn append_triangle_fan_indices(
    indices: &mut Vec<u32>,
    start: usize,
    vertex_count: usize,
    clockwise: bool,
) {
    if vertex_count < 3 {
        return;
    }
    let Ok(start) = u32::try_from(start) else {
        return;
    };
    for index in 1..vertex_count.saturating_sub(1) {
        let Ok(index) = u32::try_from(index) else {
            return;
        };
        if clockwise {
            indices.extend_from_slice(&[start, start + index + 1, start + index]);
        } else {
            indices.extend_from_slice(&[start, start + index, start + index + 1]);
        }
    }
}

pub(crate) fn append_line_segment(
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: [f32; 3],
    right: [f32; 3],
) {
    let start = positions.len();
    positions.push(left);
    positions.push(right);

    let Ok(start) = u32::try_from(start) else {
        return;
    };
    indices.extend_from_slice(&[start, start + 1]);
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper appends one colored tessellated spherical polygon."
)]
pub(crate) fn append_tessellated_spherical_polygon(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    center: DVec3,
    directions: &[DVec3],
    radius: f64,
    color: Option<[f32; 4]>,
) {
    for index in 0..directions.len() {
        let left = directions[index];
        let right = directions[(index + 1) % directions.len()];
        let edge_segments = globe_edge_segments(left, right);
        let radial_segments = edge_segments.max(2);

        for edge_index in 0..edge_segments {
            let left_t = segment_fraction(edge_index, edge_segments);
            let right_t = segment_fraction(edge_index + 1, edge_segments);
            append_tessellated_spherical_wedge(
                positions,
                normals,
                colors,
                indices,
                center,
                slerp_direction(left, right, left_t),
                slerp_direction(left, right, right_t),
                radial_segments,
                radius,
                color,
            );
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper writes positions, normals, and indices for one spherical wedge."
)]
fn append_tessellated_spherical_wedge(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    center: DVec3,
    left: DVec3,
    right: DVec3,
    radial_segments: usize,
    radius: f64,
    color: Option<[f32; 4]>,
) {
    for radial_index in 0..radial_segments {
        let inner_t = segment_fraction(radial_index, radial_segments);
        let outer_t = segment_fraction(radial_index + 1, radial_segments);
        let outer_left = slerp_direction(center, left, outer_t);
        let outer_right = slerp_direction(center, right, outer_t);

        if radial_index == 0 {
            append_spherical_triangle(
                positions,
                normals,
                colors,
                indices,
                center,
                outer_left,
                outer_right,
                radius,
                color,
            );
            continue;
        }

        let inner_left = slerp_direction(center, left, inner_t);
        let inner_right = slerp_direction(center, right, inner_t);
        append_spherical_triangle(
            positions,
            normals,
            colors,
            indices,
            inner_left,
            outer_left,
            inner_right,
            radius,
            color,
        );
        append_spherical_triangle(
            positions,
            normals,
            colors,
            indices,
            inner_right,
            outer_left,
            outer_right,
            radius,
            color,
        );
    }
}

pub(crate) fn append_tessellated_spherical_outline_edge(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: DVec3,
    right: DVec3,
    radius: f64,
) {
    let segments = globe_edge_segments(left, right);
    for index in 0..segments {
        let left_t = segment_fraction(index, segments);
        let right_t = segment_fraction(index + 1, segments);
        append_globe_line_segment(
            positions,
            normals,
            indices,
            slerp_direction(left, right, left_t),
            slerp_direction(left, right, right_t),
            radius,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper writes positions, normals, colors, and indices for one triangle."
)]
fn append_spherical_triangle(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    first: DVec3,
    second: DVec3,
    third: DVec3,
    radius: f64,
    color: Option<[f32; 4]>,
) {
    let start = positions.len();
    push_globe_vertex(positions, normals, first, radius);
    push_globe_vertex(positions, normals, second, radius);
    push_globe_vertex(positions, normals, third, radius);
    if let Some(color) = color {
        colors.extend_from_slice(&[color; 3]);
    }
    append_triangle_indices(indices, start);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Globe outline mesh coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
fn append_globe_line_segment(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: DVec3,
    right: DVec3,
    radius: f64,
) {
    let left = left.normalize_or_zero() * radius;
    let right = right.normalize_or_zero() * radius;
    append_line_segment(
        positions,
        indices,
        [left.x as f32, left.y as f32, left.z as f32],
        [right.x as f32, right.y as f32, right.z as f32],
    );
    push_normal(normals, left);
    push_normal(normals, right);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy mesh normals are f32 while spherical geometry is computed in f64."
)]
fn push_normal(normals: &mut Vec<[f32; 3]>, position: DVec3) {
    let normal = position.normalize_or_zero();
    normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Globe overlay mesh coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
fn push_globe_vertex(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    direction: DVec3,
    radius: f64,
) {
    let normal = direction.normalize_or_zero();
    let position = normal * radius;
    positions.push([position.x as f32, position.y as f32, position.z as f32]);
    normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
}

fn append_triangle_indices(indices: &mut Vec<u32>, start: usize) {
    let Ok(start) = u32::try_from(start) else {
        return;
    };
    indices.extend_from_slice(&[start, start + 1, start + 2]);
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "A finite angular span is converted into a small bounded tessellation count."
)]
pub(crate) fn globe_edge_segments(left: DVec3, right: DVec3) -> usize {
    let angle = angular_distance(left, right);
    if !angle.is_finite() || angle <= 0.0 {
        return 1;
    }

    ((angle / DEFAULT_GLOBE_MAX_EDGE_SEGMENT_RADIANS).ceil() as usize)
        .clamp(1, DEFAULT_GLOBE_MAX_EDGE_SEGMENTS)
}

fn angular_distance(left: DVec3, right: DVec3) -> f64 {
    let left = left.normalize_or_zero();
    let right = right.normalize_or_zero();
    left.dot(right).clamp(-1.0, 1.0).acos()
}

pub(crate) fn slerp_direction(left: DVec3, right: DVec3, t: f64) -> DVec3 {
    let left = left.normalize_or_zero();
    let right = right.normalize_or_zero();
    let dot = left.dot(right).clamp(-1.0, 1.0);
    if dot > 0.999_5 {
        return (left + (right - left) * t).normalize_or_zero();
    }

    let theta = dot.acos();
    let sin_theta = theta.sin();
    if sin_theta.abs() <= f64::EPSILON {
        return left;
    }

    let left_scale = ((1.0 - t) * theta).sin() / sin_theta;
    let right_scale = (t * theta).sin() / sin_theta;
    (left * left_scale + right * right_scale).normalize_or_zero()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Surface mesh tessellation segment counts are tiny and bounded."
)]
fn segment_fraction(index: usize, segments: usize) -> f64 {
    index as f64 / segments.max(1) as f64
}

pub(crate) fn polygon_center_direction(directions: &[DVec3]) -> DVec3 {
    let mut sum = DVec3::ZERO;
    for direction in directions {
        sum += direction.normalize_or_zero();
    }

    let center = sum.normalize_or_zero();
    if center.length_squared() <= f64::EPSILON {
        directions.first().copied().unwrap_or(DVec3::Z)
    } else {
        center
    }
}

pub(crate) fn globe_surface_direction(lon_lat: LonLat) -> DVec3 {
    globe_surface_position(lon_lat, 1.0).normalize_or_zero()
}

fn globe_surface_position(lon_lat: LonLat, radius: f64) -> DVec3 {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = radius * lat_rad.cos();

    DVec3::new(
        horizontal_radius * lon_rad.sin(),
        radius * lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}
