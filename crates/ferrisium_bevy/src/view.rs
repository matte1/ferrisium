//! ECS-facing 2D map view state and input systems.
//!
//! The actual projection math lives in `ferrisium_core::view`. This module
//! adapts Bevy window/input state into that pure view model.

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::window::PrimaryWindow;
use ferrisium_core::coords::{lon_lat_to_world_pixel_for_projection, projected_world_size};
use ferrisium_core::prelude::{LonLat, PlanetaryBody, Vec2d, ViewState};

use crate::globe::Globe3dView;
use crate::input_capture::FerrisiumInputCapture;
use crate::source::ActiveTileSource;

// Zoom delta for mouse wheels that report discrete line scrolls.
const LINE_SCROLL_ZOOM_STEP: f64 = 0.25;
// Zoom delta for touchpads and devices that report pixel scrolls.
const PIXEL_SCROLL_ZOOM_STEP: f64 = 0.0025;

/// Marker component for a 2D Ferrisium map view.
#[derive(Component, Debug, Default)]
pub struct Map2dView;

/// Mutable center/zoom state for a 2D map view.
#[derive(Component, Debug, Clone, Copy)]
pub struct Map2dState {
    /// Geographic center of the current 2D view.
    pub center: LonLat,
    /// Continuous zoom level for interaction and rendering.
    pub zoom: f64,
    /// When false, map tile sprites are despawned and no new tiles are
    /// requested. The map camera remains active so that other render
    /// consumers (e.g. egui) still work.
    pub render_enabled: bool,
}

impl Default for Map2dState {
    fn default() -> Self {
        Self {
            center: LonLat::default(),
            zoom: 2.0,
            render_enabled: true,
        }
    }
}

/// Minimal bundle for spawning a Ferrisium 2D map view.
#[derive(Bundle, Debug, Default)]
pub struct Map2dViewBundle {
    /// Marker component identifying the entity as a Ferrisium 2D map view.
    pub view: Map2dView,
    /// Mutable center/zoom state for the view.
    pub map_state: Map2dState,
}

/// Stable invalidation key for data projected into a 2D map view.
///
/// Custom map drawing systems can store this value alongside generated meshes
/// and rebuild only when their own source data changes or this key changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map2dRenderKey {
    /// Planetary body represented by the active map source.
    pub body: PlanetaryBody,
    /// Active projection used by the source.
    pub projection: ferrisium_core::prelude::TileProjection,
    center_lon_bits: u64,
    center_lat_bits: u64,
    zoom_bits: u64,
    viewport_width_bits: u64,
    viewport_height_bits: u64,
    tile_size_bits: u64,
}

impl Map2dRenderKey {
    fn new(body: PlanetaryBody, view_state: &ViewState) -> Self {
        Self {
            body,
            projection: view_state.projection,
            center_lon_bits: view_state.center.lon_deg.to_bits(),
            center_lat_bits: view_state.center.lat_deg.to_bits(),
            zoom_bits: view_state.zoom.to_bits(),
            viewport_width_bits: view_state.viewport_size.x.to_bits(),
            viewport_height_bits: view_state.viewport_size.y.to_bits(),
            tile_size_bits: view_state.tile_size.to_bits(),
        }
    }
}

/// Renderer-facing coordinate context for custom 2D map drawing.
///
/// Ferrisium keeps this component updated on each [`Map2dView`] entity. App
/// systems can query it to project lon/lat, world-pixel, or screen-space data
/// using the same active source, viewport constraints, and antimeridian
/// wrapping convention as the built-in map layers.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Map2dRenderContext {
    /// Planetary body represented by the active map source.
    pub body: PlanetaryBody,
    /// Shared renderer-neutral view state.
    pub view_state: ViewState,
    /// Projected world-pixel position of the current view center.
    pub center_world: Vec2d,
    /// Full projected world size in pixels at the current continuous zoom.
    pub world_size: Vec2d,
    /// Stable invalidation key for projected map geometry.
    pub key: Map2dRenderKey,
}

impl Map2dRenderContext {
    /// Builds a render context from an already constrained view state.
    #[must_use]
    pub fn from_view_state(body: PlanetaryBody, view_state: ViewState) -> Self {
        let tile_size = view_state.normalized_tile_size();
        let zoom = view_state.zoom;
        let center_world = lon_lat_to_world_pixel_for_projection(
            view_state.center,
            view_state.projection,
            zoom,
            tile_size,
        );
        let world_size = projected_world_size(view_state.projection, tile_size, zoom);
        let key = Map2dRenderKey::new(body.clone(), &view_state);

        Self {
            body,
            view_state,
            center_world,
            world_size,
            key,
        }
    }

    /// Projects lon/lat into the active map's world-pixel coordinate space.
    #[must_use]
    pub fn lon_lat_to_world(&self, lon_lat: LonLat) -> Vec2d {
        lon_lat_to_world_pixel_for_projection(
            lon_lat,
            self.view_state.projection,
            self.view_state.zoom,
            self.view_state.normalized_tile_size(),
        )
    }

    /// Converts an exact world-pixel coordinate into map-local screen space.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "Bevy map mesh positions are f32 after f64 projection math."
    )]
    pub fn world_to_screen(&self, world: Vec2d) -> Vec2 {
        Vec2::new(
            (world.x - self.center_world.x) as f32,
            -((world.y - self.center_world.y) as f32),
        )
    }

    /// Converts a world-pixel coordinate to the repeated world copy nearest the view.
    #[must_use]
    pub fn nearest_world_copy_to_view(&self, world: Vec2d) -> Vec2d {
        Vec2d::new(
            self.center_world.x
                + wrap_periodic_delta(world.x - self.center_world.x, self.world_size.x),
            world.y,
        )
    }

    /// Converts lon/lat to map-local screen space on the world copy nearest the view.
    #[must_use]
    pub fn lon_lat_to_screen(&self, lon_lat: LonLat) -> Vec2 {
        self.world_to_screen(self.nearest_world_copy_to_view(self.lon_lat_to_world(lon_lat)))
    }

    /// Converts an absolute screen position in the viewport to lon/lat.
    #[must_use]
    pub fn screen_to_lon_lat(&self, screen_position: Vec2d) -> LonLat {
        self.view_state.screen_to_lon_lat(screen_position)
    }

    /// Unwraps x coordinates so adjacent points follow the shortest periodic path.
    pub fn unwrap_periodic_x(&self, points: &mut [Vec2d]) {
        unwrap_periodic_line_x(points, self.world_size.x);
    }

    /// Shifts an already continuous world-pixel line or ring near the view center.
    pub fn shift_world_copy_near_view(&self, points: &mut [Vec2d]) {
        shift_periodic_points_near_center(points, self.center_world.x, self.world_size.x);
    }

    /// Projects a world-pixel line string while keeping it continuous across the antimeridian.
    #[must_use]
    pub fn project_world_line_to_screen(&self, world_points: &[Vec2d]) -> Vec<Vec2> {
        let mut points = world_points.to_vec();
        self.unwrap_periodic_x(&mut points);
        self.shift_world_copy_near_view(&mut points);
        points
            .into_iter()
            .map(|world| self.world_to_screen(world))
            .collect()
    }

    /// Projects lon/lat points as a continuous map line string.
    #[must_use]
    pub fn project_line_string_to_screen(&self, points: &[LonLat]) -> Vec<Vec2> {
        let world_points = points
            .iter()
            .map(|point| self.lon_lat_to_world(*point))
            .collect::<Vec<_>>();
        self.project_world_line_to_screen(&world_points)
    }

    /// Builds a simple Bevy line-list mesh from already projected screen points.
    ///
    /// This is a convenience for custom overlays that do not need a specialized
    /// renderer. More advanced users can ignore it and build their own meshes.
    #[must_use]
    pub fn line_list_mesh(screen_points: &[Vec2]) -> Mesh {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for segment in screen_points.windows(2) {
            let start = positions.len();
            positions.push([segment[0].x, segment[0].y, 0.0]);
            positions.push([segment[1].x, segment[1].y, 0.0]);
            let Ok(start) = u32::try_from(start) else {
                continue;
            };
            indices.extend_from_slice(&[start, start + 1]);
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::LineList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }
}

/// Selects which Ferrisium view receives shared mouse and wheel input.
///
/// `Auto` preserves Ferrisium's historical behavior: a 2D map takes priority
/// when present, otherwise a globe receives the input. Applications that render
/// both a globe and a minimap can set this resource explicitly.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FerrisiumViewMode {
    /// Prefer a map when one exists, otherwise use a globe.
    #[default]
    Auto,
    /// Send shared mouse and wheel input to the first `Map2dView`.
    Map2d,
    /// Send shared mouse and wheel input to the first `Globe3dView`.
    Globe3d,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FerrisiumViewKind {
    Map2d,
    Globe3d,
}

impl FerrisiumViewMode {
    pub(crate) fn active_kind(self, has_map: bool, has_globe: bool) -> Option<FerrisiumViewKind> {
        match self {
            Self::Auto => {
                if has_map {
                    Some(FerrisiumViewKind::Map2d)
                } else if has_globe {
                    Some(FerrisiumViewKind::Globe3d)
                } else {
                    None
                }
            }
            Self::Map2d => has_map.then_some(FerrisiumViewKind::Map2d),
            Self::Globe3d => has_globe.then_some(FerrisiumViewKind::Globe3d),
        }
    }
}

/// Cursor state derived from the active map view and primary window.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PointerGeoResource {
    /// Cursor position in window pixels, if the pointer is over the window.
    pub screen_position: Option<Vec2>,
    /// Geographic location under the current cursor position.
    pub lon_lat: Option<LonLat>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct DragState {
    // Stored in window coordinates so drag-pan can be derived from the cursor
    // delta each frame without accumulating its own map-space error.
    last_cursor_position: Option<Vec2>,
}

pub(crate) fn sync_map_2d_render_contexts(
    mut commands: Commands<'_, '_>,
    active_source: Res<'_, ActiveTileSource>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    map_views: Query<'_, '_, (Entity, &Map2dState), With<Map2dView>>,
    contexts: Query<'_, '_, Entity, With<Map2dRenderContext>>,
) {
    let Some(window) = windows.iter().next() else {
        for entity in &contexts {
            commands.entity(entity).remove::<Map2dRenderContext>();
        }
        return;
    };

    for (entity, map_state) in &map_views {
        let view_state = map_view_state(window, *map_state, &active_source);
        let context = Map2dRenderContext::from_view_state(active_source.0.body.clone(), view_state);
        commands.entity(entity).insert(context);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose each ECS input as an explicit system parameter."
)]
pub(crate) fn handle_map_input(
    mut commands: Commands<'_, '_>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    input_capture: Res<'_, FerrisiumInputCapture>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    active_source: Res<'_, ActiveTileSource>,
    view_mode: Res<'_, FerrisiumViewMode>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    mut map_views: Query<'_, '_, &mut Map2dState, With<Map2dView>>,
    globe_views: Query<'_, '_, (), With<Globe3dView>>,
    drag_state: Option<ResMut<'_, DragState>>,
) {
    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Map2d)
    {
        if let Some(mut drag_state) = drag_state {
            drag_state.last_cursor_position = None;
        }
        let _ = mouse_wheel_events.read().count();
        return;
    }

    if input_capture.pointer_captured() {
        if let Some(mut drag_state) = drag_state {
            drag_state.last_cursor_position = None;
        }
        let _ = mouse_wheel_events.read().count();
        return;
    }

    let Some(window) = windows.iter().next() else {
        let _ = mouse_wheel_events.read().count();
        return;
    };

    let Some(mut drag_state) = drag_state else {
        commands.insert_resource(DragState::default());
        let _ = mouse_wheel_events.read().count();
        return;
    };

    let cursor_position = window.cursor_position();
    let Some(mut active_view) = map_views.iter_mut().next() else {
        return;
    };

    // Drag-pan goes through `ViewState` so the same clamping and continuous
    // zoom semantics apply to both direct manipulation and other callers.
    if buttons.pressed(MouseButton::Left) {
        if let (Some(current), Some(previous)) = (cursor_position, drag_state.last_cursor_position)
        {
            let delta = current - previous;
            let mut view_state = map_view_state(window, *active_view, &active_source);
            view_state.pan_by_pixels(Vec2d::new(f64::from(delta.x), f64::from(delta.y)));
            view_state.constrain_to_viewport();
            active_view.center = view_state.center;
            active_view.zoom = view_state.zoom;
        }
    }

    let scroll_delta = mouse_wheel_events
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => f64::from(event.y) * LINE_SCROLL_ZOOM_STEP,
            MouseScrollUnit::Pixel => f64::from(event.y) * PIXEL_SCROLL_ZOOM_STEP,
        })
        .sum::<f64>();
    if scroll_delta.abs() > f64::EPSILON {
        let mut view_state = map_view_state(window, *active_view, &active_source);
        if let Some(cursor_position) = cursor_position {
            // Match established map UX by zooming toward the cursor when one
            // is available instead of always zooming toward the viewport center.
            view_state.zoom_about_screen_position(
                Vec2d::new(f64::from(cursor_position.x), f64::from(cursor_position.y)),
                scroll_delta,
            );
        } else {
            view_state.zoom_by_steps(scroll_delta);
        }
        view_state.constrain_to_viewport();
        let (min_zoom, max_zoom) = active_source.0.normalized_zoom_range();
        active_view.center = view_state.center;
        active_view.zoom = view_state
            .zoom
            .clamp(f64::from(min_zoom), f64::from(max_zoom));
    }

    drag_state.last_cursor_position = cursor_position;
}

pub(crate) fn update_pointer_geo(
    active_source: Res<'_, ActiveTileSource>,
    input_capture: Res<'_, FerrisiumInputCapture>,
    view_mode: Res<'_, FerrisiumViewMode>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    map_views: Query<'_, '_, &Map2dState, With<Map2dView>>,
    globe_views: Query<'_, '_, (), With<Globe3dView>>,
    mut pointer_geo: ResMut<'_, PointerGeoResource>,
) {
    if input_capture.pointer_captured() {
        pointer_geo.screen_position = None;
        pointer_geo.lon_lat = None;
        return;
    }

    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Map2d)
    {
        pointer_geo.screen_position = None;
        pointer_geo.lon_lat = None;
        return;
    }

    let Some(window) = windows.iter().next() else {
        return;
    };

    let Some(active_view) = map_views.iter().next() else {
        pointer_geo.screen_position = None;
        pointer_geo.lon_lat = None;
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        pointer_geo.screen_position = None;
        pointer_geo.lon_lat = None;
        return;
    };

    let view_state = map_view_state(window, *active_view, &active_source);
    let lon_lat = view_state.screen_to_lon_lat(Vec2d::new(
        f64::from(cursor_position.x),
        f64::from(cursor_position.y),
    ));

    pointer_geo.screen_position = Some(cursor_position);
    pointer_geo.lon_lat = Some(lon_lat);
}

/// Backwards-compatible alias for older code that used the pre-refactor name.
pub type PointerGeo = Map2dState;

/// Builds the shared renderer-agnostic view state for the active Bevy window.
pub(crate) fn map_view_state(
    window: &Window,
    map_view: Map2dState,
    active_source: &ActiveTileSource,
) -> ViewState {
    let (min_zoom, max_zoom) = active_source.0.normalized_zoom_range();
    let mut view_state = ViewState {
        center: map_view.center,
        zoom: map_view
            .zoom
            .clamp(f64::from(min_zoom), f64::from(max_zoom)),
        viewport_size: Vec2d::new(f64::from(window.width()), f64::from(window.height())),
        tile_size: f64::from(active_source.0.normalized_tile_size()),
        projection: active_source.0.projection,
    };
    view_state.constrain_to_viewport();
    view_state
}

fn unwrap_periodic_line_x(points: &mut [Vec2d], period: f64) {
    if points.len() < 2 || period <= 0.0 {
        return;
    }

    let mut previous_x = points[0].x;
    for point in points.iter_mut().skip(1) {
        let delta = wrap_periodic_delta(point.x - previous_x, period);
        point.x = previous_x + delta;
        previous_x = point.x;
    }
}

fn shift_periodic_points_near_center(points: &mut [Vec2d], center_x: f64, period: f64) {
    if points.is_empty() || period <= 0.0 || !center_x.is_finite() {
        return;
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for point in points.iter() {
        if !point.x.is_finite() {
            return;
        }
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
    }

    let points_center_x = (min_x + max_x) * 0.5;
    let copy_offset = ((center_x - points_center_x) / period).round() * period;
    for point in points {
        point.x += copy_offset;
    }
}

fn wrap_periodic_delta(delta: f64, period: f64) -> f64 {
    if period <= 0.0 {
        return delta;
    }

    (delta + period * 0.5).rem_euclid(period) - period * 0.5
}

#[cfg(test)]
mod tests {
    use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
    use bevy::prelude::*;
    use bevy::window::PrimaryWindow;
    use ferrisium_core::prelude::{LonLat, PlanetaryBody, TileProjection, Vec2d, ViewState};

    use crate::input_capture::FerrisiumInputCapture;
    use crate::source::ActiveTileSource;

    use super::{
        handle_map_input, DragState, FerrisiumViewKind, FerrisiumViewMode, Map2dRenderContext,
        Map2dState, Map2dView,
    };

    #[test]
    fn auto_view_mode_prefers_maps_but_can_be_overridden() {
        assert_eq!(
            FerrisiumViewMode::Auto.active_kind(true, true),
            Some(FerrisiumViewKind::Map2d)
        );
        assert_eq!(
            FerrisiumViewMode::Globe3d.active_kind(true, true),
            Some(FerrisiumViewKind::Globe3d)
        );
        assert_eq!(FerrisiumViewMode::Map2d.active_kind(false, true), None);
    }

    #[test]
    fn captured_pointer_blocks_map_wheel_input_and_resets_drag_state() {
        let mut capture = FerrisiumInputCapture::default();
        capture.capture_pointer();
        let mut app = map_input_test_app(capture);

        app.update();

        assert_eq!(map_zoom(&mut app), Some(2.0));
        assert_eq!(map_drag_position(&app), None);
    }

    #[test]
    fn uncaptured_pointer_allows_map_wheel_input() {
        let mut app = map_input_test_app(FerrisiumInputCapture::default());

        app.update();

        assert!(map_zoom(&mut app).is_some_and(|zoom| zoom > 2.0));
    }

    #[test]
    fn map_render_context_keeps_transmeridian_lines_continuous() {
        let context = Map2dRenderContext::from_view_state(
            PlanetaryBody::earth(),
            ViewState {
                center: LonLat::new(0.0, 0.0),
                zoom: 2.0,
                viewport_size: Vec2d::new(1024.0, 768.0),
                tile_size: 256.0,
                projection: TileProjection::WebMercator,
            },
        );
        let points = context
            .project_line_string_to_screen(&[LonLat::new(179.5, 1.0), LonLat::new(-179.5, 1.0)]);

        assert_eq!(points.len(), 2);
        assert!(
            (points[0].x - points[1].x).abs() < 8.0,
            "line should use adjacent world copies, got {points:?}"
        );
    }

    #[test]
    fn map_render_context_key_tracks_view_changes() {
        let body = PlanetaryBody::earth();
        let initial = Map2dRenderContext::from_view_state(
            body.clone(),
            ViewState {
                center: LonLat::new(-98.0, 39.0),
                zoom: 3.0,
                viewport_size: Vec2d::new(1024.0, 768.0),
                tile_size: 256.0,
                projection: TileProjection::WebMercator,
            },
        );
        let changed = Map2dRenderContext::from_view_state(
            body,
            ViewState {
                zoom: 4.0,
                ..initial.view_state
            },
        );

        assert_ne!(initial.key, changed.key);
    }

    fn map_input_test_app(capture: FerrisiumInputCapture) -> App {
        let mut app = App::new();
        app.add_message::<MouseWheel>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.insert_resource(capture);
        app.insert_resource(FerrisiumViewMode::Map2d);
        app.insert_resource(ActiveTileSource::default());
        app.insert_resource(DragState {
            last_cursor_position: Some(Vec2::new(400.0, 300.0)),
        });
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.world_mut().spawn((Map2dView, Map2dState::default()));
        app.world_mut()
            .resource_mut::<Messages<MouseWheel>>()
            .write(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: 1.0,
                window,
            });
        app.add_systems(Update, handle_map_input);
        app
    }

    fn map_zoom(app: &mut App) -> Option<f64> {
        app.world_mut()
            .query::<&Map2dState>()
            .iter(app.world())
            .map(|state| state.zoom)
            .next()
    }

    fn map_drag_position(app: &App) -> Option<Vec2> {
        app.world()
            .get_resource::<DragState>()
            .and_then(|state| state.last_cursor_position)
    }
}
