//! Geodetic polygon rendering for Ferrisium map and globe views.
//!
//! A `GeoPolygon` stores one convex, no-hole geographic ring and Ferrisium
//! renders it as a projected 2D fill/outline or as a draped region on the
//! focused globe.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use ferrisium_core::prelude::{BodyId, LonLat, PlanetaryBody};

use crate::celestial::CelestialFocus;
use crate::overlay_common::{
    color_from_rgba, overlay_is_visible, OverlayColorKey, OverlayMapViewKey,
};
use crate::surface_context::GlobeSurfaceRenderContext;
use crate::surface_depth::{
    apply_spherical_decal_material, draped_fill_radius, draped_outline_radius,
    SphericalSurfaceLayer,
};
use crate::surface_mesh::{
    append_line_segment, append_tessellated_spherical_outline_edge,
    append_tessellated_spherical_polygon, append_triangle_fan_indices, globe_surface_direction,
    line_mesh, polygon_center_direction, triangle_mesh,
};
use crate::view::{Map2dRenderContext, Map2dView};

const DEFAULT_GEO_POLYGON_MAP_Z: f32 = 34.0;
const DEFAULT_GEO_POLYGON_GLOBE_SURFACE_OFFSET_UNITS: f32 = 0.0;

/// One geographic vertex for a [`GeoPolygon`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoPolygonVertex {
    /// Longitude/latitude in degrees.
    pub lon_lat: LonLat,
}

impl GeoPolygonVertex {
    /// Creates a polygon vertex from an existing lon/lat value.
    #[must_use]
    pub const fn new(lon_lat: LonLat) -> Self {
        Self { lon_lat }
    }

    /// Creates a polygon vertex from explicit longitude and latitude degrees.
    #[must_use]
    pub const fn from_degrees(lon_deg: f64, lat_deg: f64) -> Self {
        Self {
            lon_lat: LonLat::new(lon_deg, lat_deg),
        }
    }
}

/// A convex, no-hole geodetic polygon rendered over a map and/or globe surface.
///
/// Concave polygons are not currently triangulated; split them into convex
/// pieces before rendering.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GeoPolygon {
    /// Body center that receives the globe polygon.
    pub target: BodyId,
    /// Planetary map body that receives the 2D polygon.
    pub body: PlanetaryBody,
    /// Convex closed geographic ring vertices. Do not repeat the first vertex.
    pub vertices: Vec<GeoPolygonVertex>,
    /// Constant non-premultiplied sRGBA fill color.
    pub fill_rgba: [f32; 4],
    /// Constant non-premultiplied sRGBA outline color.
    pub outline_rgba: [f32; 4],
    /// Z depth used for 2D map meshes above raster tiles.
    pub map_z: f32,
    /// Whether the polygon should render in 2D map views.
    pub map_enabled: bool,
    /// Whether the polygon should render on focused 3D globe surfaces.
    pub globe_enabled: bool,
    /// Minimum outward globe-surface visual lift in render units.
    pub globe_surface_offset_units: f32,
}

impl GeoPolygon {
    /// Creates an Earth polygon for both Earth map and Earth globe views.
    #[must_use]
    pub fn earth(
        vertices: impl IntoIterator<Item = GeoPolygonVertex>,
        fill_rgba: [f32; 4],
        outline_rgba: [f32; 4],
    ) -> Self {
        Self::for_body(
            BodyId::EARTH,
            PlanetaryBody::earth(),
            vertices,
            fill_rgba,
            outline_rgba,
        )
    }

    /// Creates a polygon for a map body and a celestial target.
    #[must_use]
    pub fn for_body(
        target: BodyId,
        body: PlanetaryBody,
        vertices: impl IntoIterator<Item = GeoPolygonVertex>,
        fill_rgba: [f32; 4],
        outline_rgba: [f32; 4],
    ) -> Self {
        Self {
            target,
            body,
            vertices: vertices.into_iter().collect(),
            fill_rgba: fill_rgba.map(normalized_color_component),
            outline_rgba: outline_rgba.map(normalized_color_component),
            map_z: DEFAULT_GEO_POLYGON_MAP_Z,
            map_enabled: true,
            globe_enabled: true,
            globe_surface_offset_units: DEFAULT_GEO_POLYGON_GLOBE_SURFACE_OFFSET_UNITS,
        }
    }

    /// Sets the 2D map z depth.
    #[must_use]
    pub const fn with_map_z(mut self, map_z: f32) -> Self {
        self.map_z = map_z;
        self
    }

    /// Enables or disables this polygon in 2D map views.
    #[must_use]
    pub const fn with_map_enabled(mut self, enabled: bool) -> Self {
        self.map_enabled = enabled;
        self
    }

    /// Enables or disables this polygon on focused 3D globe surfaces.
    #[must_use]
    pub const fn with_globe_enabled(mut self, enabled: bool) -> Self {
        self.globe_enabled = enabled;
        self
    }

    /// Sets the minimum outward globe-surface visual lift in render units.
    #[must_use]
    pub const fn with_globe_surface_offset_units(mut self, offset_units: f32) -> Self {
        self.globe_surface_offset_units = offset_units;
        self
    }
}

/// Convenience bundle for spawning a geodetic polygon.
#[derive(Bundle)]
pub struct GeoPolygonBundle {
    /// Polygon content and style.
    pub polygon: GeoPolygon,
    /// Local transform for the marker entity.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Local visibility gate; inherited hierarchy visibility is also honored.
    pub visibility: Visibility,
}

impl GeoPolygonBundle {
    /// Creates a root-visible polygon bundle that inherits parent visibility.
    #[must_use]
    pub fn new(polygon: GeoPolygon) -> Self {
        Self {
            polygon,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Spawned render assets for map and globe polygons.
#[derive(Resource, Default)]
pub(crate) struct GeoPolygonEntities {
    map: HashMap<Entity, GeoPolygonMapHandles>,
    globe: HashMap<Entity, GeoPolygonGlobeHandles>,
}

#[derive(Debug, Clone)]
struct GeoPolygonMapHandles {
    fill_entity: Entity,
    fill_mesh: Handle<Mesh>,
    fill_material: Handle<ColorMaterial>,
    fill_color_key: GeoPolygonColorKey,
    outline_entity: Entity,
    outline_mesh: Handle<Mesh>,
    outline_material: Handle<ColorMaterial>,
    outline_color_key: GeoPolygonColorKey,
    map_z_bits: u32,
    view_key: GeoPolygonMapViewKey,
}

#[derive(Debug, Clone)]
struct GeoPolygonGlobeHandles {
    fill_entity: Entity,
    fill_mesh: Handle<Mesh>,
    fill_material: Handle<StandardMaterial>,
    fill_color_key: GeoPolygonColorKey,
    outline_entity: Entity,
    outline_mesh: Handle<Mesh>,
    outline_material: Handle<StandardMaterial>,
    outline_color_key: GeoPolygonColorKey,
    parent: Entity,
    radius_bits: u32,
    offset_bits: u32,
}

type GeoPolygonColorKey = OverlayColorKey;
type GeoPolygonMapViewKey = OverlayMapViewKey;
type GeoPolygonRenderQueryItem<'a> = (
    Entity,
    Ref<'a, GeoPolygon>,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
);

/// Synchronizes geodetic polygons into the active 2D map view.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose view, polygon, and asset stores explicitly."
)]
pub(crate) fn sync_geo_polygon_map_overlays(
    mut commands: Commands<'_, '_>,
    map_contexts: Query<'_, '_, &Map2dRenderContext, With<Map2dView>>,
    polygons: Query<'_, '_, GeoPolygonRenderQueryItem<'_>>,
    mut polygon_entities: ResMut<'_, GeoPolygonEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<ColorMaterial>>,
) {
    let live_polygons = polygons
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_geo_polygon_map_overlays(&mut commands, &mut polygon_entities, &live_polygons);

    let Some(context) = map_contexts.iter().next() else {
        despawn_all_geo_polygon_map_overlays(&mut commands, &mut polygon_entities);
        return;
    };
    let view_key = geo_polygon_map_view_key(context);

    for (polygon_entity, polygon, visibility, inherited_visibility) in &polygons {
        if !polygon.map_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || polygon.body != context.body
            || polygon.vertices.len() < 3
        {
            despawn_geo_polygon_map_overlay(&mut commands, &mut polygon_entities, polygon_entity);
            continue;
        }

        let fill_color_key = GeoPolygonColorKey::from_rgba(polygon.fill_rgba);
        let outline_color_key = GeoPolygonColorKey::from_rgba(polygon.outline_rgba);
        let map_z_bits = polygon.map_z.to_bits();
        if let Some(handles) = polygon_entities.map.get_mut(&polygon_entity) {
            if polygon.is_changed() || handles.view_key != view_key {
                if let Some(mesh) = meshes.get_mut(&handles.fill_mesh) {
                    *mesh = build_geo_polygon_map_fill_mesh(&polygon, context);
                }
                if let Some(mesh) = meshes.get_mut(&handles.outline_mesh) {
                    *mesh = build_geo_polygon_map_outline_mesh(&polygon, context);
                }
                handles.view_key = view_key;
            }
            if handles.fill_color_key != fill_color_key {
                if let Some(material) = materials.get_mut(&handles.fill_material) {
                    material.color = color_from_rgba(polygon.fill_rgba);
                }
                handles.fill_color_key = fill_color_key;
            }
            if handles.outline_color_key != outline_color_key {
                if let Some(material) = materials.get_mut(&handles.outline_material) {
                    material.color = color_from_rgba(polygon.outline_rgba);
                }
                handles.outline_color_key = outline_color_key;
            }
            if handles.map_z_bits != map_z_bits {
                let transform = Transform::from_xyz(0.0, 0.0, polygon.map_z);
                commands.entity(handles.fill_entity).insert(transform);
                commands.entity(handles.outline_entity).insert(transform);
                handles.map_z_bits = map_z_bits;
            }
            continue;
        }

        let fill_mesh = meshes.add(build_geo_polygon_map_fill_mesh(&polygon, context));
        let fill_material = materials.add(ColorMaterial::from(color_from_rgba(polygon.fill_rgba)));
        let outline_mesh = meshes.add(build_geo_polygon_map_outline_mesh(&polygon, context));
        let outline_material =
            materials.add(ColorMaterial::from(color_from_rgba(polygon.outline_rgba)));
        let transform = Transform::from_xyz(0.0, 0.0, polygon.map_z);
        let fill_entity = commands
            .spawn((
                Name::new("Geo Polygon Map Overlay"),
                Mesh2d(fill_mesh.clone()),
                MeshMaterial2d(fill_material.clone()),
                transform,
            ))
            .id();
        let outline_entity = commands
            .spawn((
                Name::new("Geo Polygon Map Overlay Outline"),
                Mesh2d(outline_mesh.clone()),
                MeshMaterial2d(outline_material.clone()),
                transform,
            ))
            .id();
        polygon_entities.map.insert(
            polygon_entity,
            GeoPolygonMapHandles {
                fill_entity,
                fill_mesh,
                fill_material,
                fill_color_key,
                outline_entity,
                outline_mesh,
                outline_material,
                outline_color_key,
                map_z_bits,
                view_key,
            },
        );
    }
}

/// Synchronizes geodetic polygons into the focused 3D globe surface.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Globe polygon sync needs the focused body, camera-derived radius, and assets."
)]
pub(crate) fn sync_geo_polygon_globe_overlays(
    mut commands: Commands<'_, '_>,
    celestial_focus: Res<'_, CelestialFocus>,
    globe_contexts: Query<'_, '_, &GlobeSurfaceRenderContext>,
    polygons: Query<'_, '_, GeoPolygonRenderQueryItem<'_>>,
    mut polygon_entities: ResMut<'_, GeoPolygonEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let live_polygons = polygons
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_geo_polygon_globe_overlays(&mut commands, &mut polygon_entities, &live_polygons);

    let Some(context) = globe_contexts
        .iter()
        .find(|context| context.target == celestial_focus.target)
    else {
        despawn_all_geo_polygon_globe_overlays(&mut commands, &mut polygon_entities);
        return;
    };

    let radius = context.radius;
    let parent = context.surface_anchor;
    for (polygon_entity, polygon, visibility, inherited_visibility) in &polygons {
        if !polygon.globe_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || polygon.target != celestial_focus.target
            || polygon.vertices.len() < 3
        {
            despawn_geo_polygon_globe_overlay(&mut commands, &mut polygon_entities, polygon_entity);
            continue;
        }

        let radius_bits = radius.to_bits();
        let offset_bits = polygon.globe_surface_offset_units.to_bits();
        let fill_color_key = GeoPolygonColorKey::from_rgba(polygon.fill_rgba);
        let outline_color_key = GeoPolygonColorKey::from_rgba(polygon.outline_rgba);
        let needs_new_mesh = polygon_entities
            .globe
            .get(&polygon_entity)
            .is_none_or(|handles| {
                handles.radius_bits != radius_bits || handles.offset_bits != offset_bits
            })
            || polygon.is_changed();

        if let Some(handles) = polygon_entities.globe.get_mut(&polygon_entity) {
            if handles.parent != parent {
                commands.entity(parent).add_child(handles.fill_entity);
                commands.entity(parent).add_child(handles.outline_entity);
                handles.parent = parent;
            }
            if needs_new_mesh {
                if let Some(mesh) = meshes.get_mut(&handles.fill_mesh) {
                    *mesh = build_geo_polygon_globe_fill_mesh(
                        &polygon,
                        radius,
                        polygon.globe_surface_offset_units,
                    );
                }
                if let Some(mesh) = meshes.get_mut(&handles.outline_mesh) {
                    *mesh = build_geo_polygon_globe_outline_mesh(
                        &polygon,
                        radius,
                        polygon.globe_surface_offset_units,
                    );
                }
                handles.radius_bits = radius_bits;
                handles.offset_bits = offset_bits;
            }
            if handles.fill_color_key != fill_color_key {
                if let Some(material) = materials.get_mut(&handles.fill_material) {
                    update_geo_polygon_globe_material(
                        material,
                        color_from_rgba(polygon.fill_rgba),
                        SphericalSurfaceLayer::DrapedFill,
                    );
                }
                handles.fill_color_key = fill_color_key;
            }
            if handles.outline_color_key != outline_color_key {
                if let Some(material) = materials.get_mut(&handles.outline_material) {
                    update_geo_polygon_globe_material(
                        material,
                        color_from_rgba(polygon.outline_rgba),
                        SphericalSurfaceLayer::DrapedOutline,
                    );
                }
                handles.outline_color_key = outline_color_key;
            }
            continue;
        }

        let fill_mesh = meshes.add(build_geo_polygon_globe_fill_mesh(
            &polygon,
            radius,
            polygon.globe_surface_offset_units,
        ));
        let outline_mesh = meshes.add(build_geo_polygon_globe_outline_mesh(
            &polygon,
            radius,
            polygon.globe_surface_offset_units,
        ));
        let fill_material = materials.add(geo_polygon_globe_material(
            color_from_rgba(polygon.fill_rgba),
            SphericalSurfaceLayer::DrapedFill,
        ));
        let outline_material = materials.add(geo_polygon_globe_material(
            color_from_rgba(polygon.outline_rgba),
            SphericalSurfaceLayer::DrapedOutline,
        ));
        let fill_entity = commands
            .spawn((
                Name::new("Geo Polygon Globe Overlay"),
                Mesh3d(fill_mesh.clone()),
                MeshMaterial3d(fill_material.clone()),
            ))
            .id();
        let outline_entity = commands
            .spawn((
                Name::new("Geo Polygon Globe Overlay Outline"),
                Mesh3d(outline_mesh.clone()),
                MeshMaterial3d(outline_material.clone()),
            ))
            .id();
        commands.entity(parent).add_child(fill_entity);
        commands.entity(parent).add_child(outline_entity);
        polygon_entities.globe.insert(
            polygon_entity,
            GeoPolygonGlobeHandles {
                fill_entity,
                fill_mesh,
                fill_material,
                fill_color_key,
                outline_entity,
                outline_mesh,
                outline_material,
                outline_color_key,
                parent,
                radius_bits,
                offset_bits,
            },
        );
    }
}

fn retain_live_geo_polygon_map_overlays(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
    live_polygons: &HashSet<Entity>,
) {
    polygon_entities.map.retain(|polygon, handles| {
        if live_polygons.contains(polygon) {
            true
        } else {
            commands.entity(handles.fill_entity).despawn();
            commands.entity(handles.outline_entity).despawn();
            false
        }
    });
}

fn retain_live_geo_polygon_globe_overlays(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
    live_polygons: &HashSet<Entity>,
) {
    polygon_entities.globe.retain(|polygon, handles| {
        if live_polygons.contains(polygon) {
            true
        } else {
            commands.entity(handles.fill_entity).despawn();
            commands.entity(handles.outline_entity).despawn();
            false
        }
    });
}

fn despawn_all_geo_polygon_map_overlays(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
) {
    for handles in polygon_entities.map.drain().map(|(_, handles)| handles) {
        commands.entity(handles.fill_entity).despawn();
        commands.entity(handles.outline_entity).despawn();
    }
}

fn despawn_all_geo_polygon_globe_overlays(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
) {
    for handles in polygon_entities.globe.drain().map(|(_, handles)| handles) {
        commands.entity(handles.fill_entity).despawn();
        commands.entity(handles.outline_entity).despawn();
    }
}

fn despawn_geo_polygon_map_overlay(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
    polygon_entity: Entity,
) {
    if let Some(handles) = polygon_entities.map.remove(&polygon_entity) {
        commands.entity(handles.fill_entity).despawn();
        commands.entity(handles.outline_entity).despawn();
    }
}

fn despawn_geo_polygon_globe_overlay(
    commands: &mut Commands<'_, '_>,
    polygon_entities: &mut GeoPolygonEntities,
    polygon_entity: Entity,
) {
    if let Some(handles) = polygon_entities.globe.remove(&polygon_entity) {
        commands.entity(handles.fill_entity).despawn();
        commands.entity(handles.outline_entity).despawn();
    }
}

fn geo_polygon_map_view_key(context: &Map2dRenderContext) -> GeoPolygonMapViewKey {
    crate::overlay_common::map_view_key(&context.view_state)
}

fn build_geo_polygon_map_fill_mesh(polygon: &GeoPolygon, context: &Map2dRenderContext) -> Mesh {
    let projected_vertices = project_geo_polygon_map_vertices(polygon, context);
    if projected_vertices.len() < 3 {
        return triangle_mesh(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    }

    let positions = projected_vertices
        .iter()
        .map(|projected| [projected.x, projected.y, 0.0])
        .collect::<Vec<_>>();
    let mut indices = Vec::new();
    append_triangle_fan_indices(&mut indices, 0, positions.len(), true);

    triangle_mesh(positions, Vec::new(), Vec::new(), indices)
}

fn build_geo_polygon_map_outline_mesh(polygon: &GeoPolygon, context: &Map2dRenderContext) -> Mesh {
    let vertices = project_geo_polygon_map_vertices(polygon, context);
    if vertices.len() < 2 {
        return line_mesh(Vec::new(), Vec::new(), Vec::new());
    }

    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for index in 0..vertices.len() {
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        append_line_segment(
            &mut positions,
            &mut indices,
            [current.x, current.y, 0.0],
            [next.x, next.y, 0.0],
        );
    }

    line_mesh(positions, Vec::new(), indices)
}

fn project_geo_polygon_map_vertices(
    polygon: &GeoPolygon,
    context: &Map2dRenderContext,
) -> Vec<Vec2> {
    let mut world_points = polygon
        .vertices
        .iter()
        .map(|vertex| context.lon_lat_to_world(vertex.lon_lat))
        .collect::<Vec<_>>();
    context.unwrap_periodic_x(&mut world_points);
    context.shift_world_copy_near_view(&mut world_points);

    world_points
        .into_iter()
        .map(|world| context.world_to_screen(world))
        .collect()
}

fn build_geo_polygon_globe_fill_mesh(
    polygon: &GeoPolygon,
    radius: f32,
    surface_offset_units: f32,
) -> Mesh {
    let directions = polygon
        .vertices
        .iter()
        .map(|vertex| globe_surface_direction(vertex.lon_lat))
        .collect::<Vec<_>>();
    if directions.len() < 3 {
        return triangle_mesh(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    }

    let radius = f64::from(draped_fill_radius(radius, surface_offset_units.max(0.0)));
    let center = polygon_center_direction(&directions);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    append_tessellated_spherical_polygon(
        &mut positions,
        &mut normals,
        &mut Vec::new(),
        &mut indices,
        center,
        &directions,
        radius,
        None,
    );

    triangle_mesh(positions, normals, Vec::new(), indices)
}

fn build_geo_polygon_globe_outline_mesh(
    polygon: &GeoPolygon,
    radius: f32,
    surface_offset_units: f32,
) -> Mesh {
    let directions = polygon
        .vertices
        .iter()
        .map(|vertex| globe_surface_direction(vertex.lon_lat))
        .collect::<Vec<_>>();
    if directions.len() < 2 {
        return line_mesh(Vec::new(), Vec::new(), Vec::new());
    }

    let radius = f64::from(draped_outline_radius(radius, surface_offset_units.max(0.0)));
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for index in 0..directions.len() {
        append_tessellated_spherical_outline_edge(
            &mut positions,
            &mut normals,
            &mut indices,
            directions[index],
            directions[(index + 1) % directions.len()],
            radius,
        );
    }

    line_mesh(positions, normals, indices)
}

fn geo_polygon_globe_material(color: Color, layer: SphericalSurfaceLayer) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    update_geo_polygon_globe_material(&mut material, color, layer);
    material
}

fn update_geo_polygon_globe_material(
    material: &mut StandardMaterial,
    color: Color,
    layer: SphericalSurfaceLayer,
) {
    apply_spherical_decal_material(material, color, layer);
}

fn normalized_color_component(component: f32) -> f32 {
    if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::{Indices, VertexAttributeValues};
    use bevy::prelude::{Mesh, Vec3, Visibility};
    use ferrisium_core::prelude::{LonLat, PlanetaryBody, TileProjection, Vec2d, ViewState};

    use crate::surface_depth::{draped_fill_radius, draped_outline_radius};
    use crate::view::Map2dRenderContext;

    use super::{
        build_geo_polygon_globe_fill_mesh, build_geo_polygon_globe_outline_mesh,
        build_geo_polygon_map_fill_mesh, build_geo_polygon_map_outline_mesh, GeoPolygon,
        GeoPolygonBundle, GeoPolygonVertex, DEFAULT_GEO_POLYGON_GLOBE_SURFACE_OFFSET_UNITS,
    };

    fn test_polygon() -> GeoPolygon {
        GeoPolygon::earth(
            [
                GeoPolygonVertex::from_degrees(-105.0, 35.0),
                GeoPolygonVertex::from_degrees(-95.0, 35.0),
                GeoPolygonVertex::from_degrees(-95.0, 42.0),
                GeoPolygonVertex::from_degrees(-105.0, 42.0),
            ],
            [0.2, 0.7, 0.9, 0.2],
            [0.2, 0.7, 0.9, 0.9],
        )
    }

    fn antimeridian_view_state() -> ViewState {
        ViewState {
            center: LonLat::new(0.0, 0.0),
            zoom: 2.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        }
    }

    fn map_context(view_state: ViewState) -> Map2dRenderContext {
        Map2dRenderContext::from_view_state(PlanetaryBody::earth(), view_state)
    }

    fn mesh_positions(mesh: &Mesh) -> &[[f32; 3]] {
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap_or(&[])
    }

    fn mesh_u32_indices(mesh: &Mesh) -> &[u32] {
        match mesh.indices() {
            Some(Indices::U32(indices)) => indices,
            _ => &[],
        }
    }

    fn assert_rgba_close(lhs: [f32; 4], rhs: [f32; 4]) {
        for (left, right) in lhs.into_iter().zip(rhs) {
            assert!((left - right).abs() <= f32::EPSILON);
        }
    }

    fn assert_triangle_x_spans_below(mesh: &Mesh, max_span: f32) {
        let positions = mesh_positions(mesh);
        for triangle in mesh_u32_indices(mesh).chunks_exact(3) {
            let xs = triangle
                .iter()
                .filter_map(|index| positions.get(*index as usize))
                .map(|position| position[0])
                .collect::<Vec<_>>();
            let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
            let max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(
                max - min <= max_span,
                "triangle crosses a large map span: {min}..{max}"
            );
        }
    }

    #[test]
    fn earth_polygon_defaults_to_earth_map_and_globe_targets() {
        let polygon = test_polygon();

        assert_eq!(polygon.target, ferrisium_core::space::BodyId::EARTH);
        assert_eq!(polygon.body, PlanetaryBody::earth());
        assert!(polygon.map_enabled);
        assert!(polygon.globe_enabled);
        assert!(
            (polygon.globe_surface_offset_units - DEFAULT_GEO_POLYGON_GLOBE_SURFACE_OFFSET_UNITS)
                .abs()
                <= f32::EPSILON
        );
        assert_eq!(
            GeoPolygonBundle::new(polygon).visibility,
            Visibility::Inherited
        );
    }

    #[test]
    fn polygon_color_components_are_normalized() {
        let polygon = GeoPolygon::earth(
            [
                GeoPolygonVertex::from_degrees(0.0, 0.0),
                GeoPolygonVertex::from_degrees(1.0, 0.0),
                GeoPolygonVertex::from_degrees(0.0, 1.0),
            ],
            [-1.0, 0.5, 2.0, f32::NAN],
            [f32::INFINITY, 0.25, 0.5, 0.75],
        );

        assert_rgba_close(polygon.fill_rgba, [0.0, 0.5, 1.0, 0.0]);
        assert_rgba_close(polygon.outline_rgba, [0.0, 0.25, 0.5, 0.75]);
    }

    #[test]
    fn map_fill_mesh_projects_polygon_vertices() {
        let mesh = build_geo_polygon_map_fill_mesh(
            &test_polygon(),
            &map_context(ViewState {
                center: LonLat::new(-100.0, 38.0),
                ..antimeridian_view_state()
            }),
        );

        assert_eq!(mesh_positions(&mesh).len(), 4);
        assert_eq!(mesh_u32_indices(&mesh).len(), 6);
    }

    #[test]
    fn map_outline_mesh_closes_the_polygon_ring() {
        let mesh = build_geo_polygon_map_outline_mesh(
            &test_polygon(),
            &map_context(ViewState {
                center: LonLat::new(-100.0, 38.0),
                ..antimeridian_view_state()
            }),
        );

        assert_eq!(mesh_positions(&mesh).len(), 8);
        assert_eq!(mesh_u32_indices(&mesh).len(), 8);
    }

    #[test]
    fn map_fill_keeps_transmeridian_polygon_local() {
        let polygon = GeoPolygon::earth(
            [
                GeoPolygonVertex::from_degrees(170.0, -5.0),
                GeoPolygonVertex::from_degrees(-170.0, -5.0),
                GeoPolygonVertex::from_degrees(-170.0, 5.0),
                GeoPolygonVertex::from_degrees(170.0, 5.0),
            ],
            [0.2, 0.7, 0.9, 0.2],
            [0.2, 0.7, 0.9, 0.9],
        );

        let mesh =
            build_geo_polygon_map_fill_mesh(&polygon, &map_context(antimeridian_view_state()));

        assert_triangle_x_spans_below(&mesh, 80.0);
    }

    #[test]
    fn globe_fill_mesh_drapes_polygon_on_requested_radius() {
        let radius = 10.0;
        let offset = 0.5;
        let mesh = build_geo_polygon_globe_fill_mesh(&test_polygon(), radius, offset);
        let positions = mesh_positions(&mesh);
        let expected_radius = draped_fill_radius(radius, offset);

        assert!(!positions.is_empty());
        for position in positions {
            let distance = Vec3::from_array(*position).length();
            assert!((distance - expected_radius).abs() < 0.01);
        }
    }

    #[test]
    fn globe_outline_mesh_uses_draped_outline_radius() {
        let radius = 10.0;
        let offset = 0.5;
        let mesh = build_geo_polygon_globe_outline_mesh(&test_polygon(), radius, offset);
        let positions = mesh_positions(&mesh);
        let expected_radius = draped_outline_radius(radius, offset);

        assert!(!positions.is_empty());
        for position in positions {
            let distance = Vec3::from_array(*position).length();
            assert!((distance - expected_radius).abs() < 0.01);
        }
    }
}
