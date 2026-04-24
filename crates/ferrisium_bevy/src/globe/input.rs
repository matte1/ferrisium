//! Mouse and touch orbit, pan, and zoom handling for globe views.
//!
//! Input is consumed only when the active Ferrisium view is the globe. Wheel and
//! pinch zoom scale surface height instead of center distance, and inward zoom
//! can bias the orbit toward the hovered or touched surface point.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::touch::{Touch, Touches};
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use big_space::prelude::CellCoord;

use crate::celestial::{CelestialBodyPlacement, CelestialFocus, CelestialSpace};
use crate::globe::camera::{
    min_camera_distance_for_focus, normalized_body_rotation_or_identity, Globe3dCamera,
};
use crate::globe::focus::globe_focus_target_with_metric_pivot;
use crate::globe::ray::{
    globe_surface_direction_at_screen_position, hovered_globe_surface_direction, GlobeRayCamera,
};
use crate::globe::state::{surface_distance, Globe3dState, Globe3dView};
use crate::input_capture::FerrisiumInputCapture;
use crate::metric_scene::MetricSceneFocusPivot;
use crate::source::ActiveTileSource;
use crate::view::{FerrisiumViewKind, FerrisiumViewMode, Map2dView};

// Mouse drag orbit sensitivity in degrees per screen pixel.
const ORBIT_DEGREES_PER_PIXEL: f64 = 0.15;
// Lower bound for distance-scaled controls near the surface.
const MIN_DISTANCE_CONTROL_SCALE: f64 = 0.005;
// Max fraction of current surface height that one focus movement may cover.
const MAX_MOVEMENT_RATIO: f32 = 0.10;
// Fallback viewport height used before a Bevy window is available.
const DEFAULT_VIEWPORT_HEIGHT_PX: f64 = 720.0;
// Fallback vertical FOV for custom/non-perspective cameras.
const DEFAULT_VERTICAL_FOV_RAD: f64 = std::f64::consts::FRAC_PI_4;
// Guard against near-zero pinch distances when a second finger lands.
const MIN_PINCH_DISTANCE_PIXELS: f32 = 8.0;
// Caps wheel spikes so one scroll event cannot jump through the globe.
pub(super) const MAX_SCROLL_DELTA_PER_FRAME: f64 = 0.35;
// Wheel-line distance step before exponential zoom scaling.
const LINE_SCROLL_DISTANCE_STEP: f64 = 0.08;
// Pixel-scroll distance step before exponential zoom scaling.
const PIXEL_SCROLL_DISTANCE_STEP: f64 = 0.0008;
// Exponential multiplier applied to normalized wheel distance.
pub(super) const ZOOM_EXP_FACTOR: f64 = 0.8;
// Blend strength when zooming toward the cursor's globe hit point.
const ZOOM_IN_FOCUS_STRENGTH: f64 = 0.6;
// Pitch clamp leaves room to orbit near poles without flipping controls.
const MAX_ABS_PITCH_DEG: f64 = 80.0;

type GlobeInputCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        &'static GlobalTransform,
        &'static Transform,
        Option<&'static CellCoord>,
        &'static Projection,
    ),
    With<Globe3dCamera>,
>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct GlobeTouchGesture {
    touch_count: usize,
    motion_delta_pixels: Vec2,
    zoom_delta: f64,
    focus_position: Option<Vec2>,
    previous_focus_position: Option<Vec2>,
}

/// Applies drag orbit, surface pan, wheel zoom, and touch gestures to the active globe state.
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "Bevy systems expose each ECS input as an explicit system parameter, and globe input coordinates mouse and touch paths in one active-view gate."
)]
pub(crate) fn handle_globe_input(
    active_source: Res<'_, ActiveTileSource>,
    celestial_focus: Res<'_, CelestialFocus>,
    celestial_space: Res<'_, CelestialSpace>,
    focus_pivot: Res<'_, MetricSceneFocusPivot>,
    view_mode: Res<'_, FerrisiumViewMode>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    input_capture: Res<'_, FerrisiumInputCapture>,
    touches: Res<'_, Touches>,
    mut mouse_motion_events: MessageReader<'_, '_, MouseMotion>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    cameras: GlobeInputCameraQuery<'_, '_>,
    bodies: Query<
        '_,
        '_,
        (&CelestialBodyPlacement, Option<&CellCoord>, &Transform),
        Without<Globe3dCamera>,
    >,
    mut globe_views: Query<'_, '_, &mut Globe3dState, With<Globe3dView>>,
    map_views: Query<'_, '_, (), With<Map2dView>>,
) {
    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Globe3d)
    {
        let _ = mouse_motion_events.read().count();
        let _ = mouse_wheel_events.read().count();
        return;
    }

    let Some(mut active_globe) = globe_views.iter_mut().next() else {
        return;
    };

    let window = windows.iter().next();
    let camera = cameras.iter().next();
    let focus_target = globe_focus_target_with_metric_pivot(
        *celestial_focus,
        *active_globe,
        *celestial_space,
        &focus_pivot,
        &bodies,
    );
    let min_camera_distance = min_camera_distance_for_focus(
        *active_globe,
        focus_target,
        &active_source.0,
        window,
        camera.map(|(_, _, _, _, projection)| projection),
    );
    active_globe.radius = focus_target.radius;
    active_globe.clamp_camera_distance_for_radius(focus_target.radius, min_camera_distance);

    if input_capture.pointer_captured() {
        let _ = mouse_motion_events.read().count();
        let _ = mouse_wheel_events.read().count();
        return;
    }

    let mouse_motion_delta = mouse_motion_events
        .read()
        .fold(Vec2::ZERO, |delta, event| delta + event.delta);
    let touch_active = touches.iter().next().is_some()
        || touches.any_just_pressed()
        || touches.any_just_released()
        || touches.any_just_canceled();

    if let Some(gesture) = globe_touch_gesture(&touches) {
        let motion_delta = clamp_motion_delta(gesture.motion_delta_pixels, window);
        if gesture.touch_count == 1 {
            orbit_globe_camera_by_pixels(&mut active_globe, motion_delta, focus_target.radius);
        } else {
            let camera_ray =
                camera.map(|(camera, global, local, cell, _)| (camera, global, local, cell));
            let touched_surface_direction = if gesture.zoom_delta.abs() > f64::EPSILON {
                gesture.focus_position.and_then(|screen_position| {
                    globe_surface_direction_at_screen_position(
                        screen_position,
                        camera_ray,
                        *celestial_space,
                        focus_target.radius,
                        focus_target.center_units,
                    )
                    .map(|surface_direction| {
                        camera_frame_surface_direction(
                            *active_globe,
                            focus_target.body_rotation,
                            surface_direction,
                        )
                    })
                })
            } else {
                None
            };
            let surface_grabbed = gesture
                .previous_focus_position
                .zip(gesture.focus_position)
                .is_some_and(|(previous_position, current_position)| {
                    surface_grab_pan_globe_camera(
                        &mut active_globe,
                        previous_position,
                        current_position,
                        focus_target.radius,
                        focus_target.center_units,
                        focus_target.body_rotation,
                        *celestial_space,
                        camera_ray,
                    )
                });
            if !surface_grabbed {
                pan_globe_camera_by_pixels(
                    &mut active_globe,
                    motion_delta,
                    focus_target.radius,
                    window,
                    camera.map(|(_, _, _, _, projection)| projection),
                );
            }
            if gesture.zoom_delta.abs() > f64::EPSILON {
                zoom_globe_camera(
                    &mut active_globe,
                    gesture.zoom_delta,
                    focus_target.radius,
                    min_camera_distance,
                );
                if let Some(surface_direction) = touched_surface_direction {
                    orbit_toward_surface(
                        &mut active_globe,
                        surface_direction,
                        gesture.zoom_delta * ZOOM_IN_FOCUS_STRENGTH,
                    );
                }
            }
        }
    } else if !touch_active {
        let mouse_motion_delta = clamp_motion_delta(mouse_motion_delta, window);
        if buttons.pressed(MouseButton::Left) && mouse_motion_delta != Vec2::ZERO {
            orbit_globe_camera_by_pixels(
                &mut active_globe,
                mouse_motion_delta,
                focus_target.radius,
            );
        } else if (buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle))
            && mouse_motion_delta != Vec2::ZERO
        {
            let surface_grabbed =
                window
                    .and_then(Window::cursor_position)
                    .is_some_and(|current_position| {
                        let previous_position = current_position - mouse_motion_delta;
                        surface_grab_pan_globe_camera(
                            &mut active_globe,
                            previous_position,
                            current_position,
                            focus_target.radius,
                            focus_target.center_units,
                            focus_target.body_rotation,
                            *celestial_space,
                            camera.map(|(camera, global, local, cell, _)| {
                                (camera, global, local, cell)
                            }),
                        )
                    });
            if !surface_grabbed {
                pan_globe_camera_by_pixels(
                    &mut active_globe,
                    mouse_motion_delta,
                    focus_target.radius,
                    window,
                    camera.map(|(_, _, _, _, projection)| projection),
                );
            }
        }
    }

    let scroll_delta = mouse_wheel_events
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => f64::from(event.y) * LINE_SCROLL_DISTANCE_STEP,
            MouseScrollUnit::Pixel => f64::from(event.y) * PIXEL_SCROLL_DISTANCE_STEP,
        })
        .sum::<f64>();
    if scroll_delta.abs() > f64::EPSILON {
        let scroll_delta = clamp_scroll_delta(scroll_delta);
        let zoom_delta = wheel_zoom_delta(scroll_delta);
        let hovered_surface_direction = if scroll_delta.is_sign_positive() {
            hovered_globe_surface_direction(
                window,
                camera.map(|(camera, global, local, cell, _)| (camera, global, local, cell)),
                *celestial_space,
                focus_target.radius,
                focus_target.center_units,
            )
            .map(|surface_direction| {
                camera_frame_surface_direction(
                    *active_globe,
                    focus_target.body_rotation,
                    surface_direction,
                )
            })
        } else {
            None
        };
        zoom_globe_camera(
            &mut active_globe,
            zoom_delta,
            focus_target.radius,
            min_camera_distance,
        );
        if let Some(surface_direction) = hovered_surface_direction {
            orbit_toward_surface(
                &mut active_globe,
                surface_direction,
                zoom_delta * ZOOM_IN_FOCUS_STRENGTH,
            );
        }
    }
}

/// Applies an orbit drag delta in screen pixels.
fn orbit_globe_camera_by_pixels(
    globe_state: &mut Globe3dState,
    delta_pixels: Vec2,
    focus_radius: f32,
) {
    let orbit_scale = distance_control_scale(*globe_state, focus_radius);
    let (yaw_deg, pitch_deg) = globe_orbit_angles(
        globe_state.yaw_deg,
        globe_state.pitch_deg,
        delta_pixels,
        ORBIT_DEGREES_PER_PIXEL * orbit_scale,
    );

    globe_state.yaw_deg = yaw_deg;
    globe_state.pitch_deg = pitch_deg;
}

/// Applies a surface-pan drag delta in screen pixels.
fn pan_globe_camera_by_pixels(
    globe_state: &mut Globe3dState,
    delta_pixels: Vec2,
    focus_radius: f32,
    window: Option<&Window>,
    projection: Option<&Projection>,
) {
    let degrees_per_pixel = globe_surface_pan_degrees_per_pixel(
        *globe_state,
        focus_radius,
        viewport_height_px(window),
        vertical_fov_rad(projection),
    );
    let (yaw_deg, pitch_deg) = globe_orbit_angles(
        globe_state.yaw_deg,
        globe_state.pitch_deg,
        delta_pixels,
        degrees_per_pixel,
    );

    globe_state.yaw_deg = yaw_deg;
    globe_state.pitch_deg = pitch_deg;
}

/// Applies a direct surface-grab pan from two screen positions.
#[allow(
    clippy::too_many_arguments,
    reason = "Surface-grab pan needs the current globe state, focused body frame, scene scale, and camera ray context."
)]
fn surface_grab_pan_globe_camera(
    globe_state: &mut Globe3dState,
    previous_screen_position: Vec2,
    current_screen_position: Vec2,
    focus_radius: f32,
    focus_center_units: DVec3,
    body_rotation: Quat,
    celestial_space: CelestialSpace,
    camera: Option<GlobeRayCamera<'_>>,
) -> bool {
    if previous_screen_position.distance_squared(current_screen_position) <= f32::EPSILON {
        return false;
    }

    let Some(previous_surface_direction) = globe_surface_direction_at_screen_position(
        previous_screen_position,
        camera,
        celestial_space,
        focus_radius,
        focus_center_units,
    ) else {
        return false;
    };
    let Some(current_surface_direction) = globe_surface_direction_at_screen_position(
        current_screen_position,
        camera,
        celestial_space,
        focus_radius,
        focus_center_units,
    ) else {
        return false;
    };
    let camera_direction = globe_camera_direction(*globe_state);
    let Some(next_camera_direction) = camera_direction_for_surface_grab_pan(
        camera_frame_surface_direction(*globe_state, body_rotation, previous_surface_direction),
        camera_frame_surface_direction(*globe_state, body_rotation, current_surface_direction),
        camera_direction,
    ) else {
        return false;
    };
    let (yaw_deg, pitch_deg) = orbit_angles_for_surface_direction(next_camera_direction);

    globe_state.yaw_deg = yaw_deg;
    globe_state.pitch_deg = pitch_deg;
    true
}

pub(super) fn camera_frame_surface_direction(
    globe_state: Globe3dState,
    body_rotation: Quat,
    world_surface_direction: DVec3,
) -> DVec3 {
    match globe_state.camera_up_mode {
        crate::globe::state::GlobeCameraUpMode::BodyNorth => {
            normalized_body_rotation_or_identity(body_rotation)
                .inverse()
                .mul_vec3(world_surface_direction.as_vec3())
                .as_dvec3()
                .normalize_or_zero()
        }
        crate::globe::state::GlobeCameraUpMode::SceneUp => {
            world_surface_direction.normalize_or_zero()
        }
    }
}

fn globe_camera_direction(globe_state: Globe3dState) -> DVec3 {
    let yaw_rad = globe_state.yaw_deg.to_radians();
    let pitch_rad = globe_state.pitch_deg.to_radians();
    let horizontal_radius = pitch_rad.cos();

    DVec3::new(
        horizontal_radius * yaw_rad.sin(),
        pitch_rad.sin(),
        horizontal_radius * yaw_rad.cos(),
    )
    .normalize_or_zero()
}

pub(super) fn camera_direction_for_surface_grab_pan(
    previous_surface_direction: DVec3,
    current_surface_direction: DVec3,
    camera_direction: DVec3,
) -> Option<DVec3> {
    rotate_direction_between(
        current_surface_direction,
        previous_surface_direction,
        camera_direction,
    )
}

/// Computes yaw and pitch after an orbit-like screen drag.
pub(super) fn globe_orbit_angles(
    yaw_deg: f64,
    pitch_deg: f64,
    delta_pixels: Vec2,
    degrees_per_pixel: f64,
) -> (f64, f64) {
    let yaw_deg = normalize_yaw_deg(yaw_deg - f64::from(delta_pixels.x) * degrees_per_pixel);
    let pitch_deg = (pitch_deg + f64::from(delta_pixels.y) * degrees_per_pixel)
        .clamp(-MAX_ABS_PITCH_DEG, MAX_ABS_PITCH_DEG);

    (yaw_deg, pitch_deg)
}

/// Applies exponential zoom to camera height above the focused body surface.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "The camera distance is stored in f32 but the wheel math is accumulated in f64."
)]
pub(super) fn zoom_globe_camera(
    globe_state: &mut Globe3dState,
    scroll_delta: f64,
    focus_radius: f32,
    min_camera_distance: f32,
) {
    let zoom_scale = (-scroll_delta * ZOOM_EXP_FACTOR).exp();
    let current_surface_distance =
        surface_distance(globe_state.camera_distance, focus_radius).max(f64::EPSILON);
    globe_state.camera_distance =
        (f64::from(focus_radius) + current_surface_distance * zoom_scale) as f32;
    globe_state.clamp_camera_distance_for_radius(focus_radius, min_camera_distance);
}

/// Scales drag sensitivity down as the camera approaches the focused surface.
pub(super) fn distance_control_scale(globe_state: Globe3dState, focus_radius: f32) -> f64 {
    let current_surface_distance = surface_distance(globe_state.camera_distance, focus_radius);
    let base_surface_distance =
        surface_distance(globe_state.base_camera_distance, focus_radius).max(f64::EPSILON);

    (current_surface_distance / base_surface_distance).clamp(MIN_DISTANCE_CONTROL_SCALE, 1.0)
}

/// Returns the zoom delta applied by wheel input.
pub(super) fn wheel_zoom_delta(scroll_delta: f64) -> f64 {
    scroll_delta
}

/// Returns the angular globe pan sensitivity for one screen pixel.
pub(super) fn globe_surface_pan_degrees_per_pixel(
    globe_state: Globe3dState,
    focus_radius: f32,
    viewport_height_px: f64,
    vertical_fov_rad: f64,
) -> f64 {
    let radius = f64::from(focus_radius).max(f64::EPSILON);
    let surface_height = surface_distance(globe_state.camera_distance, focus_radius);
    let viewport_height_px = viewport_height_px.max(1.0);
    let vertical_fov_rad = vertical_fov_rad.clamp(0.001, std::f64::consts::PI - 0.001);
    let world_units_per_pixel =
        2.0 * surface_height * (vertical_fov_rad * 0.5).tan() / viewport_height_px;

    (world_units_per_pixel / radius).to_degrees()
}

/// Converts a pinch distance change into the same zoom delta used by the wheel.
pub(super) fn pinch_zoom_delta(current_distance_px: f32, previous_distance_px: f32) -> f64 {
    if current_distance_px <= MIN_PINCH_DISTANCE_PIXELS
        || previous_distance_px <= MIN_PINCH_DISTANCE_PIXELS
    {
        return 0.0;
    }

    clamp_scroll_delta(f64::from(current_distance_px / previous_distance_px).ln())
}

/// Caps one-frame mouse movement to avoid large browser focus jumps.
fn clamp_motion_delta(delta: Vec2, window: Option<&Window>) -> Vec2 {
    let Some(window) = window else {
        return delta;
    };
    let max_x = window.width() * MAX_MOVEMENT_RATIO;
    let max_y = window.height() * MAX_MOVEMENT_RATIO;

    Vec2::new(delta.x.clamp(-max_x, max_x), delta.y.clamp(-max_y, max_y))
}

/// Caps one-frame wheel input to avoid jumping through the globe.
pub(super) fn clamp_scroll_delta(scroll_delta: f64) -> f64 {
    scroll_delta.clamp(-MAX_SCROLL_DELTA_PER_FRAME, MAX_SCROLL_DELTA_PER_FRAME)
}

/// Returns the current touch gesture, if the active touches moved this frame.
fn globe_touch_gesture(touches: &Touches) -> Option<GlobeTouchGesture> {
    if touches.any_just_released() || touches.any_just_canceled() {
        return None;
    }

    let mut active_touches = touches.iter().collect::<Vec<_>>();
    active_touches.sort_by_key(|touch| touch.id());

    match active_touches.as_slice() {
        [touch] => one_touch_gesture(touches, touch),
        [first, second] => two_touch_gesture(touches, first, second),
        _ => None,
    }
}

fn one_touch_gesture(touches: &Touches, touch: &Touch) -> Option<GlobeTouchGesture> {
    if touches.just_pressed(touch.id()) {
        return None;
    }

    let motion_delta_pixels = touch.delta();
    (motion_delta_pixels != Vec2::ZERO).then_some(GlobeTouchGesture {
        touch_count: 1,
        motion_delta_pixels,
        zoom_delta: 0.0,
        focus_position: None,
        previous_focus_position: None,
    })
}

fn two_touch_gesture(
    touches: &Touches,
    first: &Touch,
    second: &Touch,
) -> Option<GlobeTouchGesture> {
    if touches.just_pressed(first.id()) || touches.just_pressed(second.id()) {
        return None;
    }

    let current_center = (first.position() + second.position()) * 0.5;
    let previous_center = (first.previous_position() + second.previous_position()) * 0.5;
    let motion_delta_pixels = current_center - previous_center;
    let zoom_delta = pinch_zoom_delta(
        first.position().distance(second.position()),
        first
            .previous_position()
            .distance(second.previous_position()),
    );

    (motion_delta_pixels != Vec2::ZERO || zoom_delta.abs() > f64::EPSILON).then_some(
        GlobeTouchGesture {
            touch_count: 2,
            motion_delta_pixels,
            zoom_delta,
            focus_position: Some(current_center),
            previous_focus_position: Some(previous_center),
        },
    )
}

fn viewport_height_px(window: Option<&Window>) -> f64 {
    window.map_or(DEFAULT_VIEWPORT_HEIGHT_PX, |window| {
        f64::from(window.height())
    })
}

fn vertical_fov_rad(projection: Option<&Projection>) -> f64 {
    match projection {
        Some(Projection::Perspective(perspective)) => f64::from(perspective.fov),
        _ => DEFAULT_VERTICAL_FOV_RAD,
    }
}

/// Blends the orbit angles toward a surface direction during cursor zoom.
fn orbit_toward_surface(
    globe_state: &mut Globe3dState,
    surface_direction: DVec3,
    focus_strength: f64,
) {
    let (target_yaw_deg, target_pitch_deg) = orbit_angles_for_surface_direction(surface_direction);
    let blend = focus_strength.clamp(0.0, 1.0);

    globe_state.yaw_deg += shortest_angle_delta_deg(globe_state.yaw_deg, target_yaw_deg) * blend;
    globe_state.yaw_deg = normalize_yaw_deg(globe_state.yaw_deg);
    globe_state.pitch_deg = (globe_state.pitch_deg
        + (target_pitch_deg - globe_state.pitch_deg) * blend)
        .clamp(-MAX_ABS_PITCH_DEG, MAX_ABS_PITCH_DEG);
}

/// Converts a body-local surface direction into yaw/pitch orbit angles.
pub(super) fn orbit_angles_for_surface_direction(surface_direction: DVec3) -> (f64, f64) {
    let normalized = surface_direction.normalize_or_zero();
    let yaw_deg = normalized.x.atan2(normalized.z).to_degrees();
    let pitch_deg = normalized
        .y
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees()
        .clamp(-MAX_ABS_PITCH_DEG, MAX_ABS_PITCH_DEG);

    (yaw_deg, pitch_deg)
}

/// Computes the shortest signed angular delta in degrees.
fn shortest_angle_delta_deg(current_deg: f64, target_deg: f64) -> f64 {
    (target_deg - current_deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Normalizes yaw to the `[-180, 180)` range.
fn normalize_yaw_deg(yaw_deg: f64) -> f64 {
    (yaw_deg + 180.0).rem_euclid(360.0) - 180.0
}

fn rotate_direction_between(from: DVec3, to: DVec3, direction: DVec3) -> Option<DVec3> {
    let from = from.normalize_or_zero();
    let to = to.normalize_or_zero();
    let direction = direction.normalize_or_zero();
    if from.length_squared() <= f64::EPSILON
        || to.length_squared() <= f64::EPSILON
        || direction.length_squared() <= f64::EPSILON
    {
        return None;
    }

    let dot = from.dot(to).clamp(-1.0, 1.0);
    if dot > 1.0 - 1.0e-12 {
        return Some(direction);
    }

    let mut axis = from.cross(to);
    let axis_length = axis.length();
    if axis_length <= 1.0e-12 {
        axis = orthogonal_axis(from);
    } else {
        axis /= axis_length;
    }
    let angle = dot.acos();

    Some(rotate_direction_about_axis(direction, axis, angle).normalize_or_zero())
}

fn orthogonal_axis(direction: DVec3) -> DVec3 {
    let candidate = if direction.x.abs() < 0.9 {
        DVec3::X
    } else {
        DVec3::Y
    };

    direction.cross(candidate).normalize_or_zero()
}

fn rotate_direction_about_axis(direction: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let (sin_angle, cos_angle) = angle.sin_cos();

    direction * cos_angle
        + axis.cross(direction) * sin_angle
        + axis * axis.dot(direction) * (1.0 - cos_angle)
}
