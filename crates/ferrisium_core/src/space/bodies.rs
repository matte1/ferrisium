//! Built-in spherical metadata for common solar-system bodies.
//!
//! These values are intentionally lightweight defaults used for rendering,
//! simple geometry, and frame selection. Higher-fidelity shape or orientation
//! data belongs in ephemeris/provider integrations.

use super::{BodyId, FrameId, Kilometers, RenderScale, Vec3d};
use crate::source::{
    NasaTrekRegularBody, EARTH_MEAN_RADIUS_METERS, MARS_MEAN_RADIUS_METERS,
    MERCURY_MEAN_RADIUS_METERS, MOON_MEAN_RADIUS_METERS, VENUS_MEAN_RADIUS_METERS,
};

/// Mean solar radius used for built-in Sun metadata, in kilometers.
pub const SUN_MEAN_RADIUS_KILOMETERS: f64 = 695_700.0;
/// WGS84 equatorial radius, in kilometers.
pub const WGS84_EQUATORIAL_RADIUS_KILOMETERS: f64 = 6_378.137;
/// WGS84 inverse flattening.
pub const WGS84_INVERSE_FLATTENING: f64 = 298.257_223_563;

/// Reference ellipsoid used for geodetic placement on a body.
///
/// Ferrisium's body-fixed render axes use `+Z` at longitude `0deg`, `+X` at
/// longitude `90deg`, and `+Y` at the north pole. This matches the existing
/// globe mesh convention while preserving the usual geodetic longitude and
/// latitude definitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceEllipsoid {
    /// Equatorial semi-major axis in kilometers.
    pub equatorial_radius_km: Kilometers,
    /// Flattening `f = (a - b) / a`.
    pub flattening: f64,
}

impl ReferenceEllipsoid {
    /// Creates a reference ellipsoid from an equatorial radius and flattening.
    #[must_use]
    pub const fn new(equatorial_radius_km: Kilometers, flattening: f64) -> Self {
        Self {
            equatorial_radius_km,
            flattening,
        }
    }

    /// Creates a spherical reference ellipsoid.
    #[must_use]
    pub const fn sphere(radius_km: Kilometers) -> Self {
        Self::new(radius_km, 0.0)
    }

    /// Returns the WGS84 reference ellipsoid used by built-in Earth metadata.
    #[must_use]
    pub fn wgs84() -> Self {
        Self::new(
            Kilometers::new(WGS84_EQUATORIAL_RADIUS_KILOMETERS),
            WGS84_INVERSE_FLATTENING.recip(),
        )
    }

    /// Returns the polar semi-minor axis in kilometers.
    #[must_use]
    pub fn polar_radius_km(self) -> Kilometers {
        Kilometers::new(self.equatorial_radius_km.as_f64() * (1.0 - self.flattening))
    }

    /// Returns the first eccentricity squared.
    #[must_use]
    pub fn eccentricity_squared(self) -> f64 {
        let flattening = self.normalized_flattening();
        flattening * (2.0 - flattening)
    }

    /// Converts geodetic coordinates to Ferrisium body-fixed kilometers.
    #[must_use]
    pub fn geodetic_position_km(self, lat_deg: f64, lon_deg: f64, alt_km: f64) -> Vec3d {
        let latitude = finite_or_zero(lat_deg).clamp(-90.0, 90.0).to_radians();
        let longitude = finite_or_zero(lon_deg).to_radians();
        let altitude = finite_or_zero(alt_km);
        let semi_major_axis = self.normalized_equatorial_radius();
        let eccentricity_squared = self.eccentricity_squared();
        let sin_latitude = latitude.sin();
        let cos_latitude = latitude.cos();
        let sin_latitude_squared = sin_latitude * sin_latitude;
        let radius_of_curvature =
            semi_major_axis / (1.0 - eccentricity_squared * sin_latitude_squared).sqrt();
        let equatorial_radius_at_altitude = (radius_of_curvature + altitude) * cos_latitude;
        let polar_radius_at_altitude =
            (radius_of_curvature * (1.0 - eccentricity_squared) + altitude) * sin_latitude;

        Vec3d::new(
            equatorial_radius_at_altitude * longitude.sin(),
            polar_radius_at_altitude,
            equatorial_radius_at_altitude * longitude.cos(),
        )
    }

    fn normalized_equatorial_radius(self) -> f64 {
        let radius = self.equatorial_radius_km.as_f64();
        if radius.is_finite() && radius > 0.0 {
            radius
        } else {
            1.0
        }
    }

    fn normalized_flattening(self) -> f64 {
        if self.flattening.is_finite() && (0.0..1.0).contains(&self.flattening) {
            self.flattening
        } else {
            0.0
        }
    }
}

/// Built-in metadata for a celestial body.
#[derive(Debug, Clone, PartialEq)]
pub struct CelestialBody {
    /// NAIF-compatible body identifier.
    pub id: BodyId,
    /// Human-readable body name.
    pub name: String,
    /// Mean spherical radius for rendering and simple geometry, in kilometers.
    pub mean_radius_km: Kilometers,
    /// Reference ellipsoid used for geodetic placement.
    pub reference_ellipsoid: ReferenceEllipsoid,
    /// Preferred inertial frame for ephemeris queries involving this body.
    pub inertial_frame: FrameId,
    /// Preferred body-fixed frame for surface rendering.
    pub body_fixed_frame: FrameId,
}

impl CelestialBody {
    /// Returns built-in Sun metadata.
    #[must_use]
    pub fn sun() -> Self {
        Self::spherical(
            BodyId::SUN,
            "Sun",
            Kilometers::new(SUN_MEAN_RADIUS_KILOMETERS),
            FrameId::j2000(),
            FrameId::named("IAU_SUN"),
        )
    }

    /// Returns built-in Earth metadata.
    #[must_use]
    pub fn earth() -> Self {
        Self::ellipsoidal(
            BodyId::EARTH,
            "Earth",
            Kilometers::from_meters(f64::from(EARTH_MEAN_RADIUS_METERS)),
            ReferenceEllipsoid::wgs84(),
            FrameId::j2000(),
            FrameId::itrf93(),
        )
    }

    /// Returns built-in Moon metadata.
    #[must_use]
    pub fn moon() -> Self {
        Self::spherical(
            BodyId::MOON,
            "Moon",
            Kilometers::from_meters(f64::from(MOON_MEAN_RADIUS_METERS)),
            FrameId::j2000(),
            FrameId::moon_me_de440(),
        )
    }

    /// Returns built-in Mercury metadata.
    #[must_use]
    pub fn mercury() -> Self {
        Self::spherical(
            BodyId::MERCURY,
            "Mercury",
            Kilometers::from_meters(f64::from(MERCURY_MEAN_RADIUS_METERS)),
            FrameId::j2000(),
            FrameId::iau_mercury(),
        )
    }

    /// Returns built-in Venus metadata.
    #[must_use]
    pub fn venus() -> Self {
        Self::spherical(
            BodyId::VENUS,
            "Venus",
            Kilometers::from_meters(f64::from(VENUS_MEAN_RADIUS_METERS)),
            FrameId::j2000(),
            FrameId::iau_venus(),
        )
    }

    /// Returns built-in Mars metadata.
    #[must_use]
    pub fn mars() -> Self {
        Self::spherical(
            BodyId::MARS,
            "Mars",
            Kilometers::from_meters(f64::from(MARS_MEAN_RADIUS_METERS)),
            FrameId::j2000(),
            FrameId::iau_mars(),
        )
    }

    /// Returns built-in Io metadata.
    #[must_use]
    pub fn io() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Io)
    }

    /// Returns built-in Europa metadata.
    #[must_use]
    pub fn europa() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Europa)
    }

    /// Returns built-in Ganymede metadata.
    #[must_use]
    pub fn ganymede() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Ganymede)
    }

    /// Returns built-in Callisto metadata.
    #[must_use]
    pub fn callisto() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Callisto)
    }

    /// Returns built-in Titan metadata.
    #[must_use]
    pub fn titan() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Titan)
    }

    /// Returns built-in Dione metadata.
    #[must_use]
    pub fn dione() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Dione)
    }

    /// Returns built-in Enceladus metadata.
    #[must_use]
    pub fn enceladus() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Enceladus)
    }

    /// Returns built-in Iapetus metadata.
    #[must_use]
    pub fn iapetus() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Iapetus)
    }

    /// Returns built-in Mimas metadata.
    #[must_use]
    pub fn mimas() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Mimas)
    }

    /// Returns built-in Rhea metadata.
    #[must_use]
    pub fn rhea() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Rhea)
    }

    /// Returns built-in Tethys metadata.
    #[must_use]
    pub fn tethys() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Tethys)
    }

    /// Returns built-in Ceres metadata.
    #[must_use]
    pub fn ceres() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Ceres)
    }

    /// Returns built-in metadata for a regular body exposed by NASA Trek.
    #[must_use]
    pub fn nasa_trek_regular_body(body: NasaTrekRegularBody) -> Self {
        match body {
            NasaTrekRegularBody::Mercury => Self::mercury(),
            NasaTrekRegularBody::Venus => Self::venus(),
            NasaTrekRegularBody::Moon => Self::moon(),
            NasaTrekRegularBody::Mars => Self::mars(),
            body @ (NasaTrekRegularBody::Io
            | NasaTrekRegularBody::Europa
            | NasaTrekRegularBody::Ganymede
            | NasaTrekRegularBody::Callisto
            | NasaTrekRegularBody::Titan
            | NasaTrekRegularBody::Dione
            | NasaTrekRegularBody::Enceladus
            | NasaTrekRegularBody::Iapetus
            | NasaTrekRegularBody::Mimas
            | NasaTrekRegularBody::Rhea
            | NasaTrekRegularBody::Tethys
            | NasaTrekRegularBody::Ceres) => {
                let descriptor = body.descriptor();
                Self::spherical(
                    BodyId::naif(descriptor.naif_id),
                    descriptor.display_name,
                    Kilometers::from_meters(f64::from(descriptor.mean_radius_meters)),
                    FrameId::j2000(),
                    FrameId::named(descriptor.iau_frame_name),
                )
            }
        }
    }

    /// Creates spherical body metadata.
    #[must_use]
    pub fn spherical(
        id: BodyId,
        name: impl Into<String>,
        mean_radius_km: Kilometers,
        inertial_frame: FrameId,
        body_fixed_frame: FrameId,
    ) -> Self {
        Self::ellipsoidal(
            id,
            name,
            mean_radius_km,
            ReferenceEllipsoid::sphere(mean_radius_km),
            inertial_frame,
            body_fixed_frame,
        )
    }

    /// Creates body metadata with an explicit reference ellipsoid.
    #[must_use]
    pub fn ellipsoidal(
        id: BodyId,
        name: impl Into<String>,
        mean_radius_km: Kilometers,
        reference_ellipsoid: ReferenceEllipsoid,
        inertial_frame: FrameId,
        body_fixed_frame: FrameId,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            mean_radius_km,
            reference_ellipsoid,
            inertial_frame,
            body_fixed_frame,
        }
    }

    /// Returns the body's radius in Bevy render units for the provided scale.
    #[must_use]
    pub fn mean_radius_units(&self, render_scale: RenderScale) -> f64 {
        render_scale.distance_to_units(self.mean_radius_km)
    }

    /// Converts geodetic coordinates to Ferrisium body-fixed kilometers.
    #[must_use]
    pub fn geodetic_position_km(&self, lat_deg: f64, lon_deg: f64, alt_km: f64) -> Vec3d {
        self.reference_ellipsoid
            .geodetic_position_km(lat_deg, lon_deg, alt_km)
    }
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
