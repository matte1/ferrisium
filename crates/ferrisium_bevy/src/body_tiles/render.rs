//! Mesh/material synchronization for fixed-zoom body surfaces.
//!
//! This module renders each tile in a body's fixed global matrix as a curved
//! patch using the same mesh/material helpers as the primary globe renderer.
//! Missing tiles stay visible through body-specific placeholder colors.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::prelude::{
    LonLat, PlanetaryBody, TileId, TileProjection, TileSource, WEB_MERCATOR_MAX_LAT_DEG,
};

use crate::body_tiles::requests::body_surface_tiles;
use crate::body_tiles::state::{
    BodySurfacePatchHandles, BodySurfacePolarCapHandles, BodySurfaceState,
    BodySurfaceTileSlotState, BodySurfaceTileState,
};
use crate::surface_depth::{spherical_surface_layer_radius, SphericalSurfaceLayer};
use crate::tile_globe::{
    build_globe_tile_material, build_globe_tile_mesh, globe_patch_segments,
    update_globe_tile_material,
};
use crate::tiles::TileTextureBinding;

// Secondary body surfaces are intentionally conservative until they have
// camera-driven LOD; the Moon demo uses zoom 3, which means 128 tiles.
const BODY_SURFACE_PATCH_SEGMENTS: u32 = 16;
const BODY_SURFACE_CAP_SEGMENTS: u32 = 64;

/// Marker attached to spawned fixed-zoom body-surface patch entities.
#[derive(Component)]
struct BodySurfaceTilePatch;

/// Marker attached to Web Mercator polar fill caps for a body surface.
#[derive(Component)]
struct BodySurfacePolarCap;

/// Spawns missing body-surface patches and refreshes their material bindings.
#[allow(
    clippy::too_many_arguments,
    reason = "Patch synchronization needs explicit ECS asset stores and body state."
)]
pub(super) fn sync_body_surface_patches(
    commands: &mut Commands<'_, '_>,
    body: Entity,
    source: &TileSource,
    zoom: u8,
    radius: f32,
    tile_state: &mut BodySurfaceTileState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let Some(body_state) = tile_state.bodies.get_mut(&body) else {
        return;
    };

    let raster_radius =
        spherical_surface_layer_radius(radius, SphericalSurfaceLayer::RasterSurface);
    let polar_cap_radius = spherical_surface_layer_radius(radius, SphericalSurfaceLayer::PolarCap);

    sync_body_surface_polar_caps(
        commands,
        body,
        source,
        polar_cap_radius,
        body_state,
        meshes,
        materials,
    );

    for tile in body_surface_tiles(source, zoom) {
        let binding = body_surface_tile_binding(tile, body_state);
        if let Some(handles) = body_state.patches.get(&tile) {
            if let Some(material) = materials.get_mut(&handles.material) {
                update_globe_tile_material(material, tile, &binding);
            }
            continue;
        }

        let mesh = meshes.add(build_globe_tile_mesh(
            tile,
            source.projection,
            raster_radius,
            globe_patch_segments(tile).max(BODY_SURFACE_PATCH_SEGMENTS),
            binding.uv_rect,
        ));
        let material = materials.add(build_globe_tile_material(tile, &binding));
        let patch = commands
            .spawn((
                Name::new(format!("Body Surface Tile {tile:?}")),
                BodySurfaceTilePatch,
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        commands.entity(body).add_child(patch);
        body_state.patches.insert(
            tile,
            BodySurfacePatchHandles {
                entity: patch,
                material,
            },
        );
    }
}

/// Ensures Web Mercator surfaces have simple lit caps where the projection ends.
fn sync_body_surface_polar_caps(
    commands: &mut Commands<'_, '_>,
    body: Entity,
    source: &TileSource,
    radius: f32,
    body_state: &mut BodySurfaceState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if source.projection != TileProjection::WebMercator {
        despawn_body_surface_polar_caps(commands, body_state);
        return;
    }

    if let Some(polar_caps) = &body_state.polar_caps {
        if (polar_caps.radius - radius).abs() <= f32::EPSILON {
            return;
        }
        despawn_body_surface_polar_caps(commands, body_state);
    }

    let material = materials.add(StandardMaterial {
        base_color: body_surface_polar_cap_color(&source.body),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        cull_mode: None,
        ..default()
    });
    let north = commands
        .spawn((
            Name::new("Body Surface North Polar Cap"),
            BodySurfacePolarCap,
            Mesh3d(meshes.add(build_body_surface_polar_cap_mesh(radius, true))),
            MeshMaterial3d(material.clone()),
        ))
        .id();
    let south = commands
        .spawn((
            Name::new("Body Surface South Polar Cap"),
            BodySurfacePolarCap,
            Mesh3d(meshes.add(build_body_surface_polar_cap_mesh(radius, false))),
            MeshMaterial3d(material),
        ))
        .id();
    commands.entity(body).add_child(north);
    commands.entity(body).add_child(south);
    body_state.polar_caps = Some(BodySurfacePolarCapHandles {
        north,
        south,
        radius,
    });
}

/// Removes any existing Web Mercator polar caps for one body surface.
fn despawn_body_surface_polar_caps(
    commands: &mut Commands<'_, '_>,
    body_state: &mut BodySurfaceState,
) {
    let Some(polar_caps) = body_state.polar_caps.take() else {
        return;
    };

    commands.entity(polar_caps.north).despawn();
    commands.entity(polar_caps.south).despawn();
}

/// Builds the shared globe-material binding for a body-surface tile.
fn body_surface_tile_binding(tile: TileId, body_state: &BodySurfaceState) -> TileTextureBinding {
    if let Some(image) = body_state
        .slots
        .get(&tile)
        .and_then(|slot| match &slot.state {
            BodySurfaceTileSlotState::Ready { image } => Some(image.clone()),
            _ => None,
        })
    {
        return TileTextureBinding {
            image: Some(image),
            uv_rect: Rect::from_corners(Vec2::ZERO, Vec2::ONE),
            color: Color::WHITE,
        };
    }

    TileTextureBinding {
        image: None,
        uv_rect: Rect::from_corners(Vec2::ZERO, Vec2::ONE),
        color: body_surface_placeholder_color(&body_state.source.body, tile),
    }
}

/// Returns a body-specific placeholder color while fixed body tiles are missing.
fn body_surface_placeholder_color(body: &PlanetaryBody, tile: TileId) -> Color {
    let alternate = (tile.x + tile.y).is_multiple_of(2);
    match body.name.as_str() {
        "Earth" => {
            if alternate {
                Color::srgb(0.18, 0.44, 0.72)
            } else {
                Color::srgb(0.16, 0.48, 0.30)
            }
        }
        "Mercury" => {
            if alternate {
                Color::srgb(0.38, 0.36, 0.33)
            } else {
                Color::srgb(0.54, 0.51, 0.47)
            }
        }
        "Venus" => {
            if alternate {
                Color::srgb(0.62, 0.42, 0.58)
            } else {
                Color::srgb(0.84, 0.63, 0.32)
            }
        }
        "Moon" => {
            if alternate {
                Color::srgb(0.38, 0.38, 0.36)
            } else {
                Color::srgb(0.52, 0.52, 0.49)
            }
        }
        "Mars" => {
            if alternate {
                Color::srgb(0.50, 0.28, 0.18)
            } else {
                Color::srgb(0.65, 0.38, 0.24)
            }
        }
        _ => {
            if alternate {
                Color::srgb(0.15, 0.18, 0.24)
            } else {
                Color::srgb(0.24, 0.29, 0.37)
            }
        }
    }
}

/// Returns a neutral cap color for Web Mercator polar gaps.
fn body_surface_polar_cap_color(body: &PlanetaryBody) -> Color {
    match body.name.as_str() {
        "Earth" => Color::srgb(0.78, 0.80, 0.78),
        "Mercury" => Color::srgb(0.54, 0.51, 0.47),
        "Venus" => Color::srgb(0.84, 0.63, 0.32),
        "Moon" => Color::srgb(0.52, 0.52, 0.49),
        "Mars" => Color::srgb(0.65, 0.38, 0.24),
        _ => Color::srgb(0.24, 0.29, 0.37),
    }
}

/// Builds one triangular fan from the pole to the Web Mercator latitude limit.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy mesh data is f32-based while cap geometry is derived in f64."
)]
fn build_body_surface_polar_cap_mesh(radius: f32, north: bool) -> Mesh {
    let pole_lat = if north { 90.0 } else { -90.0 };
    let ring_lat = if north {
        WEB_MERCATOR_MAX_LAT_DEG
    } else {
        -WEB_MERCATOR_MAX_LAT_DEG
    };
    let radius = f64::from(radius);
    let mut positions =
        Vec::with_capacity(usize::try_from(BODY_SURFACE_CAP_SEGMENTS + 2).unwrap_or(0));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::new();

    let pole = body_surface_position(LonLat::new(0.0, pole_lat), radius);
    positions.push([pole.x as f32, pole.y as f32, pole.z as f32]);
    let pole_normal = pole.normalize_or_zero();
    normals.push([
        pole_normal.x as f32,
        pole_normal.y as f32,
        pole_normal.z as f32,
    ]);

    for segment in 0..=BODY_SURFACE_CAP_SEGMENTS {
        let lon_deg = -180.0 + 360.0 * f64::from(segment) / f64::from(BODY_SURFACE_CAP_SEGMENTS);
        let position = body_surface_position(LonLat::new(lon_deg, ring_lat), radius);
        let normal = position.normalize_or_zero();

        positions.push([position.x as f32, position.y as f32, position.z as f32]);
        normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
    }

    for segment in 0..BODY_SURFACE_CAP_SEGMENTS {
        let current = segment + 1;
        let next = current + 1;
        if north {
            indices.extend_from_slice(&[0, next, current]);
        } else {
            indices.extend_from_slice(&[0, current, next]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Converts a lon/lat sample into local spherical coordinates.
fn body_surface_position(lon_lat: LonLat, radius: f64) -> bevy::math::DVec3 {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat.lat_deg.to_radians();
    let horizontal_radius = radius * lat_rad.cos();

    bevy::math::DVec3::new(
        horizontal_radius * lon_rad.sin(),
        radius * lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}

/// Removes all patch entities and in-flight bookkeeping for one body.
pub(super) fn despawn_body_surface(
    commands: &mut Commands<'_, '_>,
    tile_state: &mut BodySurfaceTileState,
    body: Entity,
) {
    let Some(mut state) = tile_state.bodies.remove(&body) else {
        return;
    };

    for request_id in state.slots.values().filter_map(|slot| match slot.state {
        BodySurfaceTileSlotState::Loading { request_id } => Some(request_id),
        _ => None,
    }) {
        tile_state.in_flight_request_ids.remove(&request_id);
    }

    despawn_body_surface_polar_caps(commands, &mut state);

    for patch in state.patches.drain().map(|(_, handles)| handles.entity) {
        commands.entity(patch).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::body_surface_tile_binding;
    use crate::body_tiles::state::{
        BodySurfaceState, BodySurfaceTileSlot, BodySurfaceTileSlotState,
    };
    use bevy::image::Image;
    use bevy::prelude::{Color, Handle, Rect, Vec2};
    use ferrisium_core::prelude::{TileId, TileSource};

    #[test]
    fn fixed_zoom_body_tiles_have_placeholder_bindings_until_ready() {
        let source = TileSource::nasa_trek_moon_lro_wac();
        let tile = TileId::new(3, 4, 2);
        let mut body_state = BodySurfaceState::new(source, 3, 1.0);
        body_state.slots.insert(tile, BodySurfaceTileSlot::queued());

        let binding = body_surface_tile_binding(tile, &body_state);

        assert!(binding.image.is_none());
        assert_eq!(binding.uv_rect, Rect::from_corners(Vec2::ZERO, Vec2::ONE));
        assert_ne!(binding.color, Color::WHITE);
    }

    #[test]
    fn earth_body_tiles_have_visible_placeholder_bindings_until_ready() {
        let source = TileSource::nasa_gibs_blue_marble();
        let tile = TileId::new(3, 4, 2);
        let mut body_state = BodySurfaceState::new(source, 3, 1.0);
        body_state.slots.insert(tile, BodySurfaceTileSlot::queued());

        let binding = body_surface_tile_binding(tile, &body_state);

        assert!(binding.image.is_none());
        assert_eq!(binding.uv_rect, Rect::from_corners(Vec2::ZERO, Vec2::ONE));
        assert_ne!(binding.color, Color::WHITE);
        assert_ne!(binding.color, Color::BLACK);
    }

    #[test]
    fn fixed_zoom_body_tiles_reuse_ready_images_in_shared_patch_bindings() {
        let source = TileSource::nasa_trek_moon_lro_wac();
        let tile = TileId::new(3, 4, 2);
        let image = Handle::<Image>::default();
        let mut body_state = BodySurfaceState::new(source, 3, 1.0);
        body_state.slots.insert(
            tile,
            BodySurfaceTileSlot {
                state: BodySurfaceTileSlotState::Ready {
                    image: image.clone(),
                },
            },
        );

        let binding = body_surface_tile_binding(tile, &body_state);

        assert_eq!(binding.image, Some(image));
        assert_eq!(binding.uv_rect, Rect::from_corners(Vec2::ZERO, Vec2::ONE));
    }
}
