//! Bevy-side celestial placement helpers backed by `big_space`.
//!
//! `ferrisium_core` owns the authoritative `f64` kilometer state. This module
//! converts those values into `big_space` grid cells plus local Bevy
//! `Transform` translations so large scenes can render without putting
//! solar-system-scale coordinates directly into `f32`.

use bevy::math::{DVec3, Vec3};
use bevy::prelude::{Component, Entity, Mut, Quat, Query, Res, Resource, Transform};
use big_space::prelude::{CellCoord, Grid};
use ferrisium_core::space::{
    BodyId, CelestialBody, EphemerisProvider, Epoch, FrameId, FrameOrientation, OrientationRequest,
    QuatD, RenderScale, StateRequest, StateVector, Vec3d,
};
use std::fmt;
use std::sync::Arc;

/// Default root grid cell edge length for celestial render placement, in kilometers.
pub const DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM: f64 = 10_000.0;

/// Shared ephemeris provider used by Bevy-side celestial placement systems.
pub type SharedEphemerisProvider = Arc<dyn EphemerisProvider + Send + Sync + 'static>;

/// Active celestial epoch used for provider-driven body placement.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CelestialEpoch {
    /// Current ephemeris query epoch.
    pub epoch: Epoch,
}

impl CelestialEpoch {
    /// Creates an active celestial epoch.
    #[must_use]
    pub const fn new(epoch: Epoch) -> Self {
        Self { epoch }
    }
}

/// Body currently used as the camera/orbit focus in celestial scenes.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CelestialFocus {
    /// Target body or barycenter to focus.
    pub target: BodyId,
}

impl CelestialFocus {
    /// Creates a focus resource for `target`.
    #[must_use]
    pub const fn target(target: BodyId) -> Self {
        Self { target }
    }

    /// Focuses Earth.
    #[must_use]
    pub const fn earth() -> Self {
        Self::target(BodyId::EARTH)
    }

    /// Focuses the Moon.
    #[must_use]
    pub const fn moon() -> Self {
        Self::target(BodyId::MOON)
    }

    /// Focuses the Mars system barycenter.
    #[must_use]
    pub const fn mars_system() -> Self {
        Self::target(BodyId::MARS_BARYCENTER)
    }
}

impl Default for CelestialFocus {
    fn default() -> Self {
        Self::earth()
    }
}

/// Optional active ephemeris provider for celestial body placement.
#[derive(Resource, Clone, Default)]
pub struct CelestialEphemeris {
    provider: Option<SharedEphemerisProvider>,
}

impl CelestialEphemeris {
    /// Creates an empty ephemeris resource.
    #[must_use]
    pub const fn empty() -> Self {
        Self { provider: None }
    }

    /// Creates an ephemeris resource from a concrete provider.
    #[must_use]
    pub fn from_provider(provider: impl EphemerisProvider + Send + Sync + 'static) -> Self {
        Self {
            provider: Some(Arc::new(provider)),
        }
    }

    /// Creates an ephemeris resource from a shared provider object.
    #[must_use]
    pub fn from_shared(provider: SharedEphemerisProvider) -> Self {
        Self {
            provider: Some(provider),
        }
    }

    /// Returns the active provider, when one is configured.
    #[must_use]
    pub fn provider(&self) -> Option<&(dyn EphemerisProvider + Send + Sync + 'static)> {
        self.provider.as_deref()
    }
}

impl fmt::Debug for CelestialEphemeris {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CelestialEphemeris")
            .field("has_provider", &self.provider.is_some())
            .finish()
    }
}

/// Shared configuration for converting celestial state vectors into `big_space` placement.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CelestialSpace {
    /// Physical size represented by one `big_space` root-grid cell.
    pub grid_cell_size_km: f64,
    /// Physical-to-render conversion used before placing values in the grid.
    pub render_scale: RenderScale,
}

impl CelestialSpace {
    /// Creates a celestial placement configuration.
    #[must_use]
    pub const fn new(grid_cell_size_km: f64, render_scale: RenderScale) -> Self {
        Self {
            grid_cell_size_km,
            render_scale,
        }
    }

    /// Returns the grid cell size in render units after normalizing invalid configuration.
    #[must_use]
    pub fn grid_cell_size_units(self) -> f64 {
        let render_scale = self.render_scale.normalized();
        let grid_cell_size_km =
            if self.grid_cell_size_km.is_finite() && self.grid_cell_size_km > 0.0 {
                self.grid_cell_size_km
            } else {
                DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM
            };
        let grid_cell_size_units = grid_cell_size_km / render_scale.kilometers_per_unit;

        if grid_cell_size_units.is_finite()
            && grid_cell_size_units > 0.0
            && grid_cell_size_units <= f64::from(f32::MAX)
        {
            grid_cell_size_units
        } else {
            DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM / RenderScale::PHYSICAL.kilometers_per_unit
        }
    }

    /// Builds the `big_space` grid that corresponds to this placement configuration.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "big_space root grids are Bevy-facing f32 render-unit values."
    )]
    pub fn grid(self) -> Grid {
        Grid::new(self.grid_cell_size_units() as f32, 0.0)
    }

    /// Converts a state vector's position into `big_space` placement.
    #[must_use]
    pub fn placement_for_state(self, state: &StateVector) -> CelestialPlacement {
        self.place_position_km(state.position_km)
    }

    /// Converts a kilometer position into `big_space` placement.
    #[must_use]
    pub fn place_position_km(self, position_km: Vec3d) -> CelestialPlacement {
        let position_units = self.render_scale.position_to_units(position_km);
        self.place_position_units(DVec3::new(
            position_units.x,
            position_units.y,
            position_units.z,
        ))
    }

    /// Converts a render-unit position into `big_space` placement.
    #[must_use]
    pub fn place_position_units(self, position_units: DVec3) -> CelestialPlacement {
        let grid = self.grid();
        let (cell, translation) = grid.translation_to_grid(position_units);

        CelestialPlacement { cell, translation }
    }
}

impl Default for CelestialSpace {
    fn default() -> Self {
        Self::new(DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM, RenderScale::PHYSICAL)
    }
}

/// Marker on the root `BigSpace` entity that anchors Ferrisium's celestial scene.
#[derive(Component, Debug, Default)]
pub struct CelestialRoot;

/// Marker on the primary body that the current globe tile mesh is parented under.
#[derive(Component, Debug, Default)]
pub struct PrimaryCelestialBody;

/// Provider-driven placement configuration for a rendered celestial body.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CelestialBodyPlacement {
    /// Rendered body metadata.
    pub body: CelestialBody,
    /// Body or barycenter that the rendered body is positioned relative to.
    pub origin: BodyId,
    /// Inertial frame used for translational state vectors.
    pub state_frame: FrameId,
    /// Body-fixed frame used to rotate the rendered surface.
    pub body_fixed_frame: FrameId,
    /// Whether provider frame orientation should drive `Transform::rotation`.
    pub apply_orientation: bool,
}

impl CelestialBodyPlacement {
    /// Creates placement for `body` relative to `origin`.
    #[must_use]
    pub fn for_body_relative_to(body: CelestialBody, origin: BodyId) -> Self {
        Self {
            state_frame: body.inertial_frame.clone(),
            body_fixed_frame: body.body_fixed_frame.clone(),
            body,
            origin,
            apply_orientation: true,
        }
    }

    /// Disables frame-orientation updates for this body.
    #[must_use]
    pub const fn without_orientation(mut self) -> Self {
        self.apply_orientation = false;
        self
    }

    fn state_request(&self, epoch: Epoch) -> StateRequest {
        StateRequest::new(self.body.id, self.origin, self.state_frame.clone(), epoch)
    }

    fn orientation_request(&self, epoch: Epoch) -> OrientationRequest {
        OrientationRequest::new(
            self.body_fixed_frame.clone(),
            self.state_frame.clone(),
            epoch,
        )
    }
}

/// `big_space` placement for an object in Ferrisium's celestial render space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CelestialPlacement {
    /// Integer root-grid cell containing the object.
    pub cell: CellCoord,
    /// Local render-unit translation relative to the center of `cell`.
    pub translation: Vec3,
}

/// Entities that anchor Ferrisium's current high-precision celestial scene.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub(crate) struct CelestialSpaceEntities {
    /// Root `BigSpace` entity for the active celestial scene.
    pub root: Option<Entity>,
    /// Primary body entity that current globe meshes are parented under.
    pub primary_body: Option<Entity>,
}

impl CelestialPlacement {
    /// Reconstructs the double-precision render-unit position represented by this placement.
    #[must_use]
    pub fn absolute_position_units(self, celestial_space: CelestialSpace) -> DVec3 {
        celestial_space
            .grid()
            .grid_position_double(&self.cell, &Transform::from_translation(self.translation))
    }
}

/// Synchronizes body transforms from the active ephemeris provider.
#[allow(
    clippy::type_complexity,
    reason = "The body query updates both BigSpace and plain Bevy placement components."
)]
pub(crate) fn sync_celestial_body_placements(
    celestial_space: Res<'_, CelestialSpace>,
    celestial_epoch: Res<'_, CelestialEpoch>,
    celestial_ephemeris: Res<'_, CelestialEphemeris>,
    mut bodies: Query<
        '_,
        '_,
        (
            &CelestialBodyPlacement,
            Option<&mut CellCoord>,
            &mut Transform,
        ),
    >,
) {
    let provider = celestial_ephemeris.provider();
    for (body, cell, mut transform) in &mut bodies {
        if let Some(state) = body_state(body, celestial_epoch.epoch, provider) {
            apply_body_state(*celestial_space, &state, cell, &mut transform);
        }

        if body.apply_orientation {
            if let Some(orientation) = body_orientation(body, celestial_epoch.epoch, provider) {
                transform.rotation = quatd_to_bevy(orientation.rotation);
            }
        }
    }
}

fn body_state(
    body: &CelestialBodyPlacement,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<StateVector> {
    if body.body.id == body.origin {
        return Some(StateVector::relative_to(
            body.body.id,
            body.origin,
            body.state_frame.clone(),
            epoch,
            Vec3d::ZERO,
            Vec3d::ZERO,
        ));
    }

    provider?.state(&body.state_request(epoch)).ok()
}

fn body_orientation(
    body: &CelestialBodyPlacement,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<FrameOrientation> {
    provider?.orientation(&body.orientation_request(epoch)).ok()
}

fn apply_body_state(
    celestial_space: CelestialSpace,
    state: &StateVector,
    cell: Option<Mut<'_, CellCoord>>,
    transform: &mut Transform,
) {
    if let Some(mut cell) = cell {
        let placement = celestial_space.placement_for_state(state);
        *cell = placement.cell;
        transform.translation = placement.translation;
    } else {
        transform.translation = position_km_to_bevy_translation(celestial_space, state.position_km);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Renderer-facing fallback transforms are f32 while core positions remain f64."
)]
fn position_km_to_bevy_translation(celestial_space: CelestialSpace, position_km: Vec3d) -> Vec3 {
    let position_units = celestial_space.render_scale.position_to_units(position_km);

    Vec3::new(
        position_units.x as f32,
        position_units.y as f32,
        position_units.z as f32,
    )
}

/// Converts a core double-precision quaternion into Bevy's renderer quaternion.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy renderer rotations are f32 while core frame orientations are f64."
)]
pub fn quatd_to_bevy(rotation: QuatD) -> Quat {
    let rotation = rotation.normalized();
    Quat::from_xyzw(
        rotation.x as f32,
        rotation.y as f32,
        rotation.z as f32,
        rotation.w as f32,
    )
    .normalize()
}

#[cfg(test)]
mod tests {
    use super::{
        quatd_to_bevy, sync_celestial_body_placements, CelestialBodyPlacement, CelestialEphemeris,
        CelestialEpoch, CelestialSpace, DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
    };
    use bevy::prelude::{App, Quat, Transform, Update};
    use big_space::prelude::CellCoord;
    use ferrisium_core::space::{
        BodyId, CelestialBody, EphemerisError, EphemerisProvider, Epoch, FrameId, FrameOrientation,
        OrientationRequest, QuatD, RenderScale, StateRequest, StateVector, Vec3d,
    };

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn default_grid_uses_physical_kilometer_scale() {
        let celestial_space = CelestialSpace::default();

        assert_close(
            celestial_space.grid_cell_size_units(),
            DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
        );
        assert_close(celestial_space.grid().cell_edge_length().into(), 10_000.0);
    }

    #[test]
    fn placement_keeps_large_positions_out_of_local_transform_space() {
        let celestial_space = CelestialSpace::default();
        let placement = celestial_space.place_position_km(Vec3d::new(384_400.0, 0.0, 0.0));

        assert_eq!(placement.cell.x, 38);
        assert_eq!(placement.cell.y, 0);
        assert_eq!(placement.cell.z, 0);
        assert!(placement.translation.x.abs() <= 5_000.0);
        assert_close(
            placement.absolute_position_units(celestial_space).x,
            384_400.0,
        );
    }

    #[test]
    fn compressed_render_scale_changes_grid_units_not_physical_state() {
        let celestial_space =
            CelestialSpace::new(10_000.0, RenderScale::from_kilometers_per_unit(100.0));
        let placement = celestial_space.place_position_km(Vec3d::new(384_400.0, 0.0, 0.0));

        assert_close(celestial_space.grid_cell_size_units(), 100.0);
        assert_eq!(placement.cell.x, 38);
        assert_close(
            placement.absolute_position_units(celestial_space).x,
            3_844.0,
        );
    }

    #[test]
    fn placement_accepts_state_vectors() {
        let celestial_space = CelestialSpace::default();
        let state = StateVector::relative_to(
            BodyId::MOON,
            BodyId::EARTH,
            FrameId::j2000(),
            Epoch::J2000,
            Vec3d::new(384_400.0, 0.0, 0.0),
            Vec3d::ZERO,
        );

        let placement = celestial_space.placement_for_state(&state);

        assert_eq!(placement.cell.x, 38);
        assert_close(
            placement.absolute_position_units(celestial_space).x,
            384_400.0,
        );
    }

    #[test]
    fn invalid_grid_configuration_falls_back_to_default() {
        let zero = CelestialSpace::new(0.0, RenderScale::PHYSICAL);
        let infinite = CelestialSpace::new(f64::INFINITY, RenderScale::PHYSICAL);
        let invalid_scale =
            CelestialSpace::new(10_000.0, RenderScale::from_kilometers_per_unit(0.0));

        assert_close(
            zero.grid_cell_size_units(),
            DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
        );
        assert_close(
            infinite.grid_cell_size_units(),
            DEFAULT_CELESTIAL_GRID_CELL_SIZE_KM,
        );
        assert_close(invalid_scale.grid_cell_size_units(), 10_000.0);
    }

    #[test]
    fn body_placement_uses_provider_state_and_orientation() {
        let mut app = App::new();
        app.init_resource::<CelestialSpace>()
            .insert_resource(CelestialEpoch::new(Epoch::J2000))
            .insert_resource(CelestialEphemeris::from_provider(FakeEphemerisProvider))
            .add_systems(Update, sync_celestial_body_placements);
        let body = app
            .world_mut()
            .spawn((
                CelestialBodyPlacement::for_body_relative_to(CelestialBody::moon(), BodyId::EARTH),
                CellCoord::default(),
                Transform::default(),
            ))
            .id();

        app.update();

        let world = app.world();
        let Some(cell) = world.get::<CellCoord>(body) else {
            return;
        };
        let Some(transform) = world.get::<Transform>(body) else {
            return;
        };

        assert_eq!(cell.x, 38);
        assert_close(f64::from(transform.translation.x), 4_400.0);
        assert_close(f64::from(transform.translation.y), 0.0);
        assert_close(f64::from(transform.translation.z), 0.0);
        assert!((transform.rotation.z - std::f32::consts::FRAC_1_SQRT_2).abs() <= 1.0e-6);
    }

    #[test]
    fn primary_body_can_stay_at_origin_without_provider() {
        let mut app = App::new();
        app.init_resource::<CelestialSpace>()
            .init_resource::<CelestialEpoch>()
            .init_resource::<CelestialEphemeris>()
            .add_systems(Update, sync_celestial_body_placements);
        let body = app
            .world_mut()
            .spawn((
                CelestialBodyPlacement::for_body_relative_to(CelestialBody::earth(), BodyId::EARTH),
                CellCoord::new(4, 5, 6),
                Transform::from_translation(bevy::prelude::Vec3::splat(42.0)),
            ))
            .id();

        app.update();

        let world = app.world();
        let Some(cell) = world.get::<CellCoord>(body) else {
            return;
        };
        let Some(transform) = world.get::<Transform>(body) else {
            return;
        };

        assert_eq!(*cell, CellCoord::default());
        assert_eq!(transform.translation, bevy::prelude::Vec3::ZERO);
    }

    #[test]
    fn quatd_conversion_preserves_unit_rotation() {
        let rotation = quatd_to_bevy(QuatD::from_xyzw(0.0, 0.0, 2.0, 2.0));
        let expected = Quat::from_xyzw(
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        );

        assert!((rotation.z - expected.z).abs() <= 1.0e-6);
        assert!((rotation.w - expected.w).abs() <= 1.0e-6);
    }

    struct FakeEphemerisProvider;

    impl EphemerisProvider for FakeEphemerisProvider {
        fn state(
            &self,
            request: &StateRequest,
        ) -> ferrisium_core::prelude::EphemerisResult<StateVector> {
            if request.target == BodyId::MOON
                && request.origin == BodyId::EARTH
                && request.frame == FrameId::j2000()
            {
                return Ok(StateVector::relative_to(
                    request.target,
                    request.origin,
                    request.frame.clone(),
                    request.epoch,
                    Vec3d::new(384_400.0, 0.0, 0.0),
                    Vec3d::ZERO,
                ));
            }

            Err(EphemerisError::MissingData(
                "fake provider only has Moon relative to Earth".to_owned(),
            ))
        }

        fn orientation(
            &self,
            request: &OrientationRequest,
        ) -> ferrisium_core::prelude::EphemerisResult<FrameOrientation> {
            if request.from == FrameId::moon_me_de440() && request.to == FrameId::j2000() {
                return Ok(FrameOrientation::new(
                    request.from.clone(),
                    request.to.clone(),
                    request.epoch,
                    QuatD::from_xyzw(
                        0.0,
                        0.0,
                        std::f64::consts::FRAC_1_SQRT_2,
                        std::f64::consts::FRAC_1_SQRT_2,
                    ),
                    None,
                ));
            }

            Err(EphemerisError::MissingData(
                "fake provider only has Moon orientation".to_owned(),
            ))
        }
    }
}
