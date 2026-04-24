//! Shared globe-space anchor vocabulary for drawing primitives.
//!
//! `GlobeAnchor` is the common physical point representation behind
//! [`crate::GlobeLink`] endpoints and [`crate::GlobeLabel`] positions. Keeping
//! anchor resolution in one place prevents anchored globe-space primitives from
//! drifting on body-frame and `big_space` placement behavior.

use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::space::{CelestialBody, Vec3d};

use crate::celestial::{CelestialBodyPlacement, CelestialSpace};
use crate::globe_position::{globe_position_scene_position_units, GlobePosition};

/// A physical anchor around one globe body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlobeAnchor {
    /// Fixed geodetic position on the owning body.
    Geodetic {
        /// Latitude in degrees.
        lat_deg: f64,
        /// Longitude in degrees.
        lon_deg: f64,
        /// Altitude above the reference ellipsoid in kilometers.
        alt_km: f64,
    },
    /// Fixed Ferrisium body-fixed Cartesian position in kilometers.
    BodyFixed {
        /// Position in body-fixed kilometers.
        position_km: Vec3d,
    },
    /// Track another entity's [`GlobePosition`].
    Entity(Entity),
}

impl GlobeAnchor {
    /// Creates a fixed geodetic anchor.
    #[must_use]
    pub const fn geodetic(lat_deg: f64, lon_deg: f64, alt_km: f64) -> Self {
        Self::Geodetic {
            lat_deg,
            lon_deg,
            alt_km,
        }
    }

    /// Creates a fixed Ferrisium body-fixed anchor.
    #[must_use]
    pub const fn body_fixed(x_km: f64, y_km: f64, z_km: f64) -> Self {
        Self::BodyFixed {
            position_km: Vec3d::new(x_km, y_km, z_km),
        }
    }

    /// Creates an anchor that follows another entity's [`GlobePosition`].
    #[must_use]
    pub const fn entity(entity: Entity) -> Self {
        Self::Entity(entity)
    }
}

pub(crate) fn resolve_globe_anchor_position<F>(
    celestial_space: CelestialSpace,
    body: &CelestialBody,
    anchor: GlobeAnchor,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
    anchor_positions: &Query<'_, '_, &GlobePosition>,
) -> Option<DVec3>
where
    F: bevy::ecs::query::QueryFilter,
{
    let position = match anchor {
        GlobeAnchor::Geodetic {
            lat_deg,
            lon_deg,
            alt_km,
        } => GlobePosition::geodetic(body, lat_deg, lon_deg, alt_km),
        GlobeAnchor::BodyFixed { position_km } => {
            GlobePosition::body_fixed(body, position_km.x, position_km.y, position_km.z)
        }
        GlobeAnchor::Entity(entity) => {
            let position = anchor_positions.get(entity).ok()?;
            if position.body().id != body.id {
                return None;
            }
            return globe_position_scene_position_units(celestial_space, position, bodies)
                .filter(|position| dvec3_is_finite(*position));
        }
    };

    globe_position_scene_position_units(celestial_space, &position, bodies)
        .filter(|position| dvec3_is_finite(*position))
}

pub(crate) fn dvec3_is_finite(position: DVec3) -> bool {
    position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
}

#[cfg(test)]
mod tests {
    use ferrisium_core::space::Vec3d;

    use super::GlobeAnchor;

    #[test]
    fn globe_anchor_constructors_preserve_coordinate_kind() {
        assert_eq!(
            GlobeAnchor::geodetic(1.0, 2.0, 3.0),
            GlobeAnchor::Geodetic {
                lat_deg: 1.0,
                lon_deg: 2.0,
                alt_km: 3.0,
            }
        );
        assert_eq!(
            GlobeAnchor::body_fixed(4.0, 5.0, 6.0),
            GlobeAnchor::BodyFixed {
                position_km: Vec3d::new(4.0, 5.0, 6.0),
            }
        );
    }
}
