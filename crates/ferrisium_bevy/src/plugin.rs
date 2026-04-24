use bevy::prelude::*;
use big_space::prelude::BigSpaceDefaultPlugins;

use crate::body_tiles::{
    drain_body_surface_tile_fetch_results, sync_tiled_body_surfaces, BodySurfaceTileFetchChannel,
    BodySurfaceTileState,
};
use crate::celestial::{
    sync_celestial_body_placements, CelestialEphemeris, CelestialEpoch, CelestialFocus,
    CelestialSpace, CelestialSpaceEntities,
};
use crate::geo_polygon::{
    sync_geo_polygon_globe_overlays, sync_geo_polygon_map_overlays, GeoPolygonEntities,
};
use crate::geo_polyline::{
    sync_geo_polyline_globe_overlays, sync_geo_polyline_map_overlays, GeoPolylineEntities,
};
use crate::globe::{handle_globe_input, spawn_globe_camera, sync_globe_camera};
use crate::globe_label::{sync_globe_labels, GlobeLabelEntities};
use crate::globe_link::{sync_globe_links, GlobeLinkEntities};
use crate::globe_position::sync_globe_positions;
use crate::h3_overlay::{
    sync_h3_cell_lookups, sync_h3_globe_overlays, sync_h3_globe_picking, sync_h3_map_overlays,
    sync_h3_map_picking, H3CellClicked, H3HoverState, H3OverlayEntities,
};
use crate::input_capture::{reset_ferrisium_input_capture, FerrisiumInputCapture};
use crate::metric_scene::{
    sync_metric_scene_focus_pivot, sync_metric_scene_focus_to_celestial_focus,
    sync_metric_scene_lod_from_camera, sync_metric_scene_object_registry,
    sync_metric_scene_presentation_layers, MetricSceneFocusPivot, MetricSceneFocusSelection,
    MetricSceneObjectRegistry,
};
use crate::metric_visual::{
    sync_metric_object_placements, sync_metric_trajectories, sync_metric_visual_objects,
    MetricVisualEntities,
};
use crate::skybox::{
    queue_deferred_globe_skybox, sync_globe_skybox, GlobeSkyboxState, GlobeSkyboxUploadSettings,
};
use crate::source::ActiveTileSource;
use crate::surface_context::sync_globe_surface_render_contexts;
use crate::tile_globe::{sync_globe_tile_patches, GlobeTileEntities};
use crate::tiles::{
    drain_tile_fetch_results, evict_tile_cache, spawn_map_camera, sync_tile_source_state,
    sync_tile_sprites, update_visible_tiles_and_queue_requests, TileFetchChannel,
    TilePipelineState, TileSpriteEntities,
};
use crate::view::{
    handle_map_input, sync_map_2d_render_contexts, update_pointer_geo, FerrisiumViewMode,
    PointerGeoResource,
};

/// Public schedule sets for systems that depend on Ferrisium view contexts.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FerrisiumSet {
    /// Ferrisium clears per-frame input-capture flags.
    ResetInputCapture,
    /// App/UI systems can mark Ferrisium input as captured in this set.
    CaptureInput,
    /// Ferrisium consumes pointer input after capture systems have run.
    HandleInput,
    /// Ferrisium has updated public 2D map and globe-surface render contexts.
    UpdateViewContexts,
    /// Systems that project or rebuild geospatial overlay geometry.
    ProjectGeoOverlays,
    /// Systems that apply projected geospatial overlay assets to the scene.
    RenderGeoOverlays,
}

/// Installs Ferrisium's current map, globe, and celestial placement systems.
///
/// The plugin provides default camera bootstrap, 2D map input, 3D globe orbit
/// input, pointer lon/lat reporting, tile fetch/cache lifecycle systems,
/// sprite-based 2D raster rendering, globe tile rendering, geospatial surface
/// layers, straight links, and globe labels. It installs `big_space` as the
/// celestial-scale transform propagation layer, so apps
/// adding Bevy's `DefaultPlugins` must disable Bevy's `TransformPlugin` before
/// adding this plugin. Apps can opt into globe skybox rendering with
/// [`crate::GlobeSkybox`] or defer large skybox loads with
/// [`crate::DeferredGlobeSkybox`].
pub struct FerrisiumPlugin;

impl Plugin for FerrisiumPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BigSpaceDefaultPlugins);
        configure_ferrisium_sets(app);
        init_ferrisium_resources(app);
        add_ferrisium_startup_systems(app);
        add_ferrisium_view_systems(app);
        add_ferrisium_render_systems(app);
    }
}

fn configure_ferrisium_sets(app: &mut App) {
    app.configure_sets(
        Update,
        (
            FerrisiumSet::ResetInputCapture,
            FerrisiumSet::CaptureInput,
            FerrisiumSet::HandleInput,
            FerrisiumSet::UpdateViewContexts,
            FerrisiumSet::ProjectGeoOverlays,
            FerrisiumSet::RenderGeoOverlays,
        )
            .chain(),
    );
}

fn init_ferrisium_resources(app: &mut App) {
    app.init_resource::<CelestialSpace>()
        .init_resource::<CelestialSpaceEntities>()
        .init_resource::<CelestialEpoch>()
        .init_resource::<CelestialFocus>()
        .init_resource::<CelestialEphemeris>()
        .init_resource::<PointerGeoResource>()
        .init_resource::<FerrisiumInputCapture>()
        .init_resource::<FerrisiumViewMode>()
        .init_resource::<ActiveTileSource>()
        .init_resource::<TileFetchChannel>()
        .init_resource::<TilePipelineState>()
        .init_resource::<BodySurfaceTileFetchChannel>()
        .init_resource::<BodySurfaceTileState>()
        .init_resource::<TileSpriteEntities>()
        .init_resource::<GlobeTileEntities>()
        .init_resource::<MetricSceneFocusSelection>()
        .init_resource::<MetricSceneFocusPivot>()
        .init_resource::<MetricSceneObjectRegistry>()
        .init_resource::<MetricVisualEntities>()
        .init_resource::<GeoPolygonEntities>()
        .init_resource::<GeoPolylineEntities>()
        .init_resource::<GlobeLabelEntities>()
        .init_resource::<GlobeLinkEntities>()
        .init_resource::<H3OverlayEntities>()
        .init_resource::<H3HoverState>()
        .init_resource::<GlobeSkyboxUploadSettings>()
        .init_resource::<GlobeSkyboxState>()
        .add_message::<H3CellClicked>();
}

fn add_ferrisium_startup_systems(app: &mut App) {
    app.add_systems(PostStartup, (spawn_map_camera, spawn_globe_camera));
}

fn add_ferrisium_view_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            sync_tile_source_state,
            sync_celestial_body_placements,
            sync_globe_positions.after(sync_celestial_body_placements),
            sync_metric_scene_object_registry,
            sync_metric_scene_focus_to_celestial_focus.after(sync_metric_scene_object_registry),
            sync_metric_object_placements,
            sync_metric_scene_focus_pivot
                .after(sync_celestial_body_placements)
                .after(sync_metric_scene_focus_to_celestial_focus)
                .after(sync_metric_object_placements),
            sync_metric_scene_lod_from_camera
                .after(sync_celestial_body_placements)
                .after(sync_metric_scene_focus_to_celestial_focus)
                .after(sync_metric_object_placements)
                .after(sync_metric_scene_focus_pivot),
            sync_metric_scene_presentation_layers.after(sync_metric_scene_lod_from_camera),
            reset_ferrisium_input_capture.in_set(FerrisiumSet::ResetInputCapture),
            handle_map_input.in_set(FerrisiumSet::HandleInput),
            handle_globe_input
                .after(sync_celestial_body_placements)
                .after(sync_metric_scene_focus_pivot)
                .in_set(FerrisiumSet::HandleInput),
            sync_globe_camera
                .after(handle_globe_input)
                .after(sync_metric_scene_focus_pivot),
            sync_map_2d_render_contexts
                .after(handle_map_input)
                .in_set(FerrisiumSet::UpdateViewContexts),
            sync_globe_surface_render_contexts
                .after(sync_globe_camera)
                .after(sync_metric_scene_focus_pivot)
                .in_set(FerrisiumSet::UpdateViewContexts),
            sync_metric_visual_objects
                .after(sync_celestial_body_placements)
                .after(sync_metric_object_placements),
            sync_metric_trajectories.after(sync_metric_object_placements),
            sync_globe_links
                .after(sync_celestial_body_placements)
                .after(sync_globe_positions),
            queue_deferred_globe_skybox,
            update_pointer_geo
                .after(handle_map_input)
                .in_set(FerrisiumSet::HandleInput),
        ),
    );
    app.add_systems(
        Update,
        sync_globe_labels
            .after(sync_globe_positions)
            .after(sync_globe_camera),
    );
}

fn add_ferrisium_render_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            drain_tile_fetch_results.after(sync_tile_source_state),
            drain_body_surface_tile_fetch_results.after(sync_tile_source_state),
            update_visible_tiles_and_queue_requests
                .after(handle_map_input)
                .after(handle_globe_input)
                .after(sync_globe_camera)
                .after(drain_tile_fetch_results),
            evict_tile_cache.after(update_visible_tiles_and_queue_requests),
            sync_tile_sprites.after(evict_tile_cache),
            sync_globe_tile_patches.after(evict_tile_cache),
            sync_geo_polyline_map_overlays
                .after(sync_tile_sprites)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_geo_polygon_map_overlays
                .after(sync_tile_sprites)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_h3_map_overlays
                .after(sync_tile_sprites)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_tiled_body_surfaces
                .after(sync_celestial_body_placements)
                .after(sync_metric_scene_presentation_layers)
                .after(drain_body_surface_tile_fetch_results),
            sync_geo_polyline_globe_overlays
                .after(sync_globe_tile_patches)
                .after(sync_tiled_body_surfaces)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_geo_polygon_globe_overlays
                .after(sync_globe_tile_patches)
                .after(sync_tiled_body_surfaces)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_h3_globe_overlays
                .after(sync_globe_tile_patches)
                .after(sync_tiled_body_surfaces)
                .in_set(FerrisiumSet::ProjectGeoOverlays),
            sync_h3_cell_lookups.after(sync_h3_globe_overlays),
            sync_h3_map_picking
                .after(sync_h3_cell_lookups)
                .after(update_pointer_geo)
                .after(sync_globe_camera),
            sync_h3_globe_picking
                .after(sync_h3_cell_lookups)
                .after(update_pointer_geo)
                .after(sync_globe_camera),
            sync_globe_skybox.after(sync_globe_camera),
        ),
    );
}
