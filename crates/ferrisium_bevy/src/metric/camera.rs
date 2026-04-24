//! Orbit camera controls for metric scenes.
//!
//! [`MetricOrbitCameraState`] stores a camera pivot, yaw, pitch, and distance in
//! render units. The systems in this module apply mouse orbit, pan, and zoom to
//! that state, then place any [`MetricOrbitCamera`] through the active
//! [`crate::CelestialSpace`] so large metric scenes can still use `big_space`.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use big_space::prelude::CellCoord;

use crate::celestial::CelestialSpace;
use crate::input_capture::FerrisiumInputCapture;
use crate::metric_scene::{MetricSceneFocusPivot, MetricSceneFocusTarget, MetricSceneId};
use crate::surface_depth::globe_surface_near_plane;

// Caps one-frame wheel spikes so a browser or touchpad burst cannot jump from
// close inspection to the scene limits in one update.
const MAX_METRIC_SCROLL_DELTA_PER_FRAME: f64 = 0.35;

/// Marks a `Camera3d` as controlled by [`MetricOrbitCameraState`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct MetricOrbitCamera;

/// Camera orbit state in metric render units.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct MetricOrbitCameraState {
    /// Point the camera orbits and pans around, in render units.
    pub pivot_units: DVec3,
    /// Horizontal orbit angle in degrees.
    pub yaw_deg: f64,
    /// Vertical orbit angle in degrees.
    pub pitch_deg: f64,
    /// Distance from camera to pivot, in render units.
    pub distance_units: f64,
}

impl MetricOrbitCameraState {
    /// Creates a camera state from explicit orbit values.
    #[must_use]
    pub const fn new(
        pivot_units: DVec3,
        yaw_deg: f64,
        pitch_deg: f64,
        distance_units: f64,
    ) -> Self {
        Self {
            pivot_units,
            yaw_deg,
            pitch_deg,
            distance_units,
        }
    }

    /// Builds a camera state from a horizontal distance and height.
    ///
    /// This is useful for overview scenes where the desired starting view is
    /// easier to describe as "stand this far back and this high above the
    /// orbital plane" than as a pitch angle and hypotenuse distance.
    #[must_use]
    pub fn from_horizontal_distance_and_height(
        horizontal_distance_units: f64,
        height_units: f64,
        yaw_deg: f64,
    ) -> Self {
        let distance_units = horizontal_distance_units.hypot(height_units);
        let pitch_deg = if distance_units > 0.0 {
            (height_units / distance_units).asin().to_degrees()
        } else {
            0.0
        };

        Self {
            pivot_units: DVec3::ZERO,
            yaw_deg,
            pitch_deg,
            distance_units,
        }
    }

    /// Returns the camera position in render units.
    #[must_use]
    pub fn position_units(self) -> DVec3 {
        let yaw_rad = self.yaw_deg.to_radians();
        let pitch_rad = self.pitch_deg.to_radians();
        let horizontal = self.distance_units * pitch_rad.cos();

        self.pivot_units
            + DVec3::new(
                horizontal * yaw_rad.sin(),
                self.distance_units * pitch_rad.sin(),
                horizontal * yaw_rad.cos(),
            )
    }

    /// Returns right and up pan axes for the current view.
    #[must_use]
    pub fn pan_axes(self) -> (DVec3, DVec3) {
        let camera_position = self.position_units();
        let forward = (self.pivot_units - camera_position).normalize_or_zero();
        let right = forward.cross(DVec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();

        (right, up)
    }

    /// Applies an orbit drag delta in pixels.
    pub fn orbit_by_pixels(&mut self, delta_pixels: Vec2, settings: MetricOrbitCameraSettings) {
        let (yaw_deg, pitch_deg) = metric_orbit_angles(
            self.yaw_deg,
            self.pitch_deg,
            f64::from(delta_pixels.x),
            f64::from(delta_pixels.y),
            settings,
        );
        self.yaw_deg = yaw_deg;
        self.pitch_deg = pitch_deg;
    }

    /// Applies a pan drag delta in pixels.
    pub fn pan_by_pixels(&mut self, delta_pixels: Vec2, settings: MetricOrbitCameraSettings) {
        let (right, up) = self.pan_axes();
        let pan_units = self.distance_units * settings.pan_units_per_pixel_at_distance;
        self.pivot_units +=
            (-right * f64::from(delta_pixels.x) + up * f64::from(delta_pixels.y)) * pan_units;
    }

    /// Applies a wheel scroll delta.
    pub fn zoom_by_scroll(&mut self, scroll_delta: f64, settings: MetricOrbitCameraSettings) {
        self.distance_units = metric_orbit_zoom_distance(
            self.distance_units,
            scroll_delta,
            settings.min_distance_units,
            settings.max_distance_units,
        );
    }
}

/// Input and clamp settings for a [`MetricOrbitCamera`].
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct MetricOrbitCameraSettings {
    /// Minimum camera-to-pivot distance in render units.
    pub min_distance_units: f64,
    /// Maximum camera-to-pivot distance in render units.
    pub max_distance_units: f64,
    /// Orbit yaw/pitch change per mouse pixel.
    pub orbit_degrees_per_pixel: f64,
    /// Maximum absolute pitch in degrees.
    pub max_abs_pitch_deg: f64,
    /// Pan scale multiplied by current camera distance.
    pub pan_units_per_pixel_at_distance: f64,
    /// Wheel zoom scale for line-based scroll events.
    pub line_scroll_zoom_step: f64,
    /// Wheel zoom scale for pixel-based scroll events.
    pub pixel_scroll_zoom_step: f64,
}

impl Default for MetricOrbitCameraSettings {
    fn default() -> Self {
        Self {
            min_distance_units: 1.0,
            max_distance_units: 10_000.0,
            orbit_degrees_per_pixel: 0.16,
            max_abs_pitch_deg: 82.0,
            pan_units_per_pixel_at_distance: 0.0015,
            line_scroll_zoom_step: 0.16,
            pixel_scroll_zoom_step: 0.0016,
        }
    }
}

type MetricOrbitCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        Option<&'static mut CellCoord>,
        Option<&'static mut Projection>,
    ),
    With<MetricOrbitCamera>,
>;

/// Applies mouse input to the active metric orbit camera state.
///
/// Left drag orbits around the pivot. Right or middle drag pans the pivot.
/// Wheel input zooms the camera distance.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy input systems expose settings, capture state, raw input, camera queries, and mutable state as explicit system parameters."
)]
pub fn handle_metric_orbit_camera_input(
    settings: Res<'_, MetricOrbitCameraSettings>,
    input_capture: Option<Res<'_, FerrisiumInputCapture>>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    mut mouse_motion_events: MessageReader<'_, '_, MouseMotion>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    cameras: Query<'_, '_, (), With<MetricOrbitCamera>>,
    mut camera_state: ResMut<'_, MetricOrbitCameraState>,
) {
    if cameras.is_empty() {
        let _ = mouse_motion_events.read().count();
        let _ = mouse_wheel_events.read().count();
        return;
    }

    if input_capture.is_some_and(|capture| capture.pointer_captured()) {
        let _ = mouse_motion_events.read().count();
        let _ = mouse_wheel_events.read().count();
        return;
    }

    let window = windows.iter().next();
    let motion_delta = mouse_motion_events
        .read()
        .fold(Vec2::ZERO, |delta, event| delta + event.delta);
    let motion_delta = clamp_metric_orbit_motion_delta(motion_delta, window);

    if buttons.pressed(MouseButton::Left) && motion_delta != Vec2::ZERO {
        camera_state.orbit_by_pixels(motion_delta, *settings);
    } else if (buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle))
        && motion_delta != Vec2::ZERO
    {
        camera_state.pan_by_pixels(motion_delta, *settings);
    }

    let scroll_delta = mouse_wheel_events
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => f64::from(event.y) * settings.line_scroll_zoom_step,
            MouseScrollUnit::Pixel => f64::from(event.y) * settings.pixel_scroll_zoom_step,
        })
        .sum::<f64>();
    if scroll_delta.abs() > f64::EPSILON {
        let scroll_delta = clamp_metric_orbit_scroll_delta(scroll_delta);
        camera_state.zoom_by_scroll(scroll_delta, *settings);
    }
}

/// Synchronizes camera transforms from [`MetricOrbitCameraState`].
pub fn sync_metric_orbit_camera(
    celestial_space: Res<'_, CelestialSpace>,
    camera_state: Res<'_, MetricOrbitCameraState>,
    focus_pivot: Option<Res<'_, MetricSceneFocusPivot>>,
    mut cameras: MetricOrbitCameraQuery<'_, '_>,
) {
    let camera_position = camera_state.position_units();
    let camera_placement = celestial_space.place_position_units(camera_position);
    let look_direction = (camera_state.pivot_units - camera_position)
        .normalize_or_zero()
        .as_vec3();
    let depth_target = metric_orbit_depth_target(*camera_state, focus_pivot.as_deref());
    let (near, far) = metric_orbit_clip_planes(camera_state.distance_units, depth_target);

    for (mut transform, cell, projection) in &mut cameras {
        if let Some(mut cell) = cell {
            *cell = camera_placement.cell;
            transform.translation = camera_placement.translation;
        } else {
            transform.translation = camera_position.as_vec3();
        }
        *transform =
            Transform::from_translation(transform.translation).looking_to(look_direction, Vec3::Y);
        if let Some(mut projection) = projection {
            if let Projection::Perspective(perspective) = projection.as_mut() {
                perspective.near = near;
                perspective.far = far;
                perspective.near_clip_plane = Vec4::new(0.0, 0.0, -1.0, -near);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MetricOrbitDepthTarget {
    physical_radius_units: f32,
    camera_surface_distance_units: f64,
}

fn metric_orbit_depth_target(
    camera_state: MetricOrbitCameraState,
    focus_pivot: Option<&MetricSceneFocusPivot>,
) -> Option<MetricOrbitDepthTarget> {
    let focus_pivot = focus_pivot?;
    if !focus_pivot.resolved || focus_pivot.scene != MetricSceneId::PRIMARY {
        return None;
    }
    let physical_radius_units = focus_pivot
        .physical_radius_units
        .filter(|radius| radius.is_finite() && *radius > 0.0)?;
    let camera_distance_to_pivot = camera_state
        .position_units()
        .distance(focus_pivot.pivot_units);
    if !camera_distance_to_pivot.is_finite() {
        return None;
    }
    let camera_surface_distance_units = match focus_pivot.target {
        MetricSceneFocusTarget::Scene => return None,
        MetricSceneFocusTarget::Object(_) => {
            (camera_distance_to_pivot - f64::from(physical_radius_units)).max(0.0)
        }
        MetricSceneFocusTarget::BodySurface(_) => camera_distance_to_pivot.max(0.0),
    };

    Some(MetricOrbitDepthTarget {
        physical_radius_units,
        camera_surface_distance_units,
    })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy perspective clip planes are f32 while metric camera state is f64."
)]
fn metric_orbit_clip_planes(
    distance_units: f64,
    depth_target: Option<MetricOrbitDepthTarget>,
) -> (f32, f32) {
    let distance_units = distance_units.max(0.0);
    let near = (distance_units * 0.001).clamp(1.0e-6, 10.0);
    let mut far = (distance_units * 8.0).max(10.0).max(near * 2.0);
    let near = depth_target.map_or(near, |depth_target| {
        f64::from(globe_surface_near_plane(
            depth_target.camera_surface_distance_units as f32,
            depth_target.physical_radius_units,
            far as f32,
        ))
    });
    far = far.max(near * 2.0);

    (near as f32, far as f32)
}

fn clamp_metric_orbit_motion_delta(delta: Vec2, window: Option<&Window>) -> Vec2 {
    let Some(window) = window else {
        return delta;
    };
    clamp_metric_orbit_motion_delta_to_extent(delta, window.width(), window.height())
}

/// Clamps a per-frame motion delta to the visible window extent.
#[must_use]
pub fn clamp_metric_orbit_motion_delta_to_extent(delta: Vec2, width: f32, height: f32) -> Vec2 {
    Vec2::new(
        delta.x.clamp(-width.abs(), width.abs()),
        delta.y.clamp(-height.abs(), height.abs()),
    )
}

/// Computes yaw and pitch after an orbit drag.
#[must_use]
pub fn metric_orbit_angles(
    yaw_deg: f64,
    pitch_deg: f64,
    delta_x: f64,
    delta_y: f64,
    settings: MetricOrbitCameraSettings,
) -> (f64, f64) {
    let yaw_deg =
        normalize_metric_orbit_yaw_deg(yaw_deg - delta_x * settings.orbit_degrees_per_pixel);
    let pitch_deg = (pitch_deg + delta_y * settings.orbit_degrees_per_pixel)
        .clamp(-settings.max_abs_pitch_deg, settings.max_abs_pitch_deg);

    (yaw_deg, pitch_deg)
}

/// Computes a clamped camera distance after wheel zoom.
#[must_use]
pub fn metric_orbit_zoom_distance(
    distance_units: f64,
    scroll_delta: f64,
    min_distance_units: f64,
    max_distance_units: f64,
) -> f64 {
    let zoomed = distance_units * (-scroll_delta).exp();
    zoomed.clamp(min_distance_units, max_distance_units)
}

fn clamp_metric_orbit_scroll_delta(scroll_delta: f64) -> f64 {
    scroll_delta.clamp(
        -MAX_METRIC_SCROLL_DELTA_PER_FRAME,
        MAX_METRIC_SCROLL_DELTA_PER_FRAME,
    )
}

/// Normalizes yaw into `[-180, 180)` degrees.
#[must_use]
pub fn normalize_metric_orbit_yaw_deg(yaw_deg: f64) -> f64 {
    (yaw_deg + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_metric_orbit_motion_delta_to_extent, clamp_metric_orbit_scroll_delta,
        metric_orbit_angles, metric_orbit_clip_planes, metric_orbit_depth_target,
        metric_orbit_zoom_distance, sync_metric_orbit_camera, MetricOrbitCamera,
        MetricOrbitCameraSettings, MetricOrbitCameraState,
    };
    use crate::metric_scene::{MetricSceneFocusPivot, MetricSceneFocusTarget, MetricSceneId};
    use crate::CelestialSpace;
    use bevy::math::DVec3;
    use bevy::prelude::{App, PerspectiveProjection, Projection, Transform, Update, Vec2};
    use ferrisium_core::space::{BodyId, RenderScale};

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    fn assert_close_f32(lhs: f32, rhs: f32) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-6,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn orbit_angles_clamp_pitch_and_wrap_yaw() {
        let settings = MetricOrbitCameraSettings {
            max_abs_pitch_deg: 82.0,
            orbit_degrees_per_pixel: 0.16,
            ..Default::default()
        };

        let (yaw, pitch) = metric_orbit_angles(179.0, 80.0, -40.0, 100.0, settings);

        assert!(yaw < -170.0);
        assert_close(pitch, settings.max_abs_pitch_deg);
    }

    #[test]
    fn zoom_distance_clamps_range() {
        assert_close(metric_orbit_zoom_distance(100.0, 10.0, 80.0, 1_800.0), 80.0);
        assert_close(
            metric_orbit_zoom_distance(1_700.0, -10.0, 80.0, 1_800.0),
            1_800.0,
        );
    }

    #[test]
    fn wheel_input_is_capped_per_frame() {
        assert_close(clamp_metric_orbit_scroll_delta(99.0), 0.35);
        assert_close(clamp_metric_orbit_scroll_delta(-99.0), -0.35);
        assert_close(clamp_metric_orbit_scroll_delta(0.1), 0.1);
    }

    #[test]
    fn clip_planes_support_tiny_close_body_distances() {
        let (near, far) = metric_orbit_clip_planes(0.010, None);

        assert!(near < 0.001);
        assert!(far >= 10.0);
        assert!(far > near);
    }

    #[test]
    fn clip_planes_use_surface_clearance_for_focused_bodies() {
        let state = MetricOrbitCameraState::new(DVec3::ZERO, 0.0, 0.0, 9_371.0);
        let focus_pivot = MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            DVec3::ZERO,
            Some(6_371.0),
        );
        let depth_target = metric_orbit_depth_target(state, Some(&focus_pivot));
        assert!(depth_target.is_some());
        let Some(depth_target) = depth_target else {
            return;
        };

        assert_close(depth_target.camera_surface_distance_units, 3_000.0);
        let (near, far) = metric_orbit_clip_planes(state.distance_units, Some(depth_target));

        assert!(near > 100.0);
        assert!(f64::from(near) < depth_target.camera_surface_distance_units);
        assert!(far > near);
    }

    #[test]
    fn position_matches_configured_distance() {
        let state =
            MetricOrbitCameraState::from_horizontal_distance_and_height(720.0, 290.0, -34.0);
        let position = state.position_units();
        let offset = position - state.pivot_units;

        assert_close(offset.length(), state.distance_units);
        assert!(position.x < 0.0);
        assert!(position.y > 0.0);
        assert!(position.z > 0.0);
    }

    #[test]
    fn pan_axes_are_orthonormal_to_view_direction() {
        let state = MetricOrbitCameraState {
            yaw_deg: 37.0,
            pitch_deg: 28.0,
            ..MetricOrbitCameraState::from_horizontal_distance_and_height(720.0, 290.0, -34.0)
        };
        let position = state.position_units();
        let forward = (state.pivot_units - position).normalize();
        let (right, up) = state.pan_axes();

        assert_close(right.length(), 1.0);
        assert_close(up.length(), 1.0);
        assert_close(right.dot(up), 0.0);
        assert_close(right.dot(forward), 0.0);
        assert_close(up.dot(forward), 0.0);
    }

    #[test]
    fn motion_delta_clamps_to_window_extent() {
        let clamped =
            clamp_metric_orbit_motion_delta_to_extent(Vec2::new(900.0, -700.0), 320.0, 180.0);

        assert_close_f32(clamped.x, 320.0);
        assert_close_f32(clamped.y, -180.0);
    }

    #[test]
    fn sync_metric_orbit_camera_updates_late_spawned_cameras() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(
            10_000.0,
            RenderScale::from_kilometers_per_unit(1_000.0),
        ));
        app.insert_resource(MetricOrbitCameraState::new(DVec3::ZERO, 0.0, 0.0, 10.0));
        app.add_systems(Update, sync_metric_orbit_camera);
        app.update();

        let camera = app
            .world_mut()
            .spawn((
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                MetricOrbitCamera,
            ))
            .id();
        app.update();

        let Some(transform) = app.world().get::<Transform>(camera) else {
            return;
        };
        assert_close_f32(transform.translation.z, 10.0);
        let Some(Projection::Perspective(projection)) = app.world().get::<Projection>(camera)
        else {
            return;
        };
        assert_close_f32(projection.near, 0.01);
        assert_close_f32(projection.far, 80.0);
        assert_close_f32(projection.near_clip_plane.w, -0.01);
    }

    #[test]
    fn sync_metric_orbit_camera_uses_focused_body_surface_depth() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(
            20_000.0,
            RenderScale::from_kilometers_per_unit(1_000.0),
        ));
        app.insert_resource(MetricOrbitCameraState::new(DVec3::ZERO, 0.0, 0.0, 9_371.0));
        app.insert_resource(MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            DVec3::ZERO,
            Some(6_371.0),
        ));
        app.add_systems(Update, sync_metric_orbit_camera);

        let camera = app
            .world_mut()
            .spawn((
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                MetricOrbitCamera,
            ))
            .id();
        app.update();

        let Some(Projection::Perspective(projection)) = app.world().get::<Projection>(camera)
        else {
            return;
        };
        assert!(projection.near > 100.0);
        assert_close_f32(projection.near_clip_plane.w, -projection.near);
    }
}
