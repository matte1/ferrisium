//! Public globe view state and renderer-facing distance helpers.
//!
//! This module keeps user-configurable orbit/LOD state separate from systems
//! that mutate it. Distances are Bevy render units here, derived from
//! `ferrisium_core::space` metadata at the crate boundary.

use bevy::prelude::*;
use ferrisium_core::space::{CelestialBody, RenderScale};

// Default world-space radius for examples that do not configure a globe size.
const DEFAULT_GLOBE_RADIUS: f32 = 4.0;
// Default camera distance expressed as a multiple of the globe radius.
const DEFAULT_CAMERA_DISTANCE_RADIUS_FACTOR: f32 = 2.5;
// Default camera distance keeps the whole globe visible on first render.
const DEFAULT_CAMERA_DISTANCE: f32 = DEFAULT_GLOBE_RADIUS * DEFAULT_CAMERA_DISTANCE_RADIUS_FACTOR;
// Default far-view tile zoom; high enough to avoid a coarse first impression.
const DEFAULT_GLOBE_TILE_ZOOM: u8 = 4;
// Patch budget that balances globe sharpness with browser mesh/request cost.
const DEFAULT_MAX_SELECTED_GLOBE_TILES: usize = 768;
// Outward zoom cap expressed as a multiple of globe radius, high enough for cislunar views.
const MAX_CAMERA_DISTANCE_FACTOR: f32 = 100.0;
// Absolute minimum surface height as a fraction of radius, used after source-resolution limits.
/// Minimum camera surface height as a fraction of focused body radius.
pub(super) const MIN_CAMERA_SURFACE_DISTANCE_FACTOR: f64 = 0.000_025;
// Same lower-bound ratio in Bevy's f32 camera-distance space.
/// f32 form of [`MIN_CAMERA_SURFACE_DISTANCE_FACTOR`] for camera clamps.
pub(super) const MIN_CAMERA_SURFACE_DISTANCE_FACTOR_F32: f32 = 0.000_025;

/// Marker component for a 3D Ferrisium globe view.
#[derive(Component, Debug, Default)]
pub struct Globe3dView;

/// Orbit/up-frame convention used by a [`Globe3dState`] camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlobeCameraUpMode {
    /// Use the focused body's body-fixed frame for orbit and north-up camera roll.
    #[default]
    BodyNorth,
    /// Use the scene frame for orbit and scene `+Y` as the preferred screen-up direction.
    SceneUp,
}

/// Mutable camera/orbit state for a 3D globe view.
#[derive(Component, Debug, Clone, Copy)]
pub struct Globe3dState {
    /// Globe radius in world units.
    pub radius: f32,
    /// Camera distance that corresponds to the base globe tile zoom.
    pub base_camera_distance: f32,
    /// Camera orbit distance from the globe center.
    pub camera_distance: f32,
    /// Horizontal orbit angle in degrees.
    pub yaw_deg: f64,
    /// Vertical orbit angle in degrees.
    pub pitch_deg: f64,
    /// Preferred camera orbit/up-frame convention.
    pub camera_up_mode: GlobeCameraUpMode,
    /// Base tile zoom used when the camera is at `base_camera_distance`.
    ///
    /// As the camera moves inward, Ferrisium may request higher globe tile
    /// zooms by stepping upward from this base level.
    pub tile_zoom: u8,
    /// Maximum number of globe tile patches selected for the current view.
    ///
    /// Higher values keep the globe sharper at wide zooms and during close
    /// inspection, at the cost of more HTTP requests and more patch meshes.
    pub max_selected_tiles: usize,
}

impl Default for Globe3dState {
    fn default() -> Self {
        Self {
            radius: DEFAULT_GLOBE_RADIUS,
            base_camera_distance: DEFAULT_CAMERA_DISTANCE,
            camera_distance: DEFAULT_CAMERA_DISTANCE,
            yaw_deg: 0.0,
            pitch_deg: 22.0,
            camera_up_mode: GlobeCameraUpMode::default(),
            tile_zoom: DEFAULT_GLOBE_TILE_ZOOM,
            max_selected_tiles: DEFAULT_MAX_SELECTED_GLOBE_TILES,
        }
    }
}

impl Globe3dState {
    /// Creates globe state from physical body metadata and a render scale.
    #[must_use]
    pub fn for_body(body: &CelestialBody, render_scale: RenderScale) -> Self {
        let radius = globe_radius_from_render_units(body.mean_radius_units(render_scale));
        let camera_distance = radius * DEFAULT_CAMERA_DISTANCE_RADIUS_FACTOR;

        Self {
            radius,
            base_camera_distance: camera_distance,
            camera_distance,
            ..Self::default()
        }
    }

    pub(super) fn clamp_camera_distance_for_radius(
        &mut self,
        focus_radius: f32,
        min_camera_distance: f32,
    ) {
        let max_camera_distance = max_camera_distance_for_radius(focus_radius);
        let min_camera_distance = min_camera_distance.clamp(focus_radius, max_camera_distance);
        self.camera_distance = self
            .camera_distance
            .clamp(min_camera_distance, max_camera_distance);
    }

    pub(crate) fn base_surface_distance(self) -> f64 {
        surface_distance(self.base_camera_distance, self.radius).max(f64::EPSILON)
    }

    pub(crate) fn current_surface_distance(self) -> f64 {
        surface_distance(self.camera_distance, self.radius)
    }
}

/// Minimal bundle for spawning a Ferrisium 3D globe view.
#[derive(Bundle, Debug, Default)]
pub struct Globe3dViewBundle {
    /// Marker component identifying the entity as a Ferrisium globe view.
    pub view: Globe3dView,
    /// Mutable orbit, tile LOD, and tile budget state for the view.
    pub globe_state: Globe3dState,
}

/// Returns camera height above the focused body surface.
pub(super) fn surface_distance(camera_distance: f32, radius: f32) -> f64 {
    f64::from((camera_distance - radius).max(0.0))
}

/// Returns the outward orbit cap for a focused body radius.
pub(super) fn max_camera_distance_for_radius(radius: f32) -> f32 {
    radius * MAX_CAMERA_DISTANCE_FACTOR
}

/// Converts a finite positive f64 radius into Bevy's f32 render space.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Renderer-facing globe radius must eventually enter Bevy's f32 transform space."
)]
pub(super) fn globe_radius_from_render_units(radius_units: f64) -> f32 {
    if radius_units.is_finite() && radius_units > 0.0 && radius_units <= f64::from(f32::MAX) {
        radius_units as f32
    } else {
        DEFAULT_GLOBE_RADIUS
    }
}
