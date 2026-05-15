//! Globe-specific tile rendering and selection adapters.
//!
//! The shared fetch/cache state lives in `crate::tiles`. This module owns the
//! 3D-specific pieces layered on top of that state:
//!
//! - adapting `Globe3dState` into the pure `ferrisium_core::globe` selector
//! - creating curved sphere patch meshes from XYZ tile samples
//! - binding ready tile images to lit globe materials

use std::collections::HashMap;

use bevy::prelude::*;
use ferrisium_core::prelude::TileId;
use ferrisium_core::space::BodyId;

use crate::celestial::{CelestialBodyPlacement, CelestialFocus, PrimaryCelestialBody};
use crate::globe::{Globe3dState, Globe3dView};
use crate::metric_scene::MetricSceneObject;
use crate::source::{supports_focused_globe_renderer, ActiveTileSource};
use crate::surface_depth::{spherical_surface_layer_radius, SphericalSurfaceLayer};
use crate::tile_globe::mesh::update_globe_tile_mesh_uvs;
use crate::tile_globe::polar::{
    despawn_globe_polar_caps, sync_globe_polar_caps, GlobePolarCapEntities,
};
use crate::tiles::{tile_texture_binding, TilePipelineState};

mod material;
mod mesh;
mod polar;
mod selection;

pub(crate) use material::{build_globe_tile_material, update_globe_tile_material};
pub(crate) use mesh::{build_globe_tile_mesh, globe_patch_segments};
#[allow(
    unused_imports,
    reason = "Legacy internal wrappers remain available while call sites move to camera-agnostic tile views."
)]
pub(crate) use selection::{
    globe_request_tiles, globe_visible_tiles, sort_tiles_by_globe_priority,
};
pub(crate) use selection::{
    globe_request_tiles_for_view, globe_visible_tiles_for_view,
    sort_tiles_by_globe_priority_for_view, GlobeTileView,
};

#[cfg(test)]
use selection::globe_camera_surface_direction;

#[derive(Component)]
struct GlobeTilePatch;

/// Camera-derived state for rendering dynamic raster tiles on the focused body.
///
/// This component lets overview cameras, such as a solar-system orbit camera,
/// drive the same camera-responsive tile selector used by [`Globe3dState`]
/// without opting into close-globe mouse controls. The body id must match the
/// active [`CelestialFocus`] and the active tile source's planetary body before
/// the dynamic renderer uses it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DynamicBodySurfaceView {
    /// Focused body whose surface should receive dynamic raster patches.
    pub target: BodyId,
    /// Whether this component currently represents an active focused body.
    pub enabled: bool,
    /// Physical body radius in render units.
    pub radius: f32,
    /// Surface height where `tile_zoom` is the base detail level.
    pub base_surface_distance: f64,
    /// Current camera height above the focused surface.
    pub current_surface_distance: f64,
    /// Camera direction in the body's local surface frame.
    pub camera_surface_direction: Vec3,
    /// Base tile zoom used at `base_surface_distance`.
    pub tile_zoom: u8,
    /// Maximum selected tile patches for this surface.
    pub max_selected_tiles: usize,
}

impl DynamicBodySurfaceView {
    /// Creates a disabled dynamic surface view placeholder.
    #[must_use]
    pub const fn disabled(target: BodyId) -> Self {
        Self {
            target,
            enabled: false,
            radius: 1.0,
            base_surface_distance: 1.0,
            current_surface_distance: 1.0,
            camera_surface_direction: Vec3::Z,
            tile_zoom: 0,
            max_selected_tiles: 0,
        }
    }

    /// Creates an enabled dynamic surface view from camera-relative surface state.
    #[must_use]
    pub fn enabled(
        target: BodyId,
        radius: f32,
        base_surface_distance: f64,
        current_surface_distance: f64,
        camera_surface_direction: Vec3,
        tile_zoom: u8,
        max_selected_tiles: usize,
    ) -> Self {
        Self {
            target,
            enabled: true,
            radius,
            base_surface_distance: base_surface_distance.max(0.0),
            current_surface_distance: current_surface_distance.max(0.0),
            camera_surface_direction: camera_surface_direction.normalize_or_zero(),
            tile_zoom,
            max_selected_tiles,
        }
    }

    fn tile_view(self) -> Option<GlobeTileView> {
        self.enabled.then(|| {
            GlobeTileView::from_dynamic_surface(
                self.radius,
                self.base_surface_distance,
                self.current_surface_distance,
                self.camera_surface_direction,
                self.tile_zoom,
                self.max_selected_tiles,
            )
        })
    }
}

impl Default for DynamicBodySurfaceView {
    fn default() -> Self {
        Self::disabled(BodyId::EARTH)
    }
}

pub(crate) type GlobeSurfaceAnchorQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static MetricSceneObject, &'static Transform),
    With<CelestialBodyPlacement>,
>;

pub(crate) type PrimaryGlobeSurfaceAnchorQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), With<PrimaryCelestialBody>>;

/// Render parent and body-fixed rotation used by primary globe patches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GlobeSurfaceAnchor {
    entity: Entity,
    rotation: Quat,
}

impl GlobeSurfaceAnchor {
    /// Entity that owns body-local surface overlays and raster patches.
    #[must_use]
    pub(crate) const fn entity(self) -> Entity {
        self.entity
    }

    /// Body-fixed rotation for camera-facing tile selection.
    #[must_use]
    pub(crate) const fn rotation(self) -> Quat {
        self.rotation
    }
}

/// Tracks the spawned Bevy entities and materials for visible globe tiles.
#[derive(Resource, Default)]
pub(crate) struct GlobeTileEntities {
    by_tile: HashMap<TileId, GlobeTileAssetHandles>,
    polar_caps: Option<GlobePolarCapEntities>,
    parent: Option<Entity>,
}

#[derive(Debug, Clone)]
struct GlobeTileAssetHandles {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    uv_rect: Rect,
    geometry_key: GlobeTileGeometryKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlobeTileGeometryKey {
    projection: ferrisium_core::source::TileProjection,
    radius_bits: u32,
}

impl GlobeTileGeometryKey {
    fn new(projection: ferrisium_core::source::TileProjection, radius: f32) -> Self {
        Self {
            projection,
            radius_bits: radius.to_bits(),
        }
    }

    fn radius(self) -> f32 {
        f32::from_bits(self.radius_bits)
    }
}

/// Synchronizes the currently visible globe patch entities against the shared
/// tile cache and request state.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Bevy systems expose each ECS input explicitly and this adapter owns the patch lifecycle in one place."
)]
pub(crate) fn sync_globe_tile_patches(
    mut commands: Commands<'_, '_>,
    active_source: Res<'_, ActiveTileSource>,
    celestial_focus: Res<'_, CelestialFocus>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    dynamic_surface_views: Query<'_, '_, &DynamicBodySurfaceView>,
    metric_body_anchors: GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
    tile_pipeline: Res<'_, TilePipelineState>,
    mut globe_tile_entities: ResMut<'_, GlobeTileEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    if !supports_focused_globe_renderer(&active_source.0, celestial_focus.target)
        || !globe_views.iter().next().map_or(true, |s| s.render_enabled)
    {
        despawn_all_globe_tiles(&mut commands, &mut globe_tile_entities);
        return;
    }

    let Some(surface_anchor) = focused_globe_surface_anchor(
        *celestial_focus,
        &metric_body_anchors,
        &primary_body_anchors,
    ) else {
        despawn_all_globe_tiles(&mut commands, &mut globe_tile_entities);
        return;
    };
    let Some(tile_view) = focused_globe_tile_view(
        *celestial_focus,
        &globe_views,
        &dynamic_surface_views,
        surface_anchor.rotation,
    ) else {
        despawn_all_globe_tiles(&mut commands, &mut globe_tile_entities);
        return;
    };
    if globe_tile_entities.parent != Some(surface_anchor.entity) {
        despawn_all_globe_tiles(&mut commands, &mut globe_tile_entities);
        globe_tile_entities.parent = Some(surface_anchor.entity);
    }

    let raster_radius =
        spherical_surface_layer_radius(tile_view.radius, SphericalSurfaceLayer::RasterSurface);
    let polar_cap_radius =
        spherical_surface_layer_radius(tile_view.radius, SphericalSurfaceLayer::PolarCap);

    sync_globe_polar_caps(
        &mut commands,
        &mut globe_tile_entities,
        &mut meshes,
        &mut materials,
        surface_anchor.entity,
        polar_cap_radius,
        active_source.0.projection,
    );

    let desired_tiles = globe_visible_tiles_for_view(tile_view, &active_source.0);
    let stale_tiles = globe_tile_entities
        .by_tile
        .iter()
        .filter(|(tile, _)| !desired_tiles.contains(tile))
        .map(|(tile, handles)| (*tile, handles.entity))
        .collect::<Vec<_>>();

    for (tile, entity) in stale_tiles {
        commands.entity(entity).despawn();
        globe_tile_entities.by_tile.remove(&tile);
    }

    let geometry_key = GlobeTileGeometryKey::new(active_source.0.projection, raster_radius);
    for tile in desired_tiles {
        if let Some(handles) = globe_tile_entities.by_tile.get_mut(&tile) {
            let binding = tile_texture_binding(tile, &tile_pipeline);
            if let Some(material) = materials.get_mut(&handles.material) {
                update_globe_tile_material(material, tile, &binding);
            }
            if handles.geometry_key != geometry_key {
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    *mesh = build_globe_tile_mesh(
                        tile,
                        geometry_key.projection,
                        geometry_key.radius(),
                        globe_patch_segments(tile),
                        binding.uv_rect,
                    );
                    handles.geometry_key = geometry_key;
                    handles.uv_rect = binding.uv_rect;
                }
            } else if !same_uv_rect(handles.uv_rect, binding.uv_rect) {
                if let Some(mesh) = meshes.get_mut(&handles.mesh) {
                    update_globe_tile_mesh_uvs(mesh, globe_patch_segments(tile), binding.uv_rect);
                    handles.uv_rect = binding.uv_rect;
                }
            }
            continue;
        }

        let binding = tile_texture_binding(tile, &tile_pipeline);
        let mesh = meshes.add(build_globe_tile_mesh(
            tile,
            active_source.0.projection,
            raster_radius,
            globe_patch_segments(tile),
            binding.uv_rect,
        ));
        let material = materials.add(build_globe_tile_material(tile, &binding));
        let entity = commands
            .spawn((
                Name::new(format!("Globe Tile {tile:?}")),
                GlobeTilePatch,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        parent_to_globe_surface_anchor(&mut commands, surface_anchor.entity, entity);
        globe_tile_entities.by_tile.insert(
            tile,
            GlobeTileAssetHandles {
                entity,
                mesh,
                material,
                uv_rect: binding.uv_rect,
                geometry_key,
            },
        );
    }
}

pub(crate) fn focused_globe_tile_view(
    focus: CelestialFocus,
    globe_views: &Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    dynamic_surface_views: &Query<'_, '_, &DynamicBodySurfaceView>,
    body_rotation: Quat,
) -> Option<GlobeTileView> {
    dynamic_surface_views
        .iter()
        .copied()
        .find(|view| view.target == focus.target)
        .and_then(DynamicBodySurfaceView::tile_view)
        .or_else(|| {
            globe_views
                .iter()
                .next()
                .copied()
                .map(|globe_view| GlobeTileView::from_globe_state(globe_view, body_rotation))
        })
}

/// Resolves the focused primary-globe surface anchor.
///
/// Metric scene body anchors are preferred because they are the shared object
/// model used by overview and close-up views. The legacy `PrimaryCelestialBody`
/// fallback keeps older custom globe setups working while the public API
/// transitions toward metric scene anchors.
pub(crate) fn focused_globe_surface_anchor(
    focus: CelestialFocus,
    metric_body_anchors: &GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: &PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
) -> Option<GlobeSurfaceAnchor> {
    focused_globe_surface_anchor_from_iter(focus, metric_body_anchors.iter()).or_else(|| {
        if focus.target != BodyId::EARTH {
            return None;
        }
        primary_body_anchors
            .iter()
            .next()
            .map(|(entity, transform)| GlobeSurfaceAnchor {
                entity,
                rotation: transform.rotation,
            })
    })
}

fn focused_globe_surface_anchor_from_iter<'a>(
    focus: CelestialFocus,
    metric_body_anchors: impl IntoIterator<Item = (Entity, &'a MetricSceneObject, &'a Transform)>,
) -> Option<GlobeSurfaceAnchor> {
    metric_body_anchors
        .into_iter()
        .find_map(|(entity, object, transform)| {
            (object.id == focus.target).then_some(GlobeSurfaceAnchor {
                entity,
                rotation: transform.rotation,
            })
        })
}

fn same_uv_rect(left: Rect, right: Rect) -> bool {
    left.min == right.min && left.max == right.max
}

fn parent_to_globe_surface_anchor(commands: &mut Commands<'_, '_>, parent: Entity, entity: Entity) {
    commands.entity(parent).add_child(entity);
}

fn despawn_all_globe_tiles(
    commands: &mut Commands<'_, '_>,
    globe_tile_entities: &mut GlobeTileEntities,
) {
    for entity in globe_tile_entities
        .by_tile
        .drain()
        .map(|(_, handles)| handles.entity)
    {
        commands.entity(entity).despawn();
    }

    despawn_globe_polar_caps(commands, globe_tile_entities);
    globe_tile_entities.parent = None;
}

#[cfg(test)]
mod tests {
    use super::{
        build_globe_tile_material, focused_globe_surface_anchor,
        focused_globe_surface_anchor_from_iter, focused_globe_tile_view,
        globe_camera_surface_direction, globe_patch_segments, globe_request_tiles,
        sort_tiles_by_globe_priority, DynamicBodySurfaceView, GlobeSurfaceAnchor,
        GlobeSurfaceAnchorQuery, GlobeTileGeometryKey, GlobeTileView,
        PrimaryGlobeSurfaceAnchorQuery,
    };
    use crate::celestial::PrimaryCelestialBody;
    use crate::globe::{Globe3dState, Globe3dView, GlobeCameraUpMode};
    use crate::metric_scene::MetricSceneObject;
    use crate::tiles::TileTextureBinding;
    use crate::CelestialFocus;
    use bevy::image::Image;
    use bevy::prelude::Handle;
    use bevy::prelude::{
        App, Color, Quat, Query, Rect, ResMut, Resource, Transform, Update, Vec2, Vec3, With,
    };
    use ferrisium_core::prelude::{BodyId, CelestialBody, TileId, TileSource};

    #[test]
    fn ready_globe_tile_materials_participate_in_scene_lighting() {
        let binding = TileTextureBinding {
            image: Some(Handle::<Image>::default()),
            uv_rect: Rect::from_corners(Vec2::ZERO, Vec2::ONE),
            color: Color::WHITE,
        };
        let material = build_globe_tile_material(TileId::new(0, 0, 0), &binding);

        assert!(!material.unlit);
        assert!((material.perceptual_roughness - 1.0).abs() <= f32::EPSILON);
        assert!(material.reflectance.abs() <= f32::EPSILON);
    }

    #[test]
    fn placeholder_globe_tile_materials_are_unlit_until_images_arrive() {
        let binding = TileTextureBinding {
            image: None,
            uv_rect: Rect::from_corners(Vec2::ZERO, Vec2::ONE),
            color: Color::srgb(0.18, 0.44, 0.72),
        };
        let material = build_globe_tile_material(TileId::new(0, 0, 0), &binding);

        assert!(material.unlit);
        assert_eq!(material.base_color, binding.color);
    }

    #[test]
    fn globe_tile_geometry_key_tracks_projection_and_radius() {
        let web =
            GlobeTileGeometryKey::new(ferrisium_core::source::TileProjection::WebMercator, 4.0);
        let equirectangular =
            GlobeTileGeometryKey::new(ferrisium_core::source::TileProjection::Equirectangular, 4.0);
        let larger =
            GlobeTileGeometryKey::new(ferrisium_core::source::TileProjection::WebMercator, 8.0);

        assert_ne!(web, equirectangular);
        assert_ne!(web, larger);
        assert!((web.radius() - 4.0).abs() <= f32::EPSILON);
    }

    #[test]
    fn dynamic_body_surface_view_builds_globe_tile_view_without_changing_radius() {
        let dynamic_view = DynamicBodySurfaceView::enabled(
            BodyId::MOON,
            1.737,
            8.0,
            2.0,
            Vec3::new(0.0, 3.0, 4.0),
            3,
            512,
        );
        let tile_view = dynamic_view.tile_view();

        assert!(
            tile_view.is_some(),
            "enabled dynamic view should produce tile state"
        );
        let Some(tile_view) = tile_view else {
            return;
        };
        assert!((tile_view.radius - 1.737).abs() <= f32::EPSILON);
        assert!((tile_view.base_surface_distance - 8.0).abs() <= f64::EPSILON);
        assert!((tile_view.current_surface_distance - 2.0).abs() <= f64::EPSILON);
        assert_eq!(tile_view.tile_zoom, 3);
        assert_eq!(tile_view.max_selected_tiles, 512);
        assert!(tile_view.camera_surface_direction.x.abs() <= 1.0e-6);
        assert!((tile_view.camera_surface_direction.y - 0.6).abs() <= 1.0e-6);
        assert!((tile_view.camera_surface_direction.z - 0.8).abs() <= 1.0e-6);
    }

    #[derive(Resource, Default)]
    struct CapturedGlobeTileView(Option<GlobeTileView>);

    fn capture_focused_globe_tile_view(
        globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
        dynamic_surface_views: Query<'_, '_, &DynamicBodySurfaceView>,
        mut captured: ResMut<'_, CapturedGlobeTileView>,
    ) {
        captured.0 = focused_globe_tile_view(
            CelestialFocus::earth(),
            &globe_views,
            &dynamic_surface_views,
            Quat::IDENTITY,
        );
    }

    #[test]
    fn focused_globe_tile_view_prefers_enabled_dynamic_surface_view() {
        let mut app = App::new();
        app.init_resource::<CapturedGlobeTileView>()
            .add_systems(Update, capture_focused_globe_tile_view);
        app.world_mut().spawn((
            Globe3dView,
            Globe3dState {
                radius: 4.0,
                tile_zoom: 4,
                ..Globe3dState::default()
            },
        ));
        app.world_mut().spawn(DynamicBodySurfaceView::enabled(
            BodyId::EARTH,
            9.0,
            18.0,
            3.0,
            Vec3::X,
            7,
            256,
        ));

        app.update();

        let captured = app.world().resource::<CapturedGlobeTileView>().0;
        assert!(captured.is_some(), "missing focused globe tile view");
        let Some(tile_view) = captured else {
            return;
        };
        assert!((tile_view.radius - 9.0).abs() <= f32::EPSILON);
        assert_eq!(tile_view.tile_zoom, 7);
    }

    #[test]
    fn focused_globe_surface_anchor_selects_matching_metric_body() {
        let mut app = App::new();
        let earth_rotation = Quat::from_rotation_y(0.25);
        let moon_rotation = Quat::from_rotation_x(0.5);
        let earth = app
            .world_mut()
            .spawn((
                MetricSceneObject::body(&CelestialBody::earth()),
                Transform::from_rotation(earth_rotation),
            ))
            .id();
        let moon = app
            .world_mut()
            .spawn((
                MetricSceneObject::body(&CelestialBody::moon()),
                Transform::from_rotation(moon_rotation),
            ))
            .id();

        let world = app.world();
        let earth_object = world.get::<MetricSceneObject>(earth);
        assert!(earth_object.is_some(), "missing Earth scene object");
        let Some(earth_object) = earth_object else {
            return;
        };
        let earth_transform = world.get::<Transform>(earth);
        assert!(earth_transform.is_some(), "missing Earth transform");
        let Some(earth_transform) = earth_transform else {
            return;
        };
        let moon_object = world.get::<MetricSceneObject>(moon);
        assert!(moon_object.is_some(), "missing Moon scene object");
        let Some(moon_object) = moon_object else {
            return;
        };
        let moon_transform = world.get::<Transform>(moon);
        assert!(moon_transform.is_some(), "missing Moon transform");
        let Some(moon_transform) = moon_transform else {
            return;
        };
        let anchor = focused_globe_surface_anchor_from_iter(
            CelestialFocus::moon(),
            [
                (earth, earth_object, earth_transform),
                (moon, moon_object, moon_transform),
            ],
        );

        assert!(anchor.is_some(), "missing focused Moon anchor");
        let Some(anchor) = anchor else {
            return;
        };
        assert_eq!(anchor.entity, moon);
        assert_eq!(anchor.rotation(), moon_rotation);
    }

    #[test]
    fn focused_globe_surface_anchor_returns_none_for_missing_metric_body() {
        let mut app = App::new();
        let earth = app
            .world_mut()
            .spawn((
                MetricSceneObject::body(&CelestialBody::earth()),
                Transform::default(),
            ))
            .id();

        let world = app.world();
        let earth_object = world.get::<MetricSceneObject>(earth);
        assert!(earth_object.is_some(), "missing Earth scene object");
        let Some(earth_object) = earth_object else {
            return;
        };
        let earth_transform = world.get::<Transform>(earth);
        assert!(earth_transform.is_some(), "missing Earth transform");
        let Some(earth_transform) = earth_transform else {
            return;
        };
        let anchor = focused_globe_surface_anchor_from_iter(
            CelestialFocus::target(BodyId::MARS_BARYCENTER),
            [(earth, earth_object, earth_transform)],
        );

        assert_eq!(anchor, None);
    }

    #[derive(Resource, Default)]
    struct CapturedGlobeSurfaceAnchor(Option<GlobeSurfaceAnchor>);

    fn capture_focused_globe_surface_anchor(
        metric_body_anchors: GlobeSurfaceAnchorQuery<'_, '_>,
        primary_body_anchors: PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
        mut captured: ResMut<'_, CapturedGlobeSurfaceAnchor>,
    ) {
        captured.0 = focused_globe_surface_anchor(
            CelestialFocus::earth(),
            &metric_body_anchors,
            &primary_body_anchors,
        );
    }

    #[test]
    fn focused_globe_surface_anchor_falls_back_to_primary_body_marker() {
        let mut app = App::new();
        let rotation = Quat::from_rotation_z(0.75);
        let primary = app
            .world_mut()
            .spawn((PrimaryCelestialBody, Transform::from_rotation(rotation)))
            .id();
        app.init_resource::<CapturedGlobeSurfaceAnchor>()
            .add_systems(Update, capture_focused_globe_surface_anchor);

        app.update();

        let anchor = app.world().resource::<CapturedGlobeSurfaceAnchor>().0;
        assert!(anchor.is_some(), "missing primary-body fallback anchor");
        let Some(anchor) = anchor else {
            return;
        };
        assert_eq!(anchor.entity, primary);
        assert_eq!(anchor.rotation(), rotation);
    }

    #[test]
    fn globe_request_tiles_include_visible_tile_ancestors() {
        let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png");
        let state = Globe3dState {
            tile_zoom: 4,
            max_selected_tiles: 128,
            ..Globe3dState::default()
        };
        let request_tiles = globe_request_tiles(state, &source, Quat::IDENTITY);
        let selected_child = request_tiles.iter().copied().find(|tile| tile.z == 4);

        assert!(selected_child.is_some());
        let Some(selected_child) = selected_child else {
            return;
        };

        assert!(
            request_tiles.contains(&selected_child.ancestor_at_zoom(3).unwrap_or(selected_child))
        );
        assert!(
            request_tiles.contains(&selected_child.ancestor_at_zoom(2).unwrap_or(selected_child))
        );
    }

    #[test]
    fn globe_request_tiles_respect_source_min_zoom() {
        let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(2, 8);
        let state = Globe3dState {
            tile_zoom: 4,
            max_selected_tiles: 128,
            ..Globe3dState::default()
        };
        let request_tiles = globe_request_tiles(state, &source, Quat::IDENTITY);

        assert!(request_tiles.iter().any(|tile| tile.z == 2));
        assert!(request_tiles.iter().all(|tile| tile.z >= 2));
    }

    #[test]
    fn low_zoom_globe_patches_use_more_segments() {
        assert!(
            globe_patch_segments(TileId::new(2, 1, 1)) > globe_patch_segments(TileId::new(6, 1, 1))
        );
    }

    #[test]
    fn globe_requests_base_fallback_before_deep_detail() {
        let source =
            TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 12);
        let state = Globe3dState {
            camera_distance: 4.01,
            tile_zoom: 4,
            ..Globe3dState::default()
        };
        let mut tiles = vec![
            TileId::new(12, 2048, 2048),
            TileId::new(6, 32, 32),
            TileId::new(4, 8, 8),
            TileId::new(0, 0, 0),
        ];

        sort_tiles_by_globe_priority(&mut tiles, state, &source, Quat::IDENTITY);

        assert_eq!(tiles[0].z, 0);
        assert_eq!(tiles[1].z, 4);
        assert_eq!(tiles[2].z, 12);
    }

    #[test]
    fn globe_camera_surface_direction_defaults_to_body_fixed_orbit_frame() {
        let state = Globe3dState {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            ..Globe3dState::default()
        };
        let direction =
            globe_camera_surface_direction(state, Quat::from_rotation_y(std::f32::consts::PI));

        assert!(direction.x.abs() <= 1.0e-6);
        assert!(direction.y.abs() <= 1.0e-6);
        assert!((direction.z - 1.0).abs() <= 1.0e-6);
    }

    #[test]
    fn scene_up_globe_camera_surface_direction_uses_body_local_frame() {
        let state = Globe3dState {
            camera_up_mode: GlobeCameraUpMode::SceneUp,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            ..Globe3dState::default()
        };
        let direction =
            globe_camera_surface_direction(state, Quat::from_rotation_y(std::f32::consts::PI));

        assert!(direction.x.abs() <= 1.0e-6);
        assert!(direction.y.abs() <= 1.0e-6);
        assert!((direction.z + 1.0).abs() <= 1.0e-6);
    }

    #[test]
    fn scene_up_globe_camera_surface_direction_uses_identity_for_invalid_body_rotation() {
        let state = Globe3dState {
            camera_up_mode: GlobeCameraUpMode::SceneUp,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            ..Globe3dState::default()
        };
        let direction = globe_camera_surface_direction(state, Quat::from_xyzw(0.0, 0.0, 0.0, 0.0));

        assert!(direction.x.abs() <= 1.0e-6);
        assert!(direction.y.abs() <= 1.0e-6);
        assert!((direction.z - 1.0).abs() <= 1.0e-6);
    }
}
