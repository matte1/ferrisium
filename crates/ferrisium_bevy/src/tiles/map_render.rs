//! Sprite-based 2D map rendering for the shared tile cache.
//!
//! The map renderer turns cache state into Bevy sprites, including exact
//! images, cropped parent fallbacks, and adjacent ready zoom levels retained
//! behind the active set during zoom transitions.

use std::collections::BTreeSet;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use ferrisium_core::coords::{lon_lat_to_world_pixel_for_projection, projected_world_size, Vec2d};
use ferrisium_core::prelude::{tile_center_world_pixel, TileId, ViewState};

use crate::source::{supports_map_renderer, ActiveTileSource};
use crate::tiles::state::{TilePipelineState, TileSlotState, TileSpriteEntities};
use crate::tiles::texture::{tile_visual, TileVisual};
use crate::view::{map_view_state, Map2dState, Map2dView};

// Adjacent ready tiles within this zoom delta can stay behind active tiles.
const BACKGROUND_READY_ZOOM_DELTA: u8 = 1;
// Parent fallbacks are dimmed to make loading state visually legible.
const FALLBACK_TILE_ALPHA: f32 = 0.72;
// Padding avoids placeholder seams touching when no image is ready.
const PLACEHOLDER_TILE_PADDING: f32 = 2.0;
// Lowest map tile layer: placeholder quads.
const PLACEHOLDER_LAYER_Z: f32 = 5.0;
// Background layer for ready adjacent zoom tiles retained during zooms.
const BACKGROUND_READY_LAYER_Z: f32 = 10.0;
// Mid layer for cropped parent fallbacks.
const FALLBACK_LAYER_Z: f32 = 15.0;
// Highest tile layer for exact ready tiles.
const EXACT_LAYER_Z: f32 = 20.0;

/// Marker for the default 2D camera spawned by Ferrisium.
#[derive(Component)]
pub(crate) struct FerrisiumCamera;

/// Marker attached to map tile sprite entities managed by the tile cache.
#[derive(Component)]
struct MapTileSprite;

/// Sprite render data calculated before entity insert/update.
struct TileSpriteSpec {
    sprite: Sprite,
    layer_z: f32,
}

/// Spawns a default 2D camera when the app has a map view and no 2D camera.
pub(crate) fn spawn_map_camera(
    mut commands: Commands<'_, '_>,
    map_views: Query<'_, '_, (), With<Map2dView>>,
    existing_cameras: Query<'_, '_, (), With<Camera2d>>,
) {
    if map_views.is_empty() || !existing_cameras.is_empty() {
        return;
    }

    commands.spawn((Camera2d, Camera::default(), FerrisiumCamera));
}

/// Synchronizes Bevy sprite entities with the map tiles that should be drawn.
#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy sprites use f32 custom sizes while map tile sizing stays in f64."
)]
pub(crate) fn sync_tile_sprites(
    mut commands: Commands<'_, '_>,
    active_source: Res<'_, ActiveTileSource>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    map_views: Query<'_, '_, &Map2dState, With<Map2dView>>,
    tile_pipeline: Res<'_, TilePipelineState>,
    mut tile_sprite_entities: ResMut<'_, TileSpriteEntities>,
) {
    let Some(window) = windows.iter().next() else {
        despawn_all_tile_sprites(&mut commands, &mut tile_sprite_entities);
        return;
    };

    let Some(active_view) = map_views.iter().next() else {
        despawn_all_tile_sprites(&mut commands, &mut tile_sprite_entities);
        return;
    };

    if !supports_map_renderer(&active_source.0) {
        despawn_all_tile_sprites(&mut commands, &mut tile_sprite_entities);
        return;
    }

    let view_state = map_view_state(window, *active_view, &active_source);
    let desired_tiles = desired_render_tiles(&tile_pipeline, &view_state);
    let stale_tiles = tile_sprite_entities
        .by_tile
        .iter()
        .filter(|(tile, _)| !desired_tiles.contains(tile))
        .map(|(tile, entity)| (*tile, *entity))
        .collect::<Vec<_>>();

    for (tile, entity) in stale_tiles {
        commands.entity(entity).despawn();
        tile_sprite_entities.by_tile.remove(&tile);
    }

    for tile in &desired_tiles {
        let sprite_spec = build_tile_sprite(*tile, &tile_pipeline, &view_state);
        let transform = tile_transform(*tile, &view_state, sprite_spec.layer_z);

        if let Some(entity) = tile_sprite_entities.by_tile.get(tile).copied() {
            commands
                .entity(entity)
                .insert((sprite_spec.sprite, transform));
        } else {
            let entity = commands
                .spawn((sprite_spec.sprite, transform, MapTileSprite))
                .id();
            tile_sprite_entities.by_tile.insert(*tile, entity);
        }
    }
}

/// Builds the sprite and z-layer for a single tile visual.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Sprite crop rectangles and world transforms use f32 in Bevy."
)]
fn build_tile_sprite(
    tile: TileId,
    tile_pipeline: &TilePipelineState,
    view_state: &ViewState,
) -> TileSpriteSpec {
    let tile_size = rendered_tile_size(view_state.zoom, tile.z, view_state.tile_size) as f32;
    let is_visible = tile_pipeline.map_visible_tiles.contains(&tile);

    match tile_visual(tile, tile_pipeline) {
        TileVisual::Exact { image } => {
            let mut sprite = Sprite::from_image(image);
            sprite.custom_size = Some(Vec2::splat(tile_size));
            TileSpriteSpec {
                sprite,
                layer_z: if is_visible {
                    EXACT_LAYER_Z
                } else {
                    BACKGROUND_READY_LAYER_Z
                },
            }
        }
        TileVisual::Fallback { image, rect } => {
            let mut sprite = Sprite::from_image(image);
            sprite.color = Color::srgba(1.0, 1.0, 1.0, FALLBACK_TILE_ALPHA);
            sprite.custom_size = Some(Vec2::splat(tile_size));
            sprite.rect = Some(rect);
            TileSpriteSpec {
                sprite,
                layer_z: FALLBACK_LAYER_Z,
            }
        }
        TileVisual::Placeholder { color } => TileSpriteSpec {
            sprite: Sprite::from_color(
                color,
                Vec2::splat((tile_size - PLACEHOLDER_TILE_PADDING).max(1.0)),
            ),
            layer_z: PLACEHOLDER_LAYER_Z,
        },
    }
}

// Keep nearby ready tiles from adjacent zoom levels around long enough to avoid
// flashing placeholders during a zoom transition.
/// Returns active map tiles plus nearby ready tiles retained during zooms.
pub(super) fn desired_render_tiles(
    tile_pipeline: &TilePipelineState,
    view_state: &ViewState,
) -> BTreeSet<TileId> {
    let current_zoom = view_state.discrete_zoom();
    let mut desired_tiles = tile_pipeline.map_visible_tiles.clone();

    for (tile, slot) in &tile_pipeline.slots {
        if !matches!(slot.state, TileSlotState::Ready { .. }) {
            continue;
        }

        if tile.z.abs_diff(current_zoom) > BACKGROUND_READY_ZOOM_DELTA {
            continue;
        }

        if tile_intersects_viewport(*tile, view_state) {
            desired_tiles.insert(*tile);
        }
    }

    desired_tiles
}

// Request center-nearest tiles first so the viewport sharpens from the middle
// outward instead of filling in tile-id order.
/// Sorts queued map tile requests by distance from the viewport center.
pub(super) fn sort_tiles_by_view_distance(tiles: &mut [TileId], view_state: &ViewState) {
    tiles.sort_by(|left, right| {
        tile_view_distance_sq(*left, view_state)
            .total_cmp(&tile_view_distance_sq(*right, view_state))
            .then_with(|| left.cmp(right))
    });
}

/// Returns squared screen-space distance from the viewport center.
fn tile_view_distance_sq(tile: TileId, view_state: &ViewState) -> f64 {
    let offset = tile_screen_offset(tile, view_state);
    offset.x.mul_add(offset.x, offset.y * offset.y)
}

/// Tests whether a tile overlaps the current viewport bounds.
fn tile_intersects_viewport(tile: TileId, view_state: &ViewState) -> bool {
    let screen_offset = tile_screen_offset(tile, view_state);
    let half_tile_size = rendered_tile_size(view_state.zoom, tile.z, view_state.tile_size) * 0.5;
    let half_viewport = view_state.viewport_size * 0.5;

    screen_offset.x.abs() <= half_viewport.x + half_tile_size
        && screen_offset.y.abs() <= half_viewport.y + half_tile_size
}

/// Returns the rendered size of a tile at the current fractional zoom.
fn rendered_tile_size(view_zoom: f64, tile_zoom: u8, tile_size: f64) -> f64 {
    tile_size * 2_f64.powf(view_zoom - f64::from(tile_zoom))
}

// Convert a tile center into screen-space offset while choosing the shortest
// wrapped x-distance across repeated projected world copies.
/// Converts a tile center into screen-space offset with antimeridian wrapping.
fn tile_screen_offset(tile: TileId, view_state: &ViewState) -> Vec2d {
    let tile_center = tile_center_world_pixel(tile, view_state.tile_size);
    let center_world = lon_lat_to_world_pixel_for_projection(
        view_state.center,
        view_state.projection,
        f64::from(tile.z),
        view_state.tile_size,
    );
    let world_width = projected_world_size(
        view_state.projection,
        view_state.tile_size,
        f64::from(tile.z),
    )
    .x;
    let world_offset = wrap_world_offset(tile_center - center_world, world_width);
    let scale = 2_f64.powf(view_state.zoom - f64::from(tile.z));

    world_offset * scale
}

/// Wraps only the horizontal world-pixel offset across repeated map copies.
fn wrap_world_offset(offset: Vec2d, world_width: f64) -> Vec2d {
    Vec2d::new(wrap_periodic_delta(offset.x, world_width), offset.y)
}

/// Chooses the shortest signed delta on a periodic axis.
fn wrap_periodic_delta(delta: f64, period: f64) -> f64 {
    if period <= 0.0 {
        return delta;
    }

    (delta + period * 0.5).rem_euclid(period) - period * 0.5
}

/// Builds the Bevy transform for a tile sprite at a specific render layer.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Bevy transforms are f32-based even though map math stays in f64."
)]
fn tile_transform(tile: TileId, view_state: &ViewState, layer_z: f32) -> Transform {
    let offset = tile_screen_offset(tile, view_state);

    Transform::from_xyz(offset.x as f32, -(offset.y as f32), layer_z)
}

/// Removes all map tile sprites from the world.
fn despawn_all_tile_sprites(
    commands: &mut Commands<'_, '_>,
    tile_sprite_entities: &mut TileSpriteEntities,
) {
    for entity in tile_sprite_entities
        .by_tile
        .drain()
        .map(|(_, entity)| entity)
    {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        desired_render_tiles, sort_tiles_by_view_distance, tile_screen_offset, wrap_periodic_delta,
    };
    use crate::tiles::state::{TilePipelineState, TileSlot, TileSlotState};
    use bevy::image::Image;
    use bevy::prelude::{Handle, UVec2};
    use ferrisium_core::coords::LonLat;
    use ferrisium_core::prelude::{TileId, TileProjection, ViewState};

    #[test]
    fn wrap_periodic_delta_prefers_the_shortest_world_path() {
        let wrapped = wrap_periodic_delta(-896.0, 1024.0);

        assert!((wrapped - 128.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tile_screen_offset_wraps_cleanly_across_the_antimeridian() {
        let view_state = ViewState {
            center: LonLat::new(179.0, 0.0),
            zoom: 2.0,
            ..ViewState::default()
        };

        let wrapped_neighbor = tile_screen_offset(TileId::new(2, 0, 2), &view_state);
        let far_tile = tile_screen_offset(TileId::new(2, 1, 2), &view_state);

        assert!(wrapped_neighbor.x.abs() < 140.0);
        assert!(wrapped_neighbor.x.abs() < far_tile.x.abs());
    }

    #[test]
    fn tile_screen_offset_uses_equirectangular_world_width() {
        let view_state = ViewState {
            center: LonLat::new(179.0, 0.0),
            zoom: 1.0,
            projection: TileProjection::Equirectangular,
            ..ViewState::default()
        };

        let wrapped_neighbor = tile_screen_offset(TileId::new(1, 0, 1), &view_state);
        let far_tile = tile_screen_offset(TileId::new(1, 1, 1), &view_state);

        assert!(wrapped_neighbor.x.abs() < 160.0);
        assert!(wrapped_neighbor.x.abs() < far_tile.x.abs());
    }

    #[test]
    fn queued_tiles_are_sorted_by_view_distance() {
        let view_state = ViewState {
            center: LonLat::new(179.0, 0.0),
            zoom: 2.0,
            ..ViewState::default()
        };
        let mut tiles = vec![
            TileId::new(2, 1, 2),
            TileId::new(2, 0, 2),
            TileId::new(2, 3, 2),
        ];

        sort_tiles_by_view_distance(&mut tiles, &view_state);

        assert_ne!(tiles.first(), Some(&TileId::new(2, 1, 2)));
        assert_eq!(tiles.last(), Some(&TileId::new(2, 1, 2)));
    }

    #[test]
    fn desired_render_tiles_keep_adjacent_ready_tiles_for_zoom_transitions() {
        let mut tile_pipeline = TilePipelineState::default();
        let visible_tile = TileId::new(2, 2, 2);
        let child_tile = TileId::new(3, 4, 4);
        let far_tile = TileId::new(0, 0, 0);

        tile_pipeline.map_visible_tiles.insert(visible_tile);
        tile_pipeline.slots.insert(
            child_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );
        tile_pipeline.slots.insert(
            far_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        let desired = desired_render_tiles(
            &tile_pipeline,
            &ViewState {
                viewport_size: ferrisium_core::coords::Vec2d::new(512.0, 512.0),
                ..ViewState::default()
            },
        );

        assert!(desired.contains(&visible_tile));
        assert!(desired.contains(&child_tile));
        assert!(!desired.contains(&far_tile));
    }
}
