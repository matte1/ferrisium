//! Component-level placement for entities near a rendered globe.
//!
//! [`GlobePosition`] is the app-facing placement API for objects with known
//! physical coordinates around a celestial body. The sync system keeps
//! `big_space` cell coordinates and Bevy transforms in step so consumers can
//! attach ordinary rendering components without handling root-grid placement.

use bevy::ecs::change_detection::Ref;
use bevy::math::{DQuat, DVec3};
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::space::{BodyId, CelestialBody, Epoch, Vec3d};

use crate::celestial::{CelestialBodyPlacement, CelestialRoot, CelestialSpace};

/// Declares the physical position of an entity around a celestial body.
///
/// Ferrisium synchronizes this component into `CellCoord` and `Transform`
/// components under the active [`CelestialRoot`]. The component only describes
/// position; applications attach their own meshes, scenes, sprites, labels, or
/// logic components to the same entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GlobePosition {
    body: CelestialBody,
    coordinates: GlobePositionCoordinates,
}

impl GlobePosition {
    /// Creates a position from geodetic coordinates on the body's reference ellipsoid.
    ///
    /// Latitude and longitude are expressed in degrees; altitude is expressed
    /// in kilometers above the ellipsoid. Earth uses WGS84. Other built-in
    /// bodies currently use spherical reference ellipsoids.
    #[must_use]
    pub fn geodetic(body: &CelestialBody, lat_deg: f64, lon_deg: f64, alt_km: f64) -> Self {
        Self {
            body: body.clone(),
            coordinates: GlobePositionCoordinates::Geodetic {
                lat_deg,
                lon_deg,
                alt_km,
            },
        }
    }

    /// Creates a position from Ferrisium body-fixed Cartesian kilometers.
    ///
    /// Body-fixed axes match the globe renderer: `+Z` is longitude `0deg`,
    /// `+X` is longitude `90deg`, and `+Y` is the north pole.
    #[must_use]
    pub fn body_fixed(body: &CelestialBody, x_km: f64, y_km: f64, z_km: f64) -> Self {
        Self {
            body: body.clone(),
            coordinates: GlobePositionCoordinates::BodyFixed {
                position_km: Vec3d::new(x_km, y_km, z_km),
            },
        }
    }

    /// Creates a position from inertial Cartesian kilometers relative to the body center.
    ///
    /// The position is interpreted in the body's preferred inertial frame.
    #[must_use]
    pub fn inertial(body: &CelestialBody, x_km: f64, y_km: f64, z_km: f64, epoch: Epoch) -> Self {
        Self {
            body: body.clone(),
            coordinates: GlobePositionCoordinates::Inertial {
                position_km: Vec3d::new(x_km, y_km, z_km),
                epoch,
            },
        }
    }

    /// Returns the body metadata associated with this position.
    #[must_use]
    pub fn body(&self) -> &CelestialBody {
        &self.body
    }

    /// Returns the raw coordinate representation.
    #[must_use]
    pub const fn coordinates(&self) -> GlobePositionCoordinates {
        self.coordinates
    }

    /// Updates this component to new geodetic coordinates on the same body.
    pub fn set_geodetic(&mut self, lat_deg: f64, lon_deg: f64, alt_km: f64) {
        self.coordinates = GlobePositionCoordinates::Geodetic {
            lat_deg,
            lon_deg,
            alt_km,
        };
    }

    /// Updates this component to a new body-fixed Cartesian position on the same body.
    pub fn set_body_fixed(&mut self, x_km: f64, y_km: f64, z_km: f64) {
        self.coordinates = GlobePositionCoordinates::BodyFixed {
            position_km: Vec3d::new(x_km, y_km, z_km),
        };
    }

    /// Updates this component to a new inertial Cartesian position on the same body.
    pub fn set_inertial(&mut self, x_km: f64, y_km: f64, z_km: f64, epoch: Epoch) {
        self.coordinates = GlobePositionCoordinates::Inertial {
            position_km: Vec3d::new(x_km, y_km, z_km),
            epoch,
        };
    }

    fn body_fixed_position_km(&self) -> Option<Vec3d> {
        match self.coordinates {
            GlobePositionCoordinates::Geodetic {
                lat_deg,
                lon_deg,
                alt_km,
            } => Some(self.body.geodetic_position_km(lat_deg, lon_deg, alt_km)),
            GlobePositionCoordinates::BodyFixed { position_km } => Some(position_km),
            GlobePositionCoordinates::Inertial { .. } => None,
        }
    }

    fn inertial_position_km(&self) -> Option<Vec3d> {
        match self.coordinates {
            GlobePositionCoordinates::Inertial { position_km, .. } => Some(position_km),
            GlobePositionCoordinates::Geodetic { .. }
            | GlobePositionCoordinates::BodyFixed { .. } => None,
        }
    }
}

/// Raw coordinate representation carried by [`GlobePosition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlobePositionCoordinates {
    /// Geodetic coordinates in degrees plus altitude in kilometers.
    Geodetic {
        /// Latitude in degrees.
        lat_deg: f64,
        /// Longitude in degrees.
        lon_deg: f64,
        /// Altitude above the reference ellipsoid in kilometers.
        alt_km: f64,
    },
    /// Body-fixed Cartesian position in kilometers.
    BodyFixed {
        /// Position in Ferrisium body-fixed axes.
        position_km: Vec3d,
    },
    /// Inertial Cartesian position in kilometers relative to the body center.
    Inertial {
        /// Position in the body's preferred inertial frame.
        position_km: Vec3d,
        /// Epoch associated with the inertial state.
        epoch: Epoch,
    },
}

#[allow(
    clippy::type_complexity,
    reason = "The placement sync has to update optional BigSpace and Bevy transform components."
)]
pub(crate) fn sync_globe_positions(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    roots: Query<'_, '_, Entity, With<CelestialRoot>>,
    bodies: Query<
        '_,
        '_,
        (&CelestialBodyPlacement, Option<&CellCoord>, &Transform),
        Without<GlobePosition>,
    >,
    mut positions: Query<
        '_,
        '_,
        (
            Entity,
            Ref<'_, GlobePosition>,
            Option<&mut CellCoord>,
            Option<&mut Transform>,
            Option<&GlobalTransform>,
            Option<&ChildOf>,
        ),
    >,
) {
    let Some(root) = roots.iter().next() else {
        return;
    };

    for (entity, position, cell, transform, global_transform, parent) in &mut positions {
        let Some(scene_position_units) =
            globe_position_scene_position_units(*celestial_space, &position, &bodies)
        else {
            continue;
        };
        let placement = celestial_space.place_position_units(scene_position_units);

        sync_globe_position_components(
            &mut commands,
            entity,
            root,
            placement.cell,
            placement.translation,
            cell,
            transform,
            global_transform,
            parent,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Component insertion must account for each optional ECS component independently."
)]
fn sync_globe_position_components(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    root: Entity,
    next_cell: CellCoord,
    next_translation: Vec3,
    cell: Option<Mut<'_, CellCoord>>,
    transform: Option<Mut<'_, Transform>>,
    global_transform: Option<&GlobalTransform>,
    parent: Option<&ChildOf>,
) {
    if let Some(mut cell) = cell {
        *cell = next_cell;
    } else {
        commands.entity(entity).try_insert(next_cell);
    }

    if let Some(mut transform) = transform {
        transform.translation = next_translation;
    } else {
        commands
            .entity(entity)
            .try_insert(Transform::from_translation(next_translation));
    }

    if global_transform.is_none() {
        commands
            .entity(entity)
            .try_insert(GlobalTransform::default());
    }

    if parent.map(ChildOf::parent) != Some(root) {
        commands.entity(entity).try_insert(ChildOf(root));
    }
}

pub(crate) fn globe_position_scene_position_units<F>(
    celestial_space: CelestialSpace,
    position: &GlobePosition,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
) -> Option<DVec3>
where
    F: bevy::ecs::query::QueryFilter,
{
    let body_frame = globe_position_body_frame(celestial_space, position.body(), bodies);
    let body_frame = body_frame.or_else(|| default_earth_body_frame(position.body()))?;
    let body_center_units = body_frame.center_units;
    if let Some(inertial_position_km) = position.inertial_position_km() {
        return Some(
            body_center_units + position_km_to_units(celestial_space, inertial_position_km),
        );
    }

    let body_fixed_position_km = position.body_fixed_position_km()?;
    let body_fixed_position_units = position_km_to_units(celestial_space, body_fixed_position_km);

    Some(body_center_units + body_frame.body_rotation.mul_vec3(body_fixed_position_units))
}

fn globe_position_body_frame<F>(
    celestial_space: CelestialSpace,
    body: &CelestialBody,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
) -> Option<GlobePositionBodyFrame>
where
    F: bevy::ecs::query::QueryFilter,
{
    bodies.iter().find_map(|(placement, cell, transform)| {
        (placement.body.id == body.id).then(|| GlobePositionBodyFrame {
            center_units: body_center_units(celestial_space, cell, transform),
            body_rotation: dquat_from_bevy(transform.rotation),
        })
    })
}

fn default_earth_body_frame(body: &CelestialBody) -> Option<GlobePositionBodyFrame> {
    (body.id == BodyId::EARTH).then_some(GlobePositionBodyFrame {
        center_units: DVec3::ZERO,
        body_rotation: DQuat::IDENTITY,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GlobePositionBodyFrame {
    center_units: DVec3,
    body_rotation: DQuat,
}

fn body_center_units(
    celestial_space: CelestialSpace,
    cell: Option<&CellCoord>,
    transform: &Transform,
) -> DVec3 {
    cell.map_or_else(
        || transform.translation.as_dvec3(),
        |cell| celestial_space.grid().grid_position_double(cell, transform),
    )
}

fn position_km_to_units(celestial_space: CelestialSpace, position_km: Vec3d) -> DVec3 {
    let position_units = celestial_space.render_scale.position_to_units(position_km);

    DVec3::new(position_units.x, position_units.y, position_units.z)
}

fn dquat_from_bevy(rotation: Quat) -> DQuat {
    if !quat_is_finite(rotation) || rotation.length_squared() <= f32::EPSILON {
        return DQuat::IDENTITY;
    }

    DQuat::from_xyzw(
        f64::from(rotation.x),
        f64::from(rotation.y),
        f64::from(rotation.z),
        f64::from(rotation.w),
    )
    .normalize()
}

fn quat_is_finite(rotation: Quat) -> bool {
    rotation.x.is_finite()
        && rotation.y.is_finite()
        && rotation.z.is_finite()
        && rotation.w.is_finite()
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{App, ChildOf, Quat, Transform, Update};
    use big_space::prelude::CellCoord;
    use ferrisium_core::space::{BodyId, CelestialBody, RenderScale, Vec3d};

    use crate::celestial::{
        CelestialBodyPlacement, CelestialRoot, CelestialSpace, DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
    };
    use crate::globe_position::{sync_globe_positions, GlobePosition, GlobePositionCoordinates};

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-6,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn globe_position_exposes_and_updates_coordinate_kind() {
        let earth = CelestialBody::earth();
        let mut position = GlobePosition::geodetic(&earth, 10.0, 20.0, 30.0);

        assert_eq!(position.body().id, BodyId::EARTH);
        assert!(matches!(
            position.coordinates(),
            GlobePositionCoordinates::Geodetic {
                lat_deg: 10.0,
                lon_deg: 20.0,
                alt_km: 30.0
            }
        ));

        position.set_body_fixed(1.0, 2.0, 3.0);
        assert_eq!(
            position.coordinates(),
            GlobePositionCoordinates::BodyFixed {
                position_km: Vec3d::new(1.0, 2.0, 3.0)
            }
        );
    }

    #[test]
    fn globe_position_sync_inserts_big_space_components_and_parent() {
        let mut app = App::new();
        app.init_resource::<CelestialSpace>()
            .add_systems(Update, sync_globe_positions);
        let root = app.world_mut().spawn(CelestialRoot).id();
        let entity = app
            .world_mut()
            .spawn(GlobePosition::body_fixed(
                &CelestialBody::earth(),
                DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM + 42.0,
                0.0,
                0.0,
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<CellCoord>(entity).is_some());
        assert!(world.get::<Transform>(entity).is_some());
        assert!(world.get::<ChildOf>(entity).is_some());
        let Some(cell) = world.get::<CellCoord>(entity) else {
            return;
        };
        let Some(transform) = world.get::<Transform>(entity) else {
            return;
        };
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return;
        };

        assert_eq!(cell.x, 1);
        assert_close(f64::from(transform.translation.x), 42.0);
        assert_eq!(parent.parent(), root);
    }

    #[test]
    fn globe_position_body_fixed_coordinates_follow_body_anchor_rotation() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(10_000.0, RenderScale::PHYSICAL))
            .add_systems(Update, sync_globe_positions);
        let root = app.world_mut().spawn(CelestialRoot).id();
        let moon = CelestialBody::moon();
        app.world_mut().spawn((
            CelestialBodyPlacement::for_body_relative_to(moon.clone(), BodyId::EARTH),
            CellCoord::default(),
            Transform {
                translation: bevy::prelude::Vec3::new(100.0, 0.0, 0.0),
                rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                ..Transform::default()
            },
        ));
        let entity = app
            .world_mut()
            .spawn(GlobePosition::body_fixed(&moon, 0.0, 0.0, 10.0))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<Transform>(entity).is_some());
        assert!(world.get::<ChildOf>(entity).is_some());
        let Some(transform) = world.get::<Transform>(entity) else {
            return;
        };
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return;
        };

        assert_close(f64::from(transform.translation.x), 110.0);
        assert_close(f64::from(transform.translation.y), 0.0);
        assert!(transform.translation.z.abs() <= 1.0e-5);
        assert_eq!(parent.parent(), root);
    }

    #[test]
    fn globe_position_waits_for_non_earth_body_anchor() {
        let mut app = App::new();
        app.init_resource::<CelestialSpace>()
            .add_systems(Update, sync_globe_positions);
        app.world_mut().spawn(CelestialRoot);
        let entity = app
            .world_mut()
            .spawn(GlobePosition::body_fixed(
                &CelestialBody::moon(),
                1.0,
                2.0,
                3.0,
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<CellCoord>(entity).is_none());
        assert!(world.get::<Transform>(entity).is_none());
    }

    #[test]
    fn globe_position_inertial_coordinates_do_not_follow_body_rotation() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(10_000.0, RenderScale::PHYSICAL))
            .add_systems(Update, sync_globe_positions);
        app.world_mut().spawn(CelestialRoot);
        let moon = CelestialBody::moon();
        app.world_mut().spawn((
            CelestialBodyPlacement::for_body_relative_to(moon.clone(), BodyId::EARTH),
            CellCoord::default(),
            Transform {
                translation: bevy::prelude::Vec3::new(100.0, 0.0, 0.0),
                rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                ..Transform::default()
            },
        ));
        let entity = app
            .world_mut()
            .spawn(GlobePosition::inertial(
                &moon,
                0.0,
                0.0,
                10.0,
                ferrisium_core::space::Epoch::J2000,
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<Transform>(entity).is_some());
        let Some(transform) = world.get::<Transform>(entity) else {
            return;
        };

        assert_close(f64::from(transform.translation.x), 100.0);
        assert_close(f64::from(transform.translation.y), 0.0);
        assert_close(f64::from(transform.translation.z), 10.0);
    }
}
