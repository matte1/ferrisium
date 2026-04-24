//! Shared tile fetch/cache pipeline plus 2D sprite rendering.
//!
//! This module owns the parts of the tile stack that are common to both views
//! or still specific to the 2D map path:
//!
//! - visible tiles are derived from the shared `ViewState`
//! - queued requests are prioritized by distance to the viewport center
//! - responses are decoded into Bevy images and stored in a bounded cache
//! - exact tiles win when ready, otherwise cropped ancestor tiles are used
//! - nearby ready tiles from adjacent zoom levels may remain rendered behind
//!   the active visible set to soften zoom transitions
//!
//! Globe-specific patch meshes and material binding live in
//! `crate::tile_globe`.

mod map_render;
mod requests;
mod state;
mod texture;

pub(crate) use map_render::{spawn_map_camera, sync_tile_sprites};
pub(crate) use requests::{
    drain_tile_fetch_results, evict_tile_cache, sync_tile_source_state,
    update_visible_tiles_and_queue_requests,
};
pub(crate) use state::{TileFetchChannel, TilePipelineState, TileSpriteEntities};
pub(crate) use texture::{tile_texture_binding, TileTextureBinding};
