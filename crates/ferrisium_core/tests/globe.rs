use std::collections::BTreeSet;

use ferrisium_core::globe::{
    globe_tile_priority, globe_tile_priority_for_projection, globe_tile_zoom, select_globe_tiles,
    sort_globe_tiles_by_priority, GlobeDirection, GlobeTileSelectorConfig, GlobeTileSelectorInputs,
};
use ferrisium_core::source::{PlanetaryBody, TileProjection, TileSource};
use ferrisium_core::tiles::{tiles_at_zoom, TileId};
use proptest::prelude::*;

fn demo_source() -> TileSource {
    TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 6)
}

fn selector_inputs(
    tile_source: &TileSource,
    current_surface_distance: f64,
    camera_surface_direction: GlobeDirection,
) -> GlobeTileSelectorInputs<'_> {
    GlobeTileSelectorInputs {
        tile_source,
        base_zoom: 2,
        base_surface_distance: 5.0,
        current_surface_distance,
        camera_surface_direction,
    }
}

fn direction_from_lon_lat(lon_deg: f64, lat_deg: f64) -> GlobeDirection {
    let lon_rad = lon_deg.to_radians();
    let lat_rad = lat_deg.to_radians();
    let horizontal_radius = lat_rad.cos();

    GlobeDirection::new(
        horizontal_radius * lon_rad.sin(),
        lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}

#[test]
fn globe_tile_zoom_increases_when_the_camera_moves_closer() {
    let source = demo_source();
    let far_inputs = selector_inputs(&source, 5.0, GlobeDirection::new(0.0, 0.0, 1.0));
    let near_inputs = selector_inputs(&source, 1.4, GlobeDirection::new(0.0, 0.0, 1.0));

    assert_eq!(
        globe_tile_zoom(far_inputs, GlobeTileSelectorConfig::default()),
        2
    );
    assert!(
        globe_tile_zoom(near_inputs, GlobeTileSelectorConfig::default())
            > globe_tile_zoom(far_inputs, GlobeTileSelectorConfig::default())
    );
}

#[test]
fn globe_tile_zoom_respects_the_inward_zoom_cap() {
    let source = demo_source();
    let inputs = selector_inputs(&source, 0.1, GlobeDirection::new(0.0, 0.0, 1.0));
    let config = GlobeTileSelectorConfig {
        max_zoom_steps_inward: 2,
        ..GlobeTileSelectorConfig::default()
    };

    assert_eq!(globe_tile_zoom(inputs, config), 4);
}

#[test]
fn visible_tiles_include_camera_facing_and_horizon_tiles() {
    let source = demo_source();
    let selection = select_globe_tiles(
        selector_inputs(&source, 5.0, GlobeDirection::new(0.0, 0.0, 1.0)),
        GlobeTileSelectorConfig::default(),
    );

    assert_eq!(selection.tile_zoom, 2);
    assert!(selection.visible_tiles.contains(&TileId::new(2, 1, 1)));
    assert!(selection.visible_tiles.contains(&TileId::new(2, 2, 1)));
    assert!(selection.visible_tiles.contains(&TileId::new(2, 0, 1)));
    assert!(selection.visible_tiles.contains(&TileId::new(2, 3, 1)));
}

#[test]
fn visible_tiles_keep_tiles_with_visible_edges() {
    let source = demo_source().with_zoom_range(0, 4);
    let selection = select_globe_tiles(
        GlobeTileSelectorInputs {
            tile_source: &source,
            base_zoom: 4,
            base_surface_distance: 5.0,
            current_surface_distance: 5.0,
            camera_surface_direction: direction_from_lon_lat(0.0, -70.0),
        },
        GlobeTileSelectorConfig {
            max_zoom_steps_inward: 0,
            max_selected_tiles: 1_000,
            ..GlobeTileSelectorConfig::default()
        },
    );

    assert!(selection.visible_tiles.contains(&TileId::new(4, 5, 6)));
}

#[test]
fn queued_tiles_are_sorted_by_globe_priority() {
    let camera_surface_direction = GlobeDirection::new(0.0, 0.0, 1.0);
    let mut tiles = vec![
        TileId::new(2, 0, 1),
        TileId::new(2, 1, 1),
        TileId::new(2, 3, 1),
    ];

    sort_globe_tiles_by_priority(&mut tiles, camera_surface_direction);

    assert_eq!(tiles.first(), Some(&TileId::new(2, 1, 1)));
    assert!(tiles.windows(2).all(|window| {
        globe_tile_priority(window[0], camera_surface_direction)
            >= globe_tile_priority(window[1], camera_surface_direction)
    }));
}

#[test]
fn globe_tile_priority_prefers_tiles_near_the_camera_direction() {
    let camera_surface_direction = GlobeDirection::new(0.0, 0.0, 1.0);

    assert!(
        globe_tile_priority(TileId::new(2, 1, 1), camera_surface_direction)
            > globe_tile_priority(TileId::new(2, 0, 1), camera_surface_direction)
    );
}

#[test]
fn globe_tile_priority_respects_equirectangular_tile_centers() {
    let camera_surface_direction = GlobeDirection::new(1.0, 0.0, 0.0);

    assert!(
        globe_tile_priority_for_projection(
            TileId::new(0, 1, 0),
            TileProjection::Equirectangular,
            camera_surface_direction
        ) > globe_tile_priority_for_projection(
            TileId::new(0, 0, 0),
            TileProjection::Equirectangular,
            camera_surface_direction
        )
    );
}

#[test]
fn close_globe_selection_refines_within_the_tile_budget() {
    let source = demo_source();
    let selection = select_globe_tiles(
        selector_inputs(&source, 0.2, GlobeDirection::new(0.0, 0.0, 1.0)),
        GlobeTileSelectorConfig {
            max_selected_tiles: 32,
            ..GlobeTileSelectorConfig::default()
        },
    );

    assert_eq!(selection.tile_zoom, 6);
    assert!(selection.visible_tiles.len() <= 32);
    assert!(selection.visible_tiles.iter().any(|tile| tile.z == 6));
    assert!(selection.visible_tiles.iter().any(|tile| tile.z < 6));
}

#[test]
fn equirectangular_globe_selection_uses_two_to_one_root_tiles() {
    let source = TileSource::nasa_trek_moon_lro_wac().with_zoom_range(0, 0);
    let selection = select_globe_tiles(
        selector_inputs(&source, 5.0, GlobeDirection::new(1.0, 0.0, 0.0)),
        GlobeTileSelectorConfig::default(),
    );

    assert_eq!(selection.tile_zoom, 0);
    assert!(selection.visible_tiles.contains(&TileId::new(0, 1, 0)));
    assert!(selection.visible_tiles.contains(&TileId::new(0, 0, 0)));
    assert_eq!(selection.visible_tiles.len(), 2);
}

#[test]
fn globe_refinement_preserves_child_quadrant_coverage() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 3);
    let selection = select_globe_tiles(
        GlobeTileSelectorInputs {
            tile_source: &source,
            base_zoom: 2,
            base_surface_distance: 5.0,
            current_surface_distance: 0.2,
            camera_surface_direction: GlobeDirection::new(
                0.0,
                -std::f64::consts::FRAC_1_SQRT_2,
                -std::f64::consts::FRAC_1_SQRT_2,
            ),
        },
        GlobeTileSelectorConfig {
            max_selected_tiles: 128,
            ..GlobeTileSelectorConfig::default()
        },
    );

    assert_eq!(selection.tile_zoom, 3);
    for parent in tiles_at_zoom(2) {
        let child_x = parent.x.saturating_mul(2);
        let child_y = parent.y.saturating_mul(2);
        let children = [
            TileId::new(3, child_x, child_y),
            TileId::new(3, child_x.saturating_add(1), child_y),
            TileId::new(3, child_x, child_y.saturating_add(1)),
            TileId::new(3, child_x.saturating_add(1), child_y.saturating_add(1)),
        ];
        let present_children = children
            .iter()
            .filter(|child| selection.visible_tiles.contains(child))
            .count();

        assert!(
            present_children == 0 || present_children == children.len(),
            "partial child coverage for {parent:?}: {present_children}/{}",
            children.len()
        );
    }
}

#[test]
fn default_globe_tile_zoom_can_reach_the_source_max_zoom() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 19);
    let inputs = GlobeTileSelectorInputs {
        tile_source: &source,
        base_zoom: 4,
        base_surface_distance: 6.5,
        current_surface_distance: 0.0001,
        camera_surface_direction: GlobeDirection::new(0.0, 0.0, 1.0),
    };

    assert_eq!(
        globe_tile_zoom(inputs, GlobeTileSelectorConfig::default()),
        19
    );
}

#[test]
fn globe_selection_handles_high_base_zoom_without_materializing_the_world() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 19);
    let selection = select_globe_tiles(
        GlobeTileSelectorInputs {
            tile_source: &source,
            base_zoom: 12,
            base_surface_distance: 6.5,
            current_surface_distance: 6.5,
            camera_surface_direction: GlobeDirection::new(0.0, 0.0, 1.0),
        },
        GlobeTileSelectorConfig {
            max_selected_tiles: 32,
            ..GlobeTileSelectorConfig::default()
        },
    );

    assert_eq!(selection.tile_zoom, 12);
    assert!(selection.visible_tiles.len() <= 32);
    assert!(selection.visible_tiles.iter().all(|tile| tile.z == 12));
}

#[test]
fn equirectangular_high_zoom_seeding_reaches_the_poles() {
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
        .with_projection(TileProjection::Equirectangular)
        .with_body(PlanetaryBody::moon())
        .with_zoom_range(0, 12);
    let selection = select_globe_tiles(
        GlobeTileSelectorInputs {
            tile_source: &source,
            base_zoom: 12,
            base_surface_distance: 6.5,
            current_surface_distance: 6.5,
            camera_surface_direction: GlobeDirection::new(0.0, 1.0, 0.0),
        },
        GlobeTileSelectorConfig {
            max_selected_tiles: 32,
            ..GlobeTileSelectorConfig::default()
        },
    );

    assert_eq!(selection.tile_zoom, 12);
    assert!(
        selection.visible_tiles.iter().any(|tile| tile.y == 0),
        "north-pole-facing selection should seed from the top equirectangular row"
    );
}

#[test]
fn globe_tile_zoom_normalizes_manual_source_zoom_ranges() {
    let source = TileSource {
        url_template: "https://tiles.example/{z}/{x}/{y}.png".into(),
        attribution: String::new(),
        min_zoom: 10,
        max_zoom: 2,
        tile_size: 256,
        raster_tile_size: 256,
        projection: TileProjection::WebMercator,
        body: PlanetaryBody::earth(),
    };
    let inputs = GlobeTileSelectorInputs {
        tile_source: &source,
        base_zoom: 6,
        base_surface_distance: 5.0,
        current_surface_distance: 5.0,
        camera_surface_direction: GlobeDirection::new(0.0, 0.0, 1.0),
    };

    assert_eq!(
        globe_tile_zoom(inputs, GlobeTileSelectorConfig::default()),
        6
    );
}

proptest! {
    #[test]
    fn moving_closer_never_reduces_selected_zoom(
        base_surface_distance_millis in 1_000_u32..20_000_u32,
        far_surface_distance_millis in 1_000_u32..20_000_u32,
        near_surface_distance_millis in 100_u32..20_000_u32,
        base_zoom in 0_u8..6_u8,
        max_zoom in 0_u8..8_u8,
    ) {
        prop_assume!(base_zoom <= max_zoom);
        let far_surface_distance = f64::from(far_surface_distance_millis) / 1_000.0;
        let near_surface_distance = f64::from(near_surface_distance_millis) / 1_000.0;
        prop_assume!(near_surface_distance <= far_surface_distance);

        let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
            .with_zoom_range(0, max_zoom);
        let inputs = |current_surface_distance| GlobeTileSelectorInputs {
            tile_source: &source,
            base_zoom,
            base_surface_distance: f64::from(base_surface_distance_millis) / 1_000.0,
            current_surface_distance,
            camera_surface_direction: GlobeDirection::new(0.0, 0.0, 1.0),
        };

        let far_zoom = globe_tile_zoom(inputs(far_surface_distance), GlobeTileSelectorConfig::default());
        let near_zoom = globe_tile_zoom(inputs(near_surface_distance), GlobeTileSelectorConfig::default());

        prop_assert!(near_zoom >= far_zoom);
    }

    #[test]
    fn selected_tiles_stay_within_the_selected_zoom(
        current_surface_distance_millis in 100_u32..20_000_u32,
        base_zoom in 0_u8..6_u8,
        max_zoom in 0_u8..8_u8,
    ) {
        prop_assume!(base_zoom <= max_zoom);
        let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
            .with_zoom_range(0, max_zoom);
        let selection = select_globe_tiles(
            GlobeTileSelectorInputs {
                tile_source: &source,
                base_zoom,
                base_surface_distance: 5.0,
                current_surface_distance: f64::from(current_surface_distance_millis) / 1_000.0,
                camera_surface_direction: GlobeDirection::new(0.0, 0.0, 1.0),
            },
            GlobeTileSelectorConfig::default(),
        );
        let all_tiles_are_in_bounds = selection.visible_tiles.iter().all(|tile| {
            let axis = 1_u32 << u32::from(tile.z);
            tile.x < axis && tile.y < axis
        });

        prop_assert!(selection.visible_tiles.len() <= GlobeTileSelectorConfig::default().max_selected_tiles);
        prop_assert!(selection.visible_tiles.iter().all(|tile| tile.z <= selection.tile_zoom));
        prop_assert!(all_tiles_are_in_bounds);
        for tile in &selection.visible_tiles {
            let all_tiles = tiles_at_zoom(tile.z)
                .into_iter()
                .collect::<BTreeSet<_>>();
            prop_assert!(all_tiles.contains(tile));
        }
    }
}
