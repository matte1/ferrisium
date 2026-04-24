//! Celestial sky-coordinate helpers for inertial J2000 views.
//!
//! Ferrisium's J2000 sky convention uses a right-handed equatorial basis:
//! `+X` points to right ascension 0h at declination 0, `+Y` points to right
//! ascension 6h at declination 0, and `+Z` points to the north celestial pole.
//! These helpers intentionally do not model local observer horizons or
//! sidereal time; they only describe directions on the inertial celestial
//! sphere.

use std::f64::consts::TAU;

use super::Vec3d;

/// Right ascension and declination in the J2000 equatorial frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquatorialDirection {
    /// Right ascension, normalized to `[0, 24)` hours.
    pub right_ascension_hours: f64,
    /// Declination in degrees.
    pub declination_degrees: f64,
}

impl EquatorialDirection {
    /// Creates an equatorial direction, wrapping right ascension and clamping declination.
    #[must_use]
    pub fn new(right_ascension_hours: f64, declination_degrees: f64) -> Self {
        let right_ascension_hours = if right_ascension_hours.is_finite() {
            right_ascension_hours.rem_euclid(24.0)
        } else {
            0.0
        };
        let declination_degrees = if declination_degrees.is_finite() {
            declination_degrees.clamp(-90.0, 90.0)
        } else {
            0.0
        };

        Self {
            right_ascension_hours,
            declination_degrees,
        }
    }

    /// Converts this right ascension/declination into a J2000 unit vector.
    #[must_use]
    pub fn to_j2000_unit_vector(self) -> Vec3d {
        j2000_unit_vector_from_equatorial(self)
    }
}

/// Converts J2000 right ascension/declination into a unit vector.
#[must_use]
pub fn j2000_unit_vector_from_equatorial(direction: EquatorialDirection) -> Vec3d {
    let right_ascension = direction.right_ascension_hours / 24.0 * TAU;
    let declination = direction.declination_degrees.to_radians();
    let cos_declination = declination.cos();

    Vec3d::new(
        cos_declination * right_ascension.cos(),
        cos_declination * right_ascension.sin(),
        declination.sin(),
    )
}

/// Converts a non-zero J2000 vector into right ascension/declination.
#[must_use]
pub fn equatorial_from_j2000_direction(direction: Vec3d) -> Option<EquatorialDirection> {
    let direction = normalized_direction(direction)?;
    let right_ascension = direction.y.atan2(direction.x).rem_euclid(TAU);
    let declination = direction.z.clamp(-1.0, 1.0).asin();

    Some(EquatorialDirection::new(
        right_ascension / TAU * 24.0,
        declination.to_degrees(),
    ))
}

/// Returns NASA SVS celestial-map UV coordinates for a J2000 direction.
///
/// The Deep Star Maps celestial products are plate-carree maps centered at
/// right ascension 0h with right ascension increasing toward image-left.
/// Returned `u` is normalized to `[0, 1)` and `v` to `[0, 1]`.
#[must_use]
pub fn nasa_celestial_map_uv_from_j2000_direction(direction: Vec3d) -> Option<(f64, f64)> {
    let equatorial = equatorial_from_j2000_direction(direction)?;
    let u = (0.5 - equatorial.right_ascension_hours / 24.0).rem_euclid(1.0);
    let v = 0.5 - equatorial.declination_degrees / 180.0;

    Some((u, v.clamp(0.0, 1.0)))
}

fn normalized_direction(direction: Vec3d) -> Option<Vec3d> {
    let length_squared = direction.x.mul_add(
        direction.x,
        direction.y.mul_add(direction.y, direction.z * direction.z),
    );
    if !length_squared.is_finite() || length_squared <= 0.0 {
        return None;
    }

    let inverse_length = length_squared.sqrt().recip();
    Some(direction.scale(inverse_length))
}

#[cfg(test)]
mod tests {
    use super::{
        equatorial_from_j2000_direction, j2000_unit_vector_from_equatorial,
        nasa_celestial_map_uv_from_j2000_direction, EquatorialDirection,
    };
    use crate::space::Vec3d;

    const EPSILON: f64 = 1.0e-12;

    fn assert_vec3d_close(lhs: Vec3d, rhs: Vec3d) {
        assert!(
            (lhs.x - rhs.x).abs() <= EPSILON,
            "x mismatch: lhs={}, rhs={}",
            lhs.x,
            rhs.x
        );
        assert!(
            (lhs.y - rhs.y).abs() <= EPSILON,
            "y mismatch: lhs={}, rhs={}",
            lhs.y,
            rhs.y
        );
        assert!(
            (lhs.z - rhs.z).abs() <= EPSILON,
            "z mismatch: lhs={}, rhs={}",
            lhs.z,
            rhs.z
        );
    }

    #[test]
    fn equatorial_basis_matches_j2000_axes() {
        assert_vec3d_close(
            j2000_unit_vector_from_equatorial(EquatorialDirection::new(0.0, 0.0)),
            Vec3d::new(1.0, 0.0, 0.0),
        );
        assert_vec3d_close(
            j2000_unit_vector_from_equatorial(EquatorialDirection::new(6.0, 0.0)),
            Vec3d::new(0.0, 1.0, 0.0),
        );
        assert_vec3d_close(
            j2000_unit_vector_from_equatorial(EquatorialDirection::new(12.0, 0.0)),
            Vec3d::new(-1.0, 0.0, 0.0),
        );
        assert_vec3d_close(
            j2000_unit_vector_from_equatorial(EquatorialDirection::new(0.0, 90.0)),
            Vec3d::new(0.0, 0.0, 1.0),
        );
    }

    #[test]
    fn equatorial_conversion_round_trips() {
        let original = EquatorialDirection::new(13.25, 48.5);
        let vector = original.to_j2000_unit_vector();
        let roundtrip = equatorial_from_j2000_direction(vector);

        assert!(roundtrip.is_some());
        let Some(roundtrip) = roundtrip else {
            return;
        };

        assert!((roundtrip.right_ascension_hours - original.right_ascension_hours).abs() < EPSILON);
        assert!((roundtrip.declination_degrees - original.declination_degrees).abs() < EPSILON);
    }

    #[test]
    fn nasa_celestial_map_uv_places_right_ascension_leftward() {
        let ra_zero = EquatorialDirection::new(0.0, 0.0).to_j2000_unit_vector();
        let ra_six = EquatorialDirection::new(6.0, 0.0).to_j2000_unit_vector();
        let north_pole = EquatorialDirection::new(0.0, 90.0).to_j2000_unit_vector();

        let ra_zero_uv = nasa_celestial_map_uv_from_j2000_direction(ra_zero);
        assert!(ra_zero_uv.is_some());
        let Some((u, v)) = ra_zero_uv else {
            return;
        };
        assert!((u - 0.5).abs() < EPSILON);
        assert!((v - 0.5).abs() < EPSILON);

        let ra_six_uv = nasa_celestial_map_uv_from_j2000_direction(ra_six);
        assert!(ra_six_uv.is_some());
        let Some((u, v)) = ra_six_uv else {
            return;
        };
        assert!((u - 0.25).abs() < EPSILON);
        assert!((v - 0.5).abs() < EPSILON);

        let north_pole_uv = nasa_celestial_map_uv_from_j2000_direction(north_pole);
        assert!(north_pole_uv.is_some());
        let Some((u, v)) = north_pole_uv else {
            return;
        };
        assert!((u - 0.5).abs() < EPSILON);
        assert!((v - 0.0).abs() < EPSILON);
    }

    #[test]
    fn zero_j2000_direction_has_no_equatorial_coordinate() {
        assert_eq!(equatorial_from_j2000_direction(Vec3d::ZERO), None);
    }
}
