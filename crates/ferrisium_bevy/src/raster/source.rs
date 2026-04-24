//! Bevy resource wrapper for the active raster tile source.

use bevy::prelude::Resource;
use ferrisium_core::source::{NasaTrekRegularBody, PlanetaryBody, TileProjection, TileSource};
use ferrisium_core::space::BodyId;

/// Resource containing the tile source used by active map, globe, and body-surface systems.
#[derive(Resource, Debug, Clone)]
pub struct ActiveTileSource(pub TileSource);

impl Default for ActiveTileSource {
    fn default() -> Self {
        Self(TileSource::nasa_gibs_blue_marble())
    }
}

impl From<TileSource> for ActiveTileSource {
    /// Promotes a pure `TileSource` into the Bevy resource used by Ferrisium.
    fn from(value: TileSource) -> Self {
        Self(value)
    }
}

pub(crate) fn supports_current_renderers(tile_source: &TileSource) -> bool {
    supports_map_renderer(tile_source) || supports_primary_globe_renderer(tile_source)
}

pub(crate) fn supports_map_renderer(tile_source: &TileSource) -> bool {
    match tile_source.projection {
        TileProjection::WebMercator => tile_source.body == PlanetaryBody::earth(),
        TileProjection::Equirectangular => true,
    }
}

pub(crate) fn supports_globe_renderer(tile_source: &TileSource) -> bool {
    matches!(
        tile_source.projection,
        TileProjection::WebMercator | TileProjection::Equirectangular
    )
}

pub(crate) fn tile_source_matches_body(tile_source: &TileSource, body: BodyId) -> bool {
    if body == BodyId::EARTH {
        return tile_source.body == PlanetaryBody::earth();
    }

    NasaTrekRegularBody::from_body_or_system_naif_id(body.naif_id)
        .is_some_and(|trek_body| tile_source.body == trek_body.planetary_body())
}

pub(crate) fn supports_focused_globe_renderer(tile_source: &TileSource, body: BodyId) -> bool {
    supports_globe_renderer(tile_source) && tile_source_matches_body(tile_source, body)
}

pub(crate) fn supports_primary_globe_renderer(tile_source: &TileSource) -> bool {
    supports_focused_globe_renderer(tile_source, BodyId::EARTH)
}

#[cfg(test)]
mod tests {
    use super::{
        supports_current_renderers, supports_focused_globe_renderer, supports_globe_renderer,
        supports_map_renderer, supports_primary_globe_renderer, tile_source_matches_body,
    };
    use ferrisium_core::prelude::{NasaTrekRegularBody, PlanetaryBody, TileProjection, TileSource};
    use ferrisium_core::space::BodyId;

    #[test]
    fn renderer_support_distinguishes_map_and_globe_sources() {
        assert!(supports_current_renderers(&TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png"
        )));
        assert!(supports_map_renderer(&TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png"
        )));
        assert!(supports_globe_renderer(&TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png"
        )));
        assert!(supports_map_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_projection(TileProjection::Equirectangular)
        ));
        assert!(supports_globe_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_projection(TileProjection::Equirectangular)
        ));
        assert!(!supports_map_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_body(PlanetaryBody::mars())
        ));
        assert!(supports_map_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_projection(TileProjection::Equirectangular)
                .with_body(PlanetaryBody::mars())
        ));
        assert!(supports_globe_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_body(PlanetaryBody::mars())
        ));
        assert!(!supports_current_renderers(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_body(PlanetaryBody::mars())
        ));
        assert!(supports_current_renderers(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_projection(TileProjection::Equirectangular)
                .with_body(PlanetaryBody::mars())
        ));
        assert!(!supports_primary_globe_renderer(
            &TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
                .with_body(PlanetaryBody::mars())
        ));
        assert!(supports_focused_globe_renderer(
            &TileSource::nasa_trek_moon_lro_wac(),
            BodyId::MOON
        ));
        assert!(supports_focused_globe_renderer(
            &TileSource::nasa_trek_mars_viking_color(),
            BodyId::MARS_BARYCENTER
        ));
        assert!(!supports_focused_globe_renderer(
            &TileSource::nasa_trek_moon_lro_wac(),
            BodyId::EARTH
        ));
        assert!(tile_source_matches_body(
            &TileSource::nasa_trek_mercury_mdis_enhanced_color(),
            BodyId::MERCURY_BARYCENTER
        ));
        assert!(tile_source_matches_body(
            &TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Europa),
            BodyId::EUROPA
        ));
        assert!(tile_source_matches_body(
            &TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Ceres),
            BodyId::CERES
        ));
    }

    #[test]
    fn active_tile_source_has_browser_safe_no_key_default() {
        let active_source = super::ActiveTileSource::default();

        assert_eq!(active_source.0, TileSource::nasa_gibs_blue_marble());
    }
}
