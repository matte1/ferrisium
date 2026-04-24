//! Browser-only diagnostics consumed by Playwright development scenarios.
//!
//! This module intentionally belongs to the demo, not the library. It exposes
//! stable browser state for smoke tests and recorded UI/UX inspection without
//! making test hooks part of Ferrisium's public API.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use ferrisium_bevy::prelude::{
    ActiveTileSource, CelestialFocus, FerrisiumViewMode, Globe3dState, Globe3dView, H3HoverState,
    H3PickSurface, Map2dState, Map2dView, MetricOrbitCameraState, MetricSceneFocusSelection,
};
use js_sys::{Object, Reflect};
use wasm_bindgen::JsValue;

use crate::demo_config::DemoMode;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct DemoTestBridgeMode(DemoMode);

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
struct DemoTestBridgeFrame {
    frame: u32,
    sample_count: u32,
    total_delta_seconds: f64,
    max_delta_seconds: f64,
}

pub(crate) fn configure_test_bridge(app: &mut App, mode: DemoMode) {
    app.insert_resource(DemoTestBridgeMode(mode))
        .init_resource::<DemoTestBridgeFrame>()
        .add_systems(PostUpdate, publish_demo_test_bridge);
}

#[allow(
    clippy::too_many_arguments,
    reason = "The bridge snapshots independent ECS resources into one browser object."
)]
fn publish_demo_test_bridge(
    mode: Res<'_, DemoTestBridgeMode>,
    mut bridge_frame: ResMut<'_, DemoTestBridgeFrame>,
    time: Res<'_, Time>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    active_source: Option<Res<'_, ActiveTileSource>>,
    view_mode: Option<Res<'_, FerrisiumViewMode>>,
    map_views: Query<'_, '_, &Map2dState, With<Map2dView>>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    celestial_focus: Option<Res<'_, CelestialFocus>>,
    metric_camera: Option<Res<'_, MetricOrbitCameraState>>,
    metric_focus: Option<Res<'_, MetricSceneFocusSelection>>,
    h3_hover: Option<Res<'_, H3HoverState>>,
) {
    bridge_frame.update(&time);

    let bridge = Object::new();
    set_string(&bridge, "version", "1");
    set_string(&bridge, "mode", demo_mode_label(mode.0));
    set_u32(&bridge, "frame", bridge_frame.frame);
    set_object(&bridge, "timing", &bridge_frame.timing_snapshot(&time));
    set_bool(
        &bridge,
        "interactive",
        bridge_frame.frame >= 2
            && match mode.0 {
                DemoMode::Map => !map_views.is_empty(),
                DemoMode::Globe => !globe_views.is_empty(),
                DemoMode::SolarSystem => metric_camera.is_some(),
            },
    );

    set_object(&bridge, "window", &window_snapshot(windows.iter().next()));
    if let Some(active_source) = active_source {
        set_object(&bridge, "tileSource", &tile_source_snapshot(&active_source));
    }
    if let Some(view_mode) = view_mode {
        set_string(
            &bridge,
            "ferrisiumViewMode",
            &format!("{:?}", view_mode.as_ref()),
        );
    }
    set_object(&bridge, "map", &map_snapshot(map_views.iter().next()));
    set_object(&bridge, "globe", &globe_snapshot(globe_views.iter().next()));
    if let Some(celestial_focus) = celestial_focus {
        set_i32(
            &bridge,
            "celestialFocusNaifId",
            celestial_focus.target.naif_id,
        );
    }
    if let Some(metric_camera) = metric_camera {
        set_object(
            &bridge,
            "metricCamera",
            &metric_camera_snapshot(*metric_camera),
        );
    }
    if let Some(metric_focus) = metric_focus {
        set_string(
            &bridge,
            "metricFocus",
            &format!("{:?}", metric_focus.target),
        );
    }
    if let Some(h3_hover) = h3_hover {
        set_object(&bridge, "h3", &h3_hover_snapshot(&h3_hover));
    }

    if let Some(window) = web_sys::window() {
        let _ = Reflect::set(
            window.as_ref(),
            &JsValue::from_str("__FERRISIUM_TEST__"),
            bridge.as_ref(),
        );
    }
}

impl DemoTestBridgeFrame {
    fn update(&mut self, time: &Time) {
        self.frame = self.frame.saturating_add(1);

        let delta_seconds = time.delta_secs_f64();
        if self.frame <= 1 || !delta_seconds.is_finite() || delta_seconds <= 0.0 {
            return;
        }

        self.sample_count = self.sample_count.saturating_add(1);
        self.total_delta_seconds += delta_seconds;
        self.max_delta_seconds = self.max_delta_seconds.max(delta_seconds);
    }

    fn timing_snapshot(self, time: &Time) -> Object {
        let snapshot = Object::new();
        let delta_seconds = time.delta_secs_f64();
        let average_delta_seconds = if self.sample_count == 0 {
            0.0
        } else {
            self.total_delta_seconds / f64::from(self.sample_count)
        };

        set_f64(&snapshot, "elapsedSeconds", time.elapsed_secs_f64());
        set_f64(&snapshot, "deltaMs", delta_seconds * 1000.0);
        set_f64(&snapshot, "averageDeltaMs", average_delta_seconds * 1000.0);
        set_f64(&snapshot, "maxDeltaMs", self.max_delta_seconds * 1000.0);
        set_f64(
            &snapshot,
            "averageFps",
            fps_from_delta_seconds(average_delta_seconds),
        );
        set_f64(
            &snapshot,
            "instantFps",
            fps_from_delta_seconds(delta_seconds),
        );
        set_u32(&snapshot, "sampleCount", self.sample_count);
        snapshot
    }
}

fn fps_from_delta_seconds(delta_seconds: f64) -> f64 {
    if delta_seconds.is_finite() && delta_seconds > 0.0 {
        1.0 / delta_seconds
    } else {
        0.0
    }
}

fn demo_mode_label(mode: DemoMode) -> &'static str {
    match mode {
        DemoMode::Globe => "globe",
        DemoMode::SolarSystem => "solar",
        DemoMode::Map => "map",
    }
}

fn window_snapshot(window: Option<&Window>) -> Object {
    let snapshot = Object::new();
    set_bool(&snapshot, "present", window.is_some());
    if let Some(window) = window {
        set_f64(&snapshot, "width", f64::from(window.width()));
        set_f64(&snapshot, "height", f64::from(window.height()));
        if let Some(cursor) = window.cursor_position() {
            let cursor_snapshot = Object::new();
            set_f64(&cursor_snapshot, "x", f64::from(cursor.x));
            set_f64(&cursor_snapshot, "y", f64::from(cursor.y));
            set_object(&snapshot, "cursor", &cursor_snapshot);
        }
    }
    snapshot
}

fn tile_source_snapshot(active_source: &ActiveTileSource) -> Object {
    let source = &active_source.0;
    let snapshot = Object::new();
    set_string(&snapshot, "body", &source.body.name);
    set_string(&snapshot, "projection", &format!("{:?}", source.projection));
    set_i32(&snapshot, "minZoom", i32::from(source.min_zoom));
    set_i32(&snapshot, "maxZoom", i32::from(source.max_zoom));
    set_i32(&snapshot, "tileSize", i32::from(source.tile_size));
    set_i32(
        &snapshot,
        "rasterTileSize",
        i32::from(source.raster_tile_size),
    );
    snapshot
}

fn map_snapshot(map_state: Option<&Map2dState>) -> Object {
    let snapshot = Object::new();
    set_bool(&snapshot, "present", map_state.is_some());
    if let Some(map_state) = map_state {
        set_f64(&snapshot, "centerLonDeg", map_state.center.lon_deg);
        set_f64(&snapshot, "centerLatDeg", map_state.center.lat_deg);
        set_f64(&snapshot, "zoom", map_state.zoom);
    }
    snapshot
}

fn globe_snapshot(globe_state: Option<&Globe3dState>) -> Object {
    let snapshot = Object::new();
    set_bool(&snapshot, "present", globe_state.is_some());
    if let Some(globe_state) = globe_state {
        set_f64(&snapshot, "yawDeg", globe_state.yaw_deg);
        set_f64(&snapshot, "pitchDeg", globe_state.pitch_deg);
        set_f64(
            &snapshot,
            "cameraDistance",
            f64::from(globe_state.camera_distance),
        );
        set_f64(&snapshot, "radius", f64::from(globe_state.radius));
        set_f64(
            &snapshot,
            "surfaceDistance",
            f64::from((globe_state.camera_distance - globe_state.radius).max(0.0)),
        );
        set_i32(&snapshot, "tileZoom", i32::from(globe_state.tile_zoom));
        set_usize(
            &snapshot,
            "maxSelectedTiles",
            globe_state.max_selected_tiles,
        );
    }
    snapshot
}

fn metric_camera_snapshot(camera: MetricOrbitCameraState) -> Object {
    let snapshot = Object::new();
    set_bool(&snapshot, "present", true);
    set_f64(&snapshot, "yawDeg", camera.yaw_deg);
    set_f64(&snapshot, "pitchDeg", camera.pitch_deg);
    set_f64(&snapshot, "distanceUnits", camera.distance_units);

    let pivot = Object::new();
    set_f64(&pivot, "x", camera.pivot_units.x);
    set_f64(&pivot, "y", camera.pivot_units.y);
    set_f64(&pivot, "z", camera.pivot_units.z);
    set_object(&snapshot, "pivotUnits", &pivot);
    snapshot
}

fn h3_hover_snapshot(hover: &H3HoverState) -> Object {
    let snapshot = Object::new();
    set_bool(&snapshot, "hovered", hover.hit.is_some());
    if let Some(hit) = &hover.hit {
        set_string(&snapshot, "layer", hit.layer.name());
        set_string(&snapshot, "cell", &hit.cell.bits().to_string());
        set_usize(&snapshot, "cellIndex", hit.cell_index);
        set_f64(&snapshot, "lonDeg", hit.lon_lat.lon_deg);
        set_f64(&snapshot, "latDeg", hit.lon_lat.lat_deg);
        set_string(
            &snapshot,
            "surface",
            match hit.surface {
                H3PickSurface::Map2d => "map",
                H3PickSurface::Globe3d => "globe",
            },
        );
    }
    snapshot
}

fn set_object(target: &Object, key: &str, value: &Object) {
    set_value(target, key, value.as_ref());
}

fn set_string(target: &Object, key: &str, value: &str) {
    set_value(target, key, &JsValue::from_str(value));
}

fn set_bool(target: &Object, key: &str, value: bool) {
    set_value(target, key, &JsValue::from_bool(value));
}

fn set_i32(target: &Object, key: &str, value: i32) {
    set_f64(target, key, f64::from(value));
}

fn set_u32(target: &Object, key: &str, value: u32) {
    set_f64(target, key, f64::from(value));
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Debug counters stay small in browser sessions and are only diagnostic."
)]
fn set_usize(target: &Object, key: &str, value: usize) {
    set_f64(target, key, value as f64);
}

fn set_f64(target: &Object, key: &str, value: f64) {
    set_value(target, key, &JsValue::from_f64(value));
}

fn set_value(target: &Object, key: &str, value: &JsValue) {
    let _ = Reflect::set(target.as_ref(), &JsValue::from_str(key), value);
}
