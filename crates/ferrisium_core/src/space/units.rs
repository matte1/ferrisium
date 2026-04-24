//! Distance units shared by celestial metadata and ephemeris data.
//!
//! Public core spatial APIs use kilometers. Meters are accepted or emitted only
//! through explicit conversion helpers so unit changes stay visible at call
//! sites.

/// Meters in one kilometer.
pub const METERS_PER_KILOMETER: f64 = 1_000.0;

/// Scalar distance in kilometers.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Kilometers {
    /// Raw distance value in kilometers.
    pub value: f64,
}

impl Kilometers {
    /// Creates a scalar kilometer value.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self { value }
    }

    /// Converts meters into kilometers.
    #[must_use]
    pub fn from_meters(meters: f64) -> Self {
        Self::new(meters / METERS_PER_KILOMETER)
    }

    /// Returns the scalar value in kilometers.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.value
    }

    /// Returns the scalar value in meters.
    #[must_use]
    pub fn as_meters(self) -> f64 {
        self.value * METERS_PER_KILOMETER
    }
}
