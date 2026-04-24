//! Request lifecycle for fixed-zoom secondary body surfaces.
//!
//! This module owns tile enumeration, zoom clamping, request budgeting, retry
//! timing, and fetch-result application. Rendering and source/body validation
//! live in sibling modules.

use bevy::image::Image;
use bevy::prelude::*;
use ferrisium_core::prelude::{
    available_focused_raster_requests, clamped_fixed_raster_zoom, plan_desired_raster_tiles,
    raster_tile_can_start_request, schedule_raster_retry, tiles_for_raster_matrix,
    validate_raster_fetch, RasterDesiredTileAction, RasterFetchValidation, RasterTileStatus,
    TileId, TileSource,
};

use crate::body_tiles::state::{
    BodySurfaceState, BodySurfaceTileFetchChannel, BodySurfaceTileFetchResult, BodySurfaceTileSlot,
    BodySurfaceTileSlotState, BodySurfaceTileState, TiledBodySurface,
};
use crate::raster_fetch::{
    decode_raster_image, drain_fetch_results, queue_raster_fetch, redacted_url,
};

// Decode a few body tiles per frame to avoid stalls when the full matrix resolves.
const MAX_BODY_SURFACE_FETCH_RESULTS_PER_FRAME: usize = 4;
// Retry failed Trek requests promptly; failed body tiles leave placeholder patches visible.
const FAILED_BODY_SURFACE_RETRY_FRAMES: u64 = 60;
// Fixed-zoom surfaces currently spawn every tile as a mesh, so keep the public
// zoom input bounded until secondary bodies get camera-driven LOD.
const MAX_BODY_SURFACE_PATCHES: usize = 2_048;
// When focus changes while another body owns the request pool, allow a small
// focused-body burst without letting every frame overfill the transport queue.
const FOCUSED_BODY_SURFACE_MIN_IN_FLIGHT_REQUESTS: usize = 4;

/// Drains completed body-surface fetches and applies them to body-scoped slots.
pub(crate) fn drain_body_surface_tile_fetch_results(
    mut images: ResMut<'_, Assets<Image>>,
    fetch_channel: Res<'_, BodySurfaceTileFetchChannel>,
    mut tile_state: ResMut<'_, BodySurfaceTileState>,
) {
    drain_fetch_results(
        &fetch_channel,
        MAX_BODY_SURFACE_FETCH_RESULTS_PER_FRAME,
        "body surface tile fetch receiver lock poisoned",
        "body surface tile fetch channel disconnected",
        |fetch_result| {
            apply_body_surface_fetch_result(fetch_result, &mut images, &mut tile_state);
        },
    );
}

/// Clamps a requested fixed zoom to source limits and the patch budget.
pub(super) fn clamped_surface_zoom(surface: &TiledBodySurface) -> u8 {
    clamped_fixed_raster_zoom(&surface.source, surface.zoom, MAX_BODY_SURFACE_PATCHES)
}

/// Returns the number of patches required for a full global matrix.
#[cfg(test)]
fn body_surface_tile_count(source: &TileSource, zoom: u8) -> usize {
    ferrisium_core::prelude::raster_tile_count_at_zoom(source, zoom)
}

/// Queues missing tiles for a body while respecting global and focus budgets.
pub(super) fn queue_body_surface_tiles(
    body: Entity,
    source: &TileSource,
    zoom: u8,
    focused: bool,
    tile_state: &mut BodySurfaceTileState,
    fetch_channel: &BodySurfaceTileFetchChannel,
) {
    let tiles = body_surface_tiles(source, zoom);
    let frame_index = tile_state.frame_index;
    let queued_tiles = {
        let Some(body_state) = tile_state.bodies.get_mut(&body) else {
            return;
        };
        apply_body_surface_desired_tile_updates(body_state, tiles, frame_index)
    };

    let available_requests = available_body_surface_requests(tile_state, body, focused);
    for tile in queued_tiles.into_iter().take(available_requests) {
        queue_body_surface_tile_request(body, tile, source, fetch_channel, tile_state);
    }
}

/// Applies renderer-neutral desired-tile planning to one body-surface cache.
fn apply_body_surface_desired_tile_updates(
    body_state: &mut BodySurfaceState,
    tiles: impl IntoIterator<Item = TileId>,
    frame_index: u64,
) -> Vec<TileId> {
    let desired_tile_updates = plan_desired_raster_tiles(
        tiles,
        |tile| body_surface_slot_status(body_state.slots.get(&tile)),
        frame_index,
    );
    let mut queued_tiles = Vec::new();

    for update in desired_tile_updates {
        match update.action {
            RasterDesiredTileAction::CreateQueued => {
                body_state
                    .slots
                    .insert(update.tile, BodySurfaceTileSlot::queued());
            }
            RasterDesiredTileAction::KeepQueued | RasterDesiredTileAction::KeepExisting => {}
            RasterDesiredTileAction::RetryFailed => {
                if let Some(slot) = body_state.slots.get_mut(&update.tile) {
                    slot.state = BodySurfaceTileSlotState::Queued;
                } else {
                    body_state
                        .slots
                        .insert(update.tile, BodySurfaceTileSlot::queued());
                }
            }
        }

        if update.should_request {
            queued_tiles.push(update.tile);
        }
    }

    queued_tiles
}

/// Computes request capacity for a body, including the focused-body burst.
fn available_body_surface_requests(
    tile_state: &BodySurfaceTileState,
    body: Entity,
    focused: bool,
) -> usize {
    available_focused_raster_requests(
        tile_state.max_in_flight_requests,
        tile_state.in_flight_requests(),
        tile_state.in_flight_requests_for_body(body),
        focused,
        FOCUSED_BODY_SURFACE_MIN_IN_FLIGHT_REQUESTS,
    )
}

/// Starts one body-surface HTTP request and marks the slot as loading.
fn queue_body_surface_tile_request(
    body: Entity,
    tile: TileId,
    source: &TileSource,
    fetch_channel: &BodySurfaceTileFetchChannel,
    tile_state: &mut BodySurfaceTileState,
) {
    let Some(tile_url) = source.checked_tile_url(tile) else {
        return;
    };
    let Some(status) = body_surface_slot_status(
        tile_state
            .bodies
            .get(&body)
            .and_then(|state| state.slots.get(&tile)),
    ) else {
        return;
    };
    if !raster_tile_can_start_request(status) {
        return;
    }

    let request_id = tile_state.next_request_id();
    let Some(slot) = tile_state
        .bodies
        .get_mut(&body)
        .and_then(|state| state.slots.get_mut(&tile))
    else {
        return;
    };
    slot.state = BodySurfaceTileSlotState::Loading { request_id };
    tile_state.in_flight_request_ids.insert(request_id, body);

    queue_raster_fetch(tile_url, fetch_channel.sender(), move |result, url| {
        BodySurfaceTileFetchResult {
            body,
            request_id,
            result,
            tile,
            url,
        }
    });
}

/// Returns every tile in a body's fixed global matrix.
pub(super) fn body_surface_tiles(source: &TileSource, zoom: u8) -> Vec<TileId> {
    tiles_for_raster_matrix(source, zoom)
}

/// Applies a completed body-surface fetch if it still matches the loading slot.
fn apply_body_surface_fetch_result(
    fetch_result: BodySurfaceTileFetchResult,
    images: &mut Assets<Image>,
    tile_state: &mut BodySurfaceTileState,
) {
    tile_state
        .in_flight_request_ids
        .remove(&fetch_result.request_id);

    match validate_raster_fetch(
        body_surface_slot_status(
            tile_state
                .bodies
                .get(&fetch_result.body)
                .and_then(|body_state| body_state.slots.get(&fetch_result.tile)),
        ),
        fetch_result.request_id,
    ) {
        RasterFetchValidation::Matches => {}
        RasterFetchValidation::MissingSlot
        | RasterFetchValidation::SlotNotLoading
        | RasterFetchValidation::RequestIdMismatch { .. } => return,
    }

    let Some(slot) = tile_state
        .bodies
        .get_mut(&fetch_result.body)
        .and_then(|body_state| body_state.slots.get_mut(&fetch_result.tile))
    else {
        return;
    };

    match fetch_result.result {
        Ok(response) if response.ok => match decode_raster_image(&response) {
            Ok(image) => {
                slot.state = BodySurfaceTileSlotState::Ready {
                    image: images.add(image),
                };
            }
            Err(error) => {
                warn!(
                    "body surface tile decode failed for {}: {error}",
                    redacted_url(&response.url)
                );
                slot.state = BodySurfaceTileSlotState::Failed {
                    retry_at_frame: schedule_raster_retry(
                        tile_state.frame_index,
                        FAILED_BODY_SURFACE_RETRY_FRAMES,
                    ),
                };
            }
        },
        Ok(response) => {
            warn!(
                "body surface tile request failed for {}: {} {}",
                redacted_url(&response.url),
                response.status,
                response.status_text
            );
            slot.state = BodySurfaceTileSlotState::Failed {
                retry_at_frame: schedule_raster_retry(
                    tile_state.frame_index,
                    FAILED_BODY_SURFACE_RETRY_FRAMES,
                ),
            };
        }
        Err(error) => {
            warn!(
                "body surface tile transport error for {}: {error}",
                redacted_url(&fetch_result.url)
            );
            slot.state = BodySurfaceTileSlotState::Failed {
                retry_at_frame: schedule_raster_retry(
                    tile_state.frame_index,
                    FAILED_BODY_SURFACE_RETRY_FRAMES,
                ),
            };
        }
    }
}

/// Converts Bevy-owned body-surface slots into renderer-neutral lifecycle state.
fn body_surface_slot_status(slot: Option<&BodySurfaceTileSlot>) -> Option<RasterTileStatus> {
    slot.map(|slot| match slot.state {
        BodySurfaceTileSlotState::Queued => RasterTileStatus::Queued,
        BodySurfaceTileSlotState::Loading { request_id } => {
            RasterTileStatus::Loading { request_id }
        }
        BodySurfaceTileSlotState::Ready { .. } => RasterTileStatus::Ready,
        BodySurfaceTileSlotState::Failed { retry_at_frame } => {
            RasterTileStatus::Failed { retry_at_frame }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_body_surface_desired_tile_updates, available_body_surface_requests,
        body_surface_tile_count, body_surface_tiles, clamped_surface_zoom,
        MAX_BODY_SURFACE_PATCHES,
    };
    use crate::body_tiles::state::{
        BodySurfaceState, BodySurfaceTileSlot, BodySurfaceTileSlotState, BodySurfaceTileState,
        TiledBodySurface,
    };
    use bevy::image::Image;
    use bevy::prelude::{Entity, Handle};
    use ferrisium_core::prelude::{TileId, TileSource};
    use std::collections::BTreeSet;

    #[test]
    fn moon_trek_surface_tiles_use_equirectangular_matrix_shape() {
        let source = TileSource::nasa_trek_moon_lro_wac();
        let tiles = body_surface_tiles(&source, 3);

        assert_eq!(tiles.len(), 128);
        assert_eq!(tiles.first(), Some(&TileId::new(3, 0, 0)));
        assert_eq!(tiles.last(), Some(&TileId::new(3, 15, 7)));
    }

    #[test]
    fn fixed_zoom_body_surface_clamps_to_patch_budget() {
        let surface = TiledBodySurface::fixed_zoom(TileSource::nasa_trek_moon_lro_wac(), 8);
        let zoom = clamped_surface_zoom(&surface);

        assert!(body_surface_tile_count(&surface.source, zoom) <= MAX_BODY_SURFACE_PATCHES);
        assert!(zoom < 8);
    }

    #[test]
    fn focused_body_can_receive_small_request_burst_when_pool_is_full() {
        let body = Entity::from_raw_u32(1).unwrap_or(Entity::PLACEHOLDER);
        let other_body = Entity::from_raw_u32(2).unwrap_or(Entity::PLACEHOLDER);
        let mut tile_state = BodySurfaceTileState {
            max_in_flight_requests: 2,
            ..BodySurfaceTileState::default()
        };
        tile_state.in_flight_request_ids.insert(1, other_body);
        tile_state.in_flight_request_ids.insert(2, other_body);

        assert_eq!(available_body_surface_requests(&tile_state, body, false), 0);
        assert!(available_body_surface_requests(&tile_state, body, true) > 0);
    }

    #[test]
    fn body_surface_desired_updates_create_retry_and_queue_expected_tiles() {
        let missing = TileId::new(2, 0, 0);
        let queued = TileId::new(2, 1, 0);
        let retryable = TileId::new(2, 2, 0);
        let waiting = TileId::new(2, 3, 0);
        let loading = TileId::new(2, 0, 1);
        let ready = TileId::new(2, 1, 1);
        let mut body_state = BodySurfaceState::new(TileSource::nasa_trek_moon_lro_wac(), 2, 1.0);

        body_state.slots.insert(
            queued,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Queued,
            },
        );
        body_state.slots.insert(
            retryable,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Failed { retry_at_frame: 10 },
            },
        );
        body_state.slots.insert(
            waiting,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Failed { retry_at_frame: 11 },
            },
        );
        body_state.slots.insert(
            loading,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Loading { request_id: 7 },
            },
        );
        body_state.slots.insert(
            ready,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Ready {
                    image: Handle::<Image>::default(),
                },
            },
        );

        let queued_tiles = apply_body_surface_desired_tile_updates(
            &mut body_state,
            [missing, queued, retryable, waiting, loading, ready],
            10,
        );

        assert_eq!(
            queued_tiles.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([missing, queued, retryable])
        );
        assert!(matches!(
            body_state.slots.get(&missing).map(|slot| &slot.state),
            Some(BodySurfaceTileSlotState::Queued)
        ));
        assert!(matches!(
            body_state.slots.get(&retryable).map(|slot| &slot.state),
            Some(BodySurfaceTileSlotState::Queued)
        ));
        assert!(matches!(
            body_state.slots.get(&waiting).map(|slot| &slot.state),
            Some(BodySurfaceTileSlotState::Failed { retry_at_frame: 11 })
        ));
        assert!(matches!(
            body_state.slots.get(&loading).map(|slot| &slot.state),
            Some(BodySurfaceTileSlotState::Loading { request_id: 7 })
        ));
    }
}
