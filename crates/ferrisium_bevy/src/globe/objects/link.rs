//! Straight 3D links between positions around a globe body.
//!
//! `GlobeLink` complements surface-following [`crate::GeoPolyline`] drapes:
//! endpoints are resolved through [`crate::GlobePosition`] semantics and then
//! rendered as a straight tube in scene space.

use std::collections::{HashMap, HashSet};

use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::space::CelestialBody;

use crate::celestial::{CelestialBodyPlacement, CelestialRoot, CelestialSpace};
use crate::globe_anchor::{dvec3_is_finite, resolve_globe_anchor_position, GlobeAnchor};
use crate::globe_position::GlobePosition;
use crate::overlay_common::{color_from_rgba, overlay_is_visible, OverlayColorKey};
use crate::tube_mesh::{build_local_tube_mesh, TubeMesh, DEFAULT_TUBE_RING_SEGMENTS};

/// Default tube width for straight globe links, in render units.
pub const DEFAULT_GLOBE_LINK_WIDTH_UNITS: f32 = 10.0;

/// One endpoint of a straight [`GlobeLink`].
pub type GlobeLinkEndpoint = GlobeAnchor;

/// Straight 3D tube between two positions around one celestial body.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GlobeLink {
    /// Body used for fixed geodetic/body-fixed endpoint conversion.
    pub body: CelestialBody,
    /// Start endpoint.
    pub start: GlobeLinkEndpoint,
    /// End endpoint.
    pub end: GlobeLinkEndpoint,
    /// Constant non-premultiplied sRGBA link color.
    pub color_rgba: [f32; 4],
    /// Tube width in render units.
    pub width_units: f32,
}

impl GlobeLink {
    /// Creates a straight link for `body`.
    #[must_use]
    pub fn for_body(
        body: CelestialBody,
        start: GlobeLinkEndpoint,
        end: GlobeLinkEndpoint,
        color_rgba: [f32; 4],
    ) -> Self {
        Self {
            body,
            start,
            end,
            color_rgba: color_rgba.map(normalized_color_component),
            width_units: DEFAULT_GLOBE_LINK_WIDTH_UNITS,
        }
    }

    /// Creates an Earth-centered straight link.
    #[must_use]
    pub fn earth(start: GlobeLinkEndpoint, end: GlobeLinkEndpoint, color_rgba: [f32; 4]) -> Self {
        Self::for_body(CelestialBody::earth(), start, end, color_rgba)
    }

    /// Sets the tube width in render units.
    #[must_use]
    pub fn with_width_units(mut self, width_units: f32) -> Self {
        self.width_units = normalized_width_units(width_units);
        self
    }
}

/// Convenience bundle for spawning a straight globe link.
#[derive(Bundle)]
pub struct GlobeLinkBundle {
    /// Link content and style.
    pub link: GlobeLink,
    /// Local transform for the marker entity.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Local visibility gate; inherited hierarchy visibility is also honored.
    pub visibility: Visibility,
}

impl GlobeLinkBundle {
    /// Creates a root-visible link bundle that inherits parent visibility.
    #[must_use]
    pub fn new(link: GlobeLink) -> Self {
        Self {
            link,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Spawned render assets for straight globe links.
#[derive(Resource, Default)]
pub(crate) struct GlobeLinkEntities {
    links: HashMap<Entity, GlobeLinkHandles>,
}

#[derive(Debug, Clone)]
struct GlobeLinkHandles {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    color_key: GlobeLinkColorKey,
    start_key: GlobeLinkPositionKey,
    end_key: GlobeLinkPositionKey,
    width_bits: u32,
    parent: Entity,
}

type GlobeLinkColorKey = OverlayColorKey;
type GlobeLinkSourceQueryItem<'a> = (
    Entity,
    Ref<'a, GlobeLink>,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlobeLinkPositionKey {
    bits: [u64; 3],
}

impl GlobeLinkPositionKey {
    fn from_position(position: DVec3) -> Self {
        Self {
            bits: [
                position.x.to_bits(),
                position.y.to_bits(),
                position.z.to_bits(),
            ],
        }
    }
}

/// Synchronizes straight globe links into 3D tube meshes.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose roots, endpoints, body anchors, and asset stores explicitly."
)]
pub(crate) fn sync_globe_links(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    roots: Query<'_, '_, Entity, With<CelestialRoot>>,
    bodies: Query<
        '_,
        '_,
        (&CelestialBodyPlacement, Option<&CellCoord>, &Transform),
        Without<GlobePosition>,
    >,
    endpoint_positions: Query<'_, '_, &GlobePosition>,
    links: Query<'_, '_, GlobeLinkSourceQueryItem<'_>>,
    mut link_entities: ResMut<'_, GlobeLinkEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let live_links = links
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_globe_links(&mut commands, &mut link_entities, &live_links);

    let Some(root) = roots.iter().next() else {
        despawn_all_globe_links(&mut commands, &mut link_entities);
        return;
    };

    for (link_entity, link, visibility, inherited_visibility) in &links {
        if !overlay_is_visible(visibility, inherited_visibility) {
            despawn_globe_link(&mut commands, &mut link_entities, link_entity);
            continue;
        }

        let Some((start_units, end_units)) =
            resolve_globe_link_endpoints(*celestial_space, &link, &bodies, &endpoint_positions)
        else {
            despawn_globe_link(&mut commands, &mut link_entities, link_entity);
            continue;
        };

        if !globe_link_segment_is_valid(start_units, end_units) {
            despawn_globe_link(&mut commands, &mut link_entities, link_entity);
            continue;
        }

        let width_units = normalized_width_units(link.width_units);
        let color_key = GlobeLinkColorKey::from_rgba(link.color_rgba);
        let start_key = GlobeLinkPositionKey::from_position(start_units);
        let end_key = GlobeLinkPositionKey::from_position(end_units);
        let width_bits = width_units.to_bits();

        if let Some(handles) = link_entities.links.get_mut(&link_entity) {
            if handles.parent != root {
                commands.entity(root).add_child(handles.entity);
                handles.parent = root;
            }
            if handles.start_key != start_key
                || handles.end_key != end_key
                || handles.width_bits != width_bits
            {
                let Some(mesh_data) =
                    build_globe_link_tube_mesh(start_units, end_units, width_units)
                else {
                    despawn_globe_link(&mut commands, &mut link_entities, link_entity);
                    continue;
                };
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    *mesh = mesh_data.mesh;
                }
                apply_globe_link_origin(
                    &mut commands,
                    *celestial_space,
                    handles.entity,
                    mesh_data.origin_units,
                );
                handles.start_key = start_key;
                handles.end_key = end_key;
                handles.width_bits = width_bits;
            }
            if handles.color_key != color_key {
                if let Some(material) = materials.get_mut(&handles.material) {
                    update_globe_link_material(material, color_from_rgba(link.color_rgba));
                }
                handles.color_key = color_key;
            }
            continue;
        }

        let Some(mesh_data) = build_globe_link_tube_mesh(start_units, end_units, width_units)
        else {
            despawn_globe_link(&mut commands, &mut link_entities, link_entity);
            continue;
        };
        let mesh = meshes.add(mesh_data.mesh);
        let material = materials.add(globe_link_material(color_from_rgba(link.color_rgba)));
        let placement = celestial_space.place_position_units(mesh_data.origin_units);
        let render_entity = commands
            .spawn((
                Name::new("Globe Link"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(placement.translation),
                GlobalTransform::default(),
                Visibility::Inherited,
                placement.cell,
            ))
            .id();
        commands.entity(root).add_child(render_entity);
        link_entities.links.insert(
            link_entity,
            GlobeLinkHandles {
                entity: render_entity,
                mesh,
                material,
                color_key,
                start_key,
                end_key,
                width_bits,
                parent: root,
            },
        );
    }
}

fn retain_live_globe_links(
    commands: &mut Commands<'_, '_>,
    link_entities: &mut GlobeLinkEntities,
    live_links: &HashSet<Entity>,
) {
    link_entities.links.retain(|link, handles| {
        if live_links.contains(link) {
            true
        } else {
            commands.entity(handles.entity).despawn();
            false
        }
    });
}

fn despawn_all_globe_links(commands: &mut Commands<'_, '_>, link_entities: &mut GlobeLinkEntities) {
    for handles in link_entities.links.drain().map(|(_, handles)| handles) {
        commands.entity(handles.entity).despawn();
    }
}

fn despawn_globe_link(
    commands: &mut Commands<'_, '_>,
    link_entities: &mut GlobeLinkEntities,
    link: Entity,
) {
    if let Some(handles) = link_entities.links.remove(&link) {
        commands.entity(handles.entity).despawn();
    }
}

fn resolve_globe_link_endpoints<F>(
    celestial_space: CelestialSpace,
    link: &GlobeLink,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
    endpoint_positions: &Query<'_, '_, &GlobePosition>,
) -> Option<(DVec3, DVec3)>
where
    F: bevy::ecs::query::QueryFilter,
{
    let start = resolve_globe_link_endpoint(
        celestial_space,
        &link.body,
        link.start,
        bodies,
        endpoint_positions,
    )?;
    let end = resolve_globe_link_endpoint(
        celestial_space,
        &link.body,
        link.end,
        bodies,
        endpoint_positions,
    )?;
    Some((start, end))
}

fn resolve_globe_link_endpoint<F>(
    celestial_space: CelestialSpace,
    body: &CelestialBody,
    endpoint: GlobeLinkEndpoint,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
    endpoint_positions: &Query<'_, '_, &GlobePosition>,
) -> Option<DVec3>
where
    F: bevy::ecs::query::QueryFilter,
{
    resolve_globe_anchor_position(celestial_space, body, endpoint, bodies, endpoint_positions)
}

fn build_globe_link_tube_mesh(
    start_units: DVec3,
    end_units: DVec3,
    width_units: f32,
) -> Option<TubeMesh> {
    let delta = end_units - start_units;
    if !globe_link_segment_is_valid(start_units, end_units) {
        return None;
    }

    let origin_units = start_units + delta * 0.5;
    build_local_tube_mesh(
        &[start_units, end_units],
        width_units,
        origin_units,
        DEFAULT_TUBE_RING_SEGMENTS,
    )
}

fn globe_link_segment_is_valid(start_units: DVec3, end_units: DVec3) -> bool {
    dvec3_is_finite(start_units)
        && dvec3_is_finite(end_units)
        && (end_units - start_units).length_squared() > f64::EPSILON
}

fn apply_globe_link_origin(
    commands: &mut Commands<'_, '_>,
    celestial_space: CelestialSpace,
    entity: Entity,
    origin_units: DVec3,
) {
    let placement = celestial_space.place_position_units(origin_units);
    commands.entity(entity).insert((
        placement.cell,
        Transform::from_translation(placement.translation),
    ));
}

fn globe_link_material(color: Color) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    update_globe_link_material(&mut material, color);
    material
}

fn update_globe_link_material(material: &mut StandardMaterial, color: Color) {
    material.base_color = color;
    material.emissive = color.into();
    material.unlit = true;
    material.cull_mode = None;
    material.alpha_mode = AlphaMode::Add;
}

fn normalized_color_component(component: f32) -> f32 {
    if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn normalized_width_units(width_units: f32) -> f32 {
    if width_units.is_finite() && width_units > 0.0 {
        width_units
    } else {
        DEFAULT_GLOBE_LINK_WIDTH_UNITS
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;
    use bevy::prelude::{
        App, Assets, ChildOf, InheritedVisibility, Mesh, Mesh3d, StandardMaterial, Update,
        Visibility,
    };
    use big_space::prelude::CellCoord;
    use ferrisium_core::space::{BodyId, CelestialBody, RenderScale};

    use super::{
        build_globe_link_tube_mesh, globe_link_material, sync_globe_links,
        DEFAULT_GLOBE_LINK_WIDTH_UNITS,
    };
    use crate::{
        CelestialRoot, CelestialSpace, GlobeLink, GlobeLinkBundle, GlobeLinkEndpoint, GlobePosition,
    };

    fn assert_rgba_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "rgba mismatch: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn earth_link_defaults_to_earth_body_and_normalizes_style() {
        let link = GlobeLink::earth(
            GlobeLinkEndpoint::body_fixed(1.0, 0.0, 0.0),
            GlobeLinkEndpoint::body_fixed(2.0, 0.0, 0.0),
            [-1.0, 0.5, 2.0, f32::NAN],
        )
        .with_width_units(f32::NAN);

        assert_eq!(link.body.id, BodyId::EARTH);
        assert_rgba_close(link.color_rgba, [0.0, 0.5, 1.0, 0.0]);
        assert!((link.width_units - DEFAULT_GLOBE_LINK_WIDTH_UNITS).abs() <= f32::EPSILON);
        assert_eq!(GlobeLinkBundle::new(link).visibility, Visibility::Inherited);
    }

    #[test]
    fn globe_link_tube_mesh_recenters_large_positions_around_midpoint() {
        let start = bevy::math::DVec3::new(1.0e12, -2.0e12, 3.0e12);
        let end = bevy::math::DVec3::new(1.0e12 + 20.0, -2.0e12, 3.0e12);
        let mesh_data = build_globe_link_tube_mesh(start, end, 2.0);
        assert!(mesh_data.is_some(), "link mesh should build");
        let Some(mesh_data) = mesh_data else {
            return;
        };

        assert_eq!(mesh_data.origin_units, (start + end) * 0.5);
        let positions = mesh_data
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        assert!(positions.is_some(), "link mesh should have positions");
        let Some(positions) = positions else {
            return;
        };
        assert!(
            positions
                .iter()
                .flatten()
                .all(|component| component.abs() < 12.0),
            "local mesh vertices should stay near the link midpoint"
        );
    }

    #[test]
    fn globe_link_material_is_unlit_transparent_tube_material() {
        let material = globe_link_material(bevy::prelude::Color::srgba(0.2, 0.8, 1.0, 0.6));

        assert!(material.unlit);
        assert_eq!(material.alpha_mode, bevy::prelude::AlphaMode::Add);
        assert!(material.cull_mode.is_none());
    }

    #[test]
    fn globe_link_sync_spawns_big_space_render_entity() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(10_000.0, RenderScale::PHYSICAL))
            .init_resource::<super::GlobeLinkEntities>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, sync_globe_links);
        let root = app.world_mut().spawn(CelestialRoot).id();
        let link = app
            .world_mut()
            .spawn((
                GlobeLinkBundle::new(
                    GlobeLink::earth(
                        GlobeLinkEndpoint::body_fixed(10_010.0, 0.0, 0.0),
                        GlobeLinkEndpoint::body_fixed(10_030.0, 0.0, 0.0),
                        [0.2, 0.8, 1.0, 0.7],
                    )
                    .with_width_units(2.0),
                ),
                InheritedVisibility::VISIBLE,
            ))
            .id();

        app.update();

        let render_entity = app
            .world()
            .resource::<super::GlobeLinkEntities>()
            .links
            .get(&link)
            .map(|handles| handles.entity);
        assert!(render_entity.is_some(), "render entity should be spawned");
        let Some(render_entity) = render_entity else {
            return;
        };
        let world = app.world();
        assert!(world.get::<Mesh3d>(render_entity).is_some());
        assert!(world.get::<CellCoord>(render_entity).is_some());
        let parent = world.get::<ChildOf>(render_entity);
        assert!(parent.is_some(), "render entity should have a parent");
        let Some(parent) = parent else {
            return;
        };
        assert_eq!(parent.parent(), root);
    }

    #[test]
    fn globe_link_entity_endpoint_tracks_matching_globe_position() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::default())
            .init_resource::<super::GlobeLinkEntities>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, sync_globe_links);
        app.world_mut().spawn(CelestialRoot);
        let endpoint = app
            .world_mut()
            .spawn(GlobePosition::body_fixed(
                &CelestialBody::earth(),
                6_378.137,
                0.0,
                0.0,
            ))
            .id();
        let link = app
            .world_mut()
            .spawn((
                GlobeLinkBundle::new(GlobeLink::earth(
                    GlobeLinkEndpoint::entity(endpoint),
                    GlobeLinkEndpoint::geodetic(0.0, 0.0, 0.0),
                    [1.0, 0.5, 0.2, 0.75],
                )),
                InheritedVisibility::VISIBLE,
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .resource::<super::GlobeLinkEntities>()
                .links
                .contains_key(&link),
            "matching GlobePosition entity endpoint should render"
        );
    }
}
