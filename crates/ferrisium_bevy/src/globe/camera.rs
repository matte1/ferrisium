//! Globe camera bootstrap, placement, and projection tuning.
//!
//! Camera placement is centered on the currently focused celestial body and can
//! operate either in `big_space` cell coordinates or regular Bevy transforms.
//! Projection near/far planes track source resolution and focused body radius
//! so close views avoid clipping while wide views retain distant bodies.

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use big_space::prelude::{BigSpaceCommands, CellCoord, FloatingOrigin};
use ferrisium_core::source::TileSource;
use ferrisium_core::space::{BodyId, CelestialBody};

use crate::celestial::{
    CelestialBodyPlacement, CelestialFocus, CelestialRoot, CelestialSpace, CelestialSpaceEntities,
    PrimaryCelestialBody,
};
use crate::globe::focus::{globe_focus_target_with_metric_pivot, GlobeFocusTarget};
use crate::globe::state::{
    max_camera_distance_for_radius, surface_distance, Globe3dState, Globe3dView, GlobeCameraUpMode,
    MIN_CAMERA_SURFACE_DISTANCE_FACTOR, MIN_CAMERA_SURFACE_DISTANCE_FACTOR_F32,
};
use crate::metric_scene::{MetricSceneBodyAnchorBundle, MetricSceneCamera, MetricSceneFocusPivot};
use crate::source::ActiveTileSource;
use crate::surface_depth::globe_surface_near_plane;

// Target ratio between one source pixel and one screen pixel at the camera floor.
const SOURCE_RESOLUTION_SCREEN_PIXELS: f64 = 1.0;
// Fallback viewport height used before a Bevy window is available.
const DEFAULT_VIEWPORT_HEIGHT_PX: f64 = 720.0;
// Fallback vertical FOV for custom/non-perspective cameras.
const DEFAULT_VERTICAL_FOV_RAD: f64 = std::f64::consts::FRAC_PI_4;
// Guard against invalid near-zero perspective FOV values.
const MIN_VERTICAL_FOV_RAD: f64 = 0.001;
// Guard against invalid near-PI perspective FOV values.
const MAX_VERTICAL_FOV_RAD: f64 = std::f64::consts::PI - MIN_VERTICAL_FOV_RAD;
// Extra radius padding added beyond max camera distance for cislunar visibility.
const GLOBE_FAR_PLANE_RADIUS_PADDING: f32 = 2.0;

/// Marker for the camera controlled by a Ferrisium 3D globe view.
///
/// Apps with a custom 3D camera can add this marker to opt that camera into the
/// built-in globe orbit controls. If no marked camera exists and no other
/// `Camera3d` exists, Ferrisium spawns one automatically.
#[derive(Component, Debug, Default)]
pub struct Globe3dCamera;

/// Spawns the default `BigSpace` hierarchy, primary Earth body, and globe camera.
pub(crate) fn spawn_globe_camera(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    mut celestial_entities: ResMut<'_, CelestialSpaceEntities>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    existing_globe_cameras: Query<'_, '_, (), With<Globe3dCamera>>,
    existing_cameras: Query<'_, '_, (), With<Camera3d>>,
) {
    if globe_views.is_empty() || !existing_globe_cameras.is_empty() || !existing_cameras.is_empty()
    {
        return;
    }

    let Some(globe_state) = globe_views.iter().next().copied() else {
        return;
    };

    let mut root_entity = None;
    let mut primary_body_entity = None;
    let (camera_cell, camera_transform) =
        globe_camera_grid_transform(globe_state, *celestial_space, DVec3::ZERO, Quat::IDENTITY);

    commands.spawn_big_space(celestial_space.grid(), |root| {
        root.insert((Name::new("Ferrisium Celestial Root"), CelestialRoot));
        root_entity = Some(root.id());
        let earth = CelestialBody::earth();
        primary_body_entity = Some(
            root.spawn_spatial((
                Name::new("Ferrisium Primary Body"),
                PrimaryCelestialBody,
                MetricSceneBodyAnchorBundle::relative_to(
                    earth,
                    BodyId::EARTH,
                    f64::from(globe_state.camera_distance),
                ),
            ))
            .id(),
        );
        root.spawn_spatial((
            Name::new("Ferrisium Globe Camera"),
            Camera3d::default(),
            camera_cell,
            camera_transform,
            Globe3dCamera,
            MetricSceneCamera::primary(),
            FloatingOrigin,
        ));
    });

    celestial_entities.root = root_entity;
    celestial_entities.primary_body = primary_body_entity;
}

/// Places the active globe camera around the focused body and updates projection planes.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "The camera system needs explicit ECS resources plus optional BigSpace and projection components."
)]
pub(crate) fn sync_globe_camera(
    active_source: Res<'_, ActiveTileSource>,
    celestial_focus: Res<'_, CelestialFocus>,
    celestial_space: Res<'_, CelestialSpace>,
    focus_pivot: Res<'_, MetricSceneFocusPivot>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    bodies: Query<
        '_,
        '_,
        (&CelestialBodyPlacement, Option<&CellCoord>, &Transform),
        Without<Globe3dCamera>,
    >,
    mut globe_views: Query<'_, '_, &mut Globe3dState, With<Globe3dView>>,
    mut cameras: Query<
        '_,
        '_,
        (
            &mut Transform,
            Option<&mut CellCoord>,
            Option<&mut Projection>,
        ),
        With<Globe3dCamera>,
    >,
) {
    let Some(mut globe_state) = globe_views.iter_mut().next() else {
        return;
    };
    let Some((mut camera_transform, camera_cell, mut camera_projection)) =
        cameras.iter_mut().next()
    else {
        return;
    };

    let window = windows.iter().next();
    let focus_target = globe_focus_target_with_metric_pivot(
        *celestial_focus,
        *globe_state,
        *celestial_space,
        &focus_pivot,
        &bodies,
    );
    globe_state.radius = focus_target.radius;
    let min_camera_distance = min_camera_distance_for_focus(
        *globe_state,
        focus_target,
        &active_source.0,
        window,
        camera_projection.as_deref(),
    );
    globe_state.clamp_camera_distance_for_radius(focus_target.radius, min_camera_distance);
    let globe_state = *globe_state;

    if let Some(mut camera_cell) = camera_cell {
        let (next_cell, next_transform) = globe_camera_grid_transform(
            globe_state,
            *celestial_space,
            focus_target.center_units,
            focus_target.body_rotation,
        );
        *camera_cell = next_cell;
        *camera_transform = next_transform;
    } else {
        let camera_position = globe_camera_absolute_position(
            globe_state,
            focus_target.center_units,
            focus_target.body_rotation,
        );
        let look_direction = (focus_target.center_units - camera_position)
            .normalize_or_zero()
            .as_vec3();
        let up_frame = globe_camera_up_frame(globe_state, focus_target.body_rotation);
        *camera_transform =
            globe_camera_transform(camera_position.as_vec3(), look_direction, up_frame);
    }
    if let Some(projection) = camera_projection.as_deref_mut() {
        sync_globe_projection(
            projection,
            globe_state,
            focus_target,
            &active_source.0,
            window,
        );
    }
}

/// Converts absolute camera placement into a `BigSpace` cell and local transform.
pub(super) fn globe_camera_grid_transform(
    globe_state: Globe3dState,
    celestial_space: CelestialSpace,
    focus_center_units: DVec3,
    body_rotation: Quat,
) -> (CellCoord, Transform) {
    let camera_position =
        globe_camera_absolute_position(globe_state, focus_center_units, body_rotation);
    let placement = celestial_space.place_position_units(camera_position);
    let look_direction = (focus_center_units - camera_position).normalize_or_zero();
    let transform = globe_camera_transform(
        placement.translation,
        look_direction.as_vec3(),
        globe_camera_up_frame(globe_state, body_rotation),
    );

    (placement.cell, transform)
}

fn globe_camera_transform(
    translation: Vec3,
    look_direction: Vec3,
    up_frame: GlobeCameraUpFrame,
) -> Transform {
    let look_direction = if look_direction.length_squared() <= f32::EPSILON {
        Vec3::NEG_Z
    } else {
        look_direction.normalize()
    };
    let camera_up =
        projected_camera_up(look_direction, up_frame.preferred_up, up_frame.fallback_up);

    Transform::from_translation(translation).looking_to(look_direction, camera_up)
}

#[derive(Debug, Clone, Copy)]
struct GlobeCameraUpFrame {
    preferred_up: Vec3,
    fallback_up: Vec3,
}

fn globe_camera_up_frame(globe_state: Globe3dState, body_rotation: Quat) -> GlobeCameraUpFrame {
    match globe_state.camera_up_mode {
        GlobeCameraUpMode::BodyNorth => {
            let body_rotation = normalized_body_rotation_or_identity(body_rotation);
            GlobeCameraUpFrame {
                preferred_up: body_rotation.mul_vec3(Vec3::Y).normalize_or_zero(),
                fallback_up: body_rotation.mul_vec3(Vec3::X).normalize_or_zero(),
            }
        }
        GlobeCameraUpMode::SceneUp => GlobeCameraUpFrame {
            preferred_up: Vec3::Y,
            fallback_up: Vec3::X,
        },
    }
}

#[cfg(test)]
pub(super) fn globe_camera_preferred_up(globe_state: Globe3dState, body_rotation: Quat) -> Vec3 {
    globe_camera_up_frame(globe_state, body_rotation).preferred_up
}

pub(super) fn projected_camera_up(
    look_direction: Vec3,
    preferred_up: Vec3,
    fallback_up: Vec3,
) -> Vec3 {
    let forward = look_direction.normalize_or_zero();
    if forward.length_squared() <= f32::EPSILON {
        return Vec3::Y;
    }

    let preferred_up =
        if vec3_is_finite(preferred_up) && preferred_up.length_squared() > f32::EPSILON {
            preferred_up.normalize()
        } else {
            Vec3::Y
        };
    let projected_up = preferred_up - forward * preferred_up.dot(forward);
    if projected_up.length_squared() > 1.0e-8 {
        return projected_up.normalize();
    }

    let fallback_up = if vec3_is_finite(fallback_up) && fallback_up.length_squared() > f32::EPSILON
    {
        fallback_up.normalize()
    } else if forward.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let projected_fallback = fallback_up - forward * fallback_up.dot(forward);
    if projected_fallback.length_squared() > 1.0e-8 {
        return projected_fallback.normalize();
    }

    let scene_fallback = if forward.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let projected_scene_fallback = scene_fallback - forward * scene_fallback.dot(forward);
    projected_scene_fallback.normalize_or_zero()
}

fn vec3_is_finite(vector: Vec3) -> bool {
    vector.x.is_finite() && vector.y.is_finite() && vector.z.is_finite()
}

pub(crate) fn normalized_body_rotation_or_identity(rotation: Quat) -> Quat {
    if quat_is_finite(rotation) && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn quat_is_finite(rotation: Quat) -> bool {
    rotation.x.is_finite()
        && rotation.y.is_finite()
        && rotation.z.is_finite()
        && rotation.w.is_finite()
}

/// Returns camera position in the active orbit frame relative to the focused body.
pub(crate) fn globe_camera_position(globe_state: Globe3dState) -> DVec3 {
    let yaw_rad = globe_state.yaw_deg.to_radians();
    let pitch_rad = globe_state.pitch_deg.to_radians();
    let distance = f64::from(globe_state.camera_distance);
    let horizontal_radius = distance * pitch_rad.cos();

    DVec3::new(
        horizontal_radius * yaw_rad.sin(),
        distance * pitch_rad.sin(),
        horizontal_radius * yaw_rad.cos(),
    )
}

/// Returns absolute camera position by offsetting the focused body center.
fn globe_camera_absolute_position(
    globe_state: Globe3dState,
    focus_center_units: DVec3,
    body_rotation: Quat,
) -> DVec3 {
    focus_center_units + globe_camera_world_offset(globe_state, body_rotation)
}

fn globe_camera_world_offset(globe_state: Globe3dState, body_rotation: Quat) -> DVec3 {
    let orbit_position = globe_camera_position(globe_state);
    match globe_state.camera_up_mode {
        GlobeCameraUpMode::BodyNorth => normalized_body_rotation_or_identity(body_rotation)
            .mul_vec3(orbit_position.as_vec3())
            .as_dvec3(),
        GlobeCameraUpMode::SceneUp => orbit_position,
    }
}

/// Computes the Earth minimum camera distance from source pixel density.
#[allow(
    clippy::cast_possible_truncation,
    reason = "Source-resolution camera math stays in f64 until writing Bevy's f32 camera distance."
)]
pub(super) fn min_camera_distance_for_source(
    globe_state: Globe3dState,
    tile_source: &TileSource,
    window: Option<&Window>,
    projection: Option<&Projection>,
) -> f32 {
    let viewport_height_px = window.map_or(DEFAULT_VIEWPORT_HEIGHT_PX, |window| {
        f64::from(window.height())
    });
    let vertical_fov_rad = perspective_vertical_fov(projection);
    let min_surface_distance = source_resolution_surface_distance(
        globe_state,
        tile_source,
        viewport_height_px,
        vertical_fov_rad,
    );

    (f64::from(globe_state.radius) + min_surface_distance) as f32
}

/// Computes the minimum camera distance for the currently focused body.
pub(super) fn min_camera_distance_for_focus(
    globe_state: Globe3dState,
    focus_target: GlobeFocusTarget,
    tile_source: &TileSource,
    window: Option<&Window>,
    projection: Option<&Projection>,
) -> f32 {
    if focus_target.body == BodyId::EARTH {
        return min_camera_distance_for_source(globe_state, tile_source, window, projection);
    }

    focus_target.radius * (1.0 + MIN_CAMERA_SURFACE_DISTANCE_FACTOR_F32)
}

/// Returns source-resolution-limited height above the globe surface.
pub(super) fn source_resolution_surface_distance(
    globe_state: Globe3dState,
    tile_source: &TileSource,
    viewport_height_px: f64,
    vertical_fov_rad: f64,
) -> f64 {
    let radius = f64::from(globe_state.radius).max(f64::EPSILON);
    let tile_size = f64::from(tile_source.normalized_raster_tile_size());
    let (_, max_zoom) = tile_source.normalized_zoom_range();
    let source_pixels_per_world = tile_size * 2.0_f64.powi(i32::from(max_zoom));
    let source_pixel_world_size = std::f64::consts::TAU * radius / source_pixels_per_world;
    let viewport_height_px = viewport_height_px.max(1.0);
    let vertical_fov_rad = vertical_fov_rad.clamp(MIN_VERTICAL_FOV_RAD, MAX_VERTICAL_FOV_RAD);
    let world_per_screen_pixel_at_unit_distance =
        2.0 * (vertical_fov_rad * 0.5).tan() / viewport_height_px;
    let source_limited_distance = source_pixel_world_size * SOURCE_RESOLUTION_SCREEN_PIXELS
        / world_per_screen_pixel_at_unit_distance;
    let fallback_distance = radius * MIN_CAMERA_SURFACE_DISTANCE_FACTOR;

    source_limited_distance.max(fallback_distance)
}

/// Extracts a usable vertical FOV, falling back for non-perspective cameras.
fn perspective_vertical_fov(projection: Option<&Projection>) -> f64 {
    match projection {
        Some(Projection::Perspective(perspective)) => f64::from(perspective.fov),
        _ => DEFAULT_VERTICAL_FOV_RAD,
    }
}

/// Updates perspective near/far planes for the current globe view.
#[allow(
    clippy::cast_possible_truncation,
    reason = "Projection near planes are f32 while source-resolution camera math stays in f64."
)]
pub(super) fn sync_globe_projection(
    projection: &mut Projection,
    globe_state: Globe3dState,
    focus_target: GlobeFocusTarget,
    tile_source: &TileSource,
    window: Option<&Window>,
) {
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    let viewport_height_px = window.map_or(DEFAULT_VIEWPORT_HEIGHT_PX, |window| {
        f64::from(window.height())
    });
    let source_min_surface_distance = if focus_target.body == BodyId::EARTH {
        source_resolution_surface_distance(
            globe_state,
            tile_source,
            viewport_height_px,
            f64::from(perspective.fov),
        )
    } else {
        f64::from(focus_target.radius) * MIN_CAMERA_SURFACE_DISTANCE_FACTOR
    };
    perspective.far = min_globe_far_plane(globe_state, focus_target.radius);

    let current_surface_distance =
        surface_distance(globe_state.camera_distance, focus_target.radius)
            .max(source_min_surface_distance);
    let near = globe_surface_near_plane(
        current_surface_distance as f32,
        focus_target.radius,
        perspective.far,
    );

    perspective.near = near;
    perspective.near_clip_plane = Vec4::new(0.0, 0.0, -1.0, -near);
}

/// Returns the minimum far plane needed for the configured orbit range.
pub(super) fn min_globe_far_plane(globe_state: Globe3dState, focus_radius: f32) -> f32 {
    max_camera_distance_for_radius(focus_radius).max(globe_state.camera_distance)
        + focus_radius * GLOBE_FAR_PLANE_RADIUS_PADDING
}
