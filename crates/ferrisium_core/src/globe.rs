//! Renderer-agnostic tile selection helpers for globe views.
//!
//! This module intentionally models only the current globe-selection problem:
//! given a tile source, coarse camera distance information, and the camera's
//! outward surface direction, choose which raster tiles should exist for the
//! current frame and which of those tiles should be requested first.
//!
//! The goal is to keep globe LOD logic pure and easily testable before the
//! Bevy integration grows more sophisticated.

use std::collections::BTreeSet;

use crate::coords::LonLat;
use crate::source::{TileProjection, TileSource};
use crate::tiles::{
    tile_matrix_dimensions, tile_sample_lon_lat_for_projection, tiles_at_zoom_for_projection,
    TileId,
};

// Default allows the source max zoom to define the inward LOD ceiling.
const DEFAULT_MAX_ZOOM_STEPS_INWARD: u8 = u8::MAX;
// Negative padding keeps near-horizon tiles alive to avoid visible edge gaps.
const DEFAULT_VISIBLE_DOT_PADDING: f64 = -0.18;
// Halving camera surface height advances roughly one Web Mercator zoom level.
const DEFAULT_ZOOM_IN_THRESHOLD_RATIO: f64 = 0.5;
// Selector budget used when the renderer does not provide a tighter cap.
const DEFAULT_MAX_SELECTED_TILES: usize = 768;
// Full-world seeding above this zoom would materialize too many tiles.
const MAX_EAGER_GLOBE_START_ZOOM: u8 = 5;
// Maximum number of high-start seed tiles kept when base zoom is already high.
const MAX_HIGH_START_SEED_TILES: usize = 128;

/// Minimal 3D direction type used by the globe selector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GlobeDirection {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl GlobeDirection {
    /// Creates a new direction vector.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns the dot product between two vectors.
    #[must_use]
    pub fn dot(self, rhs: Self) -> f64 {
        self.x.mul_add(rhs.x, self.y.mul_add(rhs.y, self.z * rhs.z))
    }

    /// Returns the squared vector length.
    #[must_use]
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    /// Returns a normalized vector, or zero if the length is too small.
    #[must_use]
    pub fn normalize_or_zero(self) -> Self {
        let length_squared = self.length_squared();
        if length_squared <= f64::EPSILON {
            return Self::default();
        }

        let inverse_length = length_squared.sqrt().recip();
        Self::new(
            self.x * inverse_length,
            self.y * inverse_length,
            self.z * inverse_length,
        )
    }
}

/// Pure inputs for selecting globe tiles.
#[derive(Debug, Clone, Copy)]
pub struct GlobeTileSelectorInputs<'a> {
    /// Tile source whose zoom limits bound the selected tiles.
    pub tile_source: &'a TileSource,
    /// Base tile zoom at the base surface distance.
    pub base_zoom: u8,
    /// Surface distance where `base_zoom` applies.
    pub base_surface_distance: f64,
    /// Current camera-to-surface distance.
    pub current_surface_distance: f64,
    /// Normalized direction from the globe center toward the camera.
    pub camera_surface_direction: GlobeDirection,
}

/// Tuning parameters for the current globe tile selector.
///
/// These defaults intentionally stay heuristic, but the zoom threshold follows
/// the Web Mercator pyramid: roughly halving camera height moves one tile zoom
/// level inward until the source's maximum zoom is reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobeTileSelectorConfig {
    /// Maximum number of zoom levels the selector may add while moving inward.
    pub max_zoom_steps_inward: u8,
    /// Dot-product threshold used to keep tiles near the horizon visible.
    pub visible_dot_padding: f64,
    /// Distance ratio that controls how early higher zoom levels are requested.
    pub zoom_in_threshold_ratio: f64,
    /// Hard cap on the number of selected globe tiles.
    ///
    /// The selector refines camera-facing tiles toward the target zoom until
    /// this budget is reached, leaving less central areas at coarser zooms.
    pub max_selected_tiles: usize,
}

impl Default for GlobeTileSelectorConfig {
    fn default() -> Self {
        Self {
            max_zoom_steps_inward: DEFAULT_MAX_ZOOM_STEPS_INWARD,
            visible_dot_padding: DEFAULT_VISIBLE_DOT_PADDING,
            zoom_in_threshold_ratio: DEFAULT_ZOOM_IN_THRESHOLD_RATIO,
            max_selected_tiles: DEFAULT_MAX_SELECTED_TILES,
        }
    }
}

/// Result of one globe tile-selection pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobeTileSelection {
    /// Target zoom level used for the highest-detail selected tiles.
    pub tile_zoom: u8,
    /// Visible tiles for the current globe view.
    ///
    /// The set may contain coarser tiles below [`Self::tile_zoom`] when the
    /// selector budget is reached.
    pub visible_tiles: BTreeSet<TileId>,
}

/// Computes the globe tile zoom implied by the selector inputs.
#[must_use]
pub fn globe_tile_zoom(inputs: GlobeTileSelectorInputs<'_>, config: GlobeTileSelectorConfig) -> u8 {
    let (min_zoom, max_zoom) = normalized_source_zoom_range(inputs.tile_source);
    let mut tile_zoom = inputs.base_zoom.clamp(min_zoom, max_zoom);
    let zoom_cap = max_zoom.min(tile_zoom.saturating_add(config.max_zoom_steps_inward));
    let threshold_ratio = config.zoom_in_threshold_ratio.clamp(f64::EPSILON, 1.0);
    let mut threshold = inputs.base_surface_distance.max(f64::EPSILON) * threshold_ratio;
    let current_surface_distance = inputs.current_surface_distance.max(0.0);

    while current_surface_distance <= threshold && tile_zoom < zoom_cap {
        tile_zoom = tile_zoom.saturating_add(1);
        threshold *= threshold_ratio;
    }

    tile_zoom
}

/// Selects the current globe tiles using the configured heuristic.
#[must_use]
pub fn select_globe_tiles(
    inputs: GlobeTileSelectorInputs<'_>,
    config: GlobeTileSelectorConfig,
) -> GlobeTileSelection {
    let tile_zoom = globe_tile_zoom(inputs, config);
    let camera_surface_direction = inputs.camera_surface_direction.normalize_or_zero();
    let visible_tiles = select_budgeted_tiles(inputs, config, tile_zoom, camera_surface_direction);

    GlobeTileSelection {
        tile_zoom,
        visible_tiles,
    }
}

/// Sorts tiles so the most camera-facing tiles come first.
pub fn sort_globe_tiles_by_priority(
    tiles: &mut [TileId],
    camera_surface_direction: GlobeDirection,
) {
    sort_globe_tiles_by_projection_priority(
        tiles,
        TileProjection::WebMercator,
        camera_surface_direction,
    );
}

/// Sorts tiles for a projected globe source so the most camera-facing tiles come first.
pub fn sort_globe_tiles_by_projection_priority(
    tiles: &mut [TileId],
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
) {
    let camera_surface_direction = camera_surface_direction.normalize_or_zero();

    tiles.sort_by(|left, right| {
        globe_tile_priority_for_projection(*right, projection, camera_surface_direction)
            .total_cmp(&globe_tile_priority_for_projection(
                *left,
                projection,
                camera_surface_direction,
            ))
            .then_with(|| left.cmp(right))
    });
}

/// Computes the current tile priority for a globe request queue.
#[must_use]
pub fn globe_tile_priority(tile: TileId, camera_surface_direction: GlobeDirection) -> f64 {
    globe_tile_priority_for_projection(tile, TileProjection::WebMercator, camera_surface_direction)
}

/// Computes tile priority for a globe source using the source projection's sampling space.
#[must_use]
pub fn globe_tile_priority_for_projection(
    tile: TileId,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
) -> f64 {
    let center_normal = globe_surface_normal(tile_center_lon_lat(tile, projection));

    center_normal.dot(camera_surface_direction.normalize_or_zero())
}

fn tile_faces_camera(
    tile: TileId,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
    visible_dot_padding: f64,
) -> bool {
    tile_max_camera_dot(tile, projection, camera_surface_direction) >= visible_dot_padding
}

fn tile_center_lon_lat(tile: TileId, projection: TileProjection) -> LonLat {
    tile_sample_lon_lat_for_projection(tile, projection, 0.5, 0.5)
}

fn tile_max_camera_dot(
    tile: TileId,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
) -> f64 {
    let camera_surface_direction = camera_surface_direction.normalize_or_zero();

    TILE_VISIBILITY_SAMPLES
        .iter()
        .map(|(column_t, row_t)| {
            globe_surface_normal(tile_sample_lon_lat_for_projection(
                tile, projection, *column_t, *row_t,
            ))
            .dot(camera_surface_direction)
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

// Center-only tests drop large horizon tiles whose centers are already behind
// the limb. Sampling corners and edge midpoints keeps coarse coverage intact.
const TILE_VISIBILITY_SAMPLES: [(f64, f64); 9] = [
    (0.0, 0.0),
    (0.5, 0.0),
    (1.0, 0.0),
    (0.0, 0.5),
    (0.5, 0.5),
    (1.0, 0.5),
    (0.0, 1.0),
    (0.5, 1.0),
    (1.0, 1.0),
];

fn select_budgeted_tiles(
    inputs: GlobeTileSelectorInputs<'_>,
    config: GlobeTileSelectorConfig,
    target_zoom: u8,
    camera_surface_direction: GlobeDirection,
) -> BTreeSet<TileId> {
    let budget = config.max_selected_tiles.max(1);
    let (min_zoom, max_zoom) = normalized_source_zoom_range(inputs.tile_source);
    let start_zoom = inputs.base_zoom.clamp(min_zoom, max_zoom).min(target_zoom);
    let projection = inputs.tile_source.projection;
    let mut selected = initial_globe_tiles(
        start_zoom,
        budget,
        projection,
        camera_surface_direction,
        config.visible_dot_padding,
    );

    if selected.len() > budget {
        return highest_priority_subset(selected, budget, projection, camera_surface_direction);
    }

    while let Some(tile_to_refine) = highest_priority_refinable_tile(
        &selected,
        target_zoom,
        projection,
        camera_surface_direction,
    ) {
        // Refine as a complete quadtree step. Dropping individual children by
        // center-facing tests removes the parent patch and can leave uncovered
        // quadrants near the horizon while panning.
        let children = child_tiles(tile_to_refine);

        let refined_len = selected
            .len()
            .saturating_sub(1)
            .saturating_add(children.len());
        if refined_len > budget {
            break;
        }

        selected.remove(&tile_to_refine);
        selected.extend(children);
    }

    selected
}

fn normalized_source_zoom_range(tile_source: &TileSource) -> (u8, u8) {
    tile_source.normalized_zoom_range()
}

fn initial_globe_tiles(
    start_zoom: u8,
    budget: usize,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
    visible_dot_padding: f64,
) -> BTreeSet<TileId> {
    if start_zoom <= MAX_EAGER_GLOBE_START_ZOOM {
        return tiles_at_zoom_for_projection(start_zoom, projection)
            .into_iter()
            .filter(|tile| {
                tile_faces_camera(
                    *tile,
                    projection,
                    camera_surface_direction,
                    visible_dot_padding,
                )
            })
            .collect();
    }

    camera_neighborhood_tiles(
        start_zoom,
        budget.min(MAX_HIGH_START_SEED_TILES),
        projection,
        camera_surface_direction,
        visible_dot_padding,
    )
}

fn camera_neighborhood_tiles(
    zoom: u8,
    budget: usize,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
    visible_dot_padding: f64,
) -> BTreeSet<TileId> {
    let mut selected = BTreeSet::new();
    let dimensions = tile_matrix_dimensions(projection, zoom);
    let columns = i64::from(dimensions.columns);
    let rows = i64::from(dimensions.rows);
    let center = tile_for_surface_direction(camera_surface_direction, projection, zoom);
    let center_x = i64::from(center.x);
    let center_y = i64::from(center.y);
    let mut radius = 0_i64;

    while selected.len() < budget && radius < columns.max(rows) {
        for y in (center_y - radius)..=(center_y + radius) {
            if !(0..rows).contains(&y) {
                continue;
            }

            for x in (center_x - radius)..=(center_x + radius) {
                if radius > 0
                    && x != center_x - radius
                    && x != center_x + radius
                    && y != center_y - radius
                    && y != center_y + radius
                {
                    continue;
                }

                let tile = TileId::new(
                    zoom,
                    u32::try_from(x.rem_euclid(columns)).unwrap_or(0),
                    u32::try_from(y).unwrap_or(0),
                );
                if tile_faces_camera(
                    tile,
                    projection,
                    camera_surface_direction,
                    visible_dot_padding,
                ) {
                    selected.insert(tile);
                    if selected.len() >= budget {
                        break;
                    }
                }
            }

            if selected.len() >= budget {
                break;
            }
        }

        radius = radius.saturating_add(1);
    }

    selected
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Tile coordinates are clamped to the finite Web Mercator axis before casting."
)]
fn tile_for_surface_direction(
    surface_direction: GlobeDirection,
    projection: TileProjection,
    zoom: u8,
) -> TileId {
    let lon_lat = lon_lat_for_surface_direction(surface_direction, projection);
    let dimensions = tile_matrix_dimensions(projection, zoom);
    let columns = f64::from(dimensions.columns);
    let rows = f64::from(dimensions.rows);
    let x = ((lon_lat.lon_deg + 180.0) / 360.0 * columns)
        .floor()
        .clamp(0.0, columns - 1.0);
    let y = match projection {
        TileProjection::WebMercator => {
            let lat_rad = lon_lat.lat_deg.to_radians();
            ((1.0 - (lat_rad.tan() + lat_rad.cos().recip()).ln() / std::f64::consts::PI)
                * 0.5
                * rows)
                .floor()
                .clamp(0.0, rows - 1.0)
        }
        TileProjection::Equirectangular => ((90.0 - lon_lat.lat_deg) / 180.0 * rows)
            .floor()
            .clamp(0.0, rows - 1.0),
    };

    TileId::new(zoom, x as u32, y as u32)
}

fn lon_lat_for_surface_direction(
    surface_direction: GlobeDirection,
    projection: TileProjection,
) -> LonLat {
    let normalized = surface_direction.normalize_or_zero();
    let lon_deg = normalized.x.atan2(normalized.z).to_degrees();
    let lat_deg = normalized.y.clamp(-1.0, 1.0).asin().to_degrees();

    LonLat::new(lon_deg, lat_deg).clamped_for_projection(projection)
}

fn highest_priority_subset(
    tiles: BTreeSet<TileId>,
    budget: usize,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
) -> BTreeSet<TileId> {
    let mut tiles = tiles.into_iter().collect::<Vec<_>>();
    sort_globe_tiles_by_projection_priority(&mut tiles, projection, camera_surface_direction);
    tiles.truncate(budget);
    tiles.into_iter().collect()
}

fn highest_priority_refinable_tile(
    selected: &BTreeSet<TileId>,
    target_zoom: u8,
    projection: TileProjection,
    camera_surface_direction: GlobeDirection,
) -> Option<TileId> {
    selected
        .iter()
        .copied()
        .filter(|tile| tile.z < target_zoom)
        .max_by(|left, right| {
            globe_tile_priority_for_projection(*left, projection, camera_surface_direction)
                .total_cmp(&globe_tile_priority_for_projection(
                    *right,
                    projection,
                    camera_surface_direction,
                ))
                .then_with(|| right.cmp(left))
        })
}

fn child_tiles(tile: TileId) -> [TileId; 4] {
    let child_zoom = tile.z.saturating_add(1);
    let child_x = tile.x.saturating_mul(2);
    let child_y = tile.y.saturating_mul(2);

    [
        TileId::new(child_zoom, child_x, child_y),
        TileId::new(child_zoom, child_x.saturating_add(1), child_y),
        TileId::new(child_zoom, child_x, child_y.saturating_add(1)),
        TileId::new(
            child_zoom,
            child_x.saturating_add(1),
            child_y.saturating_add(1),
        ),
    ]
}

fn globe_surface_normal(lon_lat: LonLat) -> GlobeDirection {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = lat_rad.cos();

    GlobeDirection::new(
        horizontal_radius * lon_rad.sin(),
        lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
    .normalize_or_zero()
}
