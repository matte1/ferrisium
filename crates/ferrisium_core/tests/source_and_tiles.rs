mod common;

use std::collections::BTreeSet;

use ferrisium_core::coords::Vec2d;
use ferrisium_core::prelude::{
    tile_center_world_pixel, tile_lon_lat_bounds, tile_matrix_dimensions, tile_origin_world_pixel,
    tile_sample_lon_lat, tile_sample_lon_lat_for_projection, tiles_at_zoom,
    tiles_at_zoom_for_projection, visible_tiles, LonLat, NasaTrekRegularBody, PlanetaryBody,
    TileId, TileProjection, TileSource, ViewState, MAPBOX_RASTER_TILE_MAX_ZOOM,
    MAPBOX_SATELLITE_GLOBAL_MAX_ZOOM, MAPBOX_STYLE_RASTER_MAX_ZOOM, MAX_TILE_ZOOM,
    NASA_TREK_REGULAR_BODIES,
};
use proptest::prelude::{prop_assert, prop_assert_eq};

use crate::common::{lat_strategy, lon_strategy, tile_size_strategy, zoom_strategy};

#[test]
fn xyz_tile_source_builders_and_url_expansion_work() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
        .with_attribution("Example Tiles")
        .with_zoom_range(2, 7)
        .with_tile_size(512);

    assert_eq!(source.attribution, "Example Tiles");
    assert_eq!(source.min_zoom, 2);
    assert_eq!(source.max_zoom, 7);
    assert_eq!(source.tile_size, 512);
    assert_eq!(source.raster_tile_size, 512);
    assert_eq!(source.projection, TileProjection::WebMercator);
    assert_eq!(source.body, PlanetaryBody::earth());
    assert_eq!(
        source.tile_url(TileId::new(3, 4, 5)),
        "https://tiles.example/3/4/5.png"
    );
    assert_eq!(source.checked_tile_url(TileId::new(1, 0, 0)), None);
    assert_eq!(
        source.checked_tile_url(TileId::new(3, 4, 5)),
        Some("https://tiles.example/3/4/5.png".to_owned())
    );
}

#[test]
fn tile_source_zoom_range_is_normalized_and_bounded() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
        .with_zoom_range(MAX_TILE_ZOOM.saturating_add(2), 7);

    assert_eq!(source.normalized_zoom_range(), (7, MAX_TILE_ZOOM));
    assert_eq!(source.min_zoom, 7);
    assert_eq!(source.max_zoom, MAX_TILE_ZOOM);
}

#[test]
fn tile_source_tile_size_is_normalized_and_bounded() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_tile_size(0);

    assert_eq!(source.tile_size, 1);
    assert_eq!(source.raster_tile_size, 1);
    assert_eq!(source.normalized_tile_size(), 1);
    assert_eq!(source.normalized_raster_tile_size(), 1);

    let manual_source = TileSource {
        url_template: "https://tiles.example/{z}/{x}/{y}.png".into(),
        attribution: String::new(),
        min_zoom: 0,
        max_zoom: 4,
        tile_size: 0,
        raster_tile_size: 0,
        projection: TileProjection::WebMercator,
        body: PlanetaryBody::earth(),
    };

    assert_eq!(manual_source.normalized_tile_size(), 1);
    assert_eq!(manual_source.normalized_raster_tile_size(), 1);
}

#[test]
fn manual_tile_source_zoom_fields_are_normalized_for_callers() {
    let source = TileSource {
        url_template: "https://tiles.example/{z}/{x}/{y}.png".into(),
        attribution: String::new(),
        min_zoom: 12,
        max_zoom: 4,
        tile_size: 256,
        raster_tile_size: 256,
        projection: TileProjection::WebMercator,
        body: PlanetaryBody::earth(),
    };

    assert_eq!(source.normalized_zoom_range(), (4, 12));
}

#[test]
fn mapbox_satellite_source_uses_public_token_template_and_hidpi_tiles() {
    let source = TileSource::mapbox_satellite("pk.test-token");

    assert_eq!(
        source.normalized_zoom_range(),
        (0, MAPBOX_SATELLITE_GLOBAL_MAX_ZOOM)
    );
    assert_eq!(source.tile_size, 256);
    assert_eq!(source.raster_tile_size, 512);
    assert_eq!(source.projection, TileProjection::WebMercator);
    assert_eq!(source.body, PlanetaryBody::earth());
    assert_eq!(
        source.tile_url(TileId::new(4, 6, 9)),
        "https://api.mapbox.com/v4/mapbox.satellite/4/6/9@2x.jpg90?access_token=pk.test-token"
    );
    assert!(source.attribution.contains("Mapbox"));
}

#[test]
fn mapbox_satellite_source_can_opt_into_regional_zoom_levels() {
    let source = TileSource::mapbox_satellite_with_max_zoom("pk.test-token", 18);

    assert_eq!(source.normalized_zoom_range(), (0, 18));
    assert_eq!(source.tile_size, 256);
    assert_eq!(source.raster_tile_size, 512);
}

#[test]
fn mapbox_satellite_source_clamps_to_provider_zoom_limit() {
    let source = TileSource::mapbox_satellite_with_max_zoom("pk.test-token", MAX_TILE_ZOOM);

    assert_eq!(
        source.normalized_zoom_range(),
        (0, MAPBOX_RASTER_TILE_MAX_ZOOM)
    );
}

#[test]
fn mapbox_style_raster_source_uses_static_tiles_api() {
    let source = TileSource::mapbox_style_raster("mapbox", "streets-v12", "pk.test-token");

    assert_eq!(source.normalized_zoom_range(), (0, 16));
    assert_eq!(source.tile_size, 256);
    assert_eq!(source.raster_tile_size, 512);
    assert_eq!(source.projection, TileProjection::WebMercator);
    assert_eq!(source.body, PlanetaryBody::earth());
    assert_eq!(
        source.tile_url(TileId::new(4, 6, 9)),
        "https://api.mapbox.com/styles/v1/mapbox/streets-v12/tiles/256/4/6/9@2x?access_token=pk.test-token"
    );
    assert!(source.attribution.contains("Mapbox"));
}

#[test]
fn mapbox_style_raster_source_clamps_to_static_tiles_zoom_limit() {
    let source = TileSource::mapbox_style_raster_with_max_zoom(
        "mapbox",
        "streets-v12",
        "pk.test-token",
        MAPBOX_STYLE_RASTER_MAX_ZOOM.saturating_add(1),
    );

    assert_eq!(
        source.normalized_zoom_range(),
        (0, MAPBOX_STYLE_RASTER_MAX_ZOOM)
    );
}

#[test]
fn nasa_gibs_blue_marble_source_uses_wmts_row_col_template() {
    let source = TileSource::nasa_gibs_blue_marble();

    assert_eq!(source.normalized_zoom_range(), (0, 8));
    assert_eq!(source.tile_size, 256);
    assert_eq!(source.projection, TileProjection::WebMercator);
    assert_eq!(source.body, PlanetaryBody::earth());
    assert_eq!(
        source.tile_url(TileId::new(3, 4, 5)),
        "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/BlueMarble_NextGeneration/default/GoogleMapsCompatible_Level8/3/5/4.jpeg"
    );
    assert!(source.attribution.contains("NASA"));
}

#[test]
fn nasa_trek_sources_describe_equirectangular_planetary_wmts() {
    let moon = TileSource::nasa_trek_moon_lro_wac();
    let mercury = TileSource::nasa_trek_mercury_mdis_enhanced_color();
    let venus_color = TileSource::nasa_trek_venus_magellan_color_topography();
    let venus_hillshade = TileSource::nasa_trek_venus_magellan_hillshade();
    let mars_color = TileSource::nasa_trek_mars_viking_color();
    let mars_hillshade = TileSource::nasa_trek_mars_mola_color();

    assert_eq!(moon.normalized_zoom_range(), (0, 8));
    assert_eq!(moon.projection, TileProjection::Equirectangular);
    assert_eq!(moon.body, PlanetaryBody::moon());
    assert_eq!(
        moon.tile_url(TileId::new(3, 4, 5)),
        "https://trek.nasa.gov/tiles/Moon/EQ/LRO_WAC_Mosaic_Global_303ppd_v02/1.0.0/default/default028mm/3/5/4.jpg"
    );
    assert!(moon.attribution.contains("NASA"));

    assert_eq!(mercury.normalized_zoom_range(), (0, 7));
    assert_eq!(mercury.projection, TileProjection::Equirectangular);
    assert_eq!(mercury.body, PlanetaryBody::mercury());
    assert_eq!(
        mercury.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Mercury/EQ/Mercury_MESSENGER_MDIS_Basemap_EnhancedColor_Mosaic_Global_665m/1.0.0/default/default028mm/2/1/3.jpg"
    );

    assert_eq!(venus_color.normalized_zoom_range(), (0, 2));
    assert_eq!(venus_color.projection, TileProjection::Equirectangular);
    assert_eq!(venus_color.body, PlanetaryBody::venus());
    assert_eq!(
        venus_color.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Venus/EQ/Clrtopogrd/1.0.0/default/default028mm/2/1/3.png"
    );

    assert_eq!(venus_hillshade.normalized_zoom_range(), (0, 4));
    assert_eq!(venus_hillshade.projection, TileProjection::Equirectangular);
    assert_eq!(venus_hillshade.body, PlanetaryBody::venus());
    assert_eq!(
        venus_hillshade.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Venus/EQ/Venus_Magellan_Hillshade_Global_4641m_gapfilled_v02/1.0.0/default/default028mm/2/1/3.png"
    );

    assert_eq!(mars_color.normalized_zoom_range(), (0, 7));
    assert_eq!(mars_color.projection, TileProjection::Equirectangular);
    assert_eq!(mars_color.body, PlanetaryBody::mars());
    assert_eq!(
        mars_color.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Mars/EQ/Mars_Viking_MDIM21_ClrMosaic_global_232m/1.0.0/default/default028mm/2/1/3.jpg"
    );

    assert_eq!(mars_hillshade.normalized_zoom_range(), (0, 7));
    assert_eq!(mars_hillshade.projection, TileProjection::Equirectangular);
    assert_eq!(mars_hillshade.body, PlanetaryBody::mars());
    assert_eq!(
        mars_hillshade.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Mars/EQ/Mars_MGS_MOLA_ClrShade_merge_global_463m/1.0.0/default/default028mm/2/1/3.jpg"
    );
}

#[test]
fn nasa_trek_regular_body_registry_exposes_supported_spherical_sources() {
    assert_eq!(NASA_TREK_REGULAR_BODIES.len(), 16);
    assert_eq!(
        NasaTrekRegularBody::from_slug("europa"),
        Some(NasaTrekRegularBody::Europa)
    );
    assert_eq!(
        NasaTrekRegularBody::from_slug("LUNA"),
        Some(NasaTrekRegularBody::Moon)
    );
    assert_eq!(
        NasaTrekRegularBody::from_naif_id(2_000_001),
        Some(NasaTrekRegularBody::Ceres)
    );
    assert_eq!(
        NasaTrekRegularBody::from_body_or_system_naif_id(4),
        Some(NasaTrekRegularBody::Mars)
    );
    assert_eq!(NasaTrekRegularBody::from_slug("phobos"), None);
    assert_eq!(NasaTrekRegularBody::from_slug("vesta"), None);

    let europa = TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Europa);
    assert_eq!(europa.normalized_zoom_range(), (0, 5));
    assert_eq!(europa.projection, TileProjection::Equirectangular);
    assert_eq!(europa.body, PlanetaryBody::europa());
    assert_eq!(
        europa.tile_url(TileId::new(3, 4, 5)),
        "https://trek.nasa.gov/tiles/Europa/EQ/20150218_europa_global_map_20000x10000/1.0.0/default/default028mm/3/5/4.png"
    );

    let ceres = TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Ceres);
    assert_eq!(ceres.normalized_zoom_range(), (0, 5));
    assert_eq!(ceres.body, PlanetaryBody::ceres());
    assert_eq!(
        ceres.tile_url(TileId::new(2, 3, 1)),
        "https://trek.nasa.gov/tiles/Ceres/EQ/Ceres_Dawn_FC_HAMO_ClrShade_DLR_Global_60ppd_Oct2016/1.0.0/default/default028mm/2/1/3.jpg"
    );
}

#[test]
fn tile_source_can_describe_planetary_equirectangular_sources() {
    let source = TileSource::xyz("https://tiles.example/mars/{z}/{x}/{y}.png")
        .with_projection(TileProjection::Equirectangular)
        .with_body(PlanetaryBody::mars());

    assert_eq!(source.projection, TileProjection::Equirectangular);
    assert_eq!(source.body, PlanetaryBody::mars());
}

#[test]
fn tile_id_checked_constructor_rejects_unaddressable_zooms() {
    assert_eq!(
        TileId::new(MAX_TILE_ZOOM.saturating_add(1), 0, 0).z,
        MAX_TILE_ZOOM
    );
    assert_eq!(
        TileId::new_checked(MAX_TILE_ZOOM.saturating_add(1), 0, 0),
        None
    );
    assert_eq!(
        TileId::new_checked(MAX_TILE_ZOOM, 1, 2),
        Some(TileId::new(MAX_TILE_ZOOM, 1, 2))
    );
}

#[test]
fn tile_origin_and_center_are_consistent() {
    let tile = TileId::new(3, 4, 5);
    let origin = tile_origin_world_pixel(tile, 256.0);
    let center = tile_center_world_pixel(tile, 256.0);

    assert!((origin.x - 1024.0).abs() < f64::EPSILON);
    assert!((origin.y - 1280.0).abs() < f64::EPSILON);
    assert!((center.x - 1152.0).abs() < f64::EPSILON);
    assert!((center.y - 1408.0).abs() < f64::EPSILON);
}

#[test]
fn tile_parent_ancestor_and_offsets_match() {
    let tile = TileId::new(5, 19, 10);
    let parent = tile.parent();
    let ancestor = tile.ancestor_at_zoom(2);
    let ancestor_offsets = ancestor.and_then(|ancestor| tile.descendant_offset_within(ancestor));

    assert_eq!(parent, Some(TileId::new(4, 9, 5)));
    assert_eq!(ancestor, Some(TileId::new(2, 2, 1)));
    assert_eq!(ancestor_offsets, Some((3, 2, 8)));
}

#[test]
fn descendant_offset_rejects_non_ancestor_tiles() {
    let tile = TileId::new(4, 6, 7);
    let non_ancestor = TileId::new(3, 2, 2);

    assert_eq!(tile.descendant_offset_within(non_ancestor), None);
}

#[test]
fn visible_tiles_at_zoom_zero_reduce_to_single_world_tile() {
    let view = ViewState {
        center: LonLat::default(),
        zoom: 0.0,
        viewport_size: Vec2d::new(256.0, 256.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };

    assert_eq!(visible_tiles(&view, 0), vec![TileId::new(0, 0, 0)]);
}

#[test]
fn visible_tiles_use_equirectangular_two_to_one_root_matrix() {
    let view = ViewState {
        center: LonLat::default(),
        zoom: 0.0,
        viewport_size: Vec2d::new(1.0, 1.0),
        tile_size: 256.0,
        projection: TileProjection::Equirectangular,
    };

    assert_eq!(
        visible_tiles(&view, 0),
        vec![TileId::new(0, 0, 0), TileId::new(0, 1, 0)]
    );
}

#[test]
fn visible_tiles_treat_right_and_bottom_edges_as_exclusive() {
    let view = ViewState {
        center: LonLat::new(-90.0, 0.0),
        zoom: 1.0,
        viewport_size: Vec2d::new(256.0, 128.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };
    let tiles = visible_tiles(&view, 0);

    assert!(tiles.iter().all(|tile| tile.x == 0));
}

#[test]
fn visible_tiles_normalize_invalid_public_tile_size() {
    let view = ViewState {
        center: LonLat::default(),
        zoom: 2.0,
        viewport_size: Vec2d::new(2.0, 2.0),
        tile_size: 0.0,
        projection: TileProjection::WebMercator,
    };
    let tiles = visible_tiles(&view, 0);

    assert!(!tiles.is_empty());
    assert!(tiles.len() <= 9);
    assert!(tiles.iter().all(|tile| tile.z == 2));
}

#[test]
fn tiles_at_zoom_covers_the_full_world_grid() {
    let tiles = tiles_at_zoom(2);

    assert_eq!(tiles.len(), 16);
    assert_eq!(tiles.first(), Some(&TileId::new(2, 0, 0)));
    assert_eq!(tiles.last(), Some(&TileId::new(2, 3, 3)));
}

#[test]
fn equirectangular_tile_matrix_uses_two_to_one_world_shape() {
    let dimensions = tile_matrix_dimensions(TileProjection::Equirectangular, 3);
    let tiles = tiles_at_zoom_for_projection(3, TileProjection::Equirectangular);

    assert_eq!(dimensions.columns, 16);
    assert_eq!(dimensions.rows, 8);
    assert_eq!(tiles.len(), 128);
    assert_eq!(tiles.first(), Some(&TileId::new(3, 0, 0)));
    assert_eq!(tiles.last(), Some(&TileId::new(3, 15, 7)));
}

#[test]
fn max_zoom_tile_matrices_fit_u32_for_all_supported_projections() {
    let web = tile_matrix_dimensions(TileProjection::WebMercator, MAX_TILE_ZOOM);
    let equirectangular = tile_matrix_dimensions(TileProjection::Equirectangular, MAX_TILE_ZOOM);

    assert_eq!(web.columns, 1_u32 << u32::from(MAX_TILE_ZOOM));
    assert_eq!(web.rows, 1_u32 << u32::from(MAX_TILE_ZOOM));
    assert_eq!(equirectangular.columns, 2_u32 << u32::from(MAX_TILE_ZOOM));
    assert_eq!(equirectangular.rows, 1_u32 << u32::from(MAX_TILE_ZOOM));
}

#[test]
fn tile_lon_lat_bounds_match_the_world_tile() {
    let bounds = tile_lon_lat_bounds(TileId::new(0, 0, 0));

    assert!((bounds.west_lon_deg + 180.0).abs() < 1.0e-9);
    assert!((bounds.east_lon_deg - 180.0).abs() < 1.0e-9);
    assert!((bounds.north_lat_deg - 85.051_128_78).abs() < 1.0e-6);
    assert!((bounds.south_lat_deg + 85.051_128_78).abs() < 1.0e-6);
}

#[test]
fn equirectangular_tile_samples_are_linear_in_lon_lat() {
    let sample = tile_sample_lon_lat_for_projection(
        TileId::new(0, 1, 0),
        TileProjection::Equirectangular,
        0.5,
        0.5,
    );

    assert!((sample.lon_deg - 90.0).abs() < f64::EPSILON);
    assert!(sample.lat_deg.abs() < f64::EPSILON);
}

#[test]
fn tile_sample_lon_lat_uses_mercator_y_space() {
    let sample = tile_sample_lon_lat(TileId::new(1, 0, 0), 0.5, 0.5);

    assert!((sample.lon_deg + 90.0).abs() < f64::EPSILON);
    assert!((sample.lat_deg - 66.513_260_443_111_86).abs() < 1.0e-12);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        ..proptest::test_runner::Config::default()
    })]

    #[test]
    fn visible_tiles_are_unique_and_within_bounds(
        lon in lon_strategy(),
        lat in lat_strategy(),
        zoom in zoom_strategy(),
        tile_size in tile_size_strategy(),
        width in 64_u16..4097_u16,
        height in 64_u16..4097_u16,
        overscan in 0_u32..3_u32,
    ) {
        let view = ViewState {
            center: LonLat::new(lon, lat),
            zoom: f64::from(zoom),
            viewport_size: Vec2d::new(f64::from(width), f64::from(height)),
            tile_size,
            projection: TileProjection::WebMercator,
        };
        let tiles = visible_tiles(&view, overscan);
        let unique_tiles = tiles.iter().copied().collect::<BTreeSet<_>>();
        let tiles_per_axis = 1_u32 << u32::from(view.discrete_zoom());

        prop_assert_eq!(tiles.len(), unique_tiles.len());

        for tile in &tiles {
            prop_assert_eq!(tile.z, view.discrete_zoom());
            prop_assert!(tile.x < tiles_per_axis);
            prop_assert!(tile.y < tiles_per_axis);
        }
    }

    #[test]
    fn visible_tiles_are_monotonic_with_overscan(
        lon in lon_strategy(),
        lat in lat_strategy(),
        zoom in zoom_strategy(),
        tile_size in tile_size_strategy(),
        width in 64_u16..4097_u16,
        height in 64_u16..4097_u16,
        base_overscan in 0_u32..3_u32,
        extra_overscan in 0_u32..3_u32,
    ) {
        let view = ViewState {
            center: LonLat::new(lon, lat),
            zoom: f64::from(zoom),
            viewport_size: Vec2d::new(f64::from(width), f64::from(height)),
            tile_size,
            projection: TileProjection::WebMercator,
        };
        let smaller = visible_tiles(&view, base_overscan)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let larger = visible_tiles(&view, base_overscan + extra_overscan)
            .into_iter()
            .collect::<BTreeSet<_>>();

        prop_assert!(smaller.is_subset(&larger));
    }

    #[test]
    fn ancestor_offsets_match_generated_tiles(
        zoom in zoom_strategy(),
        raw_x in 0_u32..(1_u32 << 22),
        raw_y in 0_u32..(1_u32 << 22),
        raw_ancestor_zoom in 0_u8..23_u8,
    ) {
        let tiles_per_axis = 1_u32 << u32::from(zoom);
        let tile = TileId::new(zoom, raw_x % tiles_per_axis, raw_y % tiles_per_axis);
        let ancestor_zoom = raw_ancestor_zoom % (zoom + 1);
        let ancestor = tile.ancestor_at_zoom(ancestor_zoom);
        let offsets = ancestor.and_then(|ancestor| tile.descendant_offset_within(ancestor));
        let expected_scale = 1_u32 << u32::from(zoom - ancestor_zoom);

        prop_assert_eq!(ancestor, Some(TileId::new(
            ancestor_zoom,
            tile.x >> u32::from(zoom - ancestor_zoom),
            tile.y >> u32::from(zoom - ancestor_zoom),
        )));
        prop_assert_eq!(offsets.map(|(_, _, scale)| scale), Some(expected_scale));

        if let Some((offset_x, offset_y, scale)) = offsets {
            prop_assert!(offset_x < scale);
            prop_assert!(offset_y < scale);
        }
    }
}
