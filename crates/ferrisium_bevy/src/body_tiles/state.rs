//! State for fixed-zoom tiled celestial body surfaces.
//!
//! Unlike the primary Earth globe cache, this state is scoped by body entity so
//! Moon and Mars surfaces can load independently while sharing a transport
//! budget and the same raster fetch helper.

use std::collections::HashMap;

use bevy::image::Image;
use bevy::prelude::*;
use ferrisium_core::prelude::{TileId, TileSource};

use crate::metric_visual::MetricVisualRadiusPolicy;
use crate::raster_fetch::RasterFetchChannel;

// Keep this lower than the primary tile pipeline so Earth interaction remains responsive.
const DEFAULT_BODY_SURFACE_MAX_IN_FLIGHT_REQUESTS: usize = 8;

/// Declares a fixed-zoom raster tile surface for a celestial body entity.
///
/// Attach this component to an entity that also has
/// [`crate::CelestialBodyPlacement`]. The renderer uses the body's physical
/// radius from that placement, then maps `source` tiles onto the body surface.
/// This first version is fixed-zoom and intended for global planetary mosaics
/// such as NASA Trek's equirectangular Moon and Mars products.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TiledBodySurface {
    /// Raster source used for this body.
    pub source: TileSource,
    /// Fixed global tile zoom to load for this surface.
    pub zoom: u8,
}

impl TiledBodySurface {
    /// Creates a fixed-zoom tiled surface.
    #[must_use]
    pub fn fixed_zoom(source: TileSource, zoom: u8) -> Self {
        Self { source, zoom }
    }
}

/// Optional display-radius policy for a tiled body surface.
///
/// Without this component, body-surface patches use the body's true radius in
/// the active [`crate::CelestialSpace`] render scale. Solar-system overview
/// scenes can attach this policy to enlarge a textured globe for legibility
/// while keeping the body's metric position and physical metadata unchanged.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TiledBodySurfaceRadiusPolicy(pub MetricVisualRadiusPolicy);

/// Cache, request, and patch state for all fixed-zoom body surfaces.
#[derive(Resource, Debug)]
pub(crate) struct BodySurfaceTileState {
    pub(super) frame_index: u64,
    pub(super) max_in_flight_requests: usize,
    pub(super) next_request_id: u64,
    pub(super) in_flight_request_ids: HashMap<u64, Entity>,
    pub(super) bodies: HashMap<Entity, BodySurfaceState>,
}

impl Default for BodySurfaceTileState {
    fn default() -> Self {
        Self {
            frame_index: 0,
            max_in_flight_requests: DEFAULT_BODY_SURFACE_MAX_IN_FLIGHT_REQUESTS,
            next_request_id: 1,
            in_flight_request_ids: HashMap::new(),
            bodies: HashMap::new(),
        }
    }
}

impl BodySurfaceTileState {
    /// Allocates a monotonically increasing request identifier.
    pub(super) fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        request_id
    }

    /// Returns the total number of body-surface requests currently in flight.
    pub(super) fn in_flight_requests(&self) -> usize {
        self.in_flight_request_ids.len()
    }

    /// Returns in-flight request count for a specific body entity.
    pub(super) fn in_flight_requests_for_body(&self, body: Entity) -> usize {
        self.in_flight_request_ids
            .values()
            .filter(|in_flight_body| **in_flight_body == body)
            .count()
    }
}

/// Per-body cache and spawned patch tracking.
#[derive(Debug)]
pub(super) struct BodySurfaceState {
    pub(super) source: TileSource,
    pub(super) zoom: u8,
    pub(super) radius: f32,
    pub(super) slots: HashMap<TileId, BodySurfaceTileSlot>,
    pub(super) patches: HashMap<TileId, BodySurfacePatchHandles>,
    pub(super) polar_caps: Option<BodySurfacePolarCapHandles>,
}

impl BodySurfaceState {
    /// Creates empty state for one fixed-zoom body surface.
    pub(super) fn new(source: TileSource, zoom: u8, radius: f32) -> Self {
        Self {
            source,
            zoom,
            radius,
            slots: HashMap::new(),
            patches: HashMap::new(),
            polar_caps: None,
        }
    }
}

/// Handles needed to update or despawn one body-surface patch.
#[derive(Debug)]
pub(super) struct BodySurfacePatchHandles {
    pub(super) entity: Entity,
    pub(super) material: Handle<StandardMaterial>,
}

/// Handles for polar cap fill meshes on Web Mercator body surfaces.
#[derive(Debug)]
pub(super) struct BodySurfacePolarCapHandles {
    pub(super) north: Entity,
    pub(super) south: Entity,
    pub(super) radius: f32,
}

/// Per-tile state for a fixed-zoom body-surface tile.
#[derive(Debug)]
pub(super) struct BodySurfaceTileSlot {
    pub(super) state: BodySurfaceTileSlotState,
}

impl BodySurfaceTileSlot {
    /// Creates a slot waiting to be requested.
    pub(super) const fn queued() -> Self {
        Self {
            state: BodySurfaceTileSlotState::Queued,
        }
    }
}

/// Lifecycle state for one fixed-zoom body-surface raster tile.
#[derive(Debug)]
pub(super) enum BodySurfaceTileSlotState {
    Queued,
    Loading { request_id: u64 },
    Ready { image: Handle<Image> },
    Failed { retry_at_frame: u64 },
}

/// Message sent from async body-surface fetch callbacks into ECS.
pub(crate) struct BodySurfaceTileFetchResult {
    pub(super) body: Entity,
    pub(super) request_id: u64,
    pub(super) result: ehttp::Result<ehttp::Response>,
    pub(super) tile: TileId,
    pub(super) url: String,
}

/// Channel bridge for secondary body tile fetch callbacks.
pub(crate) type BodySurfaceTileFetchChannel = RasterFetchChannel<BodySurfaceTileFetchResult>;

#[cfg(test)]
mod tests {
    use super::TiledBodySurface;
    use ferrisium_core::prelude::TileSource;

    #[test]
    fn fixed_zoom_surface_keeps_source_and_zoom() {
        let source = TileSource::nasa_trek_moon_lro_wac();
        let surface = TiledBodySurface::fixed_zoom(source.clone(), 3);

        assert_eq!(surface.source, source);
        assert_eq!(surface.zoom, 3);
    }
}
