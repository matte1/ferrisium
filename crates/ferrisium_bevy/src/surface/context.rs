//! Public coordinate context for custom focused-globe surface drawing.

use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::prelude::{BodyId, LonLat, PlanetaryBody, TileProjection};

use crate::celestial::CelestialFocus;
use crate::globe::{Globe3dState, Globe3dView};
use crate::source::ActiveTileSource;
use crate::surface_depth::{
    draped_fill_radius, draped_outline_radius, spherical_surface_layer_radius,
    SphericalSurfaceLayer,
};
use crate::tile_globe::{
    focused_globe_surface_anchor, focused_globe_tile_view, DynamicBodySurfaceView,
    GlobeSurfaceAnchorQuery, PrimaryGlobeSurfaceAnchorQuery,
};

/// Stable invalidation key for geometry projected onto the focused globe surface.
///
/// The key intentionally excludes [`GlobeSurfaceRenderContext::body_rotation`]:
/// body-local meshes should usually be parented to
/// [`GlobeSurfaceRenderContext::surface_anchor`], letting the anchor transform
/// carry surface rotation without forcing static geometry rebuilds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobeSurfaceRenderKey {
    /// Focused body or barycenter receiving body-local surface geometry.
    pub target: BodyId,
    /// Planetary raster body associated with the active tile source.
    pub source_body: PlanetaryBody,
    /// Active raster projection.
    pub projection: TileProjection,
    /// Entity that owns body-local surface geometry.
    pub surface_anchor: Entity,
    radius_bits: u32,
}

impl GlobeSurfaceRenderKey {
    fn new(
        target: BodyId,
        source_body: PlanetaryBody,
        projection: TileProjection,
        surface_anchor: Entity,
        radius: f32,
    ) -> Self {
        Self {
            target,
            source_body,
            projection,
            surface_anchor,
            radius_bits: radius.to_bits(),
        }
    }
}

/// Local tangent frame on a focused globe surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceFrame {
    /// Body-local position at the requested surface radius.
    pub origin: Vec3,
    /// Unit outward surface normal.
    pub normal: Vec3,
    /// Unit tangent pointing toward increasing longitude.
    pub east: Vec3,
    /// Unit tangent pointing toward increasing latitude.
    pub north: Vec3,
}

/// Renderer-facing coordinate context for custom focused-globe surface drawing.
///
/// Ferrisium keeps this component updated on the current focused surface anchor
/// entity. App systems can query it to convert lon/lat into body-local surface
/// positions, derive tangent frames, choose shared draped-layer radii, and
/// parent custom meshes to the correct `big_space` body anchor.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GlobeSurfaceRenderContext {
    /// Focused body or barycenter receiving body-local surface geometry.
    pub target: BodyId,
    /// Planetary raster body associated with the active tile source.
    pub source_body: PlanetaryBody,
    /// Active raster projection.
    pub projection: TileProjection,
    /// Base focused-body radius in render units.
    pub radius: f32,
    /// Entity that owns body-local surface geometry.
    pub surface_anchor: Entity,
    /// Body-fixed rotation applied to the surface anchor.
    pub body_rotation: Quat,
    /// Stable invalidation key for cached surface geometry.
    pub key: GlobeSurfaceRenderKey,
}

impl GlobeSurfaceRenderContext {
    /// Creates a focused-globe surface context.
    #[must_use]
    pub fn new(
        target: BodyId,
        source_body: PlanetaryBody,
        projection: TileProjection,
        radius: f32,
        surface_anchor: Entity,
        body_rotation: Quat,
    ) -> Self {
        let key = GlobeSurfaceRenderKey::new(
            target,
            source_body.clone(),
            projection,
            surface_anchor,
            radius,
        );
        Self {
            target,
            source_body,
            projection,
            radius,
            surface_anchor,
            body_rotation,
            key,
        }
    }

    /// Returns the shared radius for a spherical surface layer.
    #[must_use]
    pub fn surface_radius(&self, layer: SphericalSurfaceLayer) -> f32 {
        spherical_surface_layer_radius(self.radius, layer)
    }

    /// Returns the shared draped-fill radius, honoring a user-requested minimum lift.
    #[must_use]
    pub fn draped_fill_radius(&self, requested_offset_units: f32) -> f32 {
        draped_fill_radius(self.radius, requested_offset_units)
    }

    /// Returns the shared draped-outline radius, honoring a user-requested minimum lift.
    #[must_use]
    pub fn draped_outline_radius(&self, requested_offset_units: f32) -> f32 {
        draped_outline_radius(self.radius, requested_offset_units)
    }

    /// Converts lon/lat into Ferrisium's body-local globe direction.
    #[must_use]
    pub fn lon_lat_to_direction(&self, lon_lat: LonLat) -> DVec3 {
        globe_surface_position(lon_lat, 1.0).normalize_or_zero()
    }

    /// Converts lon/lat into a body-local position at `radius_units`.
    #[must_use]
    pub fn lon_lat_to_body_position(&self, lon_lat: LonLat, radius_units: f64) -> DVec3 {
        globe_surface_position(lon_lat, radius_units)
    }

    /// Converts lon/lat into a body-local Bevy position at `radius`.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "Renderer-facing surface positions enter Bevy as f32."
    )]
    pub fn lon_lat_to_surface_position(&self, lon_lat: LonLat, radius: f32) -> Vec3 {
        let position = self.lon_lat_to_body_position(lon_lat, f64::from(radius));
        Vec3::new(position.x as f32, position.y as f32, position.z as f32)
    }

    /// Returns a local tangent frame at `lon_lat` and `radius`.
    #[must_use]
    pub fn surface_frame(&self, lon_lat: LonLat, radius: f32) -> SurfaceFrame {
        let origin = self.lon_lat_to_surface_position(lon_lat, radius);
        let normal = origin.normalize_or_zero();
        let lon_rad = lon_lat.lon_deg.to_radians();
        let east =
            dvec3_to_vec3(DVec3::new(lon_rad.cos(), 0.0, -lon_rad.sin())).normalize_or_zero();
        let north = normal.cross(east).normalize_or_zero();

        SurfaceFrame {
            origin,
            normal,
            east,
            north,
        }
    }

    /// Tessellates lon/lat endpoints into a body-local geodesic line string.
    #[must_use]
    pub fn project_geodesic_line_string(
        &self,
        points: &[LonLat],
        radius: f32,
        max_segment_radians: f64,
    ) -> Vec<Vec3> {
        let mut positions = Vec::new();
        for segment in points.windows(2) {
            append_geodesic_positions(
                &mut positions,
                segment[0],
                segment[1],
                f64::from(radius),
                max_segment_radians,
            );
        }
        positions
    }

    /// Builds a simple Bevy line-list mesh from body-local positions.
    ///
    /// This is a convenience for custom surface overlays. More advanced users
    /// can ignore it and build their own meshes or instanced render data.
    #[must_use]
    pub fn line_list_mesh(points: &[Vec3]) -> Mesh {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for segment in points.windows(2) {
            let start = positions.len();
            positions.push(segment[0].to_array());
            positions.push(segment[1].to_array());
            normals.push(segment[0].normalize_or_zero().to_array());
            normals.push(segment[1].normalize_or_zero().to_array());
            let Ok(start) = u32::try_from(start) else {
                continue;
            };
            indices.extend_from_slice(&[start, start + 1]);
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::LineList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose focus, active source, views, anchors, and stale contexts explicitly."
)]
pub(crate) fn sync_globe_surface_render_contexts(
    mut commands: Commands<'_, '_>,
    active_source: Res<'_, ActiveTileSource>,
    celestial_focus: Res<'_, CelestialFocus>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    dynamic_surface_views: Query<'_, '_, &DynamicBodySurfaceView>,
    metric_body_anchors: GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
    contexts: Query<'_, '_, Entity, With<GlobeSurfaceRenderContext>>,
) {
    let context = focused_globe_surface_anchor(
        *celestial_focus,
        &metric_body_anchors,
        &primary_body_anchors,
    )
    .and_then(|anchor| {
        focused_globe_tile_view(
            *celestial_focus,
            &globe_views,
            &dynamic_surface_views,
            anchor.rotation(),
        )
        .map(|tile_view| {
            GlobeSurfaceRenderContext::new(
                celestial_focus.target,
                active_source.0.body.clone(),
                active_source.0.projection,
                tile_view.radius,
                anchor.entity(),
                anchor.rotation(),
            )
        })
    });

    let active_entity = context.as_ref().map(|context| context.surface_anchor);
    for entity in &contexts {
        if Some(entity) != active_entity {
            commands
                .entity(entity)
                .remove::<GlobeSurfaceRenderContext>();
        }
    }
    if let Some(context) = context {
        commands.entity(context.surface_anchor).insert(context);
    }
}

fn append_geodesic_positions(
    positions: &mut Vec<Vec3>,
    start: LonLat,
    end: LonLat,
    radius: f64,
    max_segment_radians: f64,
) {
    let start_direction = globe_surface_position(start, 1.0).normalize_or_zero();
    let end_direction = globe_surface_position(end, 1.0).normalize_or_zero();
    let segments = geodesic_segments(start_direction, end_direction, max_segment_radians);

    for index in 0..=segments {
        if index == 0 && !positions.is_empty() {
            continue;
        }
        let direction = slerp_direction(
            start_direction,
            end_direction,
            normalized_segment_index(index, segments.max(1)),
        );
        positions.push(dvec3_to_vec3(direction.normalize_or_zero() * radius));
    }
}

fn normalized_segment_index(index: usize, denominator: usize) -> f64 {
    let Ok(index) = u32::try_from(index) else {
        return 1.0;
    };
    let Ok(denominator) = u32::try_from(denominator) else {
        return 1.0;
    };
    f64::from(index) / f64::from(denominator)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Renderer-facing surface positions enter Bevy as f32."
)]
fn dvec3_to_vec3(value: DVec3) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Finite angular spans are converted into bounded tessellation counts."
)]
fn geodesic_segments(left: DVec3, right: DVec3, max_segment_radians: f64) -> usize {
    let max_segment_radians = max_segment_radians.max(f64::EPSILON);
    let angle = angular_distance(left, right);
    if !angle.is_finite() || angle <= 0.0 {
        return 1;
    }

    ((angle / max_segment_radians).ceil() as usize).clamp(1, 1024)
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

#[cfg(test)]
mod tests {
    use ferrisium_core::prelude::{BodyId, LonLat, PlanetaryBody, TileProjection};

    use super::{GlobeSurfaceRenderContext, SphericalSurfaceLayer};

    #[test]
    fn globe_surface_context_projects_cardinal_lon_lat_axes() {
        let context = GlobeSurfaceRenderContext::new(
            BodyId::EARTH,
            PlanetaryBody::earth(),
            TileProjection::WebMercator,
            10.0,
            bevy::prelude::Entity::PLACEHOLDER,
            bevy::prelude::Quat::IDENTITY,
        );

        let equator_origin = context.lon_lat_to_surface_position(LonLat::new(0.0, 0.0), 10.0);
        let east = context.lon_lat_to_surface_position(LonLat::new(90.0, 0.0), 10.0);
        let north = context.lon_lat_to_surface_position(LonLat::new(0.0, 90.0), 10.0);

        assert!((equator_origin.z - 10.0).abs() <= f32::EPSILON);
        assert!((east.x - 10.0).abs() <= f32::EPSILON);
        assert!((north.y - 10.0).abs() <= f32::EPSILON);
        assert!(context.surface_radius(SphericalSurfaceLayer::DrapedOutline) > context.radius);
    }

    #[test]
    fn globe_surface_render_key_ignores_anchor_rotation() {
        let initial = GlobeSurfaceRenderContext::new(
            BodyId::EARTH,
            PlanetaryBody::earth(),
            TileProjection::WebMercator,
            10.0,
            bevy::prelude::Entity::PLACEHOLDER,
            bevy::prelude::Quat::IDENTITY,
        );
        let rotated = GlobeSurfaceRenderContext::new(
            BodyId::EARTH,
            PlanetaryBody::earth(),
            TileProjection::WebMercator,
            10.0,
            bevy::prelude::Entity::PLACEHOLDER,
            bevy::prelude::Quat::from_rotation_y(1.0),
        );

        assert_eq!(initial.key, rotated.key);
        assert_ne!(initial.body_rotation, rotated.body_rotation);
    }
}
