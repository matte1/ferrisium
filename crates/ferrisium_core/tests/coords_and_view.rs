mod common;

use ferrisium_core::coords::{
    lon_lat_to_world_pixel, lon_lat_to_world_pixel_for_projection, map_size, projected_world_size,
    world_pixel_to_lon_lat, world_pixel_to_lon_lat_for_projection, LonLat, Vec2d,
    WEB_MERCATOR_MAX_LAT_DEG,
};
use ferrisium_core::prelude::{TileProjection, ViewState, MAX_TILE_ZOOM};
use proptest::prelude::{prop_assert, prop_assume};

use crate::common::{
    assert_lon_lat_close, lat_strategy, lon_strategy, tile_size_strategy, wrapped_lon_distance,
    zoom_strategy, GEO_EPSILON, PAN_EPSILON,
};

#[test]
fn map_size_doubles_for_each_integer_zoom_step() {
    let zoom_two = map_size(256.0, 2.0);
    let zoom_three = map_size(256.0, 3.0);

    assert!((zoom_two - 1024.0).abs() < f64::EPSILON);
    assert!((zoom_three - 2048.0).abs() < f64::EPSILON);
}

#[test]
fn equirectangular_world_size_uses_two_to_one_aspect_ratio() {
    let world_size = projected_world_size(TileProjection::Equirectangular, 256.0, 2.0);

    assert!((world_size.x - 2048.0).abs() < f64::EPSILON);
    assert!((world_size.y - 1024.0).abs() < f64::EPSILON);
}

#[test]
fn equirectangular_projection_roundtrips_body_lon_lat() {
    let lon_lat = LonLat::new(45.0, -30.0);
    let world =
        lon_lat_to_world_pixel_for_projection(lon_lat, TileProjection::Equirectangular, 3.0, 256.0);
    let roundtrip =
        world_pixel_to_lon_lat_for_projection(world, TileProjection::Equirectangular, 3.0, 256.0);

    assert_lon_lat_close(roundtrip, lon_lat, GEO_EPSILON);
}

#[test]
fn equirectangular_view_uses_world_height_for_covering_zoom() {
    let view = ViewState {
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        projection: TileProjection::Equirectangular,
        ..ViewState::default()
    };

    assert!((view.min_covering_zoom() - (720.0_f64 / 256.0).log2()).abs() < f64::EPSILON);
}

#[test]
fn min_covering_zoom_matches_the_viewport_requirement() {
    let view = ViewState {
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        ..ViewState::default()
    };

    assert!((view.min_covering_zoom() - 5.0_f64.log2()).abs() < f64::EPSILON);
}

#[test]
fn view_state_normalizes_invalid_public_tile_size_for_map_math() {
    let mut view = ViewState {
        center: LonLat::new(-122.335, 47.608),
        zoom: 4.0,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 0.0,
        projection: TileProjection::WebMercator,
    };

    assert!((view.normalized_tile_size() - 1.0).abs() <= f64::EPSILON);
    assert!(view.min_covering_zoom().is_finite());
    assert_lon_lat_close(
        view.screen_to_lon_lat(Vec2d::new(640.0, 360.0)),
        view.center.clamped(),
        GEO_EPSILON,
    );
    view.pan_by_pixels(Vec2d::new(8.0, -4.0));
    assert!(view.center.lon_deg.is_finite());
    assert!(view.center.lat_deg.is_finite());
    view.constrain_to_viewport();
    assert!(view.zoom.is_finite());
}

#[test]
fn lon_lat_clamped_wraps_longitude_and_limits_latitude() {
    let clamped = LonLat::new(540.0, 120.0).clamped();

    assert!(wrapped_lon_distance(clamped.lon_deg, -180.0) < f64::EPSILON);
    assert!((clamped.lat_deg - WEB_MERCATOR_MAX_LAT_DEG).abs() < f64::EPSILON);
}

#[test]
fn screen_center_maps_back_to_view_center() {
    let view = ViewState {
        center: LonLat::new(-73.985_7, 40.748_4),
        zoom: 4.25,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };

    let mapped = view.screen_to_lon_lat(Vec2d::new(640.0, 360.0));

    assert_lon_lat_close(mapped, view.center, GEO_EPSILON);
}

#[test]
fn pan_by_pixels_is_reversible_for_fractional_zoom() {
    let original_center = LonLat::new(-73.985_7, 40.748_4);
    let mut view = ViewState {
        center: original_center,
        zoom: 4.25,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };
    let delta = Vec2d::new(180.0, -120.0);

    view.pan_by_pixels(delta);
    view.pan_by_pixels(Vec2d::new(-delta.x, -delta.y));

    assert_lon_lat_close(view.center, original_center, PAN_EPSILON);
}

#[test]
fn zoom_about_screen_position_keeps_the_cursor_anchor_stable() {
    let cursor = Vec2d::new(960.0, 240.0);
    let mut view = ViewState {
        center: LonLat::new(-73.985_7, 40.748_4),
        zoom: 4.25,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };
    let anchored_before = view.screen_to_lon_lat(cursor);

    view.zoom_about_screen_position(cursor, 1.5);

    let anchored_after = view.screen_to_lon_lat(cursor);
    assert_lon_lat_close(anchored_after, anchored_before, GEO_EPSILON);
}

#[test]
fn constrain_to_viewport_raises_zoom_to_keep_the_map_full_screen() {
    let mut view = ViewState {
        zoom: 0.0,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        ..ViewState::default()
    };

    view.constrain_to_viewport();

    assert!((view.zoom - 5.0_f64.log2()).abs() < f64::EPSILON);
    assert_lon_lat_close(view.center, LonLat::default(), GEO_EPSILON);
}

#[test]
fn constrain_to_viewport_clamps_vertical_overpan_at_the_poles() {
    let mut view = ViewState {
        center: LonLat::new(0.0, WEB_MERCATOR_MAX_LAT_DEG),
        zoom: 4.0,
        viewport_size: Vec2d::new(1280.0, 720.0),
        tile_size: 256.0,
        projection: TileProjection::WebMercator,
    };

    view.constrain_to_viewport();

    let top_edge = view.screen_to_lon_lat(Vec2d::new(640.0, 0.0));
    assert!((top_edge.lat_deg - WEB_MERCATOR_MAX_LAT_DEG).abs() < GEO_EPSILON);
}

#[test]
fn discrete_zoom_rounds_and_clamps() {
    let lower = ViewState {
        zoom: 2.49,
        ..ViewState::default()
    };
    let upper = ViewState {
        zoom: 2.5,
        ..ViewState::default()
    };
    let clamped_low = ViewState {
        zoom: -50.0,
        ..ViewState::default()
    };
    let clamped_high = ViewState {
        zoom: 99.0,
        ..ViewState::default()
    };

    assert_eq!(lower.discrete_zoom(), 2);
    assert_eq!(upper.discrete_zoom(), 3);
    assert_eq!(clamped_low.discrete_zoom(), 0);
    assert_eq!(clamped_high.discrete_zoom(), MAX_TILE_ZOOM);
}

#[test]
fn zoom_by_steps_clamps_to_supported_range() {
    let mut view = ViewState::default();

    view.zoom_by_steps(100.0);
    assert!((view.zoom - f64::from(MAX_TILE_ZOOM)).abs() < f64::EPSILON);

    view.zoom_by_steps(-500.0);
    assert!(view.zoom.abs() < f64::EPSILON);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        ..proptest::test_runner::Config::default()
    })]

    #[test]
    fn wrap_longitude_always_returns_values_in_expected_interval(
        raw in -1_000_000_000_i32..1_000_000_000_i32,
    ) {
        let wrapped = ferrisium_core::coords::wrap_longitude(f64::from(raw) / 1_000_000.0);

        prop_assert!(wrapped >= -180.0);
        prop_assert!(wrapped < 180.0);
    }

    #[test]
    fn lon_lat_projection_roundtrip_preserves_values(
        lon in lon_strategy(),
        lat in lat_strategy(),
        zoom in zoom_strategy(),
        tile_size in tile_size_strategy(),
    ) {
        let lon_lat = LonLat::new(lon, lat);
        let world = lon_lat_to_world_pixel(lon_lat, f64::from(zoom), tile_size);
        let roundtrip = world_pixel_to_lon_lat(world, f64::from(zoom), tile_size);

        assert_lon_lat_close(roundtrip, lon_lat, GEO_EPSILON);
    }

    #[test]
    fn pan_by_pixels_is_reversible(
        lon in lon_strategy(),
        lat in lat_strategy(),
        zoom in zoom_strategy(),
        tile_size in tile_size_strategy(),
        width in 64_u16..4097_u16,
        height in 64_u16..4097_u16,
        delta_x in -2048_i32..2048_i32,
        delta_y in -2048_i32..2048_i32,
    ) {
        let original_center = LonLat::new(lon, lat);
        let view = ViewState {
            center: original_center,
            zoom: f64::from(zoom),
            viewport_size: Vec2d::new(f64::from(width), f64::from(height)),
            tile_size,
            projection: TileProjection::WebMercator,
        };
        let delta = Vec2d::new(f64::from(delta_x), f64::from(delta_y));
        let mut panned = view;

        panned.pan_by_pixels(delta);
        prop_assume!(panned.center.lat_deg.abs() < WEB_MERCATOR_MAX_LAT_DEG - GEO_EPSILON);
        panned.pan_by_pixels(Vec2d::new(-delta.x, -delta.y));

        assert_lon_lat_close(panned.center, original_center, PAN_EPSILON);
    }

    #[test]
    fn screen_center_always_maps_to_center_for_generated_views(
        lon in lon_strategy(),
        lat in lat_strategy(),
        zoom in zoom_strategy(),
        tile_size in tile_size_strategy(),
        width in 64_u16..4097_u16,
        height in 64_u16..4097_u16,
    ) {
        let view = ViewState {
            center: LonLat::new(lon, lat),
            zoom: f64::from(zoom),
            viewport_size: Vec2d::new(f64::from(width), f64::from(height)),
            tile_size,
            projection: TileProjection::WebMercator,
        };
        let screen_center = Vec2d::new(
            view.viewport_size.x * 0.5,
            view.viewport_size.y * 0.5,
        );

        let mapped = view.screen_to_lon_lat(screen_center);

        assert_lon_lat_close(mapped, view.center, GEO_EPSILON);
    }

    #[test]
    fn zoom_about_screen_position_keeps_generated_cursor_anchors_stable(
        lon in lon_strategy(),
        lat in lat_strategy(),
        raw_zoom in 0_u8..89_u8,
        tile_size in tile_size_strategy(),
        width in 64_u16..4097_u16,
        height in 64_u16..4097_u16,
        raw_cursor_x in 0_u16..4097_u16,
        raw_cursor_y in 0_u16..4097_u16,
        zoom_delta_quarters in -16_i16..17_i16,
    ) {
        let mut view = ViewState {
            center: LonLat::new(lon, lat),
            zoom: f64::from(raw_zoom) / 4.0,
            viewport_size: Vec2d::new(f64::from(width), f64::from(height)),
            tile_size,
            projection: TileProjection::WebMercator,
        };
        view.constrain_to_viewport();
        let cursor = Vec2d::new(
            f64::from(raw_cursor_x % width),
            f64::from(raw_cursor_y % height),
        );
        let anchored_before = view.screen_to_lon_lat(cursor);
        let next_zoom = (view.zoom + f64::from(zoom_delta_quarters) / 4.0)
            .clamp(view.min_covering_zoom(), 22.0);
        let anchored_world = lon_lat_to_world_pixel(anchored_before, next_zoom, tile_size);
        let screen_offset = Vec2d::new(
            cursor.x - view.viewport_size.x * 0.5,
            cursor.y - view.viewport_size.y * 0.5,
        );
        let next_center_world = anchored_world - screen_offset;
        let next_map_size = map_size(tile_size, next_zoom);
        let (min_center_y, max_center_y) = if view.viewport_size.y >= next_map_size {
            let center_y = next_map_size * 0.5;
            (center_y, center_y)
        } else {
            let half_viewport_height = view.viewport_size.y * 0.5;
            (half_viewport_height, next_map_size - half_viewport_height)
        };

        prop_assume!((min_center_y..=max_center_y).contains(&next_center_world.y));

        view.zoom_about_screen_position(cursor, f64::from(zoom_delta_quarters) / 4.0);

        let anchored_after = view.screen_to_lon_lat(cursor);
        assert_lon_lat_close(anchored_after, anchored_before, PAN_EPSILON);
    }
}
