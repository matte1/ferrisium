//! Renderer-neutral raster tile lifecycle helpers.
//!
//! This module is the first extraction boundary for behavior shared by map,
//! globe, and tiled-body renderers. It intentionally avoids renderer assets,
//! ECS state, HTTP transports, and image decoders. Bevy owns those details,
//! while this module owns small pure decisions such as retry timing, request
//! budgeting, cache eviction, and ancestor fallback geometry.

use crate::source::TileSource;
use crate::tiles::{tile_matrix_dimensions, tiles_at_zoom_for_projection, TileId};

/// Renderer-neutral tile lifecycle state without renderer-owned payloads.
///
/// Renderer integrations can store extra data for ready tiles, such as GPU
/// texture handles or decoded image sizes, while still mapping their state into
/// this payload-free form for shared planning decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterTileStatus {
    /// Tile is known to be needed and is waiting for a request slot.
    Queued,
    /// Tile has an active request identified by `request_id`.
    Loading {
        /// Monotonic renderer-local request identifier.
        request_id: u64,
    },
    /// Tile has renderer-owned ready data.
    Ready,
    /// Tile failed and may be retried at or after `retry_at_frame`.
    Failed {
        /// Frame index when this tile becomes eligible to queue again.
        retry_at_frame: u64,
    },
}

/// Planned renderer action for one desired raster tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterDesiredTileAction {
    /// Renderer should create a new queued cache slot for this tile.
    CreateQueued,
    /// Renderer should refresh metadata for an existing queued tile.
    KeepQueued,
    /// Renderer should transition an expired failed tile back to queued.
    RetryFailed,
    /// Renderer should only refresh metadata; no request should be queued.
    KeepExisting,
}

/// Planned update for one desired tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterDesiredTileUpdate {
    /// Desired tile being updated.
    pub tile: TileId,
    /// Cache action the renderer should apply before request queueing.
    pub action: RasterDesiredTileAction,
    /// Whether this tile should enter the request-priority queue.
    pub should_request: bool,
}

impl RasterDesiredTileUpdate {
    /// Creates a planned desired-tile update.
    #[must_use]
    pub const fn new(tile: TileId, action: RasterDesiredTileAction, should_request: bool) -> Self {
        Self {
            tile,
            action,
            should_request,
        }
    }
}

/// Result of validating a completed fetch against the current cache slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterFetchValidation {
    /// Fetch still matches the active loading slot and can be applied.
    Matches,
    /// Cache slot disappeared before the response arrived.
    MissingSlot,
    /// Cache slot exists but is no longer loading.
    SlotNotLoading,
    /// Cache slot is loading, but for a newer request.
    RequestIdMismatch {
        /// Request identifier currently stored by the cache slot.
        expected: u64,
        /// Request identifier carried by the completed response.
        actual: u64,
    },
}

/// Snapshot of one raster cache entry for renderer-neutral eviction decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterCacheEntry {
    /// Tile represented by this cache entry.
    pub tile: TileId,
    /// Frame index of the last useful touch.
    pub last_touched_frame: u64,
    /// Whether this tile is still required by the active view/request set.
    pub visible: bool,
    /// Whether this tile currently has an in-flight request.
    pub loading: bool,
}

impl RasterCacheEntry {
    /// Creates a cache-entry snapshot used by [`select_cache_evictions`].
    #[must_use]
    pub const fn new(tile: TileId, last_touched_frame: u64, visible: bool, loading: bool) -> Self {
        Self {
            tile,
            last_touched_frame,
            visible,
            loading,
        }
    }
}

/// Normalized source rectangle for drawing a descendant from an ancestor tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedTileRect {
    /// Minimum horizontal coordinate in the ancestor tile.
    pub min_x: f64,
    /// Minimum vertical coordinate in the ancestor tile.
    pub min_y: f64,
    /// Maximum horizontal coordinate in the ancestor tile.
    pub max_x: f64,
    /// Maximum vertical coordinate in the ancestor tile.
    pub max_y: f64,
}

impl NormalizedTileRect {
    /// Creates a normalized tile rectangle.
    #[must_use]
    pub const fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }
}

/// Returns whether a failed tile should move back to the queued state.
#[must_use]
pub const fn failed_tile_is_retryable(frame_index: u64, retry_at_frame: u64) -> bool {
    frame_index >= retry_at_frame
}

/// Computes the frame index when a failed tile may be retried.
#[must_use]
pub const fn schedule_raster_retry(frame_index: u64, retry_delay_frames: u64) -> u64 {
    frame_index.saturating_add(retry_delay_frames)
}

/// Computes remaining request capacity for a bounded raster request pool.
#[must_use]
pub const fn available_raster_requests(max_in_flight: usize, in_flight: usize) -> usize {
    max_in_flight.saturating_sub(in_flight)
}

/// Computes remaining request capacity with an optional focused-object burst.
///
/// The focused burst lets a newly focused body start a small number of requests
/// even when background body surfaces already occupy the global request pool.
#[must_use]
pub const fn available_focused_raster_requests(
    max_in_flight: usize,
    total_in_flight: usize,
    focused_in_flight: usize,
    focused: bool,
    focused_min_in_flight: usize,
) -> usize {
    let global_available = available_raster_requests(max_in_flight, total_in_flight);
    if global_available > 0 || !focused {
        return global_available;
    }

    focused_min_in_flight.saturating_sub(focused_in_flight)
}

/// Plans cache updates and request candidates for a desired tile set.
///
/// The returned order matches the input order. Renderers can sort only the
/// tiles with `should_request == true` using view-specific priority afterward.
pub fn plan_desired_raster_tiles(
    desired_tiles: impl IntoIterator<Item = TileId>,
    mut status_for_tile: impl FnMut(TileId) -> Option<RasterTileStatus>,
    frame_index: u64,
) -> Vec<RasterDesiredTileUpdate> {
    desired_tiles
        .into_iter()
        .map(|tile| {
            let (action, should_request) = match status_for_tile(tile) {
                None => (RasterDesiredTileAction::CreateQueued, true),
                Some(RasterTileStatus::Queued) => (RasterDesiredTileAction::KeepQueued, true),
                Some(RasterTileStatus::Failed { retry_at_frame })
                    if failed_tile_is_retryable(frame_index, retry_at_frame) =>
                {
                    (RasterDesiredTileAction::RetryFailed, true)
                }
                Some(
                    RasterTileStatus::Failed { .. }
                    | RasterTileStatus::Loading { .. }
                    | RasterTileStatus::Ready,
                ) => (RasterDesiredTileAction::KeepExisting, false),
            };

            RasterDesiredTileUpdate::new(tile, action, should_request)
        })
        .collect()
}

/// Returns whether a cache slot can transition into loading for a new request.
#[must_use]
pub const fn raster_tile_can_start_request(status: RasterTileStatus) -> bool {
    matches!(status, RasterTileStatus::Queued)
}

/// Validates that a completed fetch response still matches the loading slot.
#[must_use]
pub const fn validate_raster_fetch(
    status: Option<RasterTileStatus>,
    request_id: u64,
) -> RasterFetchValidation {
    match status {
        None => RasterFetchValidation::MissingSlot,
        Some(RasterTileStatus::Loading {
            request_id: expected,
        }) if expected == request_id => RasterFetchValidation::Matches,
        Some(RasterTileStatus::Loading {
            request_id: expected,
        }) => RasterFetchValidation::RequestIdMismatch {
            expected,
            actual: request_id,
        },
        Some(
            RasterTileStatus::Queued | RasterTileStatus::Ready | RasterTileStatus::Failed { .. },
        ) => RasterFetchValidation::SlotNotLoading,
    }
}

/// Selects non-visible, non-loading cache entries for LRU-style eviction.
///
/// Loading tiles are preserved because their responses may still arrive.
/// Visible tiles are preserved so renderers can continue drawing the active
/// view. If too few candidates are evictable, this returns fewer tiles than
/// the overflow count.
#[must_use]
pub fn select_cache_evictions(
    entries: impl IntoIterator<Item = RasterCacheEntry>,
    max_entries: usize,
) -> Vec<TileId> {
    let entries = entries.into_iter().collect::<Vec<_>>();
    let overflow = entries.len().saturating_sub(max_entries);
    if overflow == 0 {
        return Vec::new();
    }

    let mut candidates = entries
        .into_iter()
        .filter(|entry| !entry.visible && !entry.loading)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| entry.last_touched_frame);

    candidates
        .into_iter()
        .take(overflow)
        .map(|entry| entry.tile)
        .collect()
}

/// Finds the nearest ready ancestor for a missing descendant tile.
pub fn nearest_ready_ancestor(
    tile: TileId,
    mut is_ready: impl FnMut(TileId) -> bool,
) -> Option<TileId> {
    let mut current_ancestor = tile.parent();
    while let Some(ancestor) = current_ancestor {
        if is_ready(ancestor) {
            return Some(ancestor);
        }
        current_ancestor = ancestor.parent();
    }

    None
}

/// Computes the normalized ancestor texture rectangle for a descendant tile.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "Tile matrix offsets fit in f64 for supported tile zoom levels."
)]
pub fn descendant_normalized_rect(tile: TileId, ancestor: TileId) -> Option<NormalizedTileRect> {
    let (offset_x, offset_y, scale) = tile.descendant_offset_within(ancestor)?;
    let scale = f64::from(scale);

    Some(NormalizedTileRect::new(
        f64::from(offset_x) / scale,
        f64::from(offset_y) / scale,
        f64::from(offset_x.saturating_add(1)) / scale,
        f64::from(offset_y.saturating_add(1)) / scale,
    ))
}

/// Returns the number of tiles in a source's full matrix at `zoom`.
#[must_use]
pub fn raster_tile_count_at_zoom(source: &TileSource, zoom: u8) -> usize {
    let dimensions = tile_matrix_dimensions(source.projection, zoom);
    usize::try_from(u64::from(dimensions.columns) * u64::from(dimensions.rows))
        .unwrap_or(usize::MAX)
}

/// Clamps a fixed global raster zoom to source limits and a patch budget.
#[must_use]
pub fn clamped_fixed_raster_zoom(
    source: &TileSource,
    requested_zoom: u8,
    max_patches: usize,
) -> u8 {
    let (min_zoom, max_zoom) = source.normalized_zoom_range();
    let mut zoom = requested_zoom.clamp(min_zoom, max_zoom);
    while zoom > min_zoom && raster_tile_count_at_zoom(source, zoom) > max_patches {
        zoom = zoom.saturating_sub(1);
    }
    zoom
}

/// Returns every tile in a source's full matrix at `zoom`.
#[must_use]
pub fn tiles_for_raster_matrix(source: &TileSource, zoom: u8) -> Vec<TileId> {
    tiles_at_zoom_for_projection(zoom, source.projection)
}

#[cfg(test)]
mod tests {
    use super::{
        available_focused_raster_requests, available_raster_requests, descendant_normalized_rect,
        failed_tile_is_retryable, nearest_ready_ancestor, plan_desired_raster_tiles,
        raster_tile_can_start_request, schedule_raster_retry, select_cache_evictions,
        validate_raster_fetch, RasterCacheEntry, RasterDesiredTileAction, RasterFetchValidation,
        RasterTileStatus,
    };
    use crate::{
        source::{TileProjection, TileSource},
        tiles::TileId,
    };

    #[test]
    fn retry_timing_is_frame_based() {
        assert!(!failed_tile_is_retryable(41, 42));
        assert!(failed_tile_is_retryable(42, 42));
        assert!(failed_tile_is_retryable(43, 42));
        assert_eq!(schedule_raster_retry(u64::MAX - 1, 42), u64::MAX);
    }

    #[test]
    fn request_capacity_saturates_at_zero() {
        assert_eq!(available_raster_requests(12, 4), 8);
        assert_eq!(available_raster_requests(4, 12), 0);
    }

    #[test]
    fn focused_request_capacity_allows_small_focused_burst() {
        assert_eq!(available_focused_raster_requests(2, 2, 0, false, 4), 0);
        assert_eq!(available_focused_raster_requests(2, 2, 0, true, 4), 4);
        assert_eq!(available_focused_raster_requests(2, 2, 3, true, 4), 1);
        assert_eq!(available_focused_raster_requests(2, 1, 0, true, 4), 1);
    }

    #[test]
    fn desired_tile_planning_requests_only_missing_queued_and_retryable_tiles() {
        let missing = TileId::new(2, 0, 0);
        let queued = TileId::new(2, 1, 0);
        let retryable = TileId::new(2, 2, 0);
        let waiting = TileId::new(2, 3, 0);
        let loading = TileId::new(2, 0, 1);
        let ready = TileId::new(2, 1, 1);

        let updates = plan_desired_raster_tiles(
            [missing, queued, retryable, waiting, loading, ready],
            |tile| {
                if tile == queued {
                    Some(RasterTileStatus::Queued)
                } else if tile == retryable {
                    Some(RasterTileStatus::Failed { retry_at_frame: 10 })
                } else if tile == waiting {
                    Some(RasterTileStatus::Failed { retry_at_frame: 12 })
                } else if tile == loading {
                    Some(RasterTileStatus::Loading { request_id: 7 })
                } else if tile == ready {
                    Some(RasterTileStatus::Ready)
                } else {
                    None
                }
            },
            10,
        );

        assert_eq!(
            updates,
            vec![
                super::RasterDesiredTileUpdate::new(
                    missing,
                    RasterDesiredTileAction::CreateQueued,
                    true
                ),
                super::RasterDesiredTileUpdate::new(
                    queued,
                    RasterDesiredTileAction::KeepQueued,
                    true
                ),
                super::RasterDesiredTileUpdate::new(
                    retryable,
                    RasterDesiredTileAction::RetryFailed,
                    true
                ),
                super::RasterDesiredTileUpdate::new(
                    waiting,
                    RasterDesiredTileAction::KeepExisting,
                    false
                ),
                super::RasterDesiredTileUpdate::new(
                    loading,
                    RasterDesiredTileAction::KeepExisting,
                    false
                ),
                super::RasterDesiredTileUpdate::new(
                    ready,
                    RasterDesiredTileAction::KeepExisting,
                    false
                ),
            ]
        );
    }

    #[test]
    fn request_start_is_limited_to_queued_tiles() {
        assert!(raster_tile_can_start_request(RasterTileStatus::Queued));
        assert!(!raster_tile_can_start_request(RasterTileStatus::Ready));
        assert!(!raster_tile_can_start_request(RasterTileStatus::Loading {
            request_id: 1
        }));
        assert!(!raster_tile_can_start_request(RasterTileStatus::Failed {
            retry_at_frame: 1
        }));
    }

    #[test]
    fn completed_fetch_validation_rejects_stale_responses() {
        assert_eq!(
            validate_raster_fetch(None, 7),
            RasterFetchValidation::MissingSlot
        );
        assert_eq!(
            validate_raster_fetch(Some(RasterTileStatus::Ready), 7),
            RasterFetchValidation::SlotNotLoading
        );
        assert_eq!(
            validate_raster_fetch(Some(RasterTileStatus::Loading { request_id: 8 }), 7),
            RasterFetchValidation::RequestIdMismatch {
                expected: 8,
                actual: 7,
            }
        );
        assert_eq!(
            validate_raster_fetch(Some(RasterTileStatus::Loading { request_id: 7 }), 7),
            RasterFetchValidation::Matches
        );
    }

    #[test]
    fn cache_eviction_preserves_visible_and_loading_entries() {
        let visible = TileId::new(2, 0, 0);
        let loading = TileId::new(2, 1, 0);
        let old_ready = TileId::new(2, 2, 0);
        let newer_ready = TileId::new(2, 3, 0);

        let evictions = select_cache_evictions(
            [
                RasterCacheEntry::new(visible, 1, true, false),
                RasterCacheEntry::new(loading, 2, false, true),
                RasterCacheEntry::new(newer_ready, 8, false, false),
                RasterCacheEntry::new(old_ready, 3, false, false),
            ],
            2,
        );

        assert_eq!(evictions, vec![old_ready, newer_ready]);
    }

    #[test]
    fn nearest_ready_ancestor_uses_closest_ready_parent() {
        let grandparent = TileId::new(1, 0, 0);
        let parent = TileId::new(2, 1, 1);
        let child = TileId::new(3, 3, 2);

        assert_eq!(
            nearest_ready_ancestor(child, |tile| tile == grandparent),
            Some(grandparent)
        );
        assert_eq!(
            nearest_ready_ancestor(child, |tile| tile == grandparent || tile == parent),
            Some(parent)
        );
    }

    #[test]
    fn descendant_rect_is_normalized_within_ancestor() {
        let parent = TileId::new(2, 1, 1);
        let child = TileId::new(3, 3, 2);
        let rect = descendant_normalized_rect(child, parent);

        assert_eq!(
            rect,
            Some(super::NormalizedTileRect::new(0.5, 0.0, 1.0, 0.5))
        );
    }

    #[test]
    fn fixed_raster_zoom_clamps_to_source_limits_and_patch_budget() {
        let source = TileSource::nasa_trek_moon_lro_wac();
        let zoom = super::clamped_fixed_raster_zoom(&source, 8, 2_048);

        assert!(super::raster_tile_count_at_zoom(&source, zoom) <= 2_048);
        assert!(zoom < 8);
        assert_eq!(super::clamped_fixed_raster_zoom(&source, 0, 2_048), 0);
    }

    #[test]
    fn raster_matrix_tiles_follow_source_projection_shape() {
        let moon = TileSource::nasa_trek_moon_lro_wac();
        let web_mercator = TileSource::xyz("https://example.test/{z}/{x}/{y}.png");

        assert_eq!(moon.projection, TileProjection::Equirectangular);
        assert_eq!(super::tiles_for_raster_matrix(&moon, 3).len(), 128);
        assert_eq!(web_mercator.projection, TileProjection::WebMercator);
        assert_eq!(super::tiles_for_raster_matrix(&web_mercator, 3).len(), 64);
    }
}
