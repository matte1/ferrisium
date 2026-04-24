//! Polar cap meshes for Web Mercator primary globe sources.
//!
//! Web Mercator imagery stops before the poles. These simple lit meshes fill
//! that uncovered cap area for the Earth primary globe without affecting
//! equirectangular planetary sources that already include pole coverage.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use ferrisium_core::prelude::{LonLat, TileProjection, WEB_MERCATOR_MAX_LAT_DEG};

use crate::tile_globe::mesh::globe_surface_position;
use crate::tile_globe::{parent_to_globe_surface_anchor, GlobeTileEntities};

// Polar caps need enough segments to look round at wide globe views.
const GLOBE_CAP_SEGMENTS: u32 = 64;

/// Marker attached to spawned polar cap entities.
#[derive(Component)]
struct GlobePolarCap;

/// Entity handles and radius used to refresh polar caps when the globe changes.
#[derive(Debug, Clone, Copy)]
pub(super) struct GlobePolarCapEntities {
    pub(super) north: Entity,
    pub(super) south: Entity,
    pub(super) radius: f32,
}

/// Ensures Web Mercator polar caps exist for the current primary globe radius.
pub(super) fn sync_globe_polar_caps(
    commands: &mut Commands<'_, '_>,
    globe_tile_entities: &mut GlobeTileEntities,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    radius: f32,
    projection: TileProjection,
) {
    if projection != TileProjection::WebMercator {
        despawn_globe_polar_caps(commands, globe_tile_entities);
        return;
    }

    if let Some(polar_caps) = globe_tile_entities.polar_caps {
        if (polar_caps.radius - radius).abs() <= f32::EPSILON {
            return;
        }

        despawn_globe_polar_caps(commands, globe_tile_entities);
    }

    let material = materials.add(StandardMaterial {
        base_color: polar_cap_color(),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        cull_mode: None,
        ..default()
    });
    let north = commands
        .spawn((
            Name::new("Globe North Polar Cap"),
            GlobePolarCap,
            Mesh3d(meshes.add(build_globe_polar_cap_mesh(radius, true))),
            MeshMaterial3d(material.clone()),
        ))
        .id();
    let south = commands
        .spawn((
            Name::new("Globe South Polar Cap"),
            GlobePolarCap,
            Mesh3d(meshes.add(build_globe_polar_cap_mesh(radius, false))),
            MeshMaterial3d(material),
        ))
        .id();
    parent_to_globe_surface_anchor(commands, parent, north);
    parent_to_globe_surface_anchor(commands, parent, south);

    globe_tile_entities.polar_caps = Some(GlobePolarCapEntities {
        north,
        south,
        radius,
    });
}

/// Removes existing polar caps, if any.
pub(super) fn despawn_globe_polar_caps(
    commands: &mut Commands<'_, '_>,
    globe_tile_entities: &mut GlobeTileEntities,
) {
    let Some(polar_caps) = globe_tile_entities.polar_caps.take() else {
        return;
    };

    commands.entity(polar_caps.north).despawn();
    commands.entity(polar_caps.south).despawn();
}

/// Neutral cap color used where Web Mercator has no imagery.
fn polar_cap_color() -> Color {
    Color::srgb(0.78, 0.78, 0.74)
}

/// Builds one triangular fan from the pole to the Mercator latitude limit.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy mesh data is f32-based while cap geometry is derived in f64."
)]
fn build_globe_polar_cap_mesh(radius: f32, north: bool) -> Mesh {
    let pole_lat = if north { 90.0 } else { -90.0 };
    let ring_lat = if north {
        WEB_MERCATOR_MAX_LAT_DEG
    } else {
        -WEB_MERCATOR_MAX_LAT_DEG
    };
    let radius = f64::from(radius);
    let mut positions = Vec::with_capacity(usize::try_from(GLOBE_CAP_SEGMENTS + 2).unwrap_or(0));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::new();

    let pole = globe_surface_position(LonLat::new(0.0, pole_lat), radius);
    positions.push([pole.x as f32, pole.y as f32, pole.z as f32]);
    let pole_normal = pole.normalize_or_zero();
    normals.push([
        pole_normal.x as f32,
        pole_normal.y as f32,
        pole_normal.z as f32,
    ]);

    for segment in 0..=GLOBE_CAP_SEGMENTS {
        let lon_deg = -180.0 + 360.0 * f64::from(segment) / f64::from(GLOBE_CAP_SEGMENTS);
        let position = globe_surface_position(LonLat::new(lon_deg, ring_lat), radius);
        let normal = position.normalize_or_zero();

        positions.push([position.x as f32, position.y as f32, position.z as f32]);
        normals.push([normal.x as f32, normal.y as f32, normal.z as f32]);
    }

    for segment in 0..GLOBE_CAP_SEGMENTS {
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
