//! Conversion helpers between physical distances and renderer units.
//!
//! Core spatial data stays in kilometers. Renderer crates choose a
//! [`RenderScale`] when converting that physical state into Bevy units or other
//! scene coordinates.

use super::{Kilometers, Vec3d};

/// Default physically meaningful render scale: one Bevy unit equals one kilometer.
pub const DEFAULT_RENDER_KILOMETERS_PER_UNIT: f64 = 1.0;

/// Physical-to-render scale used when converting core positions to renderer units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderScale {
    /// Kilometers represented by one Bevy render unit.
    pub kilometers_per_unit: f64,
}

impl RenderScale {
    /// Physically meaningful default: one Bevy unit equals one kilometer.
    pub const PHYSICAL: Self = Self {
        kilometers_per_unit: DEFAULT_RENDER_KILOMETERS_PER_UNIT,
    };

    /// Creates a render scale from kilometers per render unit.
    #[must_use]
    pub const fn from_kilometers_per_unit(kilometers_per_unit: f64) -> Self {
        Self {
            kilometers_per_unit,
        }
    }

    /// Returns a finite positive render scale, falling back to physical scale.
    #[must_use]
    pub fn normalized(self) -> Self {
        if self.kilometers_per_unit.is_finite() && self.kilometers_per_unit > 0.0 {
            self
        } else {
            Self::PHYSICAL
        }
    }

    /// Converts a scalar distance to Bevy render units.
    #[must_use]
    pub fn distance_to_units(self, distance: Kilometers) -> f64 {
        distance.as_f64() / self.normalized().kilometers_per_unit
    }

    /// Converts Bevy render units back to a scalar kilometer distance.
    #[must_use]
    pub fn units_to_distance(self, units: f64) -> Kilometers {
        Kilometers::new(units * self.normalized().kilometers_per_unit)
    }

    /// Converts a position vector in kilometers to Bevy render units.
    #[must_use]
    pub fn position_to_units(self, position_km: Vec3d) -> Vec3d {
        position_km.scale(1.0 / self.normalized().kilometers_per_unit)
    }
}

impl Default for RenderScale {
    fn default() -> Self {
        Self::PHYSICAL
    }
}
