//! Texture fallback selection for map sprites and globe tile materials.
//!
//! Exact tiles are preferred, ancestors are cropped or UV-adjusted while
//! children load, and placeholders keep the rendered surface coherent when no
//! usable image is available yet.

use bevy::image::Image;
use bevy::prelude::*;
use ferrisium_core::prelude::{
    descendant_normalized_rect, nearest_ready_ancestor, PlanetaryBody, TileId,
};

use crate::tiles::state::{TilePipelineState, TileSlot, TileSlotState};

/// Texture and UV data needed to render a tile from the cache.
#[derive(Clone)]
pub(crate) struct TileTextureBinding {
    pub(crate) image: Option<Handle<Image>>,
    pub(crate) uv_rect: Rect,
    pub(crate) color: Color,
}

impl TileTextureBinding {
    /// Creates a binding for an exact ready tile image.
    fn exact(image: Handle<Image>) -> Self {
        Self {
            image: Some(image),
            uv_rect: unit_uv_rect(),
            color: Color::WHITE,
        }
    }

    /// Creates a binding for a cropped ancestor texture.
    fn fallback(image: Handle<Image>, uv_rect: Rect) -> Self {
        Self {
            image: Some(image),
            uv_rect,
            color: Color::WHITE,
        }
    }

    /// Creates a binding for a deterministic placeholder color.
    fn placeholder(tile: TileId, body: Option<&PlanetaryBody>) -> Self {
        Self {
            image: None,
            uv_rect: unit_uv_rect(),
            color: globe_placeholder_tile_color(tile, body),
        }
    }
}

/// Sprite-facing visual state for a map tile.
pub(super) enum TileVisual {
    Exact { image: Handle<Image> },
    Fallback { image: Handle<Image>, rect: Rect },
    Placeholder { color: Color },
}

/// Returns image, UV, and color data for globe/material consumers.
pub(crate) fn tile_texture_binding(
    tile: TileId,
    tile_pipeline: &TilePipelineState,
) -> TileTextureBinding {
    if let Some(TileSlot {
        state: TileSlotState::Ready { image, .. },
        ..
    }) = tile_pipeline.slots.get(&tile)
    {
        return TileTextureBinding::exact(image.clone());
    }

    fallback_tile_texture_binding(tile, tile_pipeline)
}

/// Returns the sprite render mode for the 2D map renderer.
pub(super) fn tile_visual(tile: TileId, tile_pipeline: &TilePipelineState) -> TileVisual {
    if let Some(TileSlot {
        state: TileSlotState::Ready { image, .. },
        ..
    }) = tile_pipeline.slots.get(&tile)
    {
        return TileVisual::Exact {
            image: image.clone(),
        };
    }

    if let Some(ancestor) = nearest_ready_ancestor(tile, |ancestor| {
        matches!(
            tile_pipeline.slots.get(&ancestor),
            Some(TileSlot {
                state: TileSlotState::Ready { .. },
                ..
            })
        )
    }) {
        if let Some(TileSlot {
            state: TileSlotState::Ready { image, image_size },
            ..
        }) = tile_pipeline.slots.get(&ancestor)
        {
            if let Some(rect) = descendant_crop_rect(tile, ancestor, *image_size) {
                return TileVisual::Fallback {
                    image: image.clone(),
                    rect,
                };
            }
        }
    }

    TileVisual::Placeholder {
        color: placeholder_tile_color(tile),
    }
}

/// Looks up the nearest ready ancestor and converts it into a UV fallback.
fn fallback_tile_texture_binding(
    tile: TileId,
    tile_pipeline: &TilePipelineState,
) -> TileTextureBinding {
    if let Some(ancestor) = nearest_ready_ancestor(tile, |ancestor| {
        matches!(
            tile_pipeline.slots.get(&ancestor),
            Some(TileSlot {
                state: TileSlotState::Ready { .. },
                ..
            })
        )
    }) {
        if let Some(TileSlot {
            state: TileSlotState::Ready { image, .. },
            ..
        }) = tile_pipeline.slots.get(&ancestor)
        {
            if let Some(uv_rect) = descendant_uv_rect(tile, ancestor) {
                return TileTextureBinding::fallback(image.clone(), uv_rect);
            }
        }
    }

    TileTextureBinding::placeholder(
        tile,
        tile_pipeline
            .last_source
            .as_ref()
            .map(|source| &source.body),
    )
}

/// Computes a source-image pixel crop for drawing `tile` from `ancestor`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Renderer texture crop rectangles are represented as Bevy f32 coordinates."
)]
fn descendant_crop_rect(tile: TileId, ancestor: TileId, ancestor_size: UVec2) -> Option<Rect> {
    let normalized = descendant_normalized_rect(tile, ancestor)?;
    let width = ancestor_size.x as f32;
    let height = ancestor_size.y as f32;
    let min = Vec2::new(
        (normalized.min_x as f32) * width,
        (normalized.min_y as f32) * height,
    );
    let max = Vec2::new(
        (normalized.max_x as f32) * width,
        (normalized.max_y as f32) * height,
    );

    Some(Rect { min, max })
}

/// Computes normalized mesh UVs for sampling `tile` from `ancestor`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Renderer texture UV rectangles are represented as Bevy f32 coordinates."
)]
fn descendant_uv_rect(tile: TileId, ancestor: TileId) -> Option<Rect> {
    let normalized = descendant_normalized_rect(tile, ancestor)?;
    let min = Vec2::new(normalized.min_x as f32, normalized.min_y as f32);
    let max = Vec2::new(normalized.max_x as f32, normalized.max_y as f32);

    Some(Rect { min, max })
}

/// Full-texture UV rectangle.
fn unit_uv_rect() -> Rect {
    Rect {
        min: Vec2::ZERO,
        max: Vec2::ONE,
    }
}

/// Deterministic map/globe placeholder color used before any image is ready.
pub(crate) fn placeholder_tile_color(tile: TileId) -> Color {
    if (tile.x + tile.y).is_multiple_of(2) {
        Color::srgb(0.15, 0.18, 0.24)
    } else {
        Color::srgb(0.24, 0.29, 0.37)
    }
}

/// Deterministic globe placeholder color that reflects the active body source.
fn globe_placeholder_tile_color(tile: TileId, body: Option<&PlanetaryBody>) -> Color {
    let alternate = (tile.x + tile.y).is_multiple_of(2);
    match body.map(|body| body.name.as_str()) {
        Some("Earth") => {
            if alternate {
                Color::srgb(0.18, 0.44, 0.72)
            } else {
                Color::srgb(0.16, 0.48, 0.30)
            }
        }
        Some("Mercury") => {
            if alternate {
                Color::srgb(0.38, 0.36, 0.33)
            } else {
                Color::srgb(0.54, 0.51, 0.47)
            }
        }
        Some("Venus") => {
            if alternate {
                Color::srgb(0.62, 0.42, 0.58)
            } else {
                Color::srgb(0.84, 0.63, 0.32)
            }
        }
        Some("Moon") => {
            if alternate {
                Color::srgb(0.38, 0.38, 0.36)
            } else {
                Color::srgb(0.52, 0.52, 0.49)
            }
        }
        Some("Mars") => {
            if alternate {
                Color::srgb(0.50, 0.28, 0.18)
            } else {
                Color::srgb(0.65, 0.38, 0.24)
            }
        }
        _ => placeholder_tile_color(tile),
    }
}

#[cfg(test)]
mod tests {
    use super::tile_texture_binding;
    use crate::tiles::state::{TilePipelineState, TileSlot, TileSlotState};
    use bevy::image::Image;
    use bevy::prelude::{Color, Handle, UVec2, Vec2};
    use ferrisium_core::prelude::{TileId, TileSource};

    #[test]
    fn tile_texture_binding_uses_parent_uvs_for_missing_children() {
        let mut tile_pipeline = TilePipelineState::default();
        let parent_tile = TileId::new(2, 1, 1);
        let child_tile = TileId::new(3, 3, 2);

        tile_pipeline.slots.insert(
            parent_tile,
            TileSlot {
                last_touched_frame: 1,
                state: TileSlotState::Ready {
                    image: Handle::<Image>::default(),
                    image_size: UVec2::splat(256),
                },
            },
        );

        let binding = tile_texture_binding(child_tile, &tile_pipeline);

        assert!(binding.image.is_some());
        assert_eq!(binding.uv_rect.min, Vec2::new(0.5, 0.0));
        assert_eq!(binding.uv_rect.max, Vec2::new(1.0, 0.5));
    }

    #[test]
    fn globe_tile_binding_uses_body_specific_placeholder_colors() {
        let mut tile_pipeline = TilePipelineState {
            last_source: Some(TileSource::nasa_gibs_blue_marble()),
            ..Default::default()
        };
        let tile = TileId::new(3, 4, 2);

        let binding = tile_texture_binding(tile, &tile_pipeline);

        assert!(binding.image.is_none());
        assert_ne!(binding.color, Color::WHITE);
        assert_ne!(binding.color, Color::BLACK);

        tile_pipeline.last_source = Some(TileSource::nasa_trek_mars_viking_color());
        let mars_binding = tile_texture_binding(tile, &tile_pipeline);
        assert_ne!(binding.color, mars_binding.color);
    }
}
