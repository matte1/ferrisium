//! Request lifecycle systems for the shared primary tile pipeline.
//!
//! The systems here decide which map/globe tiles are currently useful, mark
//! slots as queued/loading/ready/failed, and enforce the cache/request budgets.
//! Rendering remains in `map_render` and globe patch material binding remains
//! in `crate::tile_globe`.

use std::collections::BTreeSet;

use bevy::image::Image;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use ferrisium_core::prelude::{
    available_raster_requests, plan_desired_raster_tiles, raster_tile_can_start_request,
    schedule_raster_retry, select_cache_evictions, validate_raster_fetch, RasterCacheEntry,
    RasterDesiredTileAction, RasterFetchValidation, RasterTileStatus, TileId,
};
use ferrisium_core::tiles::visible_tiles;

use crate::celestial::CelestialFocus;
use crate::globe::{Globe3dState, Globe3dView};
use crate::raster_fetch::{
    decode_raster_image, drain_fetch_results, queue_raster_fetch, redacted_url,
};
use crate::source::{
    supports_current_renderers, supports_focused_globe_renderer, supports_map_renderer,
    ActiveTileSource,
};
use crate::tile_globe::{
    focused_globe_surface_anchor, focused_globe_tile_view, globe_request_tiles_for_view,
    sort_tiles_by_globe_priority_for_view, DynamicBodySurfaceView, GlobeSurfaceAnchorQuery,
    PrimaryGlobeSurfaceAnchorQuery,
};
use crate::tiles::state::{
    TileFetchChannel, TileFetchResult, TilePipelineState, TileSlot, TileSlotState,
};
use crate::view::{map_view_state, FerrisiumViewKind, FerrisiumViewMode, Map2dState, Map2dView};

// Per-frame decode budget keeps tile bursts from monopolizing Bevy updates.
const MAX_TILE_FETCH_RESULTS_PER_FRAME: usize = 8;
// Retry delay in frames for transient HTTP/decode failures.
const FAILED_TILE_RETRY_FRAMES: u64 = 45;

/// Clears cache state when the active tile source changes.
pub(crate) fn sync_tile_source_state(
    active_source: Res<'_, ActiveTileSource>,
    mut tile_pipeline: ResMut<'_, TilePipelineState>,
) {
    ensure_tile_pipeline_source_current(&active_source.0, &mut tile_pipeline);
}

fn ensure_tile_pipeline_source_current(
    active_source: &ferrisium_core::prelude::TileSource,
    tile_pipeline: &mut TilePipelineState,
) {
    if tile_pipeline.last_source.as_ref() == Some(active_source) {
        return;
    }

    if !supports_current_renderers(active_source) {
        warn!(
            "tile source projection {:?} on body {:?} is not supported by the current Ferrisium renderers",
            active_source.projection,
            active_source.body
        );
    }

    tile_pipeline.clear_for_source(active_source);
}

/// Applies completed async tile fetches to the cache within a per-frame budget.
pub(crate) fn drain_tile_fetch_results(
    mut images: ResMut<'_, Assets<Image>>,
    fetch_channel: Res<'_, TileFetchChannel>,
    mut tile_pipeline: ResMut<'_, TilePipelineState>,
) {
    drain_fetch_results(
        &fetch_channel,
        MAX_TILE_FETCH_RESULTS_PER_FRAME,
        "tile fetch receiver lock poisoned",
        "tile fetch channel disconnected",
        |fetch_result| {
            apply_tile_fetch_result(fetch_result, &mut images, &mut tile_pipeline);
        },
    );
}

/// Updates the union request set for active 2D/3D views and queues new fetches.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose each ECS input as an explicit system parameter."
)]
pub(crate) fn update_visible_tiles_and_queue_requests(
    active_source: Res<'_, ActiveTileSource>,
    celestial_focus: Res<'_, CelestialFocus>,
    fetch_channel: Res<'_, TileFetchChannel>,
    view_mode: Res<'_, FerrisiumViewMode>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    map_views: Query<'_, '_, &Map2dState, With<Map2dView>>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    dynamic_surface_views: Query<'_, '_, &DynamicBodySurfaceView>,
    metric_body_anchors: GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
    mut tile_pipeline: ResMut<'_, TilePipelineState>,
) {
    tile_pipeline.frame_index = tile_pipeline.frame_index.saturating_add(1);
    ensure_tile_pipeline_source_current(&active_source.0, &mut tile_pipeline);

    let map_view_state = windows
        .iter()
        .next()
        .zip(map_views.iter().next())
        .map(|(window, active_view)| map_view_state(window, *active_view, &active_source));
    let active_globe = globe_views.iter().next().copied();
    let globe_surface_anchor = focused_globe_surface_anchor(
        *celestial_focus,
        &metric_body_anchors,
        &primary_body_anchors,
    );
    let globe_tile_view = globe_surface_anchor.and_then(|surface_anchor| {
        focused_globe_tile_view(
            *celestial_focus,
            &globe_views,
            &dynamic_surface_views,
            surface_anchor.rotation(),
        )
    });
    let active_kind = view_mode.active_kind(map_view_state.is_some(), active_globe.is_some());
    let mut map_visible_tiles = BTreeSet::new();
    let mut next_visible_tiles = BTreeSet::new();

    let map_source_supported = supports_map_renderer(&active_source.0);
    let globe_source_supported =
        supports_focused_globe_renderer(&active_source.0, celestial_focus.target);
    if !map_source_supported && !globe_source_supported {
        tile_pipeline.map_visible_tiles.clear();
        tile_pipeline.visible_tiles.clear();
        return;
    }

    if map_source_supported {
        if let Some(view_state) = &map_view_state {
            map_visible_tiles.extend(visible_tiles(view_state, tile_pipeline.overscan_tiles));
            next_visible_tiles.extend(map_visible_tiles.iter().copied());
        }
    }
    if globe_source_supported {
        if let Some(tile_view) = globe_tile_view {
            next_visible_tiles.extend(globe_request_tiles_for_view(tile_view, &active_source.0));
        }
    }

    if next_visible_tiles.is_empty() {
        tile_pipeline.map_visible_tiles.clear();
        tile_pipeline.visible_tiles.clear();
        return;
    }
    let mut queued_tiles = apply_desired_tile_updates(&mut tile_pipeline, &next_visible_tiles);

    tile_pipeline.visible_tiles = next_visible_tiles;
    tile_pipeline.map_visible_tiles = map_visible_tiles;

    match active_kind {
        Some(FerrisiumViewKind::Map2d) => {
            if let Some(view_state) = &map_view_state {
                crate::tiles::map_render::sort_tiles_by_view_distance(
                    &mut queued_tiles,
                    view_state,
                );
            }
        }
        Some(FerrisiumViewKind::Globe3d) | None => {
            if let Some(tile_view) = globe_tile_view {
                sort_tiles_by_globe_priority_for_view(
                    &mut queued_tiles,
                    tile_view,
                    &active_source.0,
                );
            }
        }
    }

    let available_requests = available_raster_requests(
        tile_pipeline.max_in_flight_requests,
        tile_pipeline.in_flight_requests(),
    );

    for tile in queued_tiles.into_iter().take(available_requests) {
        queue_tile_request(
            tile,
            &active_source.0,
            fetch_channel.sender(),
            &mut tile_pipeline,
        );
    }
}

/// Applies renderer-neutral desired-tile planning to the Bevy-owned cache.
fn apply_desired_tile_updates(
    tile_pipeline: &mut TilePipelineState,
    next_visible_tiles: &BTreeSet<TileId>,
) -> Vec<TileId> {
    let frame_index = tile_pipeline.frame_index;
    let desired_tile_updates = plan_desired_raster_tiles(
        next_visible_tiles.iter().copied(),
        |tile| tile_slot_status(tile_pipeline.slots.get(&tile)),
        frame_index,
    );
    let mut queued_tiles = Vec::new();

    for update in desired_tile_updates {
        match update.action {
            RasterDesiredTileAction::CreateQueued => {
                tile_pipeline
                    .slots
                    .insert(update.tile, TileSlot::queued(frame_index));
            }
            RasterDesiredTileAction::KeepQueued | RasterDesiredTileAction::KeepExisting => {
                if let Some(slot) = tile_pipeline.slots.get_mut(&update.tile) {
                    slot.mark_touched(frame_index);
                }
            }
            RasterDesiredTileAction::RetryFailed => {
                if let Some(slot) = tile_pipeline.slots.get_mut(&update.tile) {
                    slot.mark_touched(frame_index);
                    slot.state = TileSlotState::Queued;
                } else {
                    tile_pipeline
                        .slots
                        .insert(update.tile, TileSlot::queued(frame_index));
                }
            }
        }

        if update.should_request {
            queued_tiles.push(update.tile);
        }
    }

    queued_tiles
}

/// Evicts old ready/failed cache slots while preserving visible and loading tiles.
pub(crate) fn evict_tile_cache(mut tile_pipeline: ResMut<'_, TilePipelineState>) {
    evict_tile_cache_slots(&mut tile_pipeline);
}

/// Applies the actual cache-size policy to a mutable pipeline state.
pub(super) fn evict_tile_cache_slots(tile_pipeline: &mut TilePipelineState) {
    if tile_pipeline.slots.len() <= tile_pipeline.max_cache_entries {
        return;
    }

    let eviction_candidates = tile_pipeline
        .slots
        .iter()
        .map(|(tile, slot)| {
            RasterCacheEntry::new(
                *tile,
                slot.last_touched_frame,
                tile_pipeline.visible_tiles.contains(tile),
                matches!(slot.state, TileSlotState::Loading { .. }),
            )
        })
        .collect::<Vec<_>>();

    for tile in select_cache_evictions(eviction_candidates, tile_pipeline.max_cache_entries) {
        tile_pipeline.slots.remove(&tile);
    }
}

/// Validates a completed fetch still matches the loading slot, then stores it.
fn apply_tile_fetch_result(
    fetch_result: TileFetchResult,
    images: &mut Assets<Image>,
    tile_pipeline: &mut TilePipelineState,
) {
    tile_pipeline
        .in_flight_request_ids
        .remove(&fetch_result.request_id);

    match validate_raster_fetch(
        tile_slot_status(tile_pipeline.slots.get(&fetch_result.tile)),
        fetch_result.request_id,
    ) {
        RasterFetchValidation::Matches => {}
        RasterFetchValidation::MissingSlot
        | RasterFetchValidation::SlotNotLoading
        | RasterFetchValidation::RequestIdMismatch { .. } => return,
    }

    let Some(slot) = tile_pipeline.slots.get_mut(&fetch_result.tile) else {
        return;
    };

    match fetch_result.result {
        Ok(response) if response.ok => match decode_tile_image(&response) {
            Ok((image, image_size)) => {
                let image = images.add(image);
                slot.state = TileSlotState::Ready { image, image_size };
            }
            Err(error) => {
                warn!(
                    "tile decode failed for {}: {error}",
                    redacted_url(&response.url)
                );
                slot.state = TileSlotState::Failed {
                    retry_at_frame: schedule_raster_retry(
                        tile_pipeline.frame_index,
                        FAILED_TILE_RETRY_FRAMES,
                    ),
                };
            }
        },
        Ok(response) => {
            warn!(
                "tile request failed for {}: {} {}",
                redacted_url(&response.url),
                response.status,
                response.status_text
            );
            slot.state = TileSlotState::Failed {
                retry_at_frame: schedule_raster_retry(
                    tile_pipeline.frame_index,
                    FAILED_TILE_RETRY_FRAMES,
                ),
            };
        }
        Err(error) => {
            warn!(
                "tile request transport error for {}: {error}",
                redacted_url(&fetch_result.url)
            );
            slot.state = TileSlotState::Failed {
                retry_at_frame: schedule_raster_retry(
                    tile_pipeline.frame_index,
                    FAILED_TILE_RETRY_FRAMES,
                ),
            };
        }
    }
}

/// Transitions a queued tile to loading and starts its browser request.
fn queue_tile_request(
    tile: TileId,
    tile_source: &ferrisium_core::prelude::TileSource,
    tile_fetch_sender: &std::sync::mpsc::Sender<TileFetchResult>,
    tile_pipeline: &mut TilePipelineState,
) {
    let Some(tile_url) = tile_source.checked_tile_url(tile) else {
        return;
    };
    let Some(status) = tile_slot_status(tile_pipeline.slots.get(&tile)) else {
        return;
    };
    if !raster_tile_can_start_request(status) {
        return;
    }

    let request_id = tile_pipeline.next_request_id();
    let Some(slot) = tile_pipeline.slots.get_mut(&tile) else {
        return;
    };
    slot.state = TileSlotState::Loading { request_id };
    tile_pipeline.in_flight_request_ids.insert(request_id);

    queue_raster_fetch(tile_url, tile_fetch_sender, move |result, url| {
        TileFetchResult {
            request_id,
            result,
            tile,
            url,
        }
    });
}

/// Converts Bevy-owned cache entries into renderer-neutral lifecycle state.
fn tile_slot_status(slot: Option<&TileSlot>) -> Option<RasterTileStatus> {
    slot.map(|slot| match slot.state {
        TileSlotState::Queued => RasterTileStatus::Queued,
        TileSlotState::Loading { request_id } => RasterTileStatus::Loading { request_id },
        TileSlotState::Ready { .. } => RasterTileStatus::Ready,
        TileSlotState::Failed { retry_at_frame } => RasterTileStatus::Failed { retry_at_frame },
    })
}

/// Decodes a tile image and records the pixel dimensions needed for sprite crops.
fn decode_tile_image(response: &ehttp::Response) -> Result<(Image, UVec2), String> {
    let image = decode_raster_image(response)?;
    let image_size = UVec2::new(
        image.texture_descriptor.size.width,
        image.texture_descriptor.size.height,
    );

    Ok((image, image_size))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_desired_tile_updates, ensure_tile_pipeline_source_current, evict_tile_cache_slots,
    };
    use crate::tiles::state::{TilePipelineState, TileSlot, TileSlotState};
    use bevy::image::Image;
    use bevy::prelude::{Handle, UVec2};
    use ferrisium_core::prelude::{TileId, TileSource};
    use std::collections::BTreeSet;

    #[test]
    fn cache_eviction_keeps_loading_slots_until_fetches_finish() {
        let mut tile_pipeline = TilePipelineState {
            max_cache_entries: 1,
            ..TilePipelineState::default()
        };
        let loading_tile = TileId::new(2, 1, 1);
        let ready_tile = TileId::new(2, 2, 1);

        tile_pipeline.slots.insert(
            loading_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Loading { request_id: 7 },
            },
        );
        tile_pipeline.slots.insert(
            ready_tile,
            TileSlot {
                last_touched_frame: 2,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        evict_tile_cache_slots(&mut tile_pipeline);

        assert!(tile_pipeline.slots.contains_key(&loading_tile));
        assert_eq!(tile_pipeline.slots.len(), 1);
    }

    #[test]
    fn desired_tile_updates_create_retry_touch_and_queue_expected_tiles() {
        let missing = TileId::new(2, 0, 0);
        let queued = TileId::new(2, 1, 0);
        let retryable = TileId::new(2, 2, 0);
        let waiting = TileId::new(2, 3, 0);
        let loading = TileId::new(2, 0, 1);
        let ready = TileId::new(2, 1, 1);
        let mut tile_pipeline = TilePipelineState {
            frame_index: 10,
            ..TilePipelineState::default()
        };

        tile_pipeline.slots.insert(
            queued,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Queued,
            },
        );
        tile_pipeline.slots.insert(
            retryable,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Failed { retry_at_frame: 10 },
            },
        );
        tile_pipeline.slots.insert(
            waiting,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Failed { retry_at_frame: 11 },
            },
        );
        tile_pipeline.slots.insert(
            loading,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Loading { request_id: 7 },
            },
        );
        tile_pipeline.slots.insert(
            ready,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        let desired = BTreeSet::from([missing, queued, retryable, waiting, loading, ready]);
        let queued_tiles = apply_desired_tile_updates(&mut tile_pipeline, &desired);

        assert_eq!(
            queued_tiles.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([missing, queued, retryable])
        );
        for tile in [missing, queued, retryable, waiting, loading, ready] {
            assert_eq!(
                tile_pipeline
                    .slots
                    .get(&tile)
                    .map(|slot| slot.last_touched_frame),
                Some(10)
            );
        }
        assert!(matches!(
            tile_pipeline.slots.get(&retryable).map(|slot| &slot.state),
            Some(TileSlotState::Queued)
        ));
        assert!(matches!(
            tile_pipeline.slots.get(&waiting).map(|slot| &slot.state),
            Some(TileSlotState::Failed { retry_at_frame: 11 })
        ));
        assert!(matches!(
            tile_pipeline.slots.get(&loading).map(|slot| &slot.state),
            Some(TileSlotState::Loading { request_id: 7 })
        ));
    }

    #[test]
    fn request_path_clears_cache_immediately_when_source_changes() {
        let old_source = TileSource::xyz("https://old.example/{z}/{x}/{y}.png");
        let new_source = TileSource::nasa_trek_mars_viking_color();
        let ready_tile = TileId::new(2, 1, 1);
        let mut tile_pipeline = TilePipelineState::default();
        tile_pipeline.clear_for_source(&old_source);
        tile_pipeline.in_flight_request_ids.insert(42);
        tile_pipeline.slots.insert(
            ready_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        ensure_tile_pipeline_source_current(&new_source, &mut tile_pipeline);

        assert_eq!(tile_pipeline.last_source.as_ref(), Some(&new_source));
        assert!(tile_pipeline.slots.is_empty());
        assert_eq!(tile_pipeline.in_flight_requests(), 0);
    }

    #[test]
    fn request_path_keeps_cache_when_source_is_unchanged() {
        let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png");
        let ready_tile = TileId::new(2, 1, 1);
        let mut tile_pipeline = TilePipelineState::default();
        tile_pipeline.clear_for_source(&source);
        tile_pipeline.slots.insert(
            ready_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        ensure_tile_pipeline_source_current(&source, &mut tile_pipeline);

        assert!(tile_pipeline.slots.contains_key(&ready_tile));
    }
}
