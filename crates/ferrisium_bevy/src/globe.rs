//! ECS-facing 3D globe camera state and orbit input.
//!
//! Globe tile selection itself now lives in `ferrisium_core::globe`; this
//! module is responsible for camera motion and converting Bevy cursor rays into
//! surface targets for cursor-aware zoom.

mod camera;
mod focus;
mod input;
pub(crate) mod objects;
mod ray;
pub(crate) mod skybox;
mod state;

pub use camera::Globe3dCamera;
pub use state::{Globe3dState, Globe3dView, Globe3dViewBundle, GlobeCameraUpMode};

pub(crate) use camera::{
    globe_camera_position, normalized_body_rotation_or_identity, spawn_globe_camera,
    sync_globe_camera,
};
pub(crate) use focus::globe_focus_target_with_metric_pivot;
pub(crate) use input::handle_globe_input;
pub(crate) use objects::{
    anchor as object_anchor, label as object_label, link as object_link,
    position as object_position,
};
pub(crate) use ray::{current_globe_camera_transform, hovered_globe_surface_direction};
pub(crate) use skybox as skybox_config;

#[cfg(test)]
use camera::{
    globe_camera_grid_transform, globe_camera_preferred_up, min_camera_distance_for_source,
    min_globe_far_plane, projected_camera_up, source_resolution_surface_distance,
    sync_globe_projection,
};
#[cfg(test)]
use focus::GlobeFocusTarget;
#[cfg(test)]
use input::{
    camera_direction_for_surface_grab_pan, camera_frame_surface_direction, clamp_scroll_delta,
    distance_control_scale, globe_orbit_angles, globe_surface_pan_degrees_per_pixel,
    orbit_angles_for_surface_direction, pinch_zoom_delta, wheel_zoom_delta, zoom_globe_camera,
    MAX_SCROLL_DELTA_PER_FRAME, ZOOM_EXP_FACTOR,
};
#[cfg(test)]
use ray::{ray_origin_units, ray_sphere_intersection, ray_sphere_intersection_from_origin};
#[cfg(test)]
use state::{globe_radius_from_render_units, max_camera_distance_for_radius};

#[cfg(test)]
mod tests;
