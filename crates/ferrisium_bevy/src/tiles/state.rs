//! State resources for the primary map/globe tile cache.
//!
//! This module intentionally stores the cache lifecycle separately from the
//! systems that mutate it, so source changes, fetch completion, cache eviction,
//! sprite rendering, and globe material binding all share one coherent view of
//! tile readiness.

use std::collections::{BTreeSet, HashMap, HashSet};

use bevy::image::Image;
use bevy::prelude::*;
use ferrisium_core::prelude::{TileId, TileSource};

use crate::raster_fetch::RasterFetchChannel;

// One-tile overscan hides pop-in during small pans without over-requesting.
pub(super) const DEFAULT_TILE_OVERSCAN: u32 = 1;
// Cache size large enough for adjacent zoom fallback on desktop browsers.
pub(super) const DEFAULT_MAX_CACHE_ENTRIES: usize = 2048;
// Browser-friendly request concurrency cap for remote raster tile hosts.
pub(super) const DEFAULT_MAX_IN_FLIGHT_REQUESTS: usize = 12;

/// Lifecycle state for one primary tile cache entry.
///
/// The explicit states keep request queueing, decode completion, retry, and
/// cache eviction behavior centralized instead of scattered through render
/// systems.
#[derive(Debug)]
pub(super) enum TileSlotState {
    Queued,
    Loading {
        request_id: u64,
    },
    Ready {
        image: Handle<Image>,
        image_size: UVec2,
    },
    Failed {
        retry_at_frame: u64,
    },
}

/// Per-tile cache entry tracked across request, decode, retry, and eviction.
#[derive(Debug)]
pub(super) struct TileSlot {
    pub(super) last_touched_frame: u64,
    pub(super) state: TileSlotState,
}

impl TileSlot {
    /// Creates a queued slot touched during the provided frame.
    pub(super) const fn queued(frame_index: u64) -> Self {
        Self {
            last_touched_frame: frame_index,
            state: TileSlotState::Queued,
        }
    }

    /// Refreshes the LRU-style timestamp used by cache eviction.
    pub(super) fn mark_touched(&mut self, frame_index: u64) {
        self.last_touched_frame = frame_index;
    }
}

/// Shared tile cache and request-queue state for the current Ferrisium app.
#[derive(Resource, Debug)]
pub(crate) struct TilePipelineState {
    pub(super) last_source: Option<TileSource>,
    pub(super) frame_index: u64,
    pub(super) max_cache_entries: usize,
    pub(super) max_in_flight_requests: usize,
    pub(super) next_request_id: u64,
    pub(super) overscan_tiles: u32,
    pub(super) slots: HashMap<TileId, TileSlot>,
    pub(super) in_flight_request_ids: HashSet<u64>,
    pub(super) map_visible_tiles: BTreeSet<TileId>,
    pub(super) visible_tiles: BTreeSet<TileId>,
}

impl Default for TilePipelineState {
    fn default() -> Self {
        Self {
            last_source: None,
            frame_index: 0,
            max_cache_entries: DEFAULT_MAX_CACHE_ENTRIES,
            max_in_flight_requests: DEFAULT_MAX_IN_FLIGHT_REQUESTS,
            next_request_id: 1,
            overscan_tiles: DEFAULT_TILE_OVERSCAN,
            slots: HashMap::new(),
            in_flight_request_ids: HashSet::new(),
            map_visible_tiles: BTreeSet::new(),
            visible_tiles: BTreeSet::new(),
        }
    }
}

impl TilePipelineState {
    /// Resets cache and visibility state after the active tile source changes.
    pub(super) fn clear_for_source(&mut self, tile_source: &TileSource) {
        self.last_source = Some(tile_source.clone());
        self.slots.clear();
        self.in_flight_request_ids.clear();
        self.map_visible_tiles.clear();
        self.visible_tiles.clear();
    }

    /// Returns the number of primary tile requests currently in flight.
    pub(super) fn in_flight_requests(&self) -> usize {
        self.in_flight_request_ids.len()
    }

    /// Allocates a monotonically increasing request identifier.
    pub(super) fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }
}

/// Message sent back from the async HTTP callback into the Bevy world.
pub(crate) struct TileFetchResult {
    pub(super) request_id: u64,
    pub(super) result: ehttp::Result<ehttp::Response>,
    pub(super) tile: TileId,
    pub(super) url: String,
}

/// Channel bridge between async tile fetch callbacks and ECS systems.
pub(crate) type TileFetchChannel = RasterFetchChannel<TileFetchResult>;

/// Tracks the spawned sprite entity for each rendered 2D tile.
#[derive(Resource, Default)]
pub(crate) struct TileSpriteEntities {
    pub(super) by_tile: HashMap<TileId, Entity>,
}

#[cfg(test)]
mod tests {
    use super::{TilePipelineState, TileSlot, TileSlotState};
    use bevy::image::Image;
    use bevy::prelude::{Handle, UVec2};
    use ferrisium_core::prelude::{TileId, TileSource};

    #[test]
    fn in_flight_count_tracks_fetches_even_if_loading_slots_leave_the_cache() {
        let mut tile_pipeline = TilePipelineState::default();
        let tile = TileId::new(2, 1, 1);

        tile_pipeline.in_flight_request_ids.insert(42);
        tile_pipeline.slots.insert(
            tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Loading { request_id: 42 },
            },
        );
        tile_pipeline.slots.remove(&tile);

        assert_eq!(tile_pipeline.in_flight_requests(), 1);

        tile_pipeline.in_flight_request_ids.remove(&42);

        assert_eq!(tile_pipeline.in_flight_requests(), 0);
    }

    #[test]
    fn source_changes_clear_stale_in_flight_request_ids() {
        let mut tile_pipeline = TilePipelineState::default();
        tile_pipeline.in_flight_request_ids.insert(42);
        tile_pipeline.slots.insert(
            TileId::new(2, 1, 1),
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Loading { request_id: 42 },
            },
        );

        tile_pipeline.clear_for_source(&TileSource::xyz("https://new.example/{z}/{x}/{y}.png"));

        assert_eq!(tile_pipeline.in_flight_requests(), 0);
        assert!(tile_pipeline.slots.is_empty());
    }

    #[test]
    fn ready_slots_store_image_size_for_sprite_cropping() {
        let slot = TileSlot {
            last_touched_frame: 1,
            state: TileSlotState::Ready {
                image: Handle::<Image>::default(),
                image_size: UVec2::splat(256),
            },
        };

        let TileSlotState::Ready { image_size, .. } = slot.state else {
            return;
        };
        assert_eq!(image_size, UVec2::splat(256));
    }
}
