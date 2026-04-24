//! Shared crate-private helpers for geospatial overlay and annotation renderers.

use bevy::prelude::{Color, InheritedVisibility, Visibility};
use ferrisium_core::prelude::{TileProjection, ViewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayColorKey {
    rgba_bits: [u32; 4],
}

impl OverlayColorKey {
    pub(crate) const fn from_rgba(rgba: [f32; 4]) -> Self {
        Self {
            rgba_bits: [
                rgba[0].to_bits(),
                rgba[1].to_bits(),
                rgba[2].to_bits(),
                rgba[3].to_bits(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverlayMapViewKey {
    center_lon_bits: u64,
    center_lat_bits: u64,
    zoom_bits: u64,
    viewport_width_bits: u64,
    viewport_height_bits: u64,
    tile_size_bits: u64,
    projection: TileProjection,
}

pub(crate) fn overlay_is_visible(
    visibility: Option<&Visibility>,
    inherited_visibility: Option<&InheritedVisibility>,
) -> bool {
    match visibility {
        Some(Visibility::Hidden) => false,
        Some(Visibility::Visible) => true,
        Some(Visibility::Inherited) | None => {
            inherited_visibility.is_none_or(|visibility| visibility.get())
        }
    }
}

pub(crate) fn map_view_key(view_state: &ViewState) -> OverlayMapViewKey {
    OverlayMapViewKey {
        center_lon_bits: view_state.center.lon_deg.to_bits(),
        center_lat_bits: view_state.center.lat_deg.to_bits(),
        zoom_bits: view_state.zoom.to_bits(),
        viewport_width_bits: view_state.viewport_size.x.to_bits(),
        viewport_height_bits: view_state.viewport_size.y.to_bits(),
        tile_size_bits: view_state.tile_size.to_bits(),
        projection: view_state.projection,
    }
}

pub(crate) fn color_from_rgba([red, green, blue, alpha]: [f32; 4]) -> Color {
    Color::srgba(red, green, blue, alpha)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{InheritedVisibility, Visibility};

    use super::overlay_is_visible;

    #[test]
    fn overlay_visibility_honors_local_and_inherited_visibility() {
        assert!(overlay_is_visible(None, None));
        assert!(overlay_is_visible(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(overlay_is_visible(
            Some(&Visibility::Inherited),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(!overlay_is_visible(
            Some(&Visibility::Inherited),
            Some(&InheritedVisibility::HIDDEN)
        ));
        assert!(!overlay_is_visible(
            Some(&Visibility::Hidden),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(overlay_is_visible(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::HIDDEN)
        ));
    }
}
