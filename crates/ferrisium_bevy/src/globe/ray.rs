//! Cursor-ray helpers for globe surface picking.
//!
//! The input system uses these helpers to intersect the current cursor ray with
//! the focused body's sphere. In `big_space` scenes the camera origin must be
//! reconstructed from its cell coordinate before ray/sphere intersection.

use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;

use crate::celestial::CelestialSpace;

pub(crate) type GlobeRayCamera<'a> = (
    &'a Camera,
    &'a GlobalTransform,
    &'a Transform,
    Option<&'a CellCoord>,
);

/// Returns the focused-body surface direction under the cursor, if any.
pub(crate) fn hovered_globe_surface_direction(
    window: Option<&Window>,
    camera: Option<GlobeRayCamera<'_>>,
    celestial_space: CelestialSpace,
    radius: f32,
    center: DVec3,
) -> Option<DVec3> {
    let cursor_position = window?.cursor_position()?;
    globe_surface_direction_at_screen_position(
        cursor_position,
        camera,
        celestial_space,
        radius,
        center,
    )
}

/// Returns the focused-body surface direction under a screen position, if any.
pub(crate) fn globe_surface_direction_at_screen_position(
    screen_position: Vec2,
    camera: Option<GlobeRayCamera<'_>>,
    celestial_space: CelestialSpace,
    radius: f32,
    center: DVec3,
) -> Option<DVec3> {
    let (camera, camera_global_transform, camera_local_transform, camera_cell) = camera?;
    let camera_transform = current_globe_camera_transform(
        camera_global_transform,
        camera_local_transform,
        camera_cell,
    );
    let ray = camera
        .viewport_to_world(&camera_transform, screen_position)
        .ok()?;
    let ray_origin = ray_origin_units(celestial_space, camera_cell, ray.origin);
    let ray_direction = ray.direction.as_vec3().as_dvec3();
    let intersection =
        ray_sphere_intersection_from_origin(ray_origin, ray_direction, center, radius)?;

    Some((intersection - center).normalize_or_zero())
}

/// Returns the camera transform that is current for same-frame globe picking and projection.
///
/// `sync_globe_camera` writes the `BigSpace` camera's local [`Transform`] in `Update`, while
/// [`GlobalTransform`] is propagated later. Systems that also run in `Update` need this helper to
/// avoid reading the previous frame's camera pose.
pub(crate) fn current_globe_camera_transform(
    global_transform: &GlobalTransform,
    local_transform: &Transform,
    cell: Option<&CellCoord>,
) -> GlobalTransform {
    if cell.is_some() {
        GlobalTransform::from(*local_transform)
    } else {
        *global_transform
    }
}

/// Reconstructs a camera ray origin in absolute render units.
pub(super) fn ray_origin_units(
    celestial_space: CelestialSpace,
    cell: Option<&CellCoord>,
    origin: Vec3,
) -> DVec3 {
    cell.map_or_else(
        || origin.as_dvec3(),
        |cell| {
            celestial_space
                .grid()
                .grid_position_double(cell, &Transform::from_translation(origin))
        },
    )
}

// Intersect the current camera ray with the sphere so wheel zoom can bias the
// orbit toward the hovered point instead of always moving toward screen center.
/// Test helper that accepts Bevy's `Ray3d` directly.
#[cfg(test)]
pub(super) fn ray_sphere_intersection(ray: Ray3d, center: DVec3, radius: f32) -> Option<DVec3> {
    ray_sphere_intersection_from_origin(
        ray.origin.as_dvec3(),
        ray.direction.as_vec3().as_dvec3(),
        center,
        radius,
    )
}

/// Intersects a ray in absolute render units with a sphere.
pub(super) fn ray_sphere_intersection_from_origin(
    origin: DVec3,
    direction: DVec3,
    center: DVec3,
    radius: f32,
) -> Option<DVec3> {
    let origin = origin - center;
    let radius = f64::from(radius);
    let a = direction.length_squared();
    let b = 2.0 * origin.dot(direction);
    let c = origin.length_squared() - radius * radius;
    let discriminant = b.mul_add(b, -4.0 * a * c);

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let near_distance = (-b - sqrt_discriminant) / (2.0 * a);
    let far_distance = (-b + sqrt_discriminant) / (2.0 * a);
    let distance = if near_distance > f64::EPSILON {
        near_distance
    } else if far_distance > f64::EPSILON {
        far_distance
    } else {
        return None;
    };

    Some(origin + center + direction * distance)
}

#[cfg(test)]
mod tests {
    use super::current_globe_camera_transform;
    use bevy::prelude::{GlobalTransform, Transform, Vec3};
    use big_space::prelude::CellCoord;

    #[test]
    fn current_globe_camera_transform_uses_local_transform_for_big_space_camera() {
        let stale_global = GlobalTransform::from(Transform::from_translation(Vec3::X));
        let current_local = Transform::from_translation(Vec3::Y * 2.0);
        let cell = CellCoord::default();

        let transform = current_globe_camera_transform(&stale_global, &current_local, Some(&cell));

        assert_eq!(transform.translation(), current_local.translation);
    }

    #[test]
    fn current_globe_camera_transform_keeps_global_transform_without_big_space_cell() {
        let global = GlobalTransform::from(Transform::from_translation(Vec3::X));
        let local = Transform::from_translation(Vec3::Y * 2.0);

        let transform = current_globe_camera_transform(&global, &local, None);

        assert_eq!(transform.translation(), global.translation());
    }
}
