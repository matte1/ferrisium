//! Surface projection helpers for body-relative object states.
//!
//! These helpers are intentionally renderer-neutral. A spacecraft state remains
//! an authoritative 3D vector in kilometers; callers can derive a surface
//! subpoint for map markers, globe overlays, ground tracks, or footprints
//! without changing the underlying state.

use crate::coords::{wrap_longitude, LonLat};

use super::{Kilometers, Vec3d};

/// Spherical surface subpoint derived from a body-fixed position vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSubpoint {
    /// Longitude/latitude of the radial projection onto the reference sphere.
    pub lon_lat: LonLat,
    /// Height above the reference sphere along the radial direction.
    pub altitude_km: Kilometers,
}

/// Projects a body-fixed position vector onto a spherical reference body.
///
/// `position_km` must be expressed relative to the body center in the body's
/// rotating/body-fixed frame. The returned longitude convention matches the
/// rest of Ferrisium's globe math: `+Z` is lon `0deg`, `+X` is lon `90deg`,
/// and `+Y` is the north pole. This function does not rotate inertial states
/// into a body-fixed frame; ephemeris providers remain responsible for that
/// frame transform.
#[must_use]
pub fn spherical_surface_subpoint(
    position_km: Vec3d,
    reference_radius_km: Kilometers,
) -> Option<SurfaceSubpoint> {
    let radius_km = vector_length(position_km)?;
    let lon_deg = position_km.x.atan2(position_km.z).to_degrees();
    let lat_deg = (position_km.y / radius_km)
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees();

    Some(SurfaceSubpoint {
        lon_lat: LonLat::new(wrap_longitude(lon_deg), lat_deg),
        altitude_km: Kilometers::new(radius_km - reference_radius_km.as_f64()),
    })
}

/// Converts a lon/lat/radius point into a body-fixed spherical position vector.
///
/// The axis convention is the inverse of [`spherical_surface_subpoint`]: lon
/// `0deg` points along `+Z`, lon `90deg` points along `+X`, and latitude
/// `90deg` points along `+Y`. The input is not projection-clamped so polar
/// surface points remain representable for non-Mercator bodies and globes.
#[must_use]
pub fn spherical_surface_position(lon_lat: LonLat, radius_km: Kilometers) -> Vec3d {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.clamp(-90.0, 90.0).to_radians();
    let radius = radius_km.as_f64();
    let cos_lat = lat_rad.cos();

    Vec3d::new(
        radius * cos_lat * lon_rad.sin(),
        radius * lat_rad.sin(),
        radius * cos_lat * lon_rad.cos(),
    )
}

fn vector_length(vector: Vec3d) -> Option<f64> {
    let length_squared = vector
        .x
        .mul_add(vector.x, vector.y.mul_add(vector.y, vector.z * vector.z));
    if !length_squared.is_finite() || length_squared <= f64::EPSILON {
        return None;
    }

    Some(length_squared.sqrt())
}

#[cfg(test)]
mod tests {
    use crate::coords::LonLat;

    use super::{spherical_surface_position, spherical_surface_subpoint, Kilometers, Vec3d};

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn spherical_subpoint_uses_ferrisium_globe_axes() {
        let radius = Kilometers::new(10.0);

        let prime_meridian = spherical_surface_subpoint(Vec3d::new(0.0, 0.0, 10.0), radius);
        assert!(prime_meridian.is_some());
        assert_close(
            prime_meridian.map_or(f64::NAN, |subpoint| subpoint.lon_lat.lon_deg),
            0.0,
        );
        assert_close(
            prime_meridian.map_or(f64::NAN, |subpoint| subpoint.lon_lat.lat_deg),
            0.0,
        );

        let east_quadrature = spherical_surface_subpoint(Vec3d::new(10.0, 0.0, 0.0), radius);
        assert!(east_quadrature.is_some());
        assert_close(
            east_quadrature.map_or(f64::NAN, |subpoint| subpoint.lon_lat.lon_deg),
            90.0,
        );

        let north_pole = spherical_surface_subpoint(Vec3d::new(0.0, 10.0, 0.0), radius);
        assert!(north_pole.is_some());
        assert_close(
            north_pole.map_or(f64::NAN, |subpoint| subpoint.lon_lat.lat_deg),
            90.0,
        );
    }

    #[test]
    fn spherical_subpoint_reports_altitude_without_modifying_position() {
        let subpoint =
            spherical_surface_subpoint(Vec3d::new(0.0, 0.0, 12.5), Kilometers::new(10.0));

        assert!(subpoint.is_some());
        assert_close(
            subpoint.map_or(f64::NAN, |subpoint| subpoint.altitude_km.as_f64()),
            2.5,
        );
    }

    #[test]
    fn spherical_surface_position_uses_inverse_globe_axes() {
        let radius = Kilometers::new(10.0);
        let prime_meridian = spherical_surface_position(LonLat::new(0.0, 0.0), radius);
        assert_close(prime_meridian.x, 0.0);
        assert_close(prime_meridian.y, 0.0);
        assert_close(prime_meridian.z, 10.0);

        let east_quadrature = spherical_surface_position(LonLat::new(90.0, 0.0), radius);
        assert_close(east_quadrature.x, 10.0);
        assert_close(east_quadrature.y, 0.0);
        assert!(east_quadrature.z.abs() <= 1.0e-9);

        let north_pole = spherical_surface_position(LonLat::new(42.0, 90.0), radius);
        assert!(north_pole.x.abs() <= 1.0e-9);
        assert_close(north_pole.y, 10.0);
        assert!(north_pole.z.abs() <= 1.0e-9);
    }

    #[test]
    fn spherical_subpoint_rejects_zero_or_invalid_vectors() {
        assert!(spherical_surface_subpoint(Vec3d::ZERO, Kilometers::new(10.0)).is_none());
        assert!(
            spherical_surface_subpoint(Vec3d::new(f64::NAN, 0.0, 1.0), Kilometers::new(10.0))
                .is_none()
        );
    }
}
