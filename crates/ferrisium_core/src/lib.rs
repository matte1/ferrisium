//! Core geospatial primitives shared across Ferrisium renderers.
//!
//! `ferrisium_core` owns the renderer-agnostic pieces of the stack:
//! coordinate conversion, tile identifiers, shared view-state math, and tile
//! source description. It also defines the celestial spatial vocabulary that
//! ephemeris providers and Bevy renderers share without depending on each
//! other directly. The intent is that both 2D and 3D renderers consume the same
//! model instead of each building their own map logic.

/// Coordinate and projection helpers used by both 2D and 3D views.
pub mod coords;
/// Renderer-agnostic globe tile selection and prioritization helpers.
pub mod globe;
/// Renderer-neutral raster tile lifecycle helpers.
pub mod raster;
/// Tile source descriptors such as XYZ URL templates and zoom limits.
pub mod source;
/// Celestial units, frames, epochs, bodies, and state vectors.
pub mod space;
/// Tile identifiers and viewport-driven visibility helpers.
pub mod tiles;
/// Shared view-state math for mapping between screen space and lon/lat.
pub mod view;

/// Common imports for renderer integrations and examples.
pub mod prelude {
    pub use crate::coords::{
        lon_lat_to_world_pixel_for_projection, projected_world_size,
        world_pixel_to_lon_lat_for_projection, LonLat, Vec2d, DEFAULT_TILE_SIZE,
        WEB_MERCATOR_MAX_LAT_DEG,
    };
    pub use crate::globe::{
        globe_tile_priority, globe_tile_priority_for_projection, globe_tile_zoom,
        select_globe_tiles, sort_globe_tiles_by_priority, sort_globe_tiles_by_projection_priority,
        GlobeDirection, GlobeTileSelection, GlobeTileSelectorConfig, GlobeTileSelectorInputs,
    };
    pub use crate::raster::{
        available_focused_raster_requests, available_raster_requests, clamped_fixed_raster_zoom,
        descendant_normalized_rect, failed_tile_is_retryable, nearest_ready_ancestor,
        plan_desired_raster_tiles, raster_tile_can_start_request, raster_tile_count_at_zoom,
        schedule_raster_retry, select_cache_evictions, tiles_for_raster_matrix,
        validate_raster_fetch, NormalizedTileRect, RasterCacheEntry, RasterDesiredTileAction,
        RasterDesiredTileUpdate, RasterFetchValidation, RasterTileStatus,
    };
    pub use crate::source::{
        NasaTrekRegularBody, NasaTrekRegularBodyDescriptor, PlanetaryBody, TileProjection,
        TileSource, CALLISTO_MEAN_RADIUS_METERS, CERES_MEAN_RADIUS_METERS,
        DIONE_MEAN_RADIUS_METERS, EARTH_MEAN_RADIUS_METERS, ENCELADUS_MEAN_RADIUS_METERS,
        EUROPA_MEAN_RADIUS_METERS, GANYMEDE_MEAN_RADIUS_METERS, IAPETUS_MEAN_RADIUS_METERS,
        IO_MEAN_RADIUS_METERS, MAPBOX_RASTER_TILE_MAX_ZOOM, MAPBOX_SATELLITE_GLOBAL_MAX_ZOOM,
        MAPBOX_STYLE_RASTER_DEFAULT_MAX_ZOOM, MAPBOX_STYLE_RASTER_MAX_ZOOM,
        MARS_MEAN_RADIUS_METERS, MERCURY_MEAN_RADIUS_METERS, MIMAS_MEAN_RADIUS_METERS,
        MOON_MEAN_RADIUS_METERS, NASA_TREK_REGULAR_BODIES, RHEA_MEAN_RADIUS_METERS,
        TETHYS_MEAN_RADIUS_METERS, TITAN_MEAN_RADIUS_METERS, VENUS_MEAN_RADIUS_METERS,
    };
    pub use crate::space::{
        epoch_delta_seconds, equatorial_from_j2000_direction, j2000_unit_vector_from_equatorial,
        nasa_celestial_map_uv_from_j2000_direction, offset_epoch_seconds, sample_epochs_inclusive,
        sample_trajectory_between, spherical_surface_position, spherical_surface_subpoint,
        trajectory_samples_are_stale, BodyId, CelestialBody, EphemerisError, EphemerisProvider,
        EphemerisResult, Epoch, EquatorialDirection, FrameId, FrameOrientation, Kilometers,
        OrientationRequest, QuatD, ReferenceEllipsoid, ReferenceFrame, ReferenceFrameKind,
        RenderScale, StateRequest, StateVector, SurfaceSubpoint, TrajectoryPath,
        TrajectoryTrailConfig, Vec3d, AVERAGE_GREGORIAN_MONTH_SECONDS,
        DEFAULT_RENDER_KILOMETERS_PER_UNIT, METERS_PER_KILOMETER, NANOS_PER_SECOND_F64,
        SUN_MEAN_RADIUS_KILOMETERS, WGS84_EQUATORIAL_RADIUS_KILOMETERS, WGS84_INVERSE_FLATTENING,
    };
    pub use crate::tiles::{
        tile_center_world_pixel, tile_lon_lat_bounds, tile_lon_lat_bounds_for_projection,
        tile_matrix_dimensions, tile_origin_world_pixel, tile_sample_lon_lat,
        tile_sample_lon_lat_for_projection, tiles_at_zoom, tiles_at_zoom_for_projection,
        visible_tiles, TileId, TileLonLatBounds, TileMatrixDimensions, MAX_TILE_ZOOM,
    };
    pub use crate::view::ViewState;
}
