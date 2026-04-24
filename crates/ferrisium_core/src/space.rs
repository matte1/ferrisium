//! Renderer-agnostic celestial spatial primitives.
//!
//! This module is the boundary between authoritative astrodynamics data and
//! renderer-specific placement. Distances are represented in kilometers and
//! all positions/velocities use `f64`; Bevy-facing crates are responsible for
//! converting these values into local render coordinates.

mod bodies;
mod ephemeris;
mod frames;
mod math;
mod render;
mod sky;
mod surface;
mod time;
mod trajectory;
mod units;

pub use bodies::{
    CelestialBody, ReferenceEllipsoid, SUN_MEAN_RADIUS_KILOMETERS,
    WGS84_EQUATORIAL_RADIUS_KILOMETERS, WGS84_INVERSE_FLATTENING,
};
pub use ephemeris::{
    EphemerisError, EphemerisProvider, EphemerisResult, FrameOrientation, OrientationRequest,
    StateRequest, StateVector,
};
pub use frames::{BodyId, FrameId, ReferenceFrame, ReferenceFrameKind};
pub use math::{QuatD, Vec3d};
pub use render::{RenderScale, DEFAULT_RENDER_KILOMETERS_PER_UNIT};
pub use sky::{
    equatorial_from_j2000_direction, j2000_unit_vector_from_equatorial,
    nasa_celestial_map_uv_from_j2000_direction, EquatorialDirection,
};
pub use surface::{spherical_surface_position, spherical_surface_subpoint, SurfaceSubpoint};
pub use time::Epoch;
pub use trajectory::{
    epoch_delta_seconds, offset_epoch_seconds, sample_epochs_inclusive, sample_trajectory_between,
    trajectory_samples_are_stale, TrajectoryPath, TrajectoryTrailConfig,
    AVERAGE_GREGORIAN_MONTH_SECONDS, NANOS_PER_SECOND_F64,
};
pub use units::{Kilometers, METERS_PER_KILOMETER};
