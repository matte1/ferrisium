#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::{
    apply_spherical_decal_material, GlobeSurfaceRenderContext, Map2dRenderContext,
    SphericalSurfaceLayer,
};
#[cfg(target_arch = "wasm32")]
use ferrisium_core::prelude::{BodyId, LonLat, PlanetaryBody};

#[cfg(target_arch = "wasm32")]
const CUSTOM_MAP_Z: f32 = 38.0;
#[cfg(target_arch = "wasm32")]
const CUSTOM_GLOBE_OFFSET_UNITS: f32 = 10.0;

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub(crate) struct DemoCustomMapLayer {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<ColorMaterial>>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub(crate) struct DemoCustomGlobeLayer {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    parent: Option<Entity>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn sync_demo_custom_map_layer(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    map_contexts: Query<'_, '_, &Map2dRenderContext>,
    mut layer: ResMut<'_, DemoCustomMapLayer>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<ColorMaterial>>,
) {
    let Some(context) = map_contexts.iter().next() else {
        despawn_custom_map_layer(&mut commands, &mut layer);
        return;
    };
    if context.body != PlanetaryBody::earth() {
        despawn_custom_map_layer(&mut commands, &mut layer);
        return;
    }

    let route = animated_context_route(time.elapsed_secs_f64());
    let projected = context.project_line_string_to_screen(&route);
    let mesh = Map2dRenderContext::line_list_mesh(&projected);

    if let Some(mesh_handle) = layer
        .mesh
        .as_ref()
        .and_then(|handle| meshes.get_mut(handle))
    {
        *mesh_handle = mesh;
    } else {
        layer.mesh = Some(meshes.add(mesh));
    }

    if layer.material.is_none() {
        layer.material =
            Some(materials.add(ColorMaterial::from(Color::srgba(0.1, 0.95, 0.35, 0.95))));
    }

    if layer.entity.is_none() {
        let Some(mesh) = layer.mesh.clone() else {
            return;
        };
        let Some(material) = layer.material.clone() else {
            return;
        };
        layer.entity = Some(
            commands
                .spawn((
                    Name::new("Custom Map Context Route"),
                    Mesh2d(mesh),
                    MeshMaterial2d(material),
                    Transform::from_xyz(0.0, 0.0, CUSTOM_MAP_Z),
                ))
                .id(),
        );
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn sync_demo_custom_globe_layer(
    mut commands: Commands<'_, '_>,
    time: Res<'_, Time>,
    globe_contexts: Query<'_, '_, &GlobeSurfaceRenderContext>,
    mut layer: ResMut<'_, DemoCustomGlobeLayer>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let Some(context) = globe_contexts
        .iter()
        .find(|context| context.target == BodyId::EARTH)
    else {
        despawn_custom_globe_layer(&mut commands, &mut layer);
        return;
    };

    let route = animated_context_route(time.elapsed_secs_f64());
    let radius = context.draped_outline_radius(CUSTOM_GLOBE_OFFSET_UNITS);
    let positions = context.project_geodesic_line_string(&route, radius, 0.02);
    let mesh = GlobeSurfaceRenderContext::line_list_mesh(&positions);

    if let Some(mesh_handle) = layer
        .mesh
        .as_ref()
        .and_then(|handle| meshes.get_mut(handle))
    {
        *mesh_handle = mesh;
    } else {
        layer.mesh = Some(meshes.add(mesh));
    }

    if layer.material.is_none() {
        let mut material = StandardMaterial::default();
        apply_spherical_decal_material(
            &mut material,
            Color::srgba(0.1, 0.95, 0.35, 0.95),
            SphericalSurfaceLayer::DrapedOutline,
        );
        layer.material = Some(materials.add(material));
    }

    if layer.entity.is_none() {
        let Some(mesh) = layer.mesh.clone() else {
            return;
        };
        let Some(material) = layer.material.clone() else {
            return;
        };
        let entity = commands
            .spawn((
                Name::new("Custom Globe Context Route"),
                Mesh3d(mesh),
                MeshMaterial3d(material),
            ))
            .id();
        commands.entity(context.surface_anchor).add_child(entity);
        layer.entity = Some(entity);
        layer.parent = Some(context.surface_anchor);
    } else if layer.parent != Some(context.surface_anchor) {
        if let Some(entity) = layer.entity {
            commands.entity(context.surface_anchor).add_child(entity);
            layer.parent = Some(context.surface_anchor);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn animated_context_route(elapsed_seconds: f64) -> [LonLat; 3] {
    let phase = (elapsed_seconds * 0.45).sin() * 8.0;
    [
        LonLat::new(170.0, 21.0),
        LonLat::new(-178.0 + phase, 29.0),
        LonLat::new(-157.8, 21.3),
    ]
}

#[cfg(target_arch = "wasm32")]
fn despawn_custom_map_layer(commands: &mut Commands<'_, '_>, layer: &mut DemoCustomMapLayer) {
    if let Some(entity) = layer.entity.take() {
        commands.entity(entity).despawn();
    }
    layer.mesh = None;
    layer.material = None;
}

#[cfg(target_arch = "wasm32")]
fn despawn_custom_globe_layer(commands: &mut Commands<'_, '_>, layer: &mut DemoCustomGlobeLayer) {
    if let Some(entity) = layer.entity.take() {
        commands.entity(entity).despawn();
    }
    layer.mesh = None;
    layer.material = None;
    layer.parent = None;
}
