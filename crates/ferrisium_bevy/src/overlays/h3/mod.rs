//! H3 coverage overlays for Ferrisium map and globe views.
//!
//! Applications provide validated H3 cells through [`H3Cells`]; this module
//! turns the same coverage content into either 2D projected meshes or 3D meshes
//! draped onto a focused globe surface. H3 remains an Earth/geographic indexing
//! layer: domain-specific coverage truth should be computed before cells reach
//! this renderer.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
#[cfg(test)]
use bevy::math::DVec2;
use bevy::math::DVec3;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::window::PrimaryWindow;
use big_space::prelude::CellCoord;
use ferrisium_core::coords::LonLat;
use ferrisium_core::prelude::{BodyId, PlanetaryBody};

use crate::celestial::{CelestialBodyPlacement, CelestialFocus, CelestialSpace};
use crate::globe::{
    globe_focus_target_with_metric_pivot, hovered_globe_surface_direction, Globe3dCamera,
    Globe3dState, Globe3dView,
};
use crate::input_capture::FerrisiumInputCapture;
use crate::metric_scene::MetricSceneFocusPivot;
use crate::overlay_common::{
    color_from_rgba, overlay_is_visible, OverlayColorKey, OverlayMapViewKey,
};
use crate::source::ActiveTileSource;
use crate::surface_context::GlobeSurfaceRenderContext;
use crate::surface_depth::{
    apply_spherical_decal_material, draped_fill_radius, draped_outline_radius,
    SphericalSurfaceLayer,
};
use crate::tile_globe::{
    focused_globe_surface_anchor, GlobeSurfaceAnchorQuery, PrimaryGlobeSurfaceAnchorQuery,
};
use crate::view::{
    FerrisiumViewKind, FerrisiumViewMode, Map2dRenderContext, Map2dView, PointerGeoResource,
};

mod types;

pub use types::{
    h3_cell_boundary_polygons, h3_cell_map_boundary_polygons, h3_cell_spherical_boundary, H3Cell,
    H3CellBoundary, H3CellColors, H3CellPolygon, H3Cells, H3Layer, H3Style,
};

const DEFAULT_H3_MAP_Z: f32 = 35.0;
const DEFAULT_H3_GLOBE_SURFACE_OFFSET_UNITS: f32 = 0.0;
const H3_GLOBE_MAX_EDGE_SEGMENT_RADIANS: f64 = 0.02;
const H3_GLOBE_MAX_EDGE_SEGMENTS: usize = 24;

/// H3 cells that should be rendered over a map and/or globe surface.
///
/// The same component is consumed by both Ferrisium's 2D map renderer and its
/// focused 3D globe renderer. `body` controls which map tile source can display
/// the overlay; `target` controls which celestial body receives the globe drape.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct H3Overlay {
    /// Human-readable layer descriptor supplied by the application.
    pub layer: H3Layer,
    /// Body center that receives the globe overlay.
    pub target: BodyId,
    /// Planetary map body that receives the 2D overlay.
    pub body: PlanetaryBody,
    /// Validated H3 cells to draw.
    pub cells: H3Cells,
    /// Constant fill style used by this layer.
    pub style: H3Style,
    /// Optional per-cell fill colors in `cells` insertion order.
    pub cell_colors: Option<H3CellColors>,
    /// Z depth used for 2D map meshes above raster tiles.
    pub map_z: f32,
    /// Whether the overlay should render in 2D map views.
    pub map_enabled: bool,
    /// Whether the overlay should render on focused 3D globe surfaces.
    pub globe_enabled: bool,
    /// Minimum visual lift above globe raster tiles.
    ///
    /// This is a render-only decal offset in render units. Ferrisium may apply
    /// a larger shared surface-layer lift to keep browser depth precision
    /// stable. The value does not change H3 cell geography; vertices still
    /// follow the cell boundary lon/lat.
    pub globe_surface_offset_units: f32,
}

impl H3Overlay {
    /// Creates an Earth H3 overlay for both Earth map and Earth globe views.
    #[must_use]
    pub fn earth(layer: impl Into<Cow<'static, str>>, cells: H3Cells, style: H3Style) -> Self {
        Self::for_body(
            H3Layer::new(layer),
            BodyId::EARTH,
            PlanetaryBody::earth(),
            cells,
            style,
        )
    }

    /// Creates an overlay for a map body and a celestial target.
    ///
    /// H3 itself is Earth-oriented. Supplying another body is useful only when an
    /// application intentionally interprets H3 lon/lat cells on that body's
    /// geographic coordinate system.
    #[must_use]
    pub fn for_body(
        layer: H3Layer,
        target: BodyId,
        body: PlanetaryBody,
        cells: H3Cells,
        style: H3Style,
    ) -> Self {
        Self {
            layer,
            target,
            body,
            cells,
            style,
            cell_colors: None,
            map_z: DEFAULT_H3_MAP_Z,
            map_enabled: true,
            globe_enabled: true,
            globe_surface_offset_units: DEFAULT_H3_GLOBE_SURFACE_OFFSET_UNITS,
        }
    }

    /// Sets the 2D map z depth.
    #[must_use]
    pub const fn with_map_z(mut self, map_z: f32) -> Self {
        self.map_z = map_z;
        self
    }

    /// Sets optional per-cell fill colors for this overlay.
    ///
    /// Colors are matched to `cells` by insertion order. If fewer colors are
    /// supplied than cells, the remaining cells use this overlay's constant
    /// fill style.
    #[must_use]
    pub fn with_cell_colors(mut self, colors: H3CellColors) -> Self {
        self.cell_colors = Some(colors);
        self
    }

    /// Enables or disables this overlay in 2D map views.
    #[must_use]
    pub const fn with_map_enabled(mut self, enabled: bool) -> Self {
        self.map_enabled = enabled;
        self
    }

    /// Enables or disables this overlay on focused 3D globe surfaces.
    #[must_use]
    pub const fn with_globe_enabled(mut self, enabled: bool) -> Self {
        self.globe_enabled = enabled;
        self
    }

    /// Sets the minimum outward globe-surface visual lift in render units.
    ///
    /// The renderer may increase this lift through Ferrisium's shared surface
    /// depth policy when the focused body radius needs more separation.
    #[must_use]
    pub const fn with_globe_surface_offset_units(mut self, offset_units: f32) -> Self {
        self.globe_surface_offset_units = offset_units;
        self
    }
}

/// Convenience bundle for spawning an H3 coverage overlay.
#[derive(Bundle)]
pub struct H3OverlayBundle {
    /// Coverage content and style.
    pub overlay: H3Overlay,
    /// Local transform for the overlay marker entity.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Local visibility gate; inherited hierarchy visibility is also honored.
    pub visibility: Visibility,
}

impl H3OverlayBundle {
    /// Creates a root-visible overlay bundle that inherits parent visibility.
    #[must_use]
    pub fn new(overlay: H3Overlay) -> Self {
        Self {
            overlay,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Current H3 cell under the active Ferrisium pointer, if any.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct H3HoverState {
    /// Hovered cell resolved from the active map or globe view.
    pub hit: Option<H3CellHit>,
}

/// View surface that produced an H3 pick hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3PickSurface {
    /// Hit came from the active 2D map pointer.
    Map2d,
    /// Hit came from the active 3D globe cursor ray.
    Globe3d,
}

/// Resolved H3 cell pick information.
#[derive(Debug, Clone, PartialEq)]
pub struct H3CellHit {
    /// Overlay entity that owns the hit cell.
    pub overlay: Entity,
    /// Human-readable layer descriptor copied from the overlay.
    pub layer: H3Layer,
    /// Body center receiving the globe overlay.
    pub target: BodyId,
    /// Planetary map body receiving the 2D overlay.
    pub body: PlanetaryBody,
    /// Cell index inside the overlay's `H3Cells` insertion order.
    pub cell_index: usize,
    /// Validated H3 cell that was hit.
    pub cell: H3Cell,
    /// Geographic point used for the lookup.
    pub lon_lat: LonLat,
    /// View surface that produced the hit.
    pub surface: H3PickSurface,
}

/// Message emitted when the active pointer clicks an H3 cell.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct H3CellClicked {
    /// Cell hit at the time of the click.
    pub hit: H3CellHit,
}

/// Cell-level geographic lookup generated from an [`H3Overlay`].
///
/// The lookup preserves cell insertion order, caches projected map polygons and
/// raw spherical globe boundaries, and keeps both under the same per-cell
/// styling/picking index contract.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct H3CellLookup {
    entries: Vec<H3CellLookupEntry>,
}

impl H3CellLookup {
    /// Builds a lookup from H3 cells in insertion order.
    #[must_use]
    pub fn from_cells(cells: &H3Cells) -> Self {
        Self {
            entries: cells
                .iter()
                .enumerate()
                .filter_map(|(cell_index, cell)| {
                    let map_polygons = cell.map_boundary_polygons();
                    let globe_boundary = cell.spherical_boundary();
                    (!map_polygons.is_empty() && globe_boundary.vertices().len() >= 3).then_some(
                        H3CellLookupEntry {
                            cell_index,
                            cell,
                            map_polygons,
                            globe_boundary,
                        },
                    )
                })
                .collect(),
        }
    }

    /// Returns lookup entries in the same order as the source cells.
    #[must_use]
    pub fn entries(&self) -> &[H3CellLookupEntry] {
        &self.entries
    }

    /// Returns the first cell that contains `lon_lat`.
    #[must_use]
    pub fn cell_at_lon_lat(&self, lon_lat: LonLat) -> Option<(usize, H3Cell)> {
        self.entries
            .iter()
            .find(|entry| entry.cell.contains_lon_lat(lon_lat))
            .map(|entry| (entry.cell_index, entry.cell))
    }

    /// Returns true when there are no cell entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One cell and all geographic polygons that bound it.
#[derive(Debug, Clone, PartialEq)]
pub struct H3CellLookupEntry {
    /// Cell index inside the overlay's `H3Cells` insertion order.
    pub cell_index: usize,
    /// Validated H3 cell.
    pub cell: H3Cell,
    map_polygons: Vec<H3CellPolygon>,
    globe_boundary: H3CellBoundary,
}

impl H3CellLookupEntry {
    /// Returns the projected-map, antimeridian-safe boundary polygons for this cell.
    #[must_use]
    pub fn map_polygons(&self) -> &[H3CellPolygon] {
        &self.map_polygons
    }

    /// Returns the unsplit spherical boundary ring for this cell.
    #[must_use]
    pub fn globe_boundary(&self) -> &H3CellBoundary {
        &self.globe_boundary
    }

    /// Returns the projected-map, antimeridian-safe boundary polygons for this cell.
    #[must_use]
    pub fn polygons(&self) -> &[H3CellPolygon] {
        self.map_polygons()
    }
}

/// Spawned render assets for map and globe H3 overlays.
#[derive(Resource, Default)]
pub(crate) struct H3OverlayEntities {
    map: HashMap<Entity, H3MapOverlayHandles>,
    globe: HashMap<Entity, H3GlobeOverlayHandles>,
}

#[derive(Debug, Clone)]
struct H3MapOverlayHandles {
    fill_entity: Entity,
    fill_mesh: Handle<Mesh>,
    fill_material: Handle<ColorMaterial>,
    fill_color_key: H3ColorKey,
    outline_entity: Entity,
    outline_mesh: Handle<Mesh>,
    outline_material: Handle<ColorMaterial>,
    outline_color_key: H3ColorKey,
    map_z_bits: u32,
    view_key: H3MapViewKey,
}

#[derive(Debug, Clone)]
struct H3GlobeOverlayHandles {
    fill_entity: Entity,
    fill_mesh: Handle<Mesh>,
    fill_material: Handle<StandardMaterial>,
    fill_color_key: H3ColorKey,
    outline_entity: Entity,
    outline_mesh: Handle<Mesh>,
    outline_material: Handle<StandardMaterial>,
    outline_color_key: H3ColorKey,
    parent: Entity,
    radius_bits: u32,
    offset_bits: u32,
}

type H3ColorKey = OverlayColorKey;
type H3MapViewKey = OverlayMapViewKey;
type H3OverlayRenderQueryItem<'a> = (
    Entity,
    Ref<'a, H3Overlay>,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
    Option<&'a H3CellLookup>,
);
type H3OverlayPickQueryItem<'a> = (
    Entity,
    &'a H3Overlay,
    &'a H3CellLookup,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
);

/// Synchronizes H3 coverage layers into the active 2D map view.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Bevy systems expose window, view, source, overlay, and asset stores explicitly."
)]
pub(crate) fn sync_h3_map_overlays(
    mut commands: Commands<'_, '_>,
    map_contexts: Query<'_, '_, &Map2dRenderContext, With<Map2dView>>,
    overlays: Query<'_, '_, H3OverlayRenderQueryItem<'_>>,
    mut overlay_entities: ResMut<'_, H3OverlayEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<ColorMaterial>>,
) {
    let live_overlays = overlays
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_h3_map_overlays(&mut commands, &mut overlay_entities, &live_overlays);

    let Some(context) = map_contexts.iter().next() else {
        despawn_all_h3_map_overlays(&mut commands, &mut overlay_entities);
        return;
    };
    let view_key = h3_map_view_key(context);

    for (overlay_entity, overlay, visibility, inherited_visibility, lookup) in &overlays {
        if !overlay.map_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || overlay.body != context.body
        {
            despawn_h3_map_overlay(&mut commands, &mut overlay_entities, overlay_entity);
            continue;
        }

        let fill_rgba = h3_fill_material_rgba(&overlay);
        let outline_rgba = overlay.style.outline_rgba();
        let fill_color_key = H3ColorKey::from_rgba(fill_rgba);
        let outline_color_key = H3ColorKey::from_rgba(outline_rgba);
        let map_z_bits = overlay.map_z.to_bits();
        if let Some(handles) = overlay_entities.map.get_mut(&overlay_entity) {
            if overlay.is_changed() || handles.view_key != view_key {
                let geometry = current_h3_lookup(&overlay.cells, overlay.is_changed(), lookup);
                if let Some(mesh) = meshes.get_mut(&handles.fill_mesh) {
                    *mesh = build_h3_map_mesh(
                        geometry.as_ref(),
                        overlay.cell_colors.as_ref(),
                        overlay.style.fill_rgba(),
                        context,
                    );
                }
                if let Some(mesh) = meshes.get_mut(&handles.outline_mesh) {
                    *mesh = build_h3_map_outline_mesh(geometry.as_ref(), context);
                }
                handles.view_key = view_key;
            }
            if handles.fill_color_key != fill_color_key {
                if let Some(material) = materials.get_mut(&handles.fill_material) {
                    *material = h3_map_material(fill_rgba);
                }
                handles.fill_color_key = fill_color_key;
            }
            if handles.outline_color_key != outline_color_key {
                if let Some(material) = materials.get_mut(&handles.outline_material) {
                    material.color = color_from_rgba(outline_rgba);
                }
                handles.outline_color_key = outline_color_key;
            }
            if handles.map_z_bits != map_z_bits {
                let transform = Transform::from_xyz(0.0, 0.0, overlay.map_z);
                commands.entity(handles.fill_entity).insert(transform);
                commands.entity(handles.outline_entity).insert(transform);
                handles.map_z_bits = map_z_bits;
            }
            continue;
        }

        let geometry = current_h3_lookup(&overlay.cells, overlay.is_changed(), lookup);
        let fill_mesh = meshes.add(build_h3_map_mesh(
            geometry.as_ref(),
            overlay.cell_colors.as_ref(),
            overlay.style.fill_rgba(),
            context,
        ));
        let fill_material = materials.add(h3_map_material(fill_rgba));
        let outline_mesh = meshes.add(build_h3_map_outline_mesh(geometry.as_ref(), context));
        let outline_material = materials.add(ColorMaterial::from(color_from_rgba(outline_rgba)));
        let transform = Transform::from_xyz(0.0, 0.0, overlay.map_z);
        let fill_entity = commands
            .spawn((
                Name::new(format!("H3 Map Overlay {}", overlay.layer.name())),
                Mesh2d(fill_mesh.clone()),
                MeshMaterial2d(fill_material.clone()),
                transform,
            ))
            .id();
        let outline_entity = commands
            .spawn((
                Name::new(format!("H3 Map Overlay {} Outline", overlay.layer.name())),
                Mesh2d(outline_mesh.clone()),
                MeshMaterial2d(outline_material.clone()),
                transform,
            ))
            .id();
        overlay_entities.map.insert(
            overlay_entity,
            H3MapOverlayHandles {
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

/// Synchronizes H3 coverage layers into the focused 3D globe surface.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Globe overlay sync needs the same focused body and camera state as raster globe patches."
)]
pub(crate) fn sync_h3_globe_overlays(
    mut commands: Commands<'_, '_>,
    celestial_focus: Res<'_, CelestialFocus>,
    globe_contexts: Query<'_, '_, &GlobeSurfaceRenderContext>,
    overlays: Query<'_, '_, H3OverlayRenderQueryItem<'_>>,
    mut overlay_entities: ResMut<'_, H3OverlayEntities>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let live_overlays = overlays
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_h3_globe_overlays(&mut commands, &mut overlay_entities, &live_overlays);

    let Some(context) = globe_contexts
        .iter()
        .find(|context| context.target == celestial_focus.target)
    else {
        despawn_all_h3_globe_overlays(&mut commands, &mut overlay_entities);
        return;
    };

    let radius = context.radius;
    let parent = context.surface_anchor;
    for (overlay_entity, overlay, visibility, inherited_visibility, lookup) in &overlays {
        if !overlay.globe_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || overlay.target != celestial_focus.target
        {
            despawn_h3_globe_overlay(&mut commands, &mut overlay_entities, overlay_entity);
            continue;
        }

        let radius_bits = radius.to_bits();
        let offset_bits = overlay.globe_surface_offset_units.to_bits();
        let needs_new_mesh = overlay_entities
            .globe
            .get(&overlay_entity)
            .is_none_or(|handles| {
                handles.radius_bits != radius_bits || handles.offset_bits != offset_bits
            })
            || overlay.is_changed();
        let fill_rgba = h3_fill_material_rgba(&overlay);
        let outline_rgba = overlay.style.outline_rgba();
        let fill_color_key = H3ColorKey::from_rgba(fill_rgba);
        let outline_color_key = H3ColorKey::from_rgba(outline_rgba);

        if let Some(handles) = overlay_entities.globe.get_mut(&overlay_entity) {
            if handles.parent != parent {
                commands.entity(parent).add_child(handles.fill_entity);
                commands.entity(parent).add_child(handles.outline_entity);
                handles.parent = parent;
            }
            if needs_new_mesh {
                let geometry = current_h3_lookup(&overlay.cells, overlay.is_changed(), lookup);
                if let Some(mesh) = meshes.get_mut(&handles.fill_mesh) {
                    *mesh = build_h3_globe_mesh(
                        geometry.as_ref(),
                        overlay.cell_colors.as_ref(),
                        overlay.style.fill_rgba(),
                        radius,
                        overlay.globe_surface_offset_units,
                    );
                }
                if let Some(mesh) = meshes.get_mut(&handles.outline_mesh) {
                    *mesh = build_h3_globe_outline_mesh(
                        geometry.as_ref(),
                        radius,
                        overlay.globe_surface_offset_units,
                    );
                }
                handles.radius_bits = radius_bits;
                handles.offset_bits = offset_bits;
            }
            if handles.fill_color_key != fill_color_key {
                if let Some(material) = materials.get_mut(&handles.fill_material) {
                    update_h3_globe_material(
                        material,
                        color_from_rgba(fill_rgba),
                        SphericalSurfaceLayer::DrapedFill,
                    );
                }
                handles.fill_color_key = fill_color_key;
            }
            if handles.outline_color_key != outline_color_key {
                if let Some(material) = materials.get_mut(&handles.outline_material) {
                    update_h3_globe_material(
                        material,
                        color_from_rgba(outline_rgba),
                        SphericalSurfaceLayer::DrapedOutline,
                    );
                }
                handles.outline_color_key = outline_color_key;
            }
            continue;
        }

        let geometry = current_h3_lookup(&overlay.cells, overlay.is_changed(), lookup);
        let fill_mesh = meshes.add(build_h3_globe_mesh(
            geometry.as_ref(),
            overlay.cell_colors.as_ref(),
            overlay.style.fill_rgba(),
            radius,
            overlay.globe_surface_offset_units,
        ));
        let outline_mesh = meshes.add(build_h3_globe_outline_mesh(
            geometry.as_ref(),
            radius,
            overlay.globe_surface_offset_units,
        ));
        let fill_material = materials.add(h3_globe_material(
            color_from_rgba(fill_rgba),
            SphericalSurfaceLayer::DrapedFill,
        ));
        let outline_material = materials.add(h3_globe_material(
            color_from_rgba(outline_rgba),
            SphericalSurfaceLayer::DrapedOutline,
        ));
        let fill_entity = commands
            .spawn((
                Name::new(format!("H3 Globe Overlay {}", overlay.layer.name())),
                Mesh3d(fill_mesh.clone()),
                MeshMaterial3d(fill_material.clone()),
            ))
            .id();
        let outline_entity = commands
            .spawn((
                Name::new(format!("H3 Globe Overlay {} Outline", overlay.layer.name())),
                Mesh3d(outline_mesh.clone()),
                MeshMaterial3d(outline_material.clone()),
            ))
            .id();
        commands.entity(parent).add_child(fill_entity);
        commands.entity(parent).add_child(outline_entity);
        overlay_entities.globe.insert(
            overlay_entity,
            H3GlobeOverlayHandles {
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

/// Synchronizes generated H3 pick lookups with overlay cell content.
pub(crate) fn sync_h3_cell_lookups(
    mut commands: Commands<'_, '_>,
    overlays: Query<'_, '_, (Entity, Ref<'_, H3Overlay>, Option<&H3CellLookup>)>,
) {
    for (entity, overlay, lookup) in &overlays {
        if overlay.is_changed() || lookup.is_none() {
            commands
                .entity(entity)
                .insert(H3CellLookup::from_cells(&overlay.cells));
        }
    }
}

/// Updates H3 hover state and emits H3 click messages for the active map view.
#[allow(
    clippy::too_many_arguments,
    reason = "Map picking spans active source, pointer state, view state, and overlay lookups."
)]
pub(crate) fn sync_h3_map_picking(
    active_source: Res<'_, ActiveTileSource>,
    input_capture: Res<'_, FerrisiumInputCapture>,
    view_mode: Res<'_, FerrisiumViewMode>,
    pointer_geo: Res<'_, PointerGeoResource>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    map_views: Query<'_, '_, (), With<Map2dView>>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    overlays: Query<'_, '_, H3OverlayPickQueryItem<'_>>,
    mut hover_state: ResMut<'_, H3HoverState>,
    mut clicked: MessageWriter<'_, H3CellClicked>,
) {
    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Map2d)
    {
        clear_h3_hover_for_surface(&mut hover_state, H3PickSurface::Map2d);
        return;
    }

    if input_capture.pointer_captured() {
        clear_h3_hover_for_surface(&mut hover_state, H3PickSurface::Map2d);
        return;
    }

    let hit = pointer_geo
        .lon_lat
        .and_then(|lon_lat| pick_h3_map_cell(lon_lat, &active_source, overlays.iter()));

    if buttons.just_released(MouseButton::Left) {
        if let Some(hit) = hit.clone() {
            clicked.write(H3CellClicked { hit });
        }
    }

    hover_state.hit = hit;
}

/// Updates H3 hover state and emits H3 click messages for the active globe view.
#[allow(
    clippy::too_many_arguments,
    reason = "Globe picking spans focus, ray, view, and overlay lookup state."
)]
pub(crate) fn sync_h3_globe_picking(
    celestial_focus: Res<'_, CelestialFocus>,
    celestial_space: Res<'_, CelestialSpace>,
    focus_pivot: Res<'_, MetricSceneFocusPivot>,
    view_mode: Res<'_, FerrisiumViewMode>,
    input_capture: Res<'_, FerrisiumInputCapture>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    cameras: Query<
        '_,
        '_,
        (&Camera, &GlobalTransform, &Transform, Option<&CellCoord>),
        With<Globe3dCamera>,
    >,
    bodies: Query<
        '_,
        '_,
        (&CelestialBodyPlacement, Option<&CellCoord>, &Transform),
        Without<Globe3dCamera>,
    >,
    map_views: Query<'_, '_, (), With<Map2dView>>,
    globe_views: Query<'_, '_, &Globe3dState, With<Globe3dView>>,
    metric_body_anchors: GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
    overlays: Query<'_, '_, H3OverlayPickQueryItem<'_>>,
    mut hover_state: ResMut<'_, H3HoverState>,
    mut clicked: MessageWriter<'_, H3CellClicked>,
) {
    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Globe3d)
    {
        clear_h3_hover_for_surface(&mut hover_state, H3PickSurface::Globe3d);
        return;
    }

    if input_capture.pointer_captured() {
        clear_h3_hover_for_surface(&mut hover_state, H3PickSurface::Globe3d);
        return;
    }

    let hit = hovered_h3_globe_lon_lat(
        *celestial_focus,
        *celestial_space,
        &focus_pivot,
        windows.iter().next(),
        cameras.iter().next(),
        &bodies,
        globe_views.iter().next().copied(),
        &metric_body_anchors,
        &primary_body_anchors,
    )
    .and_then(|lon_lat| pick_h3_globe_cell(lon_lat, *celestial_focus, overlays.iter()));

    if buttons.just_released(MouseButton::Left) {
        if let Some(hit) = hit.clone() {
            clicked.write(H3CellClicked { hit });
        }
    }

    hover_state.hit = hit;
}

fn pick_h3_map_cell<'a>(
    lon_lat: LonLat,
    active_source: &ActiveTileSource,
    overlays: impl IntoIterator<Item = H3OverlayPickQueryItem<'a>>,
) -> Option<H3CellHit> {
    let mut best_hit = None;
    let mut best_z = f32::NEG_INFINITY;
    for (entity, overlay, lookup, visibility, inherited_visibility) in overlays {
        if !overlay.map_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || overlay.body != active_source.0.body
        {
            continue;
        }
        let Some(hit) = h3_cell_hit(entity, overlay, lookup, lon_lat, H3PickSurface::Map2d) else {
            continue;
        };
        if best_hit.is_none() || overlay.map_z > best_z {
            best_z = overlay.map_z;
            best_hit = Some(hit);
        }
    }

    best_hit
}

fn pick_h3_globe_cell<'a>(
    lon_lat: LonLat,
    celestial_focus: CelestialFocus,
    overlays: impl IntoIterator<Item = H3OverlayPickQueryItem<'a>>,
) -> Option<H3CellHit> {
    let mut best_hit = None;
    let mut best_priority = None;
    for (entity, overlay, lookup, visibility, inherited_visibility) in overlays {
        if !overlay.globe_enabled
            || !overlay_is_visible(visibility, inherited_visibility)
            || overlay.target != celestial_focus.target
        {
            continue;
        }
        let Some(hit) = h3_cell_hit(entity, overlay, lookup, lon_lat, H3PickSurface::Globe3d)
        else {
            continue;
        };
        let priority = H3PickPriority::for_globe(entity, overlay);
        if best_priority.is_none_or(|best| priority.is_better_than(best)) {
            best_priority = Some(priority);
            best_hit = Some(hit);
        }
    }

    best_hit
}

fn h3_cell_hit(
    entity: Entity,
    overlay: &H3Overlay,
    lookup: &H3CellLookup,
    lon_lat: LonLat,
    surface: H3PickSurface,
) -> Option<H3CellHit> {
    lookup
        .cell_at_lon_lat(lon_lat)
        .map(|(cell_index, cell)| H3CellHit {
            overlay: entity,
            layer: overlay.layer.clone(),
            target: overlay.target,
            body: overlay.body.clone(),
            cell_index,
            cell,
            lon_lat,
            surface,
        })
}

fn clear_h3_hover_for_surface(hover_state: &mut H3HoverState, surface: H3PickSurface) {
    if hover_state
        .hit
        .as_ref()
        .is_some_and(|hit| hit.surface == surface)
    {
        hover_state.hit = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct H3PickPriority {
    primary: f32,
    secondary: f32,
    entity_bits: u64,
}

impl H3PickPriority {
    fn for_globe(entity: Entity, overlay: &H3Overlay) -> Self {
        Self {
            primary: finite_pick_value(overlay.globe_surface_offset_units),
            secondary: finite_pick_value(overlay.map_z),
            entity_bits: entity.to_bits(),
        }
    }

    fn is_better_than(self, other: Self) -> bool {
        self.primary
            .total_cmp(&other.primary)
            .then_with(|| self.secondary.total_cmp(&other.secondary))
            .then_with(|| self.entity_bits.cmp(&other.entity_bits))
            .is_gt()
    }
}

fn finite_pick_value(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        f32::NEG_INFINITY
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Globe picking reuses the same focus and camera inputs as globe controls."
)]
fn hovered_h3_globe_lon_lat<F>(
    celestial_focus: CelestialFocus,
    celestial_space: CelestialSpace,
    focus_pivot: &MetricSceneFocusPivot,
    window: Option<&Window>,
    camera: Option<(&Camera, &GlobalTransform, &Transform, Option<&CellCoord>)>,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
    globe_state: Option<Globe3dState>,
    metric_body_anchors: &GlobeSurfaceAnchorQuery<'_, '_>,
    primary_body_anchors: &PrimaryGlobeSurfaceAnchorQuery<'_, '_>,
) -> Option<LonLat>
where
    F: bevy::ecs::query::QueryFilter,
{
    let surface_anchor =
        focused_globe_surface_anchor(celestial_focus, metric_body_anchors, primary_body_anchors)?;
    let globe_state = globe_state?;
    let focus_target = globe_focus_target_with_metric_pivot(
        celestial_focus,
        globe_state,
        celestial_space,
        focus_pivot,
        bodies,
    );
    let world_direction = hovered_globe_surface_direction(
        window,
        camera,
        celestial_space,
        focus_target.radius,
        focus_target.center_units,
    )?;
    let body_direction = surface_anchor
        .rotation()
        .inverse()
        .mul_vec3(world_direction.as_vec3())
        .as_dvec3();

    Some(lon_lat_from_globe_direction(body_direction))
}

fn retain_live_h3_map_overlays(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
    live_overlays: &HashSet<Entity>,
) {
    overlay_entities.map.retain(|overlay, handles| {
        if live_overlays.contains(overlay) {
            true
        } else {
            despawn_h3_map_handles(commands, handles);
            false
        }
    });
}

fn retain_live_h3_globe_overlays(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
    live_overlays: &HashSet<Entity>,
) {
    overlay_entities.globe.retain(|overlay, handles| {
        if live_overlays.contains(overlay) {
            true
        } else {
            despawn_h3_globe_handles(commands, handles);
            false
        }
    });
}

fn despawn_all_h3_map_overlays(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
) {
    for entity in overlay_entities
        .map
        .drain()
        .flat_map(|(_, handles)| [handles.fill_entity, handles.outline_entity])
    {
        commands.entity(entity).despawn();
    }
}

fn despawn_all_h3_globe_overlays(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
) {
    for entity in overlay_entities
        .globe
        .drain()
        .flat_map(|(_, handles)| [handles.fill_entity, handles.outline_entity])
    {
        commands.entity(entity).despawn();
    }
}

fn despawn_h3_map_overlay(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
    overlay: Entity,
) {
    if let Some(handles) = overlay_entities.map.remove(&overlay) {
        despawn_h3_map_handles(commands, &handles);
    }
}

fn despawn_h3_globe_overlay(
    commands: &mut Commands<'_, '_>,
    overlay_entities: &mut H3OverlayEntities,
    overlay: Entity,
) {
    if let Some(handles) = overlay_entities.globe.remove(&overlay) {
        despawn_h3_globe_handles(commands, &handles);
    }
}

fn despawn_h3_map_handles(commands: &mut Commands<'_, '_>, handles: &H3MapOverlayHandles) {
    commands.entity(handles.fill_entity).despawn();
    commands.entity(handles.outline_entity).despawn();
}

fn despawn_h3_globe_handles(commands: &mut Commands<'_, '_>, handles: &H3GlobeOverlayHandles) {
    commands.entity(handles.fill_entity).despawn();
    commands.entity(handles.outline_entity).despawn();
}

fn h3_map_view_key(context: &Map2dRenderContext) -> H3MapViewKey {
    crate::overlay_common::map_view_key(&context.view_state)
}

fn current_h3_lookup<'a>(
    cells: &H3Cells,
    overlay_changed: bool,
    lookup: Option<&'a H3CellLookup>,
) -> Cow<'a, H3CellLookup> {
    match (overlay_changed, lookup) {
        (false, Some(lookup)) => Cow::Borrowed(lookup),
        _ => Cow::Owned(H3CellLookup::from_cells(cells)),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Map overlay vertices are Bevy f32 mesh positions after f64 projection math."
)]
fn build_h3_map_mesh(
    lookup: &H3CellLookup,
    cell_colors: Option<&H3CellColors>,
    fallback_fill_rgba: [f32; 4],
    context: &Map2dRenderContext,
) -> Mesh {
    let cell_colors = cell_colors.filter(|colors| !colors.is_empty());
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    for cell_polygon in h3_indexed_map_cell_polygons(lookup) {
        let polygon = cell_polygon.polygon;
        let projected_vertices = project_h3_map_polygon_to_screen(polygon, context);
        if projected_vertices.len() < 3 {
            continue;
        }

        let start = positions.len();
        positions.extend(
            projected_vertices
                .iter()
                .map(|projected| [projected.x, projected.y, 0.0]),
        );
        if let Some(cell_colors) = cell_colors {
            let color = cell_colors.color_for_cell(cell_polygon.cell_index, fallback_fill_rgba);
            colors.extend(std::iter::repeat_n(color, projected_vertices.len()));
        }
        append_triangle_fan_indices(&mut indices, start, projected_vertices.len(), true);
    }

    triangle_mesh(positions, Vec::new(), colors, indices)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Map overlay vertices are Bevy f32 mesh positions after f64 projection math."
)]
fn build_h3_map_outline_mesh(lookup: &H3CellLookup, context: &Map2dRenderContext) -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();

    for cell_polygon in h3_indexed_map_cell_polygons(lookup) {
        let polygon = cell_polygon.polygon;
        let vertices = project_h3_map_polygon_to_screen(polygon, context);
        if vertices.len() < 2 {
            continue;
        }

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
    }

    line_mesh(positions, Vec::new(), indices)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Globe overlay vertices are Bevy f32 mesh positions after f64 spherical math."
)]
fn build_h3_globe_mesh(
    lookup: &H3CellLookup,
    cell_colors: Option<&H3CellColors>,
    fallback_fill_rgba: [f32; 4],
    radius: f32,
    surface_offset_units: f32,
) -> Mesh {
    let cell_colors = cell_colors.filter(|colors| !colors.is_empty());
    let radius = f64::from(draped_fill_radius(radius, surface_offset_units.max(0.0)));
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    for cell_boundary in h3_indexed_globe_cell_boundaries(lookup) {
        let boundary = cell_boundary.boundary;
        let directions = boundary
            .vertices()
            .iter()
            .map(|vertex| h3_globe_surface_direction(*vertex))
            .collect::<Vec<_>>();
        if directions.len() < 3 {
            continue;
        }

        let center = polygon_center_direction(&directions);
        let color = cell_colors
            .map(|colors| colors.color_for_cell(cell_boundary.cell_index, fallback_fill_rgba));
        append_tessellated_spherical_polygon(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            center,
            &directions,
            radius,
            color,
        );
    }

    triangle_mesh(positions, normals, colors, indices)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Globe outline vertices are Bevy f32 mesh positions after f64 spherical math."
)]
fn build_h3_globe_outline_mesh(
    lookup: &H3CellLookup,
    radius: f32,
    surface_offset_units: f32,
) -> Mesh {
    let radius = f64::from(draped_outline_radius(radius, surface_offset_units.max(0.0)));
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for cell_boundary in h3_indexed_globe_cell_boundaries(lookup) {
        let boundary = cell_boundary.boundary;
        let directions = boundary
            .vertices()
            .iter()
            .map(|vertex| h3_globe_surface_direction(*vertex))
            .collect::<Vec<_>>();
        if directions.len() < 2 {
            continue;
        }

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
    }

    line_mesh(positions, normals, indices)
}

fn triangle_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    mesh_with_topology(
        PrimitiveTopology::TriangleList,
        positions,
        normals,
        colors,
        indices,
    )
}

fn line_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    mesh_with_topology(
        PrimitiveTopology::LineList,
        positions,
        normals,
        Vec::new(),
        indices,
    )
}

fn mesh_with_topology(
    topology: PrimitiveTopology,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        topology,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if !normals.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
    if !colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[derive(Debug, Clone, PartialEq)]
struct H3IndexedMapCellPolygon<'a> {
    cell_index: usize,
    polygon: &'a H3CellPolygon,
}

fn h3_indexed_map_cell_polygons(
    lookup: &H3CellLookup,
) -> impl Iterator<Item = H3IndexedMapCellPolygon<'_>> + '_ {
    lookup.entries().iter().flat_map(|entry| {
        entry
            .map_polygons()
            .iter()
            .map(move |polygon| H3IndexedMapCellPolygon {
                cell_index: entry.cell_index,
                polygon,
            })
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct H3IndexedGlobeCellBoundary<'a> {
    cell_index: usize,
    boundary: &'a H3CellBoundary,
}

fn h3_indexed_globe_cell_boundaries(
    lookup: &H3CellLookup,
) -> impl Iterator<Item = H3IndexedGlobeCellBoundary<'_>> + '_ {
    lookup
        .entries()
        .iter()
        .map(|entry| H3IndexedGlobeCellBoundary {
            cell_index: entry.cell_index,
            boundary: entry.globe_boundary(),
        })
}

#[cfg(test)]
fn point_in_lon_lat_polygon(point: LonLat, vertices: &[LonLat]) -> bool {
    if vertices.len() < 3 {
        return false;
    }

    let normalized_vertices = normalized_polygon_lon_lat_vertices(vertices);
    let point = normalized_point_for_polygon(point, &normalized_vertices);
    let mut inside = false;
    for index in 0..normalized_vertices.len() {
        let current = normalized_vertices[index];
        let next = normalized_vertices[(index + 1) % normalized_vertices.len()];
        if point_on_lon_lat_segment(point, current, next) {
            return true;
        }

        let crosses_lat = (current.y > point.y) != (next.y > point.y);
        if !crosses_lat {
            continue;
        }
        let intersect_lon =
            (next.x - current.x).mul_add((point.y - current.y) / (next.y - current.y), current.x);
        if point.x < intersect_lon {
            inside = !inside;
        }
    }

    inside
}

#[cfg(test)]
fn normalized_polygon_lon_lat_vertices(vertices: &[LonLat]) -> Vec<DVec2> {
    let lon_min = vertices
        .iter()
        .map(|vertex| vertex.lon_deg)
        .fold(f64::INFINITY, f64::min);
    let lon_max = vertices
        .iter()
        .map(|vertex| vertex.lon_deg)
        .fold(f64::NEG_INFINITY, f64::max);
    let crosses_antimeridian = lon_max - lon_min > 180.0;

    vertices
        .iter()
        .map(|vertex| {
            let lon = if crosses_antimeridian && vertex.lon_deg < 0.0 {
                vertex.lon_deg + 360.0
            } else {
                vertex.lon_deg
            };
            DVec2::new(lon, vertex.lat_deg)
        })
        .collect()
}

#[cfg(test)]
fn normalized_point_for_polygon(point: LonLat, vertices: &[DVec2]) -> DVec2 {
    let lon_min = vertices
        .iter()
        .map(|vertex| vertex.x)
        .fold(f64::INFINITY, f64::min);
    let lon_max = vertices
        .iter()
        .map(|vertex| vertex.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut lon = point.lon_deg;
    if lon_max > 180.0 && lon < 0.0 {
        lon += 360.0;
    }
    if lon < lon_min - 180.0 {
        lon += 360.0;
    } else if lon > lon_max + 180.0 {
        lon -= 360.0;
    }

    DVec2::new(lon, point.lat_deg)
}

#[cfg(test)]
fn point_on_lon_lat_segment(point: DVec2, start: DVec2, end: DVec2) -> bool {
    const EPSILON: f64 = 1.0e-9;
    let segment = end - start;
    let to_point = point - start;
    let cross = segment.perp_dot(to_point).abs();
    if cross > EPSILON {
        return false;
    }
    let dot = to_point.dot(segment);
    dot >= -EPSILON && dot <= segment.length_squared() + EPSILON
}

fn append_triangle_fan_indices(
    indices: &mut Vec<u32>,
    start: usize,
    vertex_count: usize,
    clockwise: bool,
) {
    if vertex_count < 3 {
        return;
    }
    let Ok(start) = u32::try_from(start) else {
        return;
    };
    for index in 1..vertex_count.saturating_sub(1) {
        let Ok(index) = u32::try_from(index) else {
            return;
        };
        if clockwise {
            indices.extend_from_slice(&[start, start + index + 1, start + index]);
        } else {
            indices.extend_from_slice(&[start, start + index, start + index + 1]);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper appends one colored tessellated spherical polygon."
)]
fn append_tessellated_spherical_polygon(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    center: DVec3,
    directions: &[DVec3],
    radius: f64,
    color: Option<[f32; 4]>,
) {
    for index in 0..directions.len() {
        let left = directions[index];
        let right = directions[(index + 1) % directions.len()];
        let edge_segments = globe_edge_segments(left, right);
        let radial_segments = edge_segments.max(2);

        for edge_index in 0..edge_segments {
            let left_t = segment_fraction(edge_index, edge_segments);
            let right_t = segment_fraction(edge_index + 1, edge_segments);
            append_tessellated_spherical_wedge(
                positions,
                normals,
                colors,
                indices,
                center,
                slerp_direction(left, right, left_t),
                slerp_direction(left, right, right_t),
                radial_segments,
                radius,
                color,
            );
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper writes positions, normals, and indices for one spherical wedge."
)]
fn append_tessellated_spherical_wedge(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    center: DVec3,
    left: DVec3,
    right: DVec3,
    radial_segments: usize,
    radius: f64,
    color: Option<[f32; 4]>,
) {
    for radial_index in 0..radial_segments {
        let inner_t = segment_fraction(radial_index, radial_segments);
        let outer_t = segment_fraction(radial_index + 1, radial_segments);
        let outer_left = slerp_direction(center, left, outer_t);
        let outer_right = slerp_direction(center, right, outer_t);

        if radial_index == 0 {
            append_spherical_triangle(
                positions,
                normals,
                colors,
                indices,
                center,
                outer_left,
                outer_right,
                radius,
                color,
            );
            continue;
        }

        let inner_left = slerp_direction(center, left, inner_t);
        let inner_right = slerp_direction(center, right, inner_t);
        append_spherical_triangle(
            positions,
            normals,
            colors,
            indices,
            inner_left,
            outer_left,
            inner_right,
            radius,
            color,
        );
        append_spherical_triangle(
            positions,
            normals,
            colors,
            indices,
            inner_right,
            outer_left,
            outer_right,
            radius,
            color,
        );
    }
}

fn append_tessellated_spherical_outline_edge(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: DVec3,
    right: DVec3,
    radius: f64,
) {
    let segments = globe_edge_segments(left, right);
    for index in 0..segments {
        let left_t = segment_fraction(index, segments);
        let right_t = segment_fraction(index + 1, segments);
        append_globe_line_segment(
            positions,
            normals,
            indices,
            slerp_direction(left, right, left_t),
            slerp_direction(left, right, right_t),
            radius,
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "This low-level mesh helper writes positions, normals, colors, and indices for one triangle."
)]
fn append_spherical_triangle(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    first: DVec3,
    second: DVec3,
    third: DVec3,
    radius: f64,
    color: Option<[f32; 4]>,
) {
    let start = positions.len();
    push_globe_vertex(positions, normals, first, radius);
    push_globe_vertex(positions, normals, second, radius);
    push_globe_vertex(positions, normals, third, radius);
    if let Some(color) = color {
        colors.extend_from_slice(&[color; 3]);
    }
    append_triangle_indices(indices, start);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Globe outline mesh coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
fn append_globe_line_segment(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    left: DVec3,
    right: DVec3,
    radius: f64,
) {
    let left = left.normalize_or_zero() * radius;
    let right = right.normalize_or_zero() * radius;
    append_line_segment(
        positions,
        indices,
        [left.x as f32, left.y as f32, left.z as f32],
        [right.x as f32, right.y as f32, right.z as f32],
    );
    push_normal(normals, left);
    push_normal(normals, right);
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Globe overlay mesh coordinates are computed in f64 before entering Bevy's f32 mesh buffers."
)]
fn push_globe_vertex(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    direction: DVec3,
    radius: f64,
) {
    let normal = direction.normalize_or_zero();
    let position = normal * radius;
    positions.push([position.x as f32, position.y as f32, position.z as f32]);
    normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
}

fn append_triangle_indices(indices: &mut Vec<u32>, start: usize) {
    let Ok(start) = u32::try_from(start) else {
        return;
    };
    indices.extend_from_slice(&[start, start + 1, start + 2]);
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
    clippy::cast_sign_loss,
    reason = "A finite angular span is converted into a small bounded tessellation count."
)]
fn globe_edge_segments(left: DVec3, right: DVec3) -> usize {
    let angle = angular_distance(left, right);
    if !angle.is_finite() || angle <= 0.0 {
        return 1;
    }

    ((angle / H3_GLOBE_MAX_EDGE_SEGMENT_RADIANS).ceil() as usize)
        .clamp(1, H3_GLOBE_MAX_EDGE_SEGMENTS)
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
    reason = "H3 mesh tessellation segment counts are tiny and bounded."
)]
fn segment_fraction(index: usize, segments: usize) -> f64 {
    index as f64 / segments.max(1) as f64
}

fn polygon_center_direction(directions: &[DVec3]) -> DVec3 {
    let mut sum = DVec3::ZERO;
    for direction in directions {
        sum += direction.normalize_or_zero();
    }

    let center = sum.normalize_or_zero();
    if center.length_squared() <= f64::EPSILON {
        directions.first().copied().unwrap_or(DVec3::Z)
    } else {
        center
    }
}

fn project_h3_map_polygon_to_screen(
    polygon: &H3CellPolygon,
    context: &Map2dRenderContext,
) -> Vec<Vec2> {
    let mut world_points = polygon
        .vertices()
        .iter()
        .map(|vertex| context.lon_lat_to_world(*vertex))
        .collect::<Vec<_>>();
    // H3 antimeridian clipping can put one ring on both x=0 and x=world_width.
    // Unwrap before triangulation so the fill fan never crosses the map.
    context.unwrap_periodic_x(&mut world_points);
    context.shift_world_copy_near_view(&mut world_points);

    world_points
        .into_iter()
        .map(|world| context.world_to_screen(world))
        .collect()
}

#[cfg(test)]
fn lon_lat_centroid(vertices: &[LonLat]) -> LonLat {
    let mut sum = DVec3::ZERO;
    for vertex in vertices {
        sum += h3_globe_surface_position(*vertex, 1.0).normalize_or_zero();
    }
    let center = sum.normalize_or_zero();
    if center.length_squared() <= f64::EPSILON {
        return vertices.first().copied().unwrap_or_default();
    }

    LonLat::new(
        center.x.atan2(center.z).to_degrees(),
        center.y.asin().to_degrees(),
    )
}

fn h3_globe_surface_position(lon_lat: LonLat, radius: f64) -> DVec3 {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = radius * lat_rad.cos();

    DVec3::new(
        horizontal_radius * lon_rad.sin(),
        radius * lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}

fn h3_globe_surface_direction(lon_lat: LonLat) -> DVec3 {
    h3_globe_surface_position(lon_lat, 1.0).normalize_or_zero()
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

fn h3_map_material(rgba: [f32; 4]) -> ColorMaterial {
    // `ColorMaterial::default()` uses `AlphaMode2d::Blend`, while `ColorMaterial::from(Color)`
    // switches to `Opaque` when alpha >= 1.0. Per-cell vertex colors can carry their own alpha, so
    // we always need blend.
    ColorMaterial {
        color: color_from_rgba(rgba),
        ..ColorMaterial::default()
    }
}

fn h3_fill_material_rgba(overlay: &H3Overlay) -> [f32; 4] {
    if overlay
        .cell_colors
        .as_ref()
        .is_some_and(|colors| !colors.is_empty())
    {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        overlay.style.fill_rgba()
    }
}

fn h3_globe_material(color: Color, layer: SphericalSurfaceLayer) -> StandardMaterial {
    let mut material = StandardMaterial::default();
    update_h3_globe_material(&mut material, color, layer);
    material
}

fn update_h3_globe_material(
    material: &mut StandardMaterial,
    color: Color,
    layer: SphericalSurfaceLayer,
) {
    apply_spherical_decal_material(material, color, layer);
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;
    use bevy::prelude::{Entity, Mesh, Vec2, Visibility};
    use ferrisium_core::prelude::{LonLat, PlanetaryBody, TileProjection, Vec2d, ViewState};

    use crate::celestial::CelestialFocus;
    use crate::source::ActiveTileSource;
    use crate::surface_depth::{
        draped_fill_radius, draped_outline_radius, spherical_surface_depth_bias,
        SphericalSurfaceLayer,
    };
    use crate::view::Map2dRenderContext;

    use super::{
        build_h3_globe_mesh, build_h3_globe_outline_mesh, build_h3_map_mesh,
        build_h3_map_outline_mesh, clear_h3_hover_for_surface, globe_edge_segments,
        h3_fill_material_rgba, h3_globe_material, h3_globe_surface_direction,
        h3_globe_surface_position, h3_map_view_key, lon_lat_centroid, lon_lat_from_globe_direction,
        pick_h3_globe_cell, pick_h3_map_cell, point_in_lon_lat_polygon,
        project_h3_map_polygon_to_screen, H3CellColors, H3CellLookup, H3CellPolygon, H3Cells,
        H3ColorKey, H3HoverState, H3Layer, H3Overlay, H3OverlayBundle, H3PickSurface, H3Style,
        DEFAULT_H3_GLOBE_SURFACE_OFFSET_UNITS,
    };

    const RAW_TEST_CELL: u64 = 0x0837_506f_ffff_ffff_u64;
    const RAW_TRANSMERIDIAN_HEX: u64 = 0x0840_d9ed_ffff_ffff_u64;

    fn test_cells() -> H3Cells {
        H3Cells::try_from_raw([RAW_TEST_CELL]).unwrap_or_default()
    }

    fn transmeridian_cells() -> H3Cells {
        H3Cells::try_from_raw([RAW_TRANSMERIDIAN_HEX]).unwrap_or_default()
    }

    fn test_lookup() -> H3CellLookup {
        H3CellLookup::from_cells(&test_cells())
    }

    fn transmeridian_lookup() -> H3CellLookup {
        H3CellLookup::from_cells(&transmeridian_cells())
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

    fn projected_x_span(points: &[Vec2]) -> f32 {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        for point in points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
        }
        max_x - min_x
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

    fn assert_triangle_x_spans_below(mesh: &Mesh, max_span: f32) {
        let positions = mesh_positions(mesh);
        for triangle in mesh_u32_indices(mesh).chunks_exact(3) {
            let xs = triangle
                .iter()
                .filter_map(|index| positions.get(*index as usize))
                .map(|position| position[0])
                .collect::<Vec<_>>();
            assert_eq!(xs.len(), 3);
            let span = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                - xs.iter().copied().fold(f32::INFINITY, f32::min);
            assert!(
                span <= max_span,
                "triangle should not cross the map wrap, got span {span} from {xs:?}"
            );
        }
    }

    fn assert_line_x_spans_below(mesh: &Mesh, max_span: f32) {
        let positions = mesh_positions(mesh);
        for segment in mesh_u32_indices(mesh).chunks_exact(2) {
            let left = positions.get(segment[0] as usize);
            let right = positions.get(segment[1] as usize);
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

    fn assert_rgba_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "rgba mismatch: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn earth_overlay_defaults_to_earth_map_and_globe_targets() {
        let overlay = H3Overlay::earth(
            "coverage",
            H3Cells::default(),
            H3Style::solid([1.0, 0.0, 0.0, 0.5]),
        );

        assert_eq!(overlay.target, ferrisium_core::prelude::BodyId::EARTH);
        assert_eq!(overlay.body, PlanetaryBody::earth());
        assert_eq!(overlay.layer.name(), "coverage");
        assert!(overlay.map_enabled);
        assert!(overlay.globe_enabled);
        assert!(overlay.cell_colors.is_none());
        assert!(
            (overlay.globe_surface_offset_units - DEFAULT_H3_GLOBE_SURFACE_OFFSET_UNITS).abs()
                <= f32::EPSILON
        );
        assert!(overlay.globe_surface_offset_units >= 0.0);
        assert_eq!(
            H3OverlayBundle::new(overlay).visibility,
            Visibility::Inherited
        );
    }

    #[test]
    fn map_mesh_projects_h3_cells_into_2d_vertices() {
        let view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let mesh = build_h3_map_mesh(
            &test_lookup(),
            None,
            [1.0, 0.0, 0.0, 0.5],
            &map_context(view_state),
        );

        assert!(mesh.indices().map_or(0, bevy::mesh::Indices::len) >= 3);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
    }

    #[test]
    fn map_polygon_projection_keeps_unsplit_antimeridian_ring_continuous() {
        let polygon = H3CellPolygon::new(vec![
            LonLat::new(179.5, 1.0),
            LonLat::new(-179.5, 1.0),
            LonLat::new(-179.5, -1.0),
            LonLat::new(179.5, -1.0),
        ]);
        let projected =
            project_h3_map_polygon_to_screen(&polygon, &map_context(antimeridian_view_state()));

        assert_eq!(projected.len(), 4);
        assert!(
            projected_x_span(&projected) < 8.0,
            "antimeridian polygon should stay local, got x positions: {projected:?}"
        );
    }

    #[test]
    fn map_polygon_projection_keeps_clipped_antimeridian_edge_continuous() {
        let polygon = H3CellPolygon::new(vec![
            LonLat::new(179.5, 1.0),
            LonLat::new(180.0, 1.0),
            LonLat::new(180.0, -1.0),
            LonLat::new(179.5, -1.0),
        ]);
        let projected =
            project_h3_map_polygon_to_screen(&polygon, &map_context(antimeridian_view_state()));

        assert_eq!(projected.len(), 4);
        assert!(
            projected_x_span(&projected) < 8.0,
            "H3 antimeridian clip edge should stay local, got x positions: {projected:?}"
        );
    }

    #[test]
    fn map_mesh_keeps_real_transmeridian_h3_edges_local() {
        let lookup = transmeridian_lookup();
        let fill = build_h3_map_mesh(
            &lookup,
            None,
            [1.0, 0.0, 0.0, 0.5],
            &map_context(antimeridian_view_state()),
        );
        let outline = build_h3_map_outline_mesh(&lookup, &map_context(antimeridian_view_state()));

        assert_triangle_x_spans_below(&fill, 128.0);
        assert_line_x_spans_below(&outline, 128.0);
    }

    #[test]
    fn map_mesh_emits_per_cell_vertex_colors_when_supplied() {
        let view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let color = [0.1, 0.2, 0.3, 0.4];
        let mesh = build_h3_map_mesh(
            &test_lookup(),
            Some(&H3CellColors::from_rgba([color])),
            [1.0, 0.0, 0.0, 0.5],
            &map_context(view_state),
        );
        let position_count = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map_or(0, VertexAttributeValues::len);
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR);

        assert!(position_count > 0);
        assert!(
            matches!(colors, Some(VertexAttributeValues::Float32x4(_))),
            "expected Float32x4 vertex colors"
        );
        let Some(VertexAttributeValues::Float32x4(colors)) = colors else {
            return;
        };
        assert_eq!(colors.len(), position_count);
        for actual in colors {
            assert_rgba_close(*actual, color);
        }
    }

    #[test]
    fn map_mesh_omits_vertex_colors_for_empty_cell_color_set() {
        let view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let mesh = build_h3_map_mesh(
            &test_lookup(),
            Some(&H3CellColors::default()),
            [1.0, 0.0, 0.0, 0.5],
            &map_context(view_state),
        );

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
    }

    #[test]
    fn map_outline_mesh_draws_h3_boundaries() {
        let view_state = ViewState {
            center: LonLat::new(-98.0, 39.0),
            zoom: 3.0,
            viewport_size: Vec2d::new(1024.0, 768.0),
            tile_size: 256.0,
            projection: TileProjection::WebMercator,
        };
        let mesh = build_h3_map_outline_mesh(&test_lookup(), &map_context(view_state));

        assert!(mesh.indices().map_or(0, bevy::mesh::Indices::len) >= 6);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
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
        let initial_key = h3_map_view_key(&map_context(view_state));

        view_state.zoom = 4.0;
        assert_ne!(h3_map_view_key(&map_context(view_state)), initial_key);
        view_state.zoom = 3.0;
        view_state.center.lon_deg = -97.5;
        assert_ne!(h3_map_view_key(&map_context(view_state)), initial_key);
    }

    #[test]
    fn color_key_changes_only_when_style_rgba_changes() {
        let color = [0.24, 0.63, 0.86, 0.42];

        assert_eq!(H3ColorKey::from_rgba(color), H3ColorKey::from_rgba(color));
        assert_ne!(
            H3ColorKey::from_rgba(color),
            H3ColorKey::from_rgba([0.24, 0.63, 0.86, 0.5])
        );
    }

    #[test]
    fn h3_cell_lookup_hits_cell_centroid() {
        let cells = test_cells();
        let lookup = H3CellLookup::from_cells(&cells);
        let centroid = cells.cells()[0].center_lon_lat();

        let hit = lookup.cell_at_lon_lat(centroid);
        assert!(hit.is_some(), "expected centroid to hit the test cell");
        let Some((cell_index, cell)) = hit else {
            return;
        };
        assert_eq!(cell_index, 0);
        assert_eq!(cell, cells.cells()[0]);
    }

    #[test]
    fn h3_cell_lookup_misses_opposite_hemisphere() {
        let cells = test_cells();
        let lookup = H3CellLookup::from_cells(&cells);
        let centroid = cells.cells()[0].center_lon_lat();
        let opposite_lon = (centroid.lon_deg + 540.0).rem_euclid(360.0) - 180.0;
        let opposite = LonLat::new(opposite_lon, -centroid.lat_deg);

        assert!(lookup.cell_at_lon_lat(opposite).is_none());
    }

    #[test]
    fn h3_cell_lookup_hits_transmeridian_cell_center() {
        let cells = transmeridian_cells();
        let lookup = H3CellLookup::from_cells(&cells);
        let center = cells.cells()[0].center_lon_lat();

        assert_eq!(lookup.cell_at_lon_lat(center), Some((0, cells.cells()[0])));
    }

    #[test]
    fn lon_lat_polygon_hit_test_includes_boundaries() {
        let polygon = [
            LonLat::new(0.0, 0.0),
            LonLat::new(1.0, 0.0),
            LonLat::new(1.0, 1.0),
            LonLat::new(0.0, 1.0),
        ];

        assert!(point_in_lon_lat_polygon(LonLat::new(0.5, 0.5), &polygon));
        assert!(point_in_lon_lat_polygon(LonLat::new(1.0, 0.5), &polygon));
        assert!(!point_in_lon_lat_polygon(LonLat::new(1.5, 0.5), &polygon));
    }

    #[test]
    fn globe_direction_round_trips_to_lon_lat_axes() {
        let east = lon_lat_from_globe_direction(bevy::math::DVec3::X);
        let north = lon_lat_from_globe_direction(bevy::math::DVec3::Y);

        assert!((east.lon_deg - 90.0).abs() <= f64::EPSILON);
        assert!(east.lat_deg.abs() <= f64::EPSILON);
        assert!((north.lat_deg - 90.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn h3_material_uses_white_base_when_vertex_colors_are_present() {
        let overlay = H3Overlay::earth(
            "coverage",
            H3Cells::default(),
            H3Style::solid([0.25, 0.5, 0.75, 0.8]),
        )
        .with_cell_colors(H3CellColors::from_rgba([[1.0, 0.0, 0.0, 1.0]]));

        assert_rgba_close(h3_fill_material_rgba(&overlay), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn globe_mesh_drapes_h3_vertices_on_requested_radius() {
        let radius = 10.0;
        let offset = 0.25;
        let mesh = build_h3_globe_mesh(&test_lookup(), None, [1.0, 0.0, 0.0, 0.5], radius, offset);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        assert!(positions.is_some());
        let Some(positions) = positions else {
            return;
        };

        assert!(!positions.is_empty());
        let expected_radius = draped_fill_radius(radius, offset);
        for position in positions {
            let length = f32::hypot(position[0], f32::hypot(position[1], position[2]));
            assert!((length - expected_radius).abs() <= 1.0e-4);
        }
    }

    #[test]
    fn globe_mesh_tessellates_h3_cells_beyond_one_fan_triangle_per_edge() {
        let lookup = test_lookup();
        let mesh = build_h3_globe_mesh(&lookup, None, [1.0, 0.0, 0.0, 0.5], 10.0, 0.25);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        let boundary_vertex_count = lookup
            .entries()
            .iter()
            .map(|entry| entry.globe_boundary().vertices().len())
            .sum::<usize>();

        assert!(positions.is_some());
        let Some(positions) = positions else {
            return;
        };
        assert!(positions.len() > boundary_vertex_count + lookup.entries().len());
    }

    #[test]
    fn globe_mesh_emits_per_cell_vertex_colors_when_supplied() {
        let color = [0.8, 0.1, 0.2, 0.6];
        let mesh = build_h3_globe_mesh(
            &test_lookup(),
            Some(&H3CellColors::from_rgba([color])),
            [0.0, 1.0, 0.0, 0.5],
            10.0,
            0.25,
        );
        let position_count = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map_or(0, VertexAttributeValues::len);
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR);

        assert!(position_count > 0);
        assert!(
            matches!(colors, Some(VertexAttributeValues::Float32x4(_))),
            "expected Float32x4 vertex colors"
        );
        let Some(VertexAttributeValues::Float32x4(colors)) = colors else {
            return;
        };
        assert_eq!(colors.len(), position_count);
        for actual in colors {
            assert_rgba_close(*actual, color);
        }
    }

    #[test]
    fn globe_mesh_omits_vertex_colors_for_empty_cell_color_set() {
        let mesh = build_h3_globe_mesh(
            &test_lookup(),
            Some(&H3CellColors::default()),
            [0.0, 1.0, 0.0, 0.5],
            10.0,
            0.25,
        );

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
    }

    #[test]
    fn globe_outline_mesh_drapes_edges_on_offset_radius() {
        let radius = 10.0;
        let offset = 0.25;
        let mesh = build_h3_globe_outline_mesh(&test_lookup(), radius, offset);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3);
        assert!(positions.is_some());
        let Some(positions) = positions else {
            return;
        };

        assert!(!positions.is_empty());
        let fill_radius = draped_fill_radius(radius, offset);
        let outline_radius = draped_outline_radius(radius, offset);
        for position in positions {
            let length = f32::hypot(position[0], f32::hypot(position[1], position[2]));
            assert!(
                length > fill_radius,
                "outline vertices should sit above the fill surface"
            );
            assert!((length - outline_radius).abs() <= 1.0e-4);
        }
    }

    #[test]
    fn spherical_centroid_stays_near_cell_boundary_directions() {
        let lookup = test_lookup();
        let Some(entry) = lookup.entries().first() else {
            return;
        };
        let centroid = lon_lat_centroid(entry.globe_boundary().vertices());
        let center = h3_globe_surface_position(centroid, 1.0);

        assert!((center.length() - 1.0).abs() <= 1.0e-9);
    }

    #[test]
    fn globe_outline_uses_unsplit_transmeridian_h3_boundary() {
        let lookup = transmeridian_lookup();
        let Some(entry) = lookup.entries().first() else {
            return;
        };
        assert_eq!(entry.map_polygons().len(), 2);
        assert_eq!(entry.globe_boundary().vertices().len(), 6);

        let mesh = build_h3_globe_outline_mesh(&lookup, 10.0, 0.25);
        let expected_segments = entry
            .globe_boundary()
            .vertices()
            .iter()
            .map(|vertex| h3_globe_surface_direction(*vertex))
            .collect::<Vec<_>>();
        let expected_index_count = (0..expected_segments.len())
            .map(|index| {
                globe_edge_segments(
                    expected_segments[index],
                    expected_segments[(index + 1) % expected_segments.len()],
                ) * 2
            })
            .sum::<usize>();

        assert_eq!(
            mesh.indices().map_or(0, bevy::mesh::Indices::len),
            expected_index_count
        );
    }

    #[test]
    fn globe_picking_prefers_higher_surface_offset_overlay() {
        let cells = test_cells();
        let lookup = H3CellLookup::from_cells(&cells);
        let low_entity = Entity::from_raw_u32(1).unwrap_or(Entity::PLACEHOLDER);
        let high_entity = Entity::from_raw_u32(2).unwrap_or(Entity::PLACEHOLDER);
        let low = H3Overlay::earth("low", cells.clone(), H3Style::solid([1.0, 0.0, 0.0, 0.5]))
            .with_globe_surface_offset_units(0.0);
        let high = H3Overlay::earth("high", cells.clone(), H3Style::solid([0.0, 1.0, 0.0, 0.5]))
            .with_globe_surface_offset_units(2.0);

        let hit = pick_h3_globe_cell(
            cells.cells()[0].center_lon_lat(),
            CelestialFocus::earth(),
            [
                (low_entity, &low, &lookup, None, None),
                (high_entity, &high, &lookup, None, None),
            ],
        );

        assert_eq!(hit.map(|hit| hit.overlay), Some(high_entity));
    }

    #[test]
    fn map_picking_prefers_higher_map_z_overlay() {
        let cells = test_cells();
        let lookup = H3CellLookup::from_cells(&cells);
        let low_entity = Entity::from_raw_u32(1).unwrap_or(Entity::PLACEHOLDER);
        let high_entity = Entity::from_raw_u32(2).unwrap_or(Entity::PLACEHOLDER);
        let low = H3Overlay::earth("low", cells.clone(), H3Style::solid([1.0, 0.0, 0.0, 0.5]))
            .with_map_z(1.0);
        let high = H3Overlay::earth("high", cells.clone(), H3Style::solid([0.0, 1.0, 0.0, 0.5]))
            .with_map_z(2.0);

        let hit = pick_h3_map_cell(
            cells.cells()[0].center_lon_lat(),
            &ActiveTileSource::default(),
            [
                (low_entity, &low, &lookup, None, None),
                (high_entity, &high, &lookup, None, None),
            ],
        );

        assert_eq!(hit.map(|hit| hit.overlay), Some(high_entity));
    }

    #[test]
    fn inactive_surface_clears_only_its_own_hover() {
        let cells = test_cells();
        let map_hit = super::H3CellHit {
            overlay: Entity::from_raw_u32(1).unwrap_or(Entity::PLACEHOLDER),
            layer: H3Layer::new("map"),
            target: ferrisium_core::prelude::BodyId::EARTH,
            body: PlanetaryBody::earth(),
            cell_index: 0,
            cell: cells.cells()[0],
            lon_lat: cells.cells()[0].center_lon_lat(),
            surface: H3PickSurface::Map2d,
        };
        let mut hover = H3HoverState {
            hit: Some(map_hit.clone()),
        };

        clear_h3_hover_for_surface(&mut hover, H3PickSurface::Globe3d);
        assert_eq!(hover.hit, Some(map_hit));
        clear_h3_hover_for_surface(&mut hover, H3PickSurface::Map2d);
        assert!(hover.hit.is_none());
    }

    #[test]
    fn globe_material_uses_depth_bias_for_decal_stability() {
        let material = h3_globe_material(
            bevy::prelude::Color::srgba(1.0, 0.0, 0.0, 0.5),
            SphericalSurfaceLayer::DrapedFill,
        );

        assert_eq!(material.alpha_mode, bevy::prelude::AlphaMode::Blend);
        assert!(
            (material.depth_bias - spherical_surface_depth_bias(SphericalSurfaceLayer::DrapedFill))
                .abs()
                <= f32::EPSILON
        );
        assert!(material.unlit);
        assert!(material.cull_mode.is_none());
    }
}
