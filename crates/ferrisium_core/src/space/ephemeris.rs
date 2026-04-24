//! Provider-neutral ephemeris request and result types.
//!
//! Core code describes positions, velocities, and frame orientations without
//! depending on a specific kernel or astrodynamics library. Provider crates can
//! implement [`EphemerisProvider`] and translate these types at the boundary.

use std::error::Error;
use std::fmt;

use super::{BodyId, Epoch, FrameId, QuatD, Vec3d};

/// Position and velocity of a target relative to an origin in a specific frame.
#[derive(Debug, Clone, PartialEq)]
pub struct StateVector {
    /// Body, spacecraft, or synthetic object whose state is represented.
    pub target: BodyId,
    /// Body or barycenter that the position and velocity are relative to.
    pub origin: BodyId,
    /// Reference frame for the state vector components.
    pub frame: FrameId,
    /// Epoch of the state vector.
    pub epoch: Epoch,
    /// Position of `target` relative to `origin`, in kilometers.
    pub position_km: Vec3d,
    /// Velocity of `target` relative to `origin`, in kilometers per second.
    pub velocity_km_s: Vec3d,
}

impl StateVector {
    /// Creates a relative state vector.
    #[must_use]
    pub fn relative_to(
        target: BodyId,
        origin: BodyId,
        frame: FrameId,
        epoch: Epoch,
        position_km: Vec3d,
        velocity_km_s: Vec3d,
    ) -> Self {
        Self {
            target,
            origin,
            frame,
            epoch,
            position_km,
            velocity_km_s,
        }
    }
}

/// Request for a translational ephemeris state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRequest {
    /// Body, spacecraft, or synthetic object whose state should be returned.
    pub target: BodyId,
    /// Body or barycenter that the state should be relative to.
    pub origin: BodyId,
    /// Reference frame for the requested state vector components.
    pub frame: FrameId,
    /// Epoch of the requested state.
    pub epoch: Epoch,
}

impl StateRequest {
    /// Creates a request for `target` relative to `origin` in `frame` at `epoch`.
    #[must_use]
    pub fn new(target: BodyId, origin: BodyId, frame: FrameId, epoch: Epoch) -> Self {
        Self {
            target,
            origin,
            frame,
            epoch,
        }
    }
}

/// Request for frame orientation at an epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientationRequest {
    /// Frame whose basis should be rotated.
    pub from: FrameId,
    /// Frame to rotate into.
    pub to: FrameId,
    /// Epoch of the requested orientation.
    pub epoch: Epoch,
}

impl OrientationRequest {
    /// Creates a request for the rotation from `from` into `to` at `epoch`.
    #[must_use]
    pub fn new(from: FrameId, to: FrameId, epoch: Epoch) -> Self {
        Self { from, to, epoch }
    }
}

/// Rotation from one reference frame into another at a specific epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameOrientation {
    /// Frame whose basis is rotated.
    pub from: FrameId,
    /// Frame that the rotation maps into.
    pub to: FrameId,
    /// Epoch of the orientation.
    pub epoch: Epoch,
    /// Unit quaternion that rotates vectors from `from` into `to`.
    pub rotation: QuatD,
    /// Optional angular velocity of `from` relative to `to`, in radians per second.
    pub angular_velocity_rad_s: Option<Vec3d>,
}

impl FrameOrientation {
    /// Creates a frame orientation and normalizes the provided quaternion.
    #[must_use]
    pub fn new(
        from: FrameId,
        to: FrameId,
        epoch: Epoch,
        rotation: QuatD,
        angular_velocity_rad_s: Option<Vec3d>,
    ) -> Self {
        Self {
            from,
            to,
            epoch,
            rotation: rotation.normalized(),
            angular_velocity_rad_s,
        }
    }

    /// Creates an identity orientation between two frame identifiers.
    #[must_use]
    pub fn identity(from: FrameId, to: FrameId, epoch: Epoch) -> Self {
        Self::new(from, to, epoch, QuatD::IDENTITY, None)
    }
}

/// Error returned by renderer-agnostic ephemeris providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EphemerisError {
    /// The provider does not have data for the requested body, frame, or epoch.
    MissingData(String),
    /// The request is valid Ferrisium state but unsupported by this provider.
    UnsupportedQuery(String),
    /// The provider failed for an implementation-specific reason.
    ProviderFailure(String),
}

impl fmt::Display for EphemerisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingData(message) => write!(formatter, "missing ephemeris data: {message}"),
            Self::UnsupportedQuery(message) => {
                write!(formatter, "unsupported ephemeris query: {message}")
            }
            Self::ProviderFailure(message) => {
                write!(formatter, "ephemeris provider failed: {message}")
            }
        }
    }
}

impl Error for EphemerisError {}

/// Result type used by ephemeris providers.
pub type EphemerisResult<T> = Result<T, EphemerisError>;

/// Renderer-agnostic source of celestial translational and rotational state.
pub trait EphemerisProvider {
    /// Returns the target state relative to the requested origin.
    ///
    /// Position components are kilometers and velocity components are kilometers per second.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError`] when the provider has no data for the request
    /// or cannot represent the requested body, frame, or epoch.
    fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector>;

    /// Returns the orientation from one frame into another at the requested epoch.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError`] when the provider has no orientation data for
    /// the request or cannot represent the requested frame conversion.
    fn orientation(&self, request: &OrientationRequest) -> EphemerisResult<FrameOrientation>;
}
