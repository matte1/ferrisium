//! Metric object and trajectory visualization helpers.
//!
//! This module keeps physical placement separate from visual legibility:
//! object centers and trajectory samples are transformed through one linear
//! [`CelestialSpace`] scale, while body or spacecraft marker meshes can use a
//! clearly tracked visual-radius proxy.

use std::collections::{HashMap, HashSet};

use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::space::{
    BodyId, Epoch, FrameId, Kilometers, StateRequest, StateVector, TrajectoryPath, Vec3d,
};

use crate::celestial::{CelestialEphemeris, CelestialEpoch, CelestialSpace};
#[cfg(test)]
use crate::tube_mesh::build_tube_mesh;
use crate::tube_mesh::{build_first_point_local_tube_mesh, TubeMesh, DEFAULT_TUBE_RING_SEGMENTS};

const TRAJECTORY_TUBE_RING_SEGMENTS: usize = DEFAULT_TUBE_RING_SEGMENTS;

type MetricTrajectoryQueryItem<'a> = (
    Entity,
    Ref<'a, MetricTrajectory>,
    Option<&'a mut CellCoord>,
    &'a mut Transform,
    Option<&'a Visibility>,
);

/// Ephemeris-backed placement for a generic metric object.
///
/// Unlike [`crate::CelestialBodyPlacement`], this component does not imply a
/// physical spherical body or body-fixed orientation. It is suitable for
/// spacecraft, barycenters, planning waypoints, and simple body markers.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct MetricObjectPlacement {
    /// Body, spacecraft, or synthetic object to place.
    pub target: BodyId,
    /// Body or barycenter that `target` is positioned relative to.
    pub origin: BodyId,
    /// Inertial frame used for the position query.
    pub frame: FrameId,
}

impl MetricObjectPlacement {
    /// Creates metric placement for `target` relative to `origin` in `frame`.
    #[must_use]
    pub fn relative_to(target: BodyId, origin: BodyId, frame: FrameId) -> Self {
        Self {
            target,
            origin,
            frame,
        }
    }

    fn state_request(&self, epoch: Epoch) -> StateRequest {
        StateRequest::new(self.target, self.origin, self.frame.clone(), epoch)
    }
}

/// Source of the physical radius used by a metric visual proxy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricVisualRadiusSource {
    /// Spherical object radius in physical kilometers.
    Physical(Kilometers),
    /// Point-like object with no modeled physical radius.
    Point,
}

/// Policy that converts physical render radius into visible mesh radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricVisualRadiusPolicy {
    /// Lower bound for the displayed mesh radius, in render units.
    pub minimum_visible_radius_units: f32,
    /// Optional upper bound for the displayed mesh radius, in render units.
    pub maximum_visible_radius_units: Option<f32>,
}

impl MetricVisualRadiusPolicy {
    /// Creates a policy with only a minimum visible radius.
    #[must_use]
    pub const fn minimum(minimum_visible_radius_units: f32) -> Self {
        Self {
            minimum_visible_radius_units,
            maximum_visible_radius_units: None,
        }
    }

    /// Computes the displayed radius and records whether it is a visual proxy.
    #[must_use]
    pub fn visual_radius(self, physical_radius_units: f32) -> MetricVisualRadius {
        let physical_radius_units = finite_non_negative(physical_radius_units);
        let minimum_visible_radius_units = finite_non_negative(self.minimum_visible_radius_units);
        let mut visual_radius_units = physical_radius_units.max(minimum_visible_radius_units);

        if let Some(maximum_visible_radius_units) =
            self.maximum_visible_radius_units.map(finite_non_negative)
        {
            if maximum_visible_radius_units >= minimum_visible_radius_units
                && maximum_visible_radius_units > 0.0
            {
                visual_radius_units = visual_radius_units.min(maximum_visible_radius_units);
            }
        }

        let scale_factor = if physical_radius_units > 0.0 {
            visual_radius_units / physical_radius_units
        } else {
            1.0
        };

        MetricVisualRadius {
            physical_radius_units,
            visual_radius_units,
            scale_factor,
            enlarged: visual_radius_units > physical_radius_units,
        }
    }
}

/// Computed physical and displayed radius for one metric visual object.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MetricVisualRadius {
    /// True physical radius converted through the active render scale.
    pub physical_radius_units: f32,
    /// Radius used by the visible proxy mesh.
    pub visual_radius_units: f32,
    /// Display radius divided by physical radius; `1.0` for point markers.
    pub scale_factor: f32,
    /// Whether the visible mesh is larger than the physical radius.
    pub enlarged: bool,
}

/// Visible marker for a metric object.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct MetricVisualObject {
    /// Physical-radius source used before applying the visual policy.
    pub radius_source: MetricVisualRadiusSource,
    /// Display-radius policy for legibility.
    pub radius_policy: MetricVisualRadiusPolicy,
    /// Base color for the marker mesh.
    pub color: Color,
    /// Whether the marker should be emissive/unlit.
    pub emissive: bool,
}

impl MetricVisualObject {
    /// Creates a spherical-body visual with a known physical radius.
    #[must_use]
    pub fn physical_radius(
        radius: Kilometers,
        radius_policy: MetricVisualRadiusPolicy,
        color: Color,
    ) -> Self {
        Self {
            radius_source: MetricVisualRadiusSource::Physical(radius),
            radius_policy,
            color,
            emissive: false,
        }
    }

    /// Creates a point-like marker visual for spacecraft or waypoints.
    #[must_use]
    pub fn point(radius_policy: MetricVisualRadiusPolicy, color: Color) -> Self {
        Self {
            radius_source: MetricVisualRadiusSource::Point,
            radius_policy,
            color,
            emissive: false,
        }
    }

    /// Marks the visual as emissive and unlit.
    #[must_use]
    pub const fn emissive(mut self) -> Self {
        self.emissive = true;
        self
    }
}

/// Render styling for a metric trajectory ribbon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricTrajectoryVisual {
    /// Ribbon width in render units.
    pub width_units: f32,
    /// Ribbon color.
    pub color: Color,
}

/// Renderable sampled trajectory.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct MetricTrajectory {
    /// Provider-neutral sampled trajectory.
    pub path: TrajectoryPath,
    /// Browser-safe ribbon styling.
    pub visual: MetricTrajectoryVisual,
}

impl MetricTrajectory {
    /// Creates a renderable trajectory from sampled path data and visual style.
    #[must_use]
    pub const fn new(path: TrajectoryPath, visual: MetricTrajectoryVisual) -> Self {
        Self { path, visual }
    }
}

#[derive(Debug, Clone)]
struct MetricVisualObjectHandles {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone)]
struct MetricTrajectoryHandles {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

type MetricTrajectoryMesh = TubeMesh;

/// Spawned mesh/material handles for metric visual objects and trajectories.
#[derive(Resource, Default)]
pub(crate) struct MetricVisualEntities {
    objects: HashMap<Entity, MetricVisualObjectHandles>,
    trajectories: HashMap<Entity, MetricTrajectoryHandles>,
}

/// Converts a physical kilometer position into metric render units.
#[must_use]
pub fn metric_position_units(celestial_space: CelestialSpace, position_km: Vec3d) -> DVec3 {
    let position_units = celestial_space.render_scale.position_to_units(position_km);

    DVec3::new(position_units.x, position_units.y, position_units.z)
}

/// Converts a state-vector position into metric render units.
#[must_use]
pub fn metric_state_position_units(celestial_space: CelestialSpace, state: &StateVector) -> DVec3 {
    metric_position_units(celestial_space, state.position_km)
}

/// Synchronizes generic metric object centers from the active ephemeris provider.
#[allow(
    clippy::type_complexity,
    reason = "The object query updates both BigSpace and plain Bevy placement components."
)]
pub(crate) fn sync_metric_object_placements(
    celestial_space: Res<'_, CelestialSpace>,
    celestial_epoch: Res<'_, CelestialEpoch>,
    celestial_ephemeris: Res<'_, CelestialEphemeris>,
    mut objects: Query<
        '_,
        '_,
        (
            &MetricObjectPlacement,
            Option<&mut CellCoord>,
            &mut Transform,
        ),
    >,
) {
    let provider = celestial_ephemeris.provider();
    for (object, cell, mut transform) in &mut objects {
        let Some(state) = metric_object_state(object, celestial_epoch.epoch, provider) else {
            continue;
        };
        apply_metric_state(*celestial_space, &state, cell, &mut transform);
    }
}

/// Synchronizes visible proxy meshes for metric objects.
pub(crate) fn sync_metric_visual_objects(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    mut entities: ResMut<'_, MetricVisualEntities>,
    objects: Query<'_, '_, (Entity, &MetricVisualObject, Option<&MetricVisualRadius>)>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let live_objects = objects
        .iter()
        .map(|(entity, _, _)| entity)
        .collect::<HashSet<_>>();
    entities.objects.retain(|object, handles| {
        if live_objects.contains(object) {
            true
        } else {
            commands.entity(handles.entity).despawn();
            false
        }
    });

    for (object, visual, existing_radius) in &objects {
        let radius = metric_visual_radius(*celestial_space, visual);
        if existing_radius != Some(&radius) {
            commands.entity(object).insert(radius);
        }

        if let Some(handles) = entities.objects.get(&object) {
            if existing_radius != Some(&radius) {
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    *mesh = visual_sphere_mesh(radius.visual_radius_units);
                }
            }
            if let Some(material) = materials.get_mut(&handles.material) {
                update_metric_visual_material(material, visual);
            }
            continue;
        }

        let mesh = meshes.add(visual_sphere_mesh(radius.visual_radius_units));
        let material = materials.add(metric_visual_material(visual));
        let visual_entity = commands
            .spawn((
                Name::new("Metric Visual Object"),
                Transform::default(),
                GlobalTransform::default(),
                Visibility::Inherited,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        commands.entity(object).add_child(visual_entity);
        entities.objects.insert(
            object,
            MetricVisualObjectHandles {
                entity: visual_entity,
                mesh,
                material,
            },
        );
    }
}

/// Synchronizes metric trajectory ribbon meshes from sampled state vectors.
pub(crate) fn sync_metric_trajectories(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    mut entities: ResMut<'_, MetricVisualEntities>,
    mut trajectories: Query<'_, '_, MetricTrajectoryQueryItem<'_>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let celestial_space_changed = celestial_space.is_changed();
    let live_trajectories = trajectories
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<HashSet<_>>();
    entities.trajectories.retain(|trajectory, _| {
        if live_trajectories.contains(trajectory) {
            true
        } else {
            commands.entity(*trajectory).remove::<Mesh3d>();
            commands
                .entity(*trajectory)
                .remove::<MeshMaterial3d<StandardMaterial>>();
            false
        }
    });

    for (trajectory_entity, trajectory, cell, mut transform, visibility) in &mut trajectories {
        let has_handles = entities.trajectories.contains_key(&trajectory_entity);
        if has_handles && !trajectory.is_changed() && !celestial_space_changed {
            continue;
        }

        let Some(mesh_data) = metric_trajectory_mesh(*celestial_space, &trajectory) else {
            if entities.trajectories.remove(&trajectory_entity).is_some() {
                commands.entity(trajectory_entity).remove::<Mesh3d>();
                commands
                    .entity(trajectory_entity)
                    .remove::<MeshMaterial3d<StandardMaterial>>();
            }
            continue;
        };
        apply_trajectory_origin(
            *celestial_space,
            mesh_data.origin_units,
            cell,
            &mut transform,
        );

        if let Some(handles) = entities.trajectories.get(&trajectory_entity) {
            if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                *mesh = mesh_data.mesh;
            }
            if let Some(material) = materials.get_mut(&handles.material) {
                update_metric_trajectory_material(material, trajectory.visual);
            }
            continue;
        }

        let mesh = meshes.add(mesh_data.mesh);
        let material = materials.add(metric_trajectory_material(trajectory.visual));
        commands
            .entity(trajectory_entity)
            .insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
        if visibility.is_none() {
            commands
                .entity(trajectory_entity)
                .insert(Visibility::Inherited);
        }
        entities.trajectories.insert(
            trajectory_entity,
            MetricTrajectoryHandles { mesh, material },
        );
    }
}

fn metric_object_state(
    object: &MetricObjectPlacement,
    epoch: Epoch,
    provider: Option<&(dyn ferrisium_core::space::EphemerisProvider + Send + Sync + 'static)>,
) -> Option<StateVector> {
    if object.target == object.origin {
        return Some(StateVector::relative_to(
            object.target,
            object.origin,
            object.frame.clone(),
            epoch,
            Vec3d::ZERO,
            Vec3d::ZERO,
        ));
    }

    provider?.state(&object.state_request(epoch)).ok()
}

fn apply_metric_state(
    celestial_space: CelestialSpace,
    state: &StateVector,
    cell: Option<Mut<'_, CellCoord>>,
    transform: &mut Transform,
) {
    let placement = celestial_space.placement_for_state(state);
    if let Some(mut cell) = cell {
        *cell = placement.cell;
        transform.translation = placement.translation;
    } else {
        transform.translation = placement.absolute_position_units(celestial_space).as_vec3();
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Physical radius enters Bevy mesh generation as f32 render units."
)]
fn metric_visual_radius(
    celestial_space: CelestialSpace,
    visual: &MetricVisualObject,
) -> MetricVisualRadius {
    let physical_radius_units = match visual.radius_source {
        MetricVisualRadiusSource::Physical(radius) => {
            celestial_space.render_scale.distance_to_units(radius) as f32
        }
        MetricVisualRadiusSource::Point => 0.0,
    };

    visual.radius_policy.visual_radius(physical_radius_units)
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn visual_sphere_mesh(radius: f32) -> Mesh {
    Sphere::new(radius.max(f32::EPSILON)).mesh().uv(32, 16)
}

fn metric_visual_material(visual: &MetricVisualObject) -> StandardMaterial {
    let mut material = StandardMaterial {
        base_color: visual.color,
        ..default()
    };
    update_metric_visual_material(&mut material, visual);
    material
}

fn update_metric_visual_material(material: &mut StandardMaterial, visual: &MetricVisualObject) {
    material.base_color = visual.color;
    material.unlit = visual.emissive;
    material.emissive = if visual.emissive {
        visual.color.into()
    } else {
        Color::BLACK.into()
    };
}

fn metric_trajectory_mesh(
    celestial_space: CelestialSpace,
    trajectory: &MetricTrajectory,
) -> Option<MetricTrajectoryMesh> {
    let points = trajectory
        .path
        .samples
        .iter()
        .map(|sample| metric_state_position_units(celestial_space, sample))
        .collect::<Vec<_>>();

    build_first_point_local_tube_mesh(
        &points,
        trajectory.visual.width_units,
        TRAJECTORY_TUBE_RING_SEGMENTS,
    )
}

fn apply_trajectory_origin(
    celestial_space: CelestialSpace,
    origin_units: DVec3,
    cell: Option<Mut<'_, CellCoord>>,
    transform: &mut Transform,
) {
    if let Some(mut cell) = cell {
        let placement = celestial_space.place_position_units(origin_units);
        *cell = placement.cell;
        transform.translation = placement.translation;
    } else {
        transform.translation = origin_units.as_vec3();
    }
}

fn metric_trajectory_material(visual: MetricTrajectoryVisual) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    update_metric_trajectory_material(&mut material, visual);
    material
}

fn update_metric_trajectory_material(
    material: &mut StandardMaterial,
    visual: MetricTrajectoryVisual,
) {
    material.base_color = visual.color;
    material.emissive = visual.color.into();
    material.unlit = true;
    material.cull_mode = None;
    material.alpha_mode = AlphaMode::Add;
}

#[cfg(test)]
fn build_trajectory_tube_mesh(points: &[DVec3], width_units: f32) -> Option<Mesh> {
    build_tube_mesh(points, width_units, TRAJECTORY_TUBE_RING_SEGMENTS)
}

#[cfg(test)]
fn build_local_trajectory_tube_mesh(
    points: &[DVec3],
    width_units: f32,
) -> Option<MetricTrajectoryMesh> {
    build_first_point_local_tube_mesh(points, width_units, TRAJECTORY_TUBE_RING_SEGMENTS)
}

#[cfg(test)]
mod tests {
    use super::{
        build_local_trajectory_tube_mesh, build_trajectory_tube_mesh, metric_position_units,
        metric_state_position_units, sync_metric_visual_objects, MetricTrajectory,
        MetricTrajectoryVisual, MetricVisualEntities, MetricVisualObject, MetricVisualRadiusPolicy,
        TRAJECTORY_TUBE_RING_SEGMENTS,
    };
    use crate::CelestialSpace;
    use bevy::prelude::{
        App, Assets, Color, GlobalTransform, Mesh, StandardMaterial, Update, Visibility,
    };
    use ferrisium_core::space::{
        BodyId, Epoch, FrameId, Kilometers, RenderScale, StateVector, TrajectoryPath, Vec3d,
    };

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    fn assert_close_f32(lhs: f32, rhs: f32) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-6,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn metric_position_transform_preserves_distance_ratios() {
        let celestial_space =
            CelestialSpace::new(1_000_000.0, RenderScale::from_kilometers_per_unit(1_000.0));
        let near = metric_position_units(celestial_space, Vec3d::new(10_000.0, 0.0, 0.0));
        let far = metric_position_units(celestial_space, Vec3d::new(30_000.0, 0.0, 0.0));

        assert_close(far.length() / near.length(), 3.0);
    }

    #[test]
    fn visual_radius_policy_preserves_large_bodies_and_enlarges_small_bodies() {
        let policy = MetricVisualRadiusPolicy::minimum(5.0);
        let large = policy.visual_radius(10.0);
        let small = policy.visual_radius(1.0);

        assert_close_f32(large.visual_radius_units, 10.0);
        assert!(!large.enlarged);
        assert_close_f32(small.visual_radius_units, 5.0);
        assert!(small.enlarged);
        assert_close_f32(small.scale_factor, 5.0);
    }

    #[test]
    fn visual_radius_policy_can_cap_display_radius_without_changing_physical_radius() {
        let policy = MetricVisualRadiusPolicy {
            minimum_visible_radius_units: 2.0,
            maximum_visible_radius_units: Some(8.0),
        };
        let radius = policy.visual_radius(20.0);

        assert_close_f32(radius.physical_radius_units, 20.0);
        assert_close_f32(radius.visual_radius_units, 8.0);
        assert_close_f32(radius.scale_factor, 0.4);
        assert!(!radius.enlarged);
    }

    #[test]
    fn visual_radius_does_not_modify_state_vectors() {
        let state = StateVector::relative_to(
            BodyId::EARTH,
            BodyId::SUN,
            FrameId::j2000(),
            Epoch::J2000,
            Vec3d::new(149_598_023.0, 0.0, 0.0),
            Vec3d::ZERO,
        );
        let visual = MetricVisualObject::physical_radius(
            Kilometers::new(6_371.0),
            MetricVisualRadiusPolicy::minimum(10.0),
            Color::srgb(0.2, 0.4, 1.0),
        );
        let radius = visual.radius_policy.visual_radius(0.006_371);

        assert!(radius.enlarged);
        assert_close(state.position_km.x, 149_598_023.0);
    }

    #[test]
    fn trajectory_samples_use_same_metric_transform_as_objects() {
        let celestial_space =
            CelestialSpace::new(1_000_000.0, RenderScale::from_kilometers_per_unit(1_000.0));
        let sample = StateVector::relative_to(
            BodyId::EARTH,
            BodyId::SUN,
            FrameId::j2000(),
            Epoch::J2000,
            Vec3d::new(10_000.0, 0.0, 0.0),
            Vec3d::ZERO,
        );

        assert_eq!(
            metric_state_position_units(celestial_space, &sample),
            metric_position_units(celestial_space, sample.position_km)
        );
    }

    #[test]
    fn trajectory_tube_mesh_requires_at_least_two_samples() {
        let one_point = [bevy::math::DVec3::ZERO];
        let two_points = [bevy::math::DVec3::ZERO, bevy::math::DVec3::X];

        assert!(build_trajectory_tube_mesh(&one_point, 0.1).is_none());
        assert!(build_trajectory_tube_mesh(&two_points, 0.1).is_some());
    }

    #[test]
    fn trajectory_tube_mesh_is_continuous_for_curved_paths() {
        let points = [
            bevy::math::DVec3::new(1.0, 0.0, 0.0),
            bevy::math::DVec3::new(0.0, 0.0, 1.0),
            bevy::math::DVec3::new(-1.0, 0.0, 0.0),
        ];

        let mesh = build_trajectory_tube_mesh(&points, 0.1);

        assert_eq!(
            mesh.map_or(0, |mesh| mesh.count_vertices()),
            points.len() * TRAJECTORY_TUBE_RING_SEGMENTS
        );
    }

    #[test]
    fn trajectory_tube_mesh_builds_triangle_indices_for_each_segment() {
        let points = [
            bevy::math::DVec3::ZERO,
            bevy::math::DVec3::X,
            bevy::math::DVec3::new(2.0, 0.0, 1.0),
            bevy::math::DVec3::new(3.0, 0.0, 1.0),
        ];
        let mesh = build_trajectory_tube_mesh(&points, 0.2);
        assert!(mesh.is_some(), "mesh should build");
        let Some(mesh) = mesh else {
            return;
        };

        assert_eq!(
            mesh.indices().map_or(0, bevy::mesh::Indices::len),
            points.len().saturating_sub(1) * TRAJECTORY_TUBE_RING_SEGMENTS * 6
        );
    }

    #[test]
    fn trajectory_tube_mesh_recenters_large_positions_into_local_vertices() {
        let points = [
            bevy::math::DVec3::new(1.0e12, -2.0e12, 3.0e12),
            bevy::math::DVec3::new(1.0e12 + 10.0, -2.0e12, 3.0e12),
        ];
        let mesh_data = build_local_trajectory_tube_mesh(&points, 0.2);
        assert!(mesh_data.is_some(), "mesh should build");
        let Some(mesh_data) = mesh_data else {
            return;
        };
        let positions = mesh_data
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(bevy::mesh::VertexAttributeValues::as_float3);
        assert!(
            positions.is_some(),
            "trajectory mesh should have float3 positions"
        );
        let Some(positions) = positions else {
            return;
        };

        assert_eq!(mesh_data.origin_units, points[0]);
        assert!(
            positions
                .iter()
                .flatten()
                .all(|component| component.abs() < 11.0),
            "local mesh vertices should not contain solar-scale absolute coordinates"
        );
    }

    #[test]
    fn metric_visual_object_children_include_render_required_components() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::default())
            .init_resource::<MetricVisualEntities>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, sync_metric_visual_objects);

        let object = app
            .world_mut()
            .spawn(MetricVisualObject::point(
                MetricVisualRadiusPolicy::minimum(1.0),
                Color::srgb(0.2, 0.6, 1.0),
            ))
            .id();
        app.update();

        let visual_entity = app
            .world()
            .resource::<MetricVisualEntities>()
            .objects
            .get(&object)
            .map(|handles| handles.entity);
        assert!(visual_entity.is_some(), "visual child should be spawned");
        let Some(visual_entity) = visual_entity else {
            return;
        };
        assert!(app.world().get::<GlobalTransform>(visual_entity).is_some());
        assert!(app.world().get::<Visibility>(visual_entity).is_some());
    }

    #[test]
    fn metric_trajectory_keeps_authoritative_path_data() {
        let path = TrajectoryPath::new(
            BodyId::EARTH,
            BodyId::SUN,
            FrameId::j2000(),
            Epoch::J2000,
            Epoch::from_tdb_nanoseconds_since_j2000(1),
            vec![StateVector::relative_to(
                BodyId::EARTH,
                BodyId::SUN,
                FrameId::j2000(),
                Epoch::J2000,
                Vec3d::new(1.0, 2.0, 3.0),
                Vec3d::ZERO,
            )],
        );
        let trajectory = MetricTrajectory::new(
            path.clone(),
            MetricTrajectoryVisual {
                width_units: 0.1,
                color: Color::WHITE,
            },
        );

        assert_eq!(trajectory.path, path);
    }
}
