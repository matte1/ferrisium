use std::collections::BTreeSet;
use std::f64::consts::PI;

use crate::coords::{lon_lat_to_world_pixel_for_projection, LonLat, Vec2d};
use crate::source::TileProjection;
use crate::view::ViewState;

/// Highest tile zoom that can be represented for all supported projections with
/// `u32` tile coordinates.
///
/// Equirectangular sources use a `2x1` root matrix, so zoom 31 would require
/// `2^32` columns. Capping the shared public pyramid at 30 keeps every supported
/// matrix dimension exactly representable.
pub const MAX_TILE_ZOOM: u8 = 30;

/// A single raster tile address in a projected tile matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    /// Discrete zoom level within the raster tile pyramid.
    pub z: u8,
    /// Horizontal tile index at zoom `z`.
    pub x: u32,
    /// Vertical tile index at zoom `z`.
    pub y: u32,
}

/// Geographic bounds of a projected raster tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileLonLatBounds {
    /// Western longitude in degrees.
    pub west_lon_deg: f64,
    /// Eastern longitude in degrees.
    pub east_lon_deg: f64,
    /// Northern latitude in degrees.
    pub north_lat_deg: f64,
    /// Southern latitude in degrees.
    pub south_lat_deg: f64,
}

/// Tile matrix dimensions for one zoom level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileMatrixDimensions {
    /// Number of tile columns at this zoom.
    pub columns: u32,
    /// Number of tile rows at this zoom.
    pub rows: u32,
}

impl TileId {
    /// Creates a tile identifier.
    ///
    /// Zoom values above [`MAX_TILE_ZOOM`] are clamped to keep public tile
    /// addresses inside the `u32` tile pyramid used by Ferrisium.
    #[must_use]
    pub const fn new(mut z: u8, x: u32, y: u32) -> Self {
        if z > MAX_TILE_ZOOM {
            z = MAX_TILE_ZOOM;
        }
        Self { z, x, y }
    }

    /// Creates a tile identifier if the zoom is addressable by Ferrisium.
    #[must_use]
    pub const fn new_checked(z: u8, x: u32, y: u32) -> Option<Self> {
        if z > MAX_TILE_ZOOM {
            None
        } else {
            Some(Self { z, x, y })
        }
    }

    /// Returns the parent tile one zoom level up, if one exists.
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        if self.z == 0 {
            None
        } else {
            Some(Self::new(self.z - 1, self.x / 2, self.y / 2))
        }
    }

    /// Returns the ancestor tile at `zoom`, if `zoom <= self.z`.
    #[must_use]
    pub fn ancestor_at_zoom(self, zoom: u8) -> Option<Self> {
        if zoom > self.z {
            return None;
        }
        if self.z > MAX_TILE_ZOOM || zoom > MAX_TILE_ZOOM {
            return None;
        }

        let shift = u32::from(self.z - zoom);
        Some(Self::new(zoom, self.x >> shift, self.y >> shift))
    }

    /// Returns this tile's offset within one of its ancestors.
    ///
    /// The tuple is `(offset_x, offset_y, scale)`, where `scale` is the number
    /// of child tiles per axis between `ancestor` and `self`.
    #[must_use]
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Tile zoom deltas are bounded to the tile pyramid used by the library."
    )]
    pub fn descendant_offset_within(self, ancestor: Self) -> Option<(u32, u32, u32)> {
        if ancestor.z > self.z {
            return None;
        }
        if self.z > MAX_TILE_ZOOM || ancestor.z > MAX_TILE_ZOOM {
            return None;
        }

        let zoom_delta = u32::from(self.z - ancestor.z);
        if self.x >> zoom_delta != ancestor.x || self.y >> zoom_delta != ancestor.y {
            return None;
        }

        let scale = 1_u32.checked_shl(zoom_delta)?;
        let mask = scale.saturating_sub(1);
        Some((self.x & mask, self.y & mask, scale))
    }
}

/// Returns the top-left world-pixel origin of a tile at its own zoom level.
#[must_use]
pub fn tile_origin_world_pixel(tile: TileId, tile_size: f64) -> Vec2d {
    Vec2d::new(f64::from(tile.x) * tile_size, f64::from(tile.y) * tile_size)
}

/// Returns the world-pixel center of a tile at its own zoom level.
#[must_use]
pub fn tile_center_world_pixel(tile: TileId, tile_size: f64) -> Vec2d {
    tile_origin_world_pixel(tile, tile_size) + Vec2d::splat(tile_size * 0.5)
}

/// Returns every tile identifier at a specific zoom level.
///
/// This is useful for globe-style renderers that need one complete world copy
/// rather than a viewport-clipped slippy-map subset.
#[must_use]
pub fn tiles_at_zoom(zoom: u8) -> Vec<TileId> {
    tiles_at_zoom_for_projection(zoom, TileProjection::WebMercator)
}

/// Returns every tile identifier at a zoom level for the provided projection.
#[must_use]
pub fn tiles_at_zoom_for_projection(zoom: u8, projection: TileProjection) -> Vec<TileId> {
    let dimensions = tile_matrix_dimensions(projection, zoom);
    let mut tiles = Vec::new();

    for y in 0..dimensions.rows {
        for x in 0..dimensions.columns {
            tiles.push(TileId::new(zoom, x, y));
        }
    }

    tiles
}

/// Returns tile matrix dimensions for Ferrisium's supported raster projections.
#[must_use]
pub fn tile_matrix_dimensions(projection: TileProjection, zoom: u8) -> TileMatrixDimensions {
    let zoom = zoom.min(MAX_TILE_ZOOM);
    let rows = 1_u32 << u32::from(zoom);
    let columns = match projection {
        TileProjection::WebMercator => rows,
        TileProjection::Equirectangular => rows.saturating_mul(2),
    };

    TileMatrixDimensions { columns, rows }
}

/// Returns the geographic lon/lat bounds of a Web Mercator tile.
///
/// The result is expressed in degrees and follows the usual slippy-map
/// convention where `y` increases toward the south.
#[must_use]
pub fn tile_lon_lat_bounds(tile: TileId) -> TileLonLatBounds {
    tile_lon_lat_bounds_for_projection(tile, TileProjection::WebMercator)
}

/// Returns geographic lon/lat bounds for a tile in the provided projection.
#[must_use]
pub fn tile_lon_lat_bounds_for_projection(
    tile: TileId,
    projection: TileProjection,
) -> TileLonLatBounds {
    let northwest = tile_sample_lon_lat_for_projection(tile, projection, 0.0, 0.0);
    let southeast = tile_sample_lon_lat_for_projection(tile, projection, 1.0, 1.0);

    TileLonLatBounds {
        west_lon_deg: northwest.lon_deg,
        east_lon_deg: southeast.lon_deg,
        north_lat_deg: northwest.lat_deg,
        south_lat_deg: southeast.lat_deg,
    }
}

/// Returns the geographic coordinate at a tile-local normalized position.
///
/// `(0, 0)` is the tile's northwest corner and `(1, 1)` is its southeast
/// corner. The vertical coordinate is linear in Web Mercator tile space for
/// this compatibility helper.
#[must_use]
pub fn tile_sample_lon_lat(tile: TileId, u: f64, v: f64) -> LonLat {
    tile_sample_lon_lat_for_projection(tile, TileProjection::WebMercator, u, v)
}

/// Returns the geographic coordinate at a tile-local normalized position for a projection.
#[must_use]
pub fn tile_sample_lon_lat_for_projection(
    tile: TileId,
    projection: TileProjection,
    u: f64,
    v: f64,
) -> LonLat {
    let tile = TileId::new(tile.z, tile.x, tile.y);
    let dimensions = tile_matrix_dimensions(projection, tile.z);
    let tile_x = f64::from(tile.x.min(dimensions.columns.saturating_sub(1))) + u.clamp(0.0, 1.0);
    let tile_y = f64::from(tile.y.min(dimensions.rows.saturating_sub(1))) + v.clamp(0.0, 1.0);
    let lon_deg = tile_x / f64::from(dimensions.columns) * 360.0 - 180.0;
    let lat_deg = match projection {
        TileProjection::WebMercator => mercator_row_to_lat_deg(tile_y, f64::from(dimensions.rows)),
        TileProjection::Equirectangular => 90.0 - tile_y / f64::from(dimensions.rows) * 180.0,
    };

    LonLat::new(lon_deg, lat_deg)
}

fn mercator_row_to_lat_deg(row: f64, tiles_per_axis: f64) -> f64 {
    let mercator_y = PI * (1.0 - 2.0 * row / tiles_per_axis);
    mercator_y.sinh().atan().to_degrees()
}

/// Returns the set of tiles needed for the current viewport.
///
/// `x` coordinates wrap around the world while `y` coordinates are clamped to
/// the valid projection row range at the current zoom. The returned tiles are
/// unique and sorted in increasing `(z, x, y)` order.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Tile bounds are derived from floored viewport math and then clamped to the valid zoom pyramid."
)]
pub fn visible_tiles(view: &ViewState, overscan_tiles: u32) -> Vec<TileId> {
    let zoom = view.discrete_zoom();
    let dimensions = tile_matrix_dimensions(view.projection, zoom);
    let columns = i64::from(dimensions.columns);
    let rows = i64::from(dimensions.rows);
    let tile_size = view.normalized_tile_size();
    let center_world = lon_lat_to_world_pixel_for_projection(
        view.center,
        view.projection,
        f64::from(zoom),
        tile_size,
    );
    let overscan = tile_size * f64::from(overscan_tiles);
    let half_viewport = view.viewport_size * 0.5;

    let min_world = center_world - half_viewport - Vec2d::splat(overscan);
    let max_world = center_world + half_viewport + Vec2d::splat(overscan);

    let min_tile_x = (min_world.x / tile_size).floor() as i64;
    let max_tile_x = max_visible_tile_index(max_world.x, tile_size);
    let min_tile_y = (min_world.y / tile_size).floor() as i64;
    let max_tile_y = max_visible_tile_index(max_world.y, tile_size);

    let mut tiles = BTreeSet::new();

    for tile_y in min_tile_y..=max_tile_y {
        if !(0..rows).contains(&tile_y) {
            continue;
        }

        for tile_x in min_tile_x..=max_tile_x {
            let Ok(wrapped_x) = u32::try_from(tile_x.rem_euclid(columns)) else {
                continue;
            };
            let Ok(tile_y) = u32::try_from(tile_y) else {
                continue;
            };

            tiles.insert(TileId::new(zoom, wrapped_x, tile_y));
        }
    }

    tiles.into_iter().collect()
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Tile index bounds are derived from floored viewport math before range checks."
)]
fn max_visible_tile_index(max_world: f64, tile_size: f64) -> i64 {
    let boundary_epsilon = max_world.abs().max(tile_size).max(1.0) * f64::EPSILON * 4.0;

    ((max_world - boundary_epsilon) / tile_size).floor() as i64
}
