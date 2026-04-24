//! Curved sphere patch mesh generation for raster tiles.
//!
//! Tile-local samples are converted through the source projection into lon/lat
//! positions, then projected onto the body sphere. The same mesh path is used
//! by the primary Earth globe and fixed-zoom secondary body surfaces.

use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::prelude::{tile_sample_lon_lat_for_projection, LonLat, TileId, TileProjection};

/// Builds a curved sphere patch mesh for one raster tile.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy mesh data is f32-based while globe bounds are derived in f64."
)]
pub(crate) fn build_globe_tile_mesh(
    tile: TileId,
    projection: TileProjection,
    radius: f32,
    segments: u32,
    uv_rect: Rect,
) -> Mesh {
    let stride = segments.saturating_add(1);
    let vertex_count = usize::try_from(stride.saturating_mul(stride)).unwrap_or(0);
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    let mut indices = Vec::new();

    for row in 0..=segments {
        let row_t = if segments == 0 {
            0.0
        } else {
            f64::from(row) / f64::from(segments)
        };
        for column in 0..=segments {
            let column_t = if segments == 0 {
                0.0
            } else {
                f64::from(column) / f64::from(segments)
            };
            let lon_lat = tile_sample_lon_lat_for_projection(tile, projection, column_t, row_t);
            let position = globe_surface_position(lon_lat, f64::from(radius));
            let normal = position.normalize_or_zero();

            positions.push([position.x as f32, position.y as f32, position.z as f32]);
            normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
            uvs.push(tile_uv(column_t, row_t, uv_rect));
        }
    }

    for row in 0..segments {
        for column in 0..segments {
            let top_left = row * stride + column;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride;
            let bottom_right = bottom_left + 1;

            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
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
    mesh
}

/// Rewrites only UVs when a patch switches between exact and fallback textures.
pub(super) fn update_globe_tile_mesh_uvs(mesh: &mut Mesh, segments: u32, uv_rect: Rect) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, globe_tile_uvs(segments, uv_rect));
}

/// Builds the full UV grid for a patch with the provided subdivision count.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy mesh UV coordinates are f32 while tile interpolation is f64."
)]
fn globe_tile_uvs(segments: u32, uv_rect: Rect) -> Vec<[f32; 2]> {
    let stride = segments.saturating_add(1);
    let vertex_count = usize::try_from(stride.saturating_mul(stride)).unwrap_or(0);
    let mut uvs = Vec::with_capacity(vertex_count);

    for row in 0..=segments {
        let row_t = if segments == 0 {
            0.0
        } else {
            f64::from(row) / f64::from(segments)
        };

        for column in 0..=segments {
            let column_t = if segments == 0 {
                0.0
            } else {
                f64::from(column) / f64::from(segments)
            };
            uvs.push(tile_uv(column_t, row_t, uv_rect));
        }
    }

    uvs
}

/// Interpolates one tile-local UV within a binding rectangle.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy mesh UV coordinates are f32 while tile interpolation is f64."
)]
fn tile_uv(column_t: f64, row_t: f64, uv_rect: Rect) -> [f32; 2] {
    let u =
        f64::from(uv_rect.min.x) + (f64::from(uv_rect.max.x) - f64::from(uv_rect.min.x)) * column_t;
    let v =
        f64::from(uv_rect.min.y) + (f64::from(uv_rect.max.y) - f64::from(uv_rect.min.y)) * row_t;

    [u as f32, v as f32]
}

/// Returns the default subdivision count for a globe patch at a tile zoom.
pub(crate) fn globe_patch_segments(tile: TileId) -> u32 {
    match tile.z {
        0 | 1 => 48,
        2 => 32,
        3 => 24,
        4 => 16,
        5 => 10,
        _ => 6,
    }
}

/// Converts a lon/lat sample into Bevy globe-space coordinates.
pub(super) fn globe_surface_position(lon_lat: LonLat, radius: f64) -> DVec3 {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = radius * lat_rad.cos();

    DVec3::new(
        horizontal_radius * lon_rad.sin(),
        radius * lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}
