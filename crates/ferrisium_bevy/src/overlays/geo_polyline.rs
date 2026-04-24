//! Geodetic polyline rendering for Ferrisium map and globe views.
//!
//! A `GeoPolyline` stores geographic endpoints once and Ferrisium renders it as
//! projected 2D line segments or as a great-circle drape on the focused globe.

use std::collections::{HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::coords::{LonLat, Vec2d};
use ferrisium_core::prelude::{BodyId, PlanetaryBody};

use crate::celestial::CelestialFocus;
use crate::overlay_common::{
    color_from_rgba, overlay_is_visible, OverlayColorKey, OverlayMapViewKey,
};
use crate::surface_context::GlobeSurfaceRenderContext;
use crate::surface_depth::{
    apply_spherical_decal_material, draped_outline_radius, SphericalSurfaceLayer,
};
use crate::view::{Map2dRenderContext, Map2dView};

const DEFAULT_GEO_POLYLINE_MAP_Z: f32 = 36.0;
const DEFAULT_GEO_POLYLINE_GLOBE_SURFACE_OFFSET_UNITS: f32 = 0.0;
const GEO_POLYLINE_MAX_SEGMENT_RADIANS: f64 = 0.02;
const GEO_POLYLINE_MAX_SEGMENTS: usize = 256;

/// One geographic endpoint for a [`GeoPolyline`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeoEndpoint {
    /// Longitude/latitude in degrees.
    pub lon_lat: LonLat,
    /// Additional render-only lift above the globe surface in render units.
    pub altitude_units: f32,
}

impl GeoEndpoint {
    /// Creates a surface endpoint with no extra globe altitude.
    #[must_use]
    pub fn surface(lon_lat: LonLat) -> Self {
        Self::with_altitude_units(lon_lat, 0.0)
    }

    /// Creates an endpoint with a non-negative render altitude above the globe.
    #[must_use]
    pub fn with_altitude_units(lon_lat: LonLat, altitude_units: f32) -> Self {
        Self {
            lon_lat,
            altitude_units: finite_non_negative(altitude_units),
        }
    }
}

/// Geodetic line string rendered over a map and/or globe surface.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GeoPolyline {
    /// Body center that receives the globe polyline.
    pub target: BodyId,
    /// Planetary map body that receives the 2D polyline.
    pub body: PlanetaryBody,
    /// Ordered geographic endpoints.
    pub endpoints: Vec<GeoEndpoint>,
    /// Constant non-premultiplied sRGBA line color.
    pub color_rgba: [f32; 4],
    /// Z depth used for 2D map line meshes above raster tiles.
    pub map_z: f32,
    /// Whether the polyline should render in 2D map views.
    pub map_enabled: bool,
    /// Whether the polyline should render on focused 3D globe surfaces.
    pub globe_enabled: bool,
    /// Minimum globe-surface visual lift in render units.
    pub globe_surface_offset_units: f32,
}

impl GeoPolyline {
    /// Creates an Earth polyline for both Earth map and Earth globe views.
    #[must_use]
    pub fn earth(endpoints: impl IntoIterator<Item = GeoEndpoint>, color_rgba: [f32; 4]) -> Self {
        Self::for_body(BodyId::EARTH, PlanetaryBody::earth(), endpoints, color_rgba)
    }

    /// Creates a polyline for a map body and a celestial target.
    #[must_use]
    pub fn for_body(
        target: BodyId,
        body: PlanetaryBody,
        endpoints: impl IntoIterator<Item = GeoEndpoint>,
        color_rgba: [f32; 4],
    ) -> Self {
        Self {
            target,
            body,
            endpoints: endpoints.into_iter().collect(),
            color_rgba: color_rgba.map(normalized_color_component),
            map_z: DEFAULT_GEO_POLYLINE_MAP_Z,
            map_enabled: true,
            globe_enabled: true,
            globe_surface_offset_units: DEFAULT_GEO_POLYLINE_GLOBE_SURFACE_OFFSET_UNITS,
        }
    }

    /// Sets the 2D map z depth.
    #[must_use]
    pub const fn with_map_z(mut self, map_z: f32) -> Self {
        self.map_z = map_z;
        self
    }

    /// Enables or disables this polyline in 2D map views.
    #[must_use]
    pub const fn with_map_enabled(mut self, enabled: bool) -> Self {
        self.map_enabled = enabled;
        self
    }

    /// Enables or disables this polyline on focused 3D globe surfaces.
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

/// Convenience bundle for spawning a geodetic polyline.
#[derive(Bundle)]
pub struct GeoPolylineBundle {
    /// Polyline content and style.
    pub polyline: GeoPolyline,
    /// Local transform for the marker entity.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Local visibility gate; inherited hierarchy visibility is also honored.
    pub visibility: Visibility,
}

impl GeoPolylineBundle {
    /// Creates a root-visible polyline bundle that inherits parent visibility.
    #[must_use]
    pub fn new(polyline: GeoPolyline) -> Self {
        Self {
            polyline,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Spawned render assets for map and globe polylines.
#[derive(Resource, Default)]
pub(crate) struct GeoPolylineEntities {
    map: HashMap<Entity, GeoPolylineMapHandles>,
    globe: HashMap<Entity, GeoPolylineGlobeHandles>,
}

#[derive(Debug, Clone)]
struct GeoPolylineMapHandles {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
    color_key: GeoPolylineColorKey,
    map_z_bits: u32,
    view_key: GeoPolylineMapViewKey,
}

#[derive(Debug, Clone)]
struct GeoPolylineGlobeHandles {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    color_key: GeoPolylineColorKey,
    parent: Entity,
    radius_bits: u32,
    offset_bits: u32,
}

type GeoPolylineColorKey = OverlayColorKey;
type GeoPolylineMapViewKey = OverlayMapViewKey;
type GeoPolylineRenderQueryItem<'a> = (
    Entity,
    Ref<'a, GeoPolyline>,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
);

/// Synchronizes geodetic polylines into the active 2D map view.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose window, view, source, polyline, and asset stores explicitly."
)]
pub(crate) fn sync_geo_polyline_map_overlays(
    mut commands: Commands<'_, '_>,
    map_contexts: Query<'_, '_, &Map2dRenderContext, With<Map2dView>>,
    polylines: Query<'_, '_, GeoPolylineRenderQueryItem<'_>>,
    mut polyline_entities: ResMut<'_, GeoPolylineEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<ColorMaterial>>,
) {
    let live_polylines = polylines
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_geo_polyline_map_overlays(&mut commands, &mut polyline_entities, &live_polylines);

    let Some(context) = map_contexts.iter().next() else {
        despawn_all_geo_polyline_map_overlays(&mut commands, &mut polyline_entities);
        return;
    };
    let view_key = geo_polyline_map_view_key(context);

    for (polyline_entity, polyline, visibility, inherited_visibility) in &polylines {
        if !polyline.map_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || polyline.body != context.body
            || polyline.endpoints.len() < 2
        {
            despawn_geo_polyline_map_overlay(
                &mut commands,
                &mut polyline_entities,
                polyline_entity,
            );
            continue;
        }

        let color_key = GeoPolylineColorKey::from_rgba(polyline.color_rgba);
        let map_z_bits = polyline.map_z.to_bits();
        if let Some(handles) = polyline_entities.map.get_mut(&polyline_entity) {
            if polyline.is_changed() || handles.view_key != view_key {
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    *mesh = build_geo_polyline_map_mesh(&polyline, context);
                }
                handles.view_key = view_key;
            }
            if handles.color_key != color_key {
                if let Some(material) = materials.get_mut(&handles.material) {
                    material.color = color_from_rgba(polyline.color_rgba);
                }
                handles.color_key = color_key;
            }
            if handles.map_z_bits != map_z_bits {
                commands.entity(handles.entity).insert(Transform::from_xyz(
                    0.0,
                    0.0,
                    polyline.map_z,
                ));
                handles.map_z_bits = map_z_bits;
            }
            continue;
        }

        let mesh = meshes.add(build_geo_polyline_map_mesh(&polyline, context));
        let material = materials.add(ColorMaterial::from(color_from_rgba(polyline.color_rgba)));
        let entity = commands
            .spawn((
                Name::new("Geo Polyline Map Overlay"),
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                Transform::from_xyz(0.0, 0.0, polyline.map_z),
            ))
            .id();
        polyline_entities.map.insert(
            polyline_entity,
            GeoPolylineMapHandles {
                entity,
                mesh,
                material,
                color_key,
                map_z_bits,
                view_key,
            },
        );
    }
}

/// Synchronizes geodetic polylines into the focused 3D globe surface.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Globe polyline sync needs the focused body, camera-derived radius, and assets."
)]
pub(crate) fn sync_geo_polyline_globe_overlays(
    mut commands: Commands<'_, '_>,
    celestial_focus: Res<'_, CelestialFocus>,
    globe_contexts: Query<'_, '_, &GlobeSurfaceRenderContext>,
    polylines: Query<'_, '_, GeoPolylineRenderQueryItem<'_>>,
    mut polyline_entities: ResMut<'_, GeoPolylineEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let live_polylines = polylines
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_geo_polyline_globe_overlays(&mut commands, &mut polyline_entities, &live_polylines);

    let Some(context) = globe_contexts
        .iter()
        .find(|context| context.target == celestial_focus.target)
    else {
        despawn_all_geo_polyline_globe_overlays(&mut commands, &mut polyline_entities);
        return;
    };

    let radius = context.radius;
    let parent = context.surface_anchor;
    for (polyline_entity, polyline, visibility, inherited_visibility) in &polylines {
        if !polyline.globe_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || polyline.target != celestial_focus.target
            || polyline.endpoints.len() < 2
        {
            despawn_geo_polyline_globe_overlay(
                &mut commands,
                &mut polyline_entities,
                polyline_entity,
            );
            continue;
        }

        let radius_bits = radius.to_bits();
        let offset_bits = polyline.globe_surface_offset_units.to_bits();
        let color_key = GeoPolylineColorKey::from_rgba(polyline.color_rgba);
        let needs_new_mesh = polyline_entities
            .globe
            .get(&polyline_entity)
            .is_none_or(|handles| {
                handles.radius_bits != radius_bits || handles.offset_bits != offset_bits
            })
            || polyline.is_changed();

        if let Some(handles) = polyline_entities.globe.get_mut(&polyline_entity) {
            if handles.parent != parent {
                commands.entity(parent).add_child(handles.entity);
                handles.parent = parent;
            }
            if needs_new_mesh {
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    *mesh = build_geo_polyline_globe_mesh(
                        &polyline,
                        radius,
                        polyline.globe_surface_offset_units,
                    );
                }
                handles.radius_bits = radius_bits;
                handles.offset_bits = offset_bits;
            }
            if handles.color_key != color_key {
                if let Some(material) = materials.get_mut(&handles.material) {
                    update_geo_polyline_globe_material(
                        material,
                        color_from_rgba(polyline.color_rgba),
                    );
                }
                handles.color_key = color_key;
            }
            continue;
        }

        let mesh = meshes.add(build_geo_polyline_globe_mesh(
            &polyline,
            radius,
            polyline.globe_surface_offset_units,
        ));
        let material = materials.add(geo_polyline_globe_material(color_from_rgba(
            polyline.color_rgba,
        )));
        let entity = commands
            .spawn((
                Name::new("Geo Polyline Globe Overlay"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        commands.entity(parent).add_child(entity);
        polyline_entities.globe.insert(
            polyline_entity,
            GeoPolylineGlobeHandles {
                entity,
                mesh,
                material,
                color_key,
                parent,
                radius_bits,
                offset_bits,
            },
        );
    }
}

fn retain_live_geo_polyline_map_overlays(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
    live_polylines: &HashSet<Entity>,
) {
    polyline_entities.map.retain(|polyline, handles| {
        if live_polylines.contains(polyline) {
            true
        } else {
            commands.entity(handles.entity).despawn();
            false
        }
    });
}

fn retain_live_geo_polyline_globe_overlays(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
    live_polylines: &HashSet<Entity>,
) {
    polyline_entities.globe.retain(|polyline, handles| {
        if live_polylines.contains(polyline) {
            true
        } else {
            commands.entity(handles.entity).despawn();
            false
        }
    });
}

fn despawn_all_geo_polyline_map_overlays(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
) {
    for entity in polyline_entities
        .map
        .drain()
        .map(|(_, handles)| handles.entity)
    {
        commands.entity(entity).despawn();
    }
}

fn despawn_all_geo_polyline_globe_overlays(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
) {
    for entity in polyline_entities
        .globe
        .drain()
        .map(|(_, handles)| handles.entity)
    {
        commands.entity(entity).despawn();
    }
}

fn despawn_geo_polyline_map_overlay(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
    polyline: Entity,
) {
    if let Some(handles) = polyline_entities.map.remove(&polyline) {
        commands.entity(handles.entity).despawn();
    }
}

fn despawn_geo_polyline_globe_overlay(
    commands: &mut Commands<'_, '_>,
    polyline_entities: &mut GeoPolylineEntities,
    polyline: Entity,
) {
    if let Some(handles) = polyline_entities.globe.remove(&polyline) {
        commands.entity(handles.entity).despawn();
    }
}

fn geo_polyline_map_view_key(context: &Map2dRenderContext) -> GeoPolylineMapViewKey {
    crate::overlay_common::map_view_key(&context.view_state)
}

fn build_geo_polyline_map_mesh(polyline: &GeoPolyline, context: &Map2dRenderContext) -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();

    append_geodesic_map_polyline(&mut positions, &mut indices, &polyline.endpoints, context);

    line_mesh(positions, Vec::new(), indices)
}

fn build_geo_polyline_globe_mesh(
    polyline: &GeoPolyline,
    radius: f32,
    surface_offset_units: f32,
) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for endpoints in polyline.endpoints.windows(2) {
        append_geodesic_globe_line(
            &mut positions,
            &mut normals,
            &mut indices,
            endpoints[0],
            endpoints[1],
            radius,
            surface_offset_units,
        );
    }

    line_mesh(positions, normals, indices)
}

fn append_geodesic_map_polyline(
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    endpoints: &[GeoEndpoint],
    context: &Map2dRenderContext,
) {
    let mut world_points = Vec::new();
    for segment in endpoints.windows(2) {
        append_geodesic_segment_world_points(
            &mut world_points,
            segment[0].lon_lat,
            segment[1].lon_lat,
            context,
        );
    }
    let line = context.project_world_line_to_screen(&world_points);

    for segment in line.windows(2) {
        append_line_segment(
            positions,
            indices,
            [segment[0].x, segment[0].y, 0.0],
            [segment[1].x, segment[1].y, 0.0],
        );
    }
}

fn append_geodesic_globe_line(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    start: GeoEndpoint,
    end: GeoEndpoint,
    radius: f32,
    surface_offset_units: f32,
) {
    let start_direction = globe_surface_direction(start.lon_lat);
    let end_direction = globe_surface_direction(end.lon_lat);
    let segments = globe_edge_segments(start_direction, end_direction);

    for index in 0..segments {
        let left_t = segment_fraction(index, segments);
        let right_t = segment_fraction(index + 1, segments);
        let left = slerp_direction(start_direction, end_direction, left_t);
        let right = slerp_direction(start_direction, end_direction, right_t);
        let left_radius = draped_outline_radius(
            radius,
            surface_offset_units.max(0.0)
                + lerp_altitude_units(start.altitude_units, end.altitude_units, left_t),
        );
        let right_radius = draped_outline_radius(
            radius,
            surface_offset_units.max(0.0)
                + lerp_altitude_units(start.altitude_units, end.altitude_units, right_t),
        );
        append_globe_line_segment(
            positions,
            normals,
            indices,
            left,
            right,
            f64::from(left_radius),
            f64::from(right_radius),
        );
    }
}

fn line_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if !normals.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn append_line_segment(
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: [f32; 3],
    right: [f32; 3],
) {
    let start = positions.len();
    positions.push(left);
    positions.push(right);

    let Ok(start) = u32::try_from(start) else {
        return;
    };
    indices.extend_from_slice(&[start, start + 1]);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Globe polyline coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
fn append_globe_line_segment(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: DVec3,
    right: DVec3,
    left_radius: f64,
    right_radius: f64,
) {
    let left_position = left.normalize_or_zero() * left_radius;
    let right_position = right.normalize_or_zero() * right_radius;
    append_line_segment(
        positions,
        indices,
        [
            left_position.x as f32,
            left_position.y as f32,
            left_position.z as f32,
        ],
        [
            right_position.x as f32,
            right_position.y as f32,
            right_position.z as f32,
        ],
    );
    push_normal(normals, left_position);
    push_normal(normals, right_position);
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "A finite angular span is converted into a bounded tessellation count."
)]
fn globe_edge_segments(left: DVec3, right: DVec3) -> usize {
    let angle = angular_distance(left, right);
    if !angle.is_finite() || angle <= 0.0 {
        return 1;
    }

    ((angle / GEO_POLYLINE_MAX_SEGMENT_RADIANS).ceil() as usize).clamp(1, GEO_POLYLINE_MAX_SEGMENTS)
}

fn angular_distance(left: DVec3, right: DVec3) -> f64 {
    let left = left.normalize_or_zero();
    let right = right.normalize_or_zero();
    left.dot(right).clamp(-1.0, 1.0).acos()
}

fn slerp_direction(left: DVec3, right: DVec3, t: f64) -> DVec3 {
    let left = left.normalize_or_zero();
    let right = right.normalize_or_zero();
    let dot = left.dot(right).clamp(-1.0, 1.0);
    if dot > 0.999_5 {
        return (left + (right - left) * t).normalize_or_zero();
    }

    let theta = dot.acos();
    let sin_theta = theta.sin();
    if sin_theta.abs() <= f64::EPSILON {
        return left;
    }

    let left_scale = ((1.0 - t) * theta).sin() / sin_theta;
    let right_scale = (t * theta).sin() / sin_theta;
    (left * left_scale + right * right_scale).normalize_or_zero()
}

#[allow(
    clippy::cast_precision_loss,
    reason = "Polyline tessellation segment counts are bounded."
)]
fn segment_fraction(index: usize, segments: usize) -> f64 {
    index as f64 / segments.max(1) as f64
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Endpoint altitude is renderer-facing f32; interpolation happens in f64 segment space."
)]
fn lerp_altitude_units(start: f32, end: f32, t: f64) -> f32 {
    f64::from(start).mul_add(1.0 - t, f64::from(end) * t) as f32
}

fn append_geodesic_segment_world_points(
    world_points: &mut Vec<Vec2d>,
    start: LonLat,
    end: LonLat,
    context: &Map2dRenderContext,
) {
    let start_direction = globe_surface_direction(start);
    let end_direction = globe_surface_direction(end);
    let segments = globe_edge_segments(start_direction, end_direction);

    for index in 0..=segments {
        if index == 0 && !world_points.is_empty() {
            continue;
        }
        let lon_lat = lon_lat_from_globe_direction(slerp_direction(
            start_direction,
            end_direction,
            segment_fraction(index, segments),
        ));
        world_points.push(context.lon_lat_to_world(lon_lat));
    }
}

fn globe_surface_position(lon_lat: LonLat, radius: f64) -> DVec3 {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = radius * lat_rad.cos();

    DVec3::new(
        horizontal_radius * lon_rad.sin(),
        radius * lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}

fn globe_surface_direction(lon_lat: LonLat) -> DVec3 {
    globe_surface_position(lon_lat, 1.0).normalize_or_zero()
}

fn lon_lat_from_globe_direction(direction: DVec3) -> LonLat {
    let normalized = direction.normalize_or_zero();
    LonLat::new(
        normalized.x.atan2(normalized.z).to_degrees(),
        normalized.y.clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy mesh normals are f32 while spherical geometry is computed in f64."
)]
fn push_normal(normals: &mut Vec<[f32; 3]>, position: DVec3) {
    let normal = position.normalize_or_zero();
    normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
}

fn geo_polyline_globe_material(color: Color) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    update_geo_polyline_globe_material(&mut material, color);
    material
}

fn update_geo_polyline_globe_material(material: &mut StandardMaterial, color: Color) {
    apply_spherical_decal_material(material, color, SphericalSurfaceLayer::DrapedOutline);
}

fn normalized_color_component(component: f32) -> f32 {
    if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;
    use bevy::prelude::{Mesh, Visibility};
    use ferrisium_core::prelude::{LonLat, PlanetaryBody, TileProjection, Vec2d, ViewState};

    use crate::surface_depth::draped_outline_radius;
    use crate::view::Map2dRenderContext;

    use super::{
        build_geo_polyline_globe_mesh, build_geo_polyline_map_mesh, geo_polyline_globe_material,
        geo_polyline_map_view_key, globe_edge_segments, globe_surface_direction, GeoEndpoint,
        GeoPolyline, GeoPolylineBundle, GEO_POLYLINE_MAX_SEGMENTS,
    };

    fn test_polyline() -> GeoPolyline {
        GeoPolyline::earth(
            [
                GeoEndpoint::surface(LonLat::new(-122.0, 37.0)),
                GeoEndpoint::surface(LonLat::new(-73.0, 40.0)),
            ],
            [0.2, 0.4, 0.8, 0.9],
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
            Some(bevy::mesh::Indices::U32(indices)) => indices,
            _ => &[],
        }
    }

    fn assert_line_x_spans_below(mesh: &Mesh, max_span: f32) {
        let positions = mesh_positions(mesh);
        for segment in mesh_u32_indices(mesh).chunks_exact(2) {
            let left = usize::try_from(segment[0])
                .ok()
                .and_then(|index| positions.get(index));
            let right = usize::try_from(segment[1])
                .ok()
                .and_then(|index| positions.get(index));
            assert!(left.is_some(), "line index missing left position");
            assert!(right.is_some(), "line index missing right position");
            let (Some(left), Some(right)) = (left, right) else {
                continue;
            };
            let span = (left[0] - right[0]).abs();
            assert!(
                span <= max_span,
                "line should not cross the map wrap, got span {span} from {left:?} to {right:?}"
            );
        }
    }

    fn mesh_x_span(mesh: &Mesh) -> f32 {
        let positions = mesh_positions(mesh);
        assert!(!positions.is_empty(), "mesh should have positions");

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        for position in positions {
            min_x = min_x.min(position[0]);
            max_x = max_x.max(position[0]);
        }
        max_x - min_x
    }

    fn expected_segment_count(start: GeoEndpoint, end: GeoEndpoint) -> usize {
        globe_edge_segments(
            globe_surface_direction(start.lon_lat),
            globe_surface_direction(end.lon_lat),
        )
    }

    fn expected_polyline_segment_count(endpoints: &[GeoEndpoint]) -> usize {
        endpoints
            .windows(2)
            .map(|segment| expected_segment_count(segment[0], segment[1]))
            .sum()
    }

    fn assert_rgba_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "rgba mismatch: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn earth_polyline_defaults_to_earth_map_and_globe_targets() {
        let polyline = test_polyline();

        assert_eq!(polyline.target, ferrisium_core::prelude::BodyId::EARTH);
        assert_eq!(polyline.body, PlanetaryBody::earth());
        assert!(polyline.map_enabled);
        assert!(polyline.globe_enabled);
        assert_eq!(polyline.endpoints.len(), 2);
        assert_eq!(
            GeoPolylineBundle::new(polyline).visibility,
            Visibility::Inherited
        );
    }

    #[test]
    fn polyline_color_and_endpoint_altitude_are_normalized() {
        let endpoint = GeoEndpoint::with_altitude_units(LonLat::new(0.0, 0.0), f32::NAN);
        let polyline = GeoPolyline::earth([endpoint, endpoint], [-1.0, 0.5, 2.0, f32::NAN]);

        assert!(endpoint.altitude_units.abs() <= f32::EPSILON);
        assert_rgba_close(polyline.color_rgba, [0.0, 0.5, 1.0, 0.0]);
    }

    #[test]
    fn map_mesh_projects_geodetic_line_segments() {
        let view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let mesh = build_geo_polyline_map_mesh(&test_polyline(), &map_context(view_state));

        assert!(mesh.indices().map_or(0, bevy::mesh::Indices::len) >= 2);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
    }

    #[test]
    fn map_mesh_keeps_split_antimeridian_polyline_segments_local() {
        let start = GeoEndpoint::surface(LonLat::new(179.5, 1.0));
        let end = GeoEndpoint::surface(LonLat::new(180.0, 1.0));
        let polyline = GeoPolyline::earth([start, end], [1.0, 1.0, 1.0, 1.0]);
        let mesh = build_geo_polyline_map_mesh(&polyline, &map_context(antimeridian_view_state()));

        assert_eq!(
            mesh_u32_indices(&mesh).len(),
            expected_segment_count(start, end) * 2
        );
        assert_line_x_spans_below(&mesh, 8.0);
    }

    #[test]
    fn map_mesh_draws_unsplit_transmeridian_polyline_without_gap() {
        let start = GeoEndpoint::surface(LonLat::new(179.5, 1.0));
        let end = GeoEndpoint::surface(LonLat::new(-179.5, 1.0));
        let polyline = GeoPolyline::earth([start, end], [1.0, 1.0, 1.0, 1.0]);
        let mesh = build_geo_polyline_map_mesh(&polyline, &map_context(antimeridian_view_state()));

        assert_eq!(
            mesh_u32_indices(&mesh).len(),
            expected_segment_count(start, end) * 2
        );
        assert_line_x_spans_below(&mesh, 8.0);
    }

    #[test]
    fn map_mesh_keeps_multisegment_transmeridian_polyline_on_one_world_copy() {
        let endpoints = [
            GeoEndpoint::surface(LonLat::new(170.0, 1.0)),
            GeoEndpoint::surface(LonLat::new(-170.0, 1.0)),
            GeoEndpoint::surface(LonLat::new(-160.0, 1.0)),
        ];
        let polyline = GeoPolyline::earth(endpoints, [1.0, 1.0, 1.0, 1.0]);
        let mesh = build_geo_polyline_map_mesh(&polyline, &map_context(antimeridian_view_state()));

        assert_eq!(
            mesh_u32_indices(&mesh).len(),
            expected_polyline_segment_count(&endpoints) * 2
        );
        assert_line_x_spans_below(&mesh, 128.0);
        assert!(
            mesh_x_span(&mesh) < 128.0,
            "entire polyline should stay on one world copy"
        );
    }

    #[test]
    fn map_view_key_changes_with_projected_view_inputs() {
        let mut view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let initial_key = geo_polyline_map_view_key(&map_context(view_state));

        view_state.zoom = 4.0;
        assert_ne!(
            geo_polyline_map_view_key(&map_context(view_state)),
            initial_key
        );
    }

    #[test]
    fn globe_mesh_drapes_polyline_on_requested_radius() {
        let radius = 10.0;
        let offset = 0.25;
        let mesh = build_geo_polyline_globe_mesh(&test_polyline(), radius, offset);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        assert!(positions.is_some());
        let Some(positions) = positions else {
            return;
        };

        assert!(!positions.is_empty());
        let expected_radius = draped_outline_radius(radius, offset);
        for position in positions {
            let length = f32::hypot(position[0], f32::hypot(position[1], position[2]));
            assert!((length - expected_radius).abs() <= 1.0e-4);
        }
    }

    #[test]
    fn globe_mesh_caps_geodetic_tessellation() {
        let polyline = GeoPolyline::earth(
            [
                GeoEndpoint::surface(LonLat::new(0.0, 0.0)),
                GeoEndpoint::surface(LonLat::new(179.0, 0.0)),
            ],
            [1.0, 1.0, 1.0, 1.0],
        );
        let mesh = build_geo_polyline_globe_mesh(&polyline, 10.0, 0.0);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        let Some(positions) = positions else {
            return;
        };

        assert!(positions.len() <= GEO_POLYLINE_MAX_SEGMENTS * 2);
    }

    #[test]
    fn globe_material_uses_draped_outline_depth_bias() {
        let material =
            geo_polyline_globe_material(bevy::prelude::Color::srgba(1.0, 1.0, 1.0, 0.75));

        assert_eq!(material.alpha_mode, bevy::prelude::AlphaMode::Blend);
        assert!(material.unlit);
        assert!(material.cull_mode.is_none());
    }
}
