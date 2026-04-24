//! Material helpers for lit globe and body-surface tile patches.
//!
//! Ready curved tile patches use PBR materials so the demo's Sun light can
//! shade Earth, Moon, and Mars consistently. Placeholder patches render unlit
//! so a focused body remains visible while remote images are still pending.

use bevy::prelude::*;
use ferrisium_core::prelude::TileId;

use crate::tiles::TileTextureBinding;

/// Builds a lit material from an initial tile texture binding.
pub(crate) fn build_globe_tile_material(
    tile: TileId,
    binding: &TileTextureBinding,
) -> StandardMaterial {
    let mut material = StandardMaterial {
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        cull_mode: None,
        ..default()
    };
    update_globe_tile_material(&mut material, tile, binding);
    material
}

/// Refreshes an existing material after a tile image or fallback binding changes.
pub(crate) fn update_globe_tile_material(
    material: &mut StandardMaterial,
    _tile: TileId,
    binding: &TileTextureBinding,
) {
    material.base_color_texture.clone_from(&binding.image);
    material.base_color = binding.color;
    material.unlit = binding.image.is_none();
    material.emissive = if binding.image.is_none() {
        binding.color.into()
    } else {
        Color::BLACK.into()
    };
}
