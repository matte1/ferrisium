//! Primary globe tile selection and request prioritization.
//!
//! This module adapts `Globe3dState` into the pure selector in
//! `ferrisium_core::globe`, then expands visible tiles with ancestor requests
//! so globe patches can use progressive texture fallback.

use std::collections::BTreeSet;

use bevy::prelude::{Quat, Vec3};
use ferrisium_core::globe::{
    globe_tile_priority_for_projection, select_globe_tiles, GlobeDirection, GlobeTileSelection,
    GlobeTileSelectorConfig, GlobeTileSelectorInputs,
};
use ferrisium_core::prelude::{TileId, TileSource};

use crate::globe::GlobeCameraUpMode;
use crate::globe::{globe_camera_position, normalized_body_rotation_or_identity, Globe3dState};

/// Renderer-facing dynamic surface tile-selection state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlobeTileView {
    pub(crate) radius: f32,
    pub(crate) base_surface_distance: f64,
    pub(crate) current_surface_distance: f64,
    pub(crate) camera_surface_direction: GlobeDirection,
    pub(crate) tile_zoom: u8,
    pub(crate) max_selected_tiles: usize,
}

impl GlobeTileView {
    pub(crate) fn from_globe_state(globe_view: Globe3dState, body_rotation: Quat) -> Self {
        Self {
            radius: globe_view.radius,
            base_surface_distance: globe_view.base_surface_distance(),
            current_surface_distance: globe_view.current_surface_distance(),
            camera_surface_direction: globe_camera_surface_direction(globe_view, body_rotation),
            tile_zoom: globe_view.tile_zoom,
            max_selected_tiles: globe_view.max_selected_tiles,
        }
    }

    pub(crate) fn from_dynamic_surface(
        radius: f32,
        base_surface_distance: f64,
        current_surface_distance: f64,
        camera_surface_direction: Vec3,
        tile_zoom: u8,
        max_selected_tiles: usize,
    ) -> Self {
        let camera_surface_direction = camera_surface_direction.normalize_or_zero();
        Self {
            radius,
            base_surface_distance: base_surface_distance.max(0.0),
            current_surface_distance: current_surface_distance.max(0.0),
            camera_surface_direction: GlobeDirection::new(
                f64::from(camera_surface_direction.x),
                f64::from(camera_surface_direction.y),
                f64::from(camera_surface_direction.z),
            ),
            tile_zoom,
            max_selected_tiles,
        }
    }
}

/// Returns the visible raster tiles for the current globe view.
#[must_use]
#[allow(
    dead_code,
    reason = "Legacy internal wrapper retained while callers migrate to GlobeTileView."
)]
pub(crate) fn globe_visible_tiles(
    globe_view: Globe3dState,
    tile_source: &TileSource,
    body_rotation: Quat,
) -> BTreeSet<TileId> {
    globe_visible_tiles_for_view(
        GlobeTileView::from_globe_state(globe_view, body_rotation),
        tile_source,
    )
}

/// Returns visible raster tiles for any dynamic body-surface view.
#[must_use]
pub(crate) fn globe_visible_tiles_for_view(
    tile_view: GlobeTileView,
    tile_source: &TileSource,
) -> BTreeSet<TileId> {
    globe_tile_selection(tile_view, tile_source).visible_tiles
}

/// Returns the visible globe tiles plus their ancestors for progressive texture fallback.
#[must_use]
#[allow(
    dead_code,
    reason = "Legacy internal wrapper retained while callers migrate to GlobeTileView."
)]
pub(crate) fn globe_request_tiles(
    globe_view: Globe3dState,
    tile_source: &TileSource,
    body_rotation: Quat,
) -> BTreeSet<TileId> {
    globe_request_tiles_for_view(
        GlobeTileView::from_globe_state(globe_view, body_rotation),
        tile_source,
    )
}

/// Returns visible dynamic body-surface tiles plus ancestors for progressive fallback.
#[must_use]
pub(crate) fn globe_request_tiles_for_view(
    tile_view: GlobeTileView,
    tile_source: &TileSource,
) -> BTreeSet<TileId> {
    let visible_tiles = globe_visible_tiles_for_view(tile_view, tile_source);
    let mut request_tiles = visible_tiles.clone();
    let (min_zoom, _) = tile_source.normalized_zoom_range();

    for tile in visible_tiles {
        let mut ancestor = tile.parent();
        while let Some(parent) = ancestor {
            if parent.z < min_zoom {
                break;
            }
            request_tiles.insert(parent);
            ancestor = parent.parent();
        }
    }

    request_tiles
}

/// Prioritizes queued globe tile requests toward useful fallback coverage first,
/// then toward sharp camera-facing detail.
#[allow(
    dead_code,
    reason = "Legacy internal wrapper retained while callers migrate to GlobeTileView."
)]
pub(crate) fn sort_tiles_by_globe_priority(
    tiles: &mut [TileId],
    globe_view: Globe3dState,
    tile_source: &TileSource,
    body_rotation: Quat,
) {
    sort_tiles_by_globe_priority_for_view(
        tiles,
        GlobeTileView::from_globe_state(globe_view, body_rotation),
        tile_source,
    );
}

/// Prioritizes queued requests for any dynamic body-surface view.
pub(crate) fn sort_tiles_by_globe_priority_for_view(
    tiles: &mut [TileId],
    tile_view: GlobeTileView,
    tile_source: &TileSource,
) {
    let camera_surface_direction = tile_view.camera_surface_direction;
    let target_zoom = globe_tile_selection(tile_view, tile_source).tile_zoom;
    let (min_zoom, max_zoom) = tile_source.normalized_zoom_range();
    let base_zoom = tile_view
        .tile_zoom
        .clamp(min_zoom, max_zoom)
        .min(target_zoom);

    tiles.sort_by(|left, right| {
        let left_phase = globe_request_priority_phase(*left, base_zoom);
        let right_phase = globe_request_priority_phase(*right, base_zoom);

        left_phase
            .cmp(&right_phase)
            .then_with(|| {
                if left_phase == GlobeRequestPriorityPhase::BaseFallback {
                    left.z.cmp(&right.z)
                } else {
                    right.z.cmp(&left.z)
                }
            })
            .then_with(|| {
                globe_tile_priority_for_projection(
                    *right,
                    tile_source.projection,
                    camera_surface_direction,
                )
                .total_cmp(&globe_tile_priority_for_projection(
                    *left,
                    tile_source.projection,
                    camera_surface_direction,
                ))
            })
            .then_with(|| left.cmp(right))
    });
}

/// Coarse ordering phase used before fine-grained camera-facing priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GlobeRequestPriorityPhase {
    BaseFallback,
    Detail,
}

/// Groups ancestor/base tiles ahead of detail tiles so fallbacks arrive first.
fn globe_request_priority_phase(tile: TileId, base_zoom: u8) -> GlobeRequestPriorityPhase {
    if tile.z <= base_zoom {
        GlobeRequestPriorityPhase::BaseFallback
    } else {
        GlobeRequestPriorityPhase::Detail
    }
}

/// Adapts the active globe view and source into the core globe selector.
fn globe_tile_selection(tile_view: GlobeTileView, tile_source: &TileSource) -> GlobeTileSelection {
    select_globe_tiles(
        GlobeTileSelectorInputs {
            tile_source,
            base_zoom: tile_view.tile_zoom,
            base_surface_distance: tile_view.base_surface_distance,
            current_surface_distance: tile_view.current_surface_distance,
            camera_surface_direction: tile_view.camera_surface_direction,
        },
        GlobeTileSelectorConfig {
            max_selected_tiles: tile_view.max_selected_tiles,
            ..GlobeTileSelectorConfig::default()
        },
    )
}

/// Returns camera-facing direction in the body's local frame.
pub(super) fn globe_camera_surface_direction(
    globe_view: Globe3dState,
    body_rotation: Quat,
) -> GlobeDirection {
    let camera_position = globe_camera_position(globe_view).normalize_or_zero();
    let body_local_camera_position = match globe_view.camera_up_mode {
        GlobeCameraUpMode::BodyNorth => camera_position.as_vec3(),
        GlobeCameraUpMode::SceneUp => normalized_body_rotation_or_identity(body_rotation)
            .inverse()
            .mul_vec3(camera_position.as_vec3()),
    }
    .normalize_or_zero();

    GlobeDirection::new(
        f64::from(body_local_camera_position.x),
        f64::from(body_local_camera_position.y),
        f64::from(body_local_camera_position.z),
    )
}
