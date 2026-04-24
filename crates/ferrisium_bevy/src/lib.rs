//! Bevy integration for Ferrisium.
//!
//! This crate owns the ECS-facing layer: plugins, bundles, input handling, and
//! the current raster tile pipeline for both Ferrisium's 2D map and 3D globe
//! paths. Shared map math remains in `ferrisium_core`.
//!
//! The current 2D path supports drag-pan, cursor-anchored wheel zoom, bounded
//! HTTP tile fetching, ancestor-tile fallbacks, and simple adjacent-zoom tile
//! retention to smooth visible zoom transitions. The current 3D path adds a
//! mixed-LOD textured globe with mouse/touch orbit, surface-grab pan, and zoom
//! controls backed by that same tile cache. Globe cameras default to a
//! focused-body-north orbit and screen-up convention, with an opt-in scene-up
//! mode for applications that want Bevy `+Y` as the camera up frame.
//! Globe placement is backed by `big_space`; applications using Bevy's
//! `DefaultPlugins` must disable Bevy's `TransformPlugin` before adding
//! `FerrisiumPlugin`.
//!
//! ## API layers
//!
//! [`prelude`] is the intended app-facing import layer: plugin entry points,
//! resources, components, bundles, and the shared Ferrisium core/H3 value types
//! most apps need while spawning views and scene content. The project is still
//! pre-release, so this is the recommended surface rather than a stability
//! guarantee. It intentionally avoids raw Bevy systems and lower-level
//! conversion/math helpers.
//!
//! The crate root re-exports a broader advanced layer for applications that
//! need explicit scheduling or custom integration points. Import systems such
//! as [`handle_metric_orbit_camera_input`], [`sync_metric_orbit_camera`],
//! [`sync_metric_scene_focus_to_celestial_focus`], and
//! [`sync_metric_scene_focus_pivot`] from `ferrisium_bevy::...` when wiring a
//! schedule by hand.
//!
//! [`FerrisiumPlugin`] installs the default map, close-globe, tile, celestial,
//! metric-scene, H3, geodetic surface overlay, globe-link, globe-label, and
//! skybox systems. It does not install metric orbit camera controls for
//! arbitrary metric scenes. Apps that spawn a
//! [`MetricOrbitCamera`] should provide [`MetricOrbitCameraState`] and
//! [`MetricOrbitCameraSettings`], then schedule
//! [`handle_metric_orbit_camera_input`] before [`sync_metric_orbit_camera`]
//! after any app-specific focus updates and before systems that consume the
//! metric camera transform. Apps with overlay UI can set
//! [`FerrisiumInputCapture`] in [`FerrisiumSet::CaptureInput`] to suppress
//! Ferrisium pointer controls while the UI owns the pointer.

mod body_tiles;
mod celestial;
mod geometry;
mod globe;
mod input_capture;
mod metric;
mod overlays;
mod plugin;
mod raster;
mod surface;
mod tile_globe;
mod tiles;
mod view;

pub(crate) use geometry::tube_mesh;
pub(crate) use globe::{
    object_anchor as globe_anchor, object_label as globe_label, object_link as globe_link,
    object_position as globe_position, skybox_config as skybox,
};
pub(crate) use metric::{camera as metric_camera, scene as metric_scene, visual as metric_visual};
pub(crate) use overlays::{common as overlay_common, geo_polygon, geo_polyline, h3 as h3_overlay};
pub(crate) use raster::{fetch as raster_fetch, source};
pub(crate) use surface::{
    context as surface_context, depth as surface_depth, mesh as surface_mesh,
};

/// Fixed-zoom tiled raster surfaces for secondary celestial bodies.
pub use body_tiles::{TiledBodySurface, TiledBodySurfaceRadiusPolicy};
/// Bevy-side celestial placement configuration and conversion helpers.
pub use celestial::{
    quatd_to_bevy, CelestialBodyPlacement, CelestialEphemeris, CelestialEpoch, CelestialFocus,
    CelestialPlacement, CelestialRoot, CelestialSpace, PrimaryCelestialBody,
    SharedEphemerisProvider, DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
};
/// Geodetic polygons for map and globe views.
pub use geo_polygon::{GeoPolygon, GeoPolygonBundle, GeoPolygonVertex};
/// Geodetic polylines for map and globe views.
pub use geo_polyline::{GeoEndpoint, GeoPolyline, GeoPolylineBundle};
/// ECS-facing 3D globe view components.
pub use globe::{Globe3dCamera, Globe3dState, Globe3dView, Globe3dViewBundle, GlobeCameraUpMode};
/// Shared globe-space anchors for link endpoints, label positions, and anchored drawing.
pub use globe_anchor::GlobeAnchor;
/// Camera-facing labels anchored to positions around globe bodies.
pub use globe_label::{
    GlobeLabel, GlobeLabelBundle, GlobeLabelCamera, GlobeLabelPosition,
    DEFAULT_GLOBE_LABEL_FONT_SIZE, DEFAULT_GLOBE_LABEL_Z, GLOBE_LABEL_RENDER_LAYER,
};
/// Straight 3D links between positions around globe bodies.
pub use globe_link::{
    GlobeLink, GlobeLinkBundle, GlobeLinkEndpoint, DEFAULT_GLOBE_LINK_WIDTH_UNITS,
};
/// Physical object placement around rendered globe bodies.
pub use globe_position::{GlobePosition, GlobePositionCoordinates};
/// H3 coverage overlays for map and globe views.
pub use h3_overlay::{
    h3_cell_boundary_polygons, h3_cell_map_boundary_polygons, h3_cell_spherical_boundary, H3Cell,
    H3CellBoundary, H3CellClicked, H3CellColors, H3CellHit, H3CellLookup, H3CellLookupEntry,
    H3CellPolygon, H3Cells, H3HoverState, H3Layer, H3Overlay, H3OverlayBundle, H3PickSurface,
    H3Style,
};
/// Shared pointer-capture state for UI integrations.
pub use input_capture::FerrisiumInputCapture;
/// Advanced orbit camera systems and math helpers for large metric scenes.
pub use metric_camera::{
    clamp_metric_orbit_motion_delta_to_extent, handle_metric_orbit_camera_input,
    metric_orbit_angles, metric_orbit_zoom_distance, normalize_metric_orbit_yaw_deg,
    sync_metric_orbit_camera, MetricOrbitCamera, MetricOrbitCameraSettings, MetricOrbitCameraState,
};
/// Higher-level object, focus, and advanced scheduling vocabulary for metric scenes.
pub use metric_scene::{
    metric_scene_celestial_focus_target, metric_scene_focus_pivot_units,
    sync_metric_scene_focus_pivot, sync_metric_scene_focus_to_celestial_focus,
    MetricSceneBodyAnchorBundle, MetricSceneBodySurfaceLayerBundle,
    MetricSceneBodyVisualLayerBundle, MetricSceneCamera, MetricSceneFocusPivot,
    MetricSceneFocusPolicy, MetricSceneFocusSelection, MetricSceneFocusTarget, MetricSceneId,
    MetricSceneLodPolicy, MetricSceneMarkerBundle, MetricSceneMember, MetricSceneObject,
    MetricSceneObjectRegistry, MetricSceneObjectRole, MetricScenePresentation,
    MetricScenePresentationLayer, MetricScenePresentationLayerBundle, MetricScenePresentationSet,
    MetricScenePresentationState, MetricSceneSurfaceTarget,
};
/// Metric object, visual-radius, and trajectory rendering primitives.
pub use metric_visual::{
    metric_position_units, metric_state_position_units, MetricObjectPlacement, MetricTrajectory,
    MetricTrajectoryVisual, MetricVisualObject, MetricVisualRadius, MetricVisualRadiusPolicy,
    MetricVisualRadiusSource,
};
/// Main plugin that installs Ferrisium systems into a Bevy app.
pub use plugin::{FerrisiumPlugin, FerrisiumSet};
/// Optional skybox resources for globe cameras.
pub use skybox::{
    DeferredGlobeSkybox, GlobeSkybox, GlobeSkyboxUploadSettings, MilkyWaySkyboxResolution,
    DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES, DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES,
    DEFAULT_MILKY_WAY_SKYBOX_RESOLUTION, FERRISIUM_SKYBOX_ASSET_ROOT,
};
/// Active raster tile source used by map, globe, and body-surface systems.
pub use source::ActiveTileSource;
/// Public coordinate context for custom focused-globe surface drawing.
pub use surface_context::{GlobeSurfaceRenderContext, GlobeSurfaceRenderKey, SurfaceFrame};
/// Shared spherical surface depth helpers for custom globe-surface drawing.
pub use surface_depth::{
    apply_spherical_decal_material, apply_spherical_surface_depth_bias, draped_fill_radius,
    draped_outline_radius, spherical_surface_depth_bias, spherical_surface_layer_offset,
    spherical_surface_layer_radius, SphericalSurfaceLayer,
};
/// Dynamic body-surface tile selection driven by arbitrary metric cameras.
pub use tile_globe::DynamicBodySurfaceView;
/// ECS-facing 2D view components and resources.
pub use view::{
    FerrisiumViewMode, Map2dRenderContext, Map2dRenderKey, Map2dState, Map2dView, Map2dViewBundle,
    PointerGeo, PointerGeoResource,
};

/// Common imports for applications and examples.
pub mod prelude {
    pub use crate::{
        apply_spherical_decal_material, apply_spherical_surface_depth_bias, draped_fill_radius,
        draped_outline_radius, h3_cell_boundary_polygons, h3_cell_map_boundary_polygons,
        h3_cell_spherical_boundary, spherical_surface_depth_bias, spherical_surface_layer_offset,
        spherical_surface_layer_radius, ActiveTileSource, CelestialBodyPlacement,
        CelestialEphemeris, CelestialEpoch, CelestialFocus, CelestialPlacement, CelestialRoot,
        CelestialSpace, DeferredGlobeSkybox, DynamicBodySurfaceView, FerrisiumInputCapture,
        FerrisiumPlugin, FerrisiumSet, FerrisiumViewMode, GeoEndpoint, GeoPolygon,
        GeoPolygonBundle, GeoPolygonVertex, GeoPolyline, GeoPolylineBundle, Globe3dCamera,
        Globe3dState, Globe3dView, Globe3dViewBundle, GlobeAnchor, GlobeCameraUpMode, GlobeLabel,
        GlobeLabelBundle, GlobeLabelCamera, GlobeLabelPosition, GlobeLink, GlobeLinkBundle,
        GlobeLinkEndpoint, GlobePosition, GlobePositionCoordinates, GlobeSkybox,
        GlobeSkyboxUploadSettings, GlobeSurfaceRenderContext, GlobeSurfaceRenderKey, H3Cell,
        H3CellBoundary, H3CellClicked, H3CellColors, H3CellHit, H3CellLookup, H3CellLookupEntry,
        H3CellPolygon, H3Cells, H3HoverState, H3Layer, H3Overlay, H3OverlayBundle, H3PickSurface,
        H3Style, Map2dRenderContext, Map2dRenderKey, Map2dState, Map2dView, Map2dViewBundle,
        MetricObjectPlacement, MetricOrbitCamera, MetricOrbitCameraSettings,
        MetricOrbitCameraState, MetricSceneBodyAnchorBundle, MetricSceneBodySurfaceLayerBundle,
        MetricSceneBodyVisualLayerBundle, MetricSceneCamera, MetricSceneFocusPivot,
        MetricSceneFocusPolicy, MetricSceneFocusSelection, MetricSceneFocusTarget, MetricSceneId,
        MetricSceneLodPolicy, MetricSceneMarkerBundle, MetricSceneMember, MetricSceneObject,
        MetricSceneObjectRegistry, MetricSceneObjectRole, MetricScenePresentation,
        MetricScenePresentationLayer, MetricScenePresentationLayerBundle,
        MetricScenePresentationSet, MetricScenePresentationState, MetricSceneSurfaceTarget,
        MetricTrajectory, MetricTrajectoryVisual, MetricVisualObject, MetricVisualRadius,
        MetricVisualRadiusPolicy, MetricVisualRadiusSource, MilkyWaySkyboxResolution, PointerGeo,
        PointerGeoResource, PrimaryCelestialBody, SharedEphemerisProvider, SphericalSurfaceLayer,
        SurfaceFrame, TiledBodySurface, TiledBodySurfaceRadiusPolicy,
        DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM, DEFAULT_GLOBE_LABEL_FONT_SIZE, DEFAULT_GLOBE_LABEL_Z,
        DEFAULT_GLOBE_LINK_WIDTH_UNITS, DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES,
        DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES, DEFAULT_MILKY_WAY_SKYBOX_RESOLUTION,
        FERRISIUM_SKYBOX_ASSET_ROOT, GLOBE_LABEL_RENDER_LAYER,
    };
    pub use ferrisium_core::prelude::*;
}
