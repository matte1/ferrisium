//! Shared browser-safe tube mesh helpers for spatial line primitives.

use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

/// Default radial tessellation for narrow tube/ribbon-like scene lines.
pub(crate) const DEFAULT_TUBE_RING_SEGMENTS: usize = 8;

/// Mesh plus the absolute render-space origin it was localized around.
pub(crate) struct TubeMesh {
    pub(crate) origin_units: DVec3,
    pub(crate) mesh: Mesh,
}

pub(crate) fn build_first_point_local_tube_mesh(
    points: &[DVec3],
    width_units: f32,
    ring_segments: usize,
) -> Option<TubeMesh> {
    let origin_units = *points.first()?;
    build_local_tube_mesh(points, width_units, origin_units, ring_segments)
}

pub(crate) fn build_local_tube_mesh(
    points: &[DVec3],
    width_units: f32,
    origin_units: DVec3,
    ring_segments: usize,
) -> Option<TubeMesh> {
    let local_points = points
        .iter()
        .map(|point| *point - origin_units)
        .collect::<Vec<_>>();
    let mesh = build_tube_mesh(&local_points, width_units, ring_segments)?;

    Some(TubeMesh { origin_units, mesh })
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Tube coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
pub(crate) fn build_tube_mesh(
    points: &[DVec3],
    width_units: f32,
    ring_segments: usize,
) -> Option<Mesh> {
    if points.len() < 2 {
        return None;
    }

    let ring_segments = ring_segments.max(3);
    let radius = f64::from(width_units.max(f32::EPSILON)) * 0.5;
    let mut positions = Vec::with_capacity(points.len() * ring_segments);
    let mut normals = Vec::with_capacity(points.len() * ring_segments);
    let mut uvs = Vec::with_capacity(points.len() * ring_segments);
    let mut indices = Vec::with_capacity(points.len().saturating_sub(1) * ring_segments * 6);

    for (index, point) in points.iter().enumerate() {
        let tangent = tube_tangent(points, index);
        let (axis_a, axis_b) = tube_axes(tangent);
        let v = index as f32 / points.len().saturating_sub(1) as f32;

        for segment in 0..ring_segments {
            let angle = std::f64::consts::TAU * segment as f64 / ring_segments as f64;
            let normal = (axis_a * angle.cos() + axis_b * angle.sin()).normalize_or_zero();
            let position = *point + normal * radius;
            positions.push([position.x as f32, position.y as f32, position.z as f32]);
            normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
            uvs.push([segment as f32 / ring_segments as f32, v]);
        }
    }

    for segment in 0..points.len().saturating_sub(1) {
        for ring in 0..ring_segments {
            let next_ring = (ring + 1) % ring_segments;
            let current_0 = u32::try_from(segment * ring_segments + ring).ok()?;
            let next_0 = u32::try_from(segment * ring_segments + next_ring).ok()?;
            let current_1 = u32::try_from((segment + 1) * ring_segments + ring).ok()?;
            let next_1 = u32::try_from((segment + 1) * ring_segments + next_ring).ok()?;
            indices.extend_from_slice(&[current_0, current_1, next_0, next_0, current_1, next_1]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

fn tube_tangent(points: &[DVec3], index: usize) -> DVec3 {
    let previous = points.get(index.saturating_sub(1)).copied();
    let next = points.get(index.saturating_add(1)).copied();
    let tangent = match (previous, next) {
        (Some(previous), Some(next)) => next - previous,
        (Some(previous), None) => points[index] - previous,
        (None, Some(next)) => next - points[index],
        (None, None) => DVec3::X,
    };

    tangent.normalize_or_zero()
}

fn tube_width_axis(tangent: DVec3) -> DVec3 {
    let axis = tangent.cross(DVec3::Y).normalize_or_zero();
    if axis.length_squared() > 0.0 {
        axis
    } else {
        tangent.cross(DVec3::Z).normalize_or_zero()
    }
}

fn tube_axes(tangent: DVec3) -> (DVec3, DVec3) {
    let axis_a = tube_width_axis(tangent);
    let axis_b = tangent.cross(axis_a).normalize_or_zero();
    if axis_a.length_squared() > 0.0 && axis_b.length_squared() > 0.0 {
        (axis_a, axis_b)
    } else {
        (DVec3::X, DVec3::Y)
    }
}
