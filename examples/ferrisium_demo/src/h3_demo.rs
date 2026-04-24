#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use crate::browser_params::browser_query_param;
#[cfg(any(target_arch = "wasm32", test))]
use crate::browser_params::parse_bool_flag as parse_h3_inspection_flag;
#[cfg(test)]
use crate::browser_params::query_param;
use bevy::prelude::*;
use ferrisium_bevy::prelude::{
    CelestialBody, GeoEndpoint, GeoPolygon, GeoPolygonBundle, GeoPolygonVertex, GeoPolyline,
    GeoPolylineBundle, GlobeLabel, GlobeLabelBundle, GlobeLabelPosition, GlobeLink,
    GlobeLinkBundle, GlobeLinkEndpoint, GlobePosition, H3CellClicked, H3CellColors, H3Cells,
    H3Overlay, H3OverlayBundle, H3Style,
};
use ferrisium_core::prelude::LonLat;
use geo::{LineString, MultiPolygon, Polygon};
use h3o::geom::{ContainmentMode, TilerBuilder};
use h3o::Resolution;

// Low enough for fast startup, high enough to make H3 coverage visible.
const USA_HEX_RESOLUTION: Resolution = Resolution::Three;
const H3_INSPECTION_GLOBE_OFFSET_UNITS: f32 = 8.0;

pub(crate) fn spawn_usa_h3_overlay(mut commands: Commands<'_, '_>) {
    let cells = usa_h3_cells();
    let inspection_enabled = h3_inspection_enabled();
    let cell_colors = usa_h3_cell_colors(&cells, inspection_enabled);
    let style = if inspection_enabled {
        H3Style::solid([1.0, 0.08, 0.85, 0.72]).with_outline([1.0, 0.92, 1.0, 0.95])
    } else {
        H3Style::solid([0.24, 0.63, 0.86, 0.42]).with_outline([0.05, 0.12, 0.16, 0.62])
    };
    let mut overlay = H3Overlay::earth("usa_demo_coverage", cells, style)
        .with_globe_enabled(inspection_enabled)
        .with_cell_colors(cell_colors);
    if inspection_enabled {
        overlay = overlay.with_globe_surface_offset_units(H3_INSPECTION_GLOBE_OFFSET_UNITS);
    }

    commands.spawn((
        Name::new("USA H3 Coverage Overlay"),
        H3OverlayBundle::new(overlay),
    ));
    commands.spawn((
        Name::new("USA Demo Geodetic Polyline"),
        GeoPolylineBundle::new(demo_geodetic_polyline(inspection_enabled)),
    ));
    commands.spawn((
        Name::new("USA Demo Geo Polygon Footprint"),
        GeoPolygonBundle::new(demo_geo_polygon_footprint(inspection_enabled)),
    ));
    if inspection_enabled {
        let remote_marker = commands
            .spawn((
                Name::new("USA Demo GlobeLink Remote Endpoint"),
                GlobePosition::geodetic(&CelestialBody::earth(), 42.4, -107.0, 1_500.0),
            ))
            .id();
        commands.spawn((
            Name::new("USA Demo GlobeLink"),
            GlobeLinkBundle::new(demo_globe_link(remote_marker)),
        ));
        commands.spawn((
            Name::new("USA Demo Globe Label"),
            GlobeLabelBundle::new(demo_globe_label()),
        ));
    }
}

pub(crate) fn log_h3_cell_clicks(mut clicks: MessageReader<'_, '_, H3CellClicked>) {
    for click in clicks.read() {
        info!(
            "H3 cell clicked: layer={} cell={} lon={:.3} lat={:.3}",
            click.hit.layer.name(),
            click.hit.cell.bits(),
            click.hit.lon_lat.lon_deg,
            click.hit.lon_lat.lat_deg
        );
    }
}

fn usa_h3_cells() -> H3Cells {
    let mut tiler = TilerBuilder::new(USA_HEX_RESOLUTION)
        .containment_mode(ContainmentMode::ContainsCentroid)
        .build();
    let geometry = usa_demo_geometry();
    if tiler.add_batch(geometry.0).is_err() {
        warn!("failed to build USA demo H3 coverage");
        return H3Cells::default();
    }

    tiler.into_coverage().collect::<H3Cells>()
}

fn usa_h3_cell_colors(cells: &H3Cells, inspection_enabled: bool) -> H3CellColors {
    H3CellColors::from_rgba(cells.iter().enumerate().map(|(index, cell)| {
        let index = u64::try_from(index).unwrap_or(0);
        let band = u8::try_from((cell.bits() ^ index) % 5).unwrap_or(0);
        let t = f32::from(band) / 4.0;
        if inspection_enabled {
            [0.92 + 0.08 * t, 0.05 + 0.25 * t, 0.72 + 0.22 * t, 0.78]
        } else {
            [0.12 + 0.20 * t, 0.48 + 0.26 * t, 0.70 + 0.18 * t, 0.44]
        }
    }))
}

fn demo_geodetic_polyline(inspection_enabled: bool) -> GeoPolyline {
    let mut polyline = GeoPolyline::earth(
        [
            GeoEndpoint::surface(LonLat::new(-122.3321, 47.6062)),
            GeoEndpoint::surface(LonLat::new(-104.9903, 39.7392)),
            GeoEndpoint::surface(LonLat::new(-77.0369, 38.9072)),
        ],
        [1.0, 0.82, 0.24, 0.88],
    )
    .with_map_z(37.0)
    .with_globe_enabled(inspection_enabled);
    if inspection_enabled {
        polyline = polyline.with_globe_surface_offset_units(H3_INSPECTION_GLOBE_OFFSET_UNITS + 2.0);
    }

    polyline
}

fn demo_geo_polygon_footprint(inspection_enabled: bool) -> GeoPolygon {
    let mut polygon = GeoPolygon::earth(
        [
            GeoPolygonVertex::from_degrees(-109.2, 37.3),
            GeoPolygonVertex::from_degrees(-103.0, 36.8),
            GeoPolygonVertex::from_degrees(-99.2, 40.3),
            GeoPolygonVertex::from_degrees(-104.5, 43.2),
            GeoPolygonVertex::from_degrees(-110.4, 41.0),
        ],
        [0.0, 0.95, 0.88, 0.24],
        [0.0, 0.95, 0.88, 0.86],
    )
    .with_map_z(36.0)
    .with_globe_enabled(inspection_enabled);
    if inspection_enabled {
        polygon = polygon.with_globe_surface_offset_units(H3_INSPECTION_GLOBE_OFFSET_UNITS + 1.0);
    }

    polygon
}

fn demo_globe_link(remote_marker: Entity) -> GlobeLink {
    GlobeLink::earth(
        GlobeLinkEndpoint::entity(remote_marker),
        GlobeLinkEndpoint::geodetic(39.7392, -104.9903, 0.1),
        [0.24, 0.95, 1.0, 0.72],
    )
    .with_width_units(26.0)
}

fn demo_globe_label() -> GlobeLabel {
    GlobeLabel::earth(
        GlobeLabelPosition::geodetic(39.7392, -104.9903, 0.1),
        "Denver uplink",
        [0.86, 1.0, 1.0, 0.95],
    )
    .with_font_size(15.0)
    .with_screen_offset_px(Vec2::new(14.0, -12.0))
}

fn usa_demo_geometry() -> MultiPolygon<f64> {
    MultiPolygon(vec![
        polygon_from_lon_lat(&[
            (-124.8, 48.8),
            (-124.0, 42.0),
            (-117.0, 32.5),
            (-111.0, 31.3),
            (-106.5, 31.4),
            (-103.0, 29.5),
            (-97.2, 25.9),
            (-90.0, 28.9),
            (-83.0, 24.6),
            (-80.0, 26.0),
            (-80.0, 30.7),
            (-75.0, 35.0),
            (-74.0, 40.0),
            (-69.8, 44.8),
            (-71.5, 45.8),
            (-83.0, 46.8),
            (-89.5, 48.0),
            (-95.0, 49.0),
            (-110.0, 49.0),
            (-124.8, 48.8),
        ]),
        polygon_from_lon_lat(&[
            (-179.0, 51.0),
            (-170.0, 53.0),
            (-165.0, 55.0),
            (-160.0, 58.0),
            (-154.0, 59.5),
            (-150.0, 61.0),
            (-145.0, 60.0),
            (-141.0, 69.5),
            (-151.0, 70.0),
            (-168.0, 66.0),
            (-178.0, 58.0),
            (-179.0, 51.0),
        ]),
        polygon_from_lon_lat(&[
            (-160.6, 22.5),
            (-159.0, 21.0),
            (-157.0, 20.0),
            (-154.5, 18.8),
            (-154.5, 22.5),
            (-160.6, 22.5),
        ]),
    ])
}

fn polygon_from_lon_lat(points: &[(f64, f64)]) -> Polygon<f64> {
    let ring = points
        .iter()
        .map(|(lon_deg, lat_deg)| (*lon_deg, *lat_deg))
        .collect::<Vec<_>>();

    Polygon::new(LineString::from(ring), Vec::new())
}

fn h3_inspection_enabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        browser_query_param("h3_inspect")
            .as_deref()
            .is_some_and(parse_h3_inspection_flag)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        demo_geo_polygon_footprint, demo_geodetic_polyline, demo_globe_label, demo_globe_link,
        parse_h3_inspection_flag, polygon_from_lon_lat, query_param, usa_h3_cell_colors,
        usa_h3_cells,
    };
    use bevy::prelude::Entity;

    #[test]
    fn polygon_from_lon_lat_preserves_the_input_ring() {
        let polygon =
            polygon_from_lon_lat(&[(-100.0, 40.0), (-99.0, 40.0), (-99.0, 39.0), (-100.0, 40.0)]);

        assert_eq!(polygon.exterior().0.len(), 4);
    }

    #[test]
    fn usa_h3_cells_builds_demo_coverage() {
        assert!(!usa_h3_cells().is_empty());
    }

    #[test]
    fn usa_h3_cell_colors_match_cell_count() {
        let cells = usa_h3_cells();
        let colors = usa_h3_cell_colors(&cells, false);

        assert_eq!(colors.len(), cells.len());
        assert!(!colors.is_empty());
    }

    #[test]
    fn demo_geodetic_polyline_keeps_globe_opt_in() {
        assert!(!demo_geodetic_polyline(false).globe_enabled);
        assert!(demo_geodetic_polyline(true).globe_enabled);
    }

    #[test]
    fn demo_geo_polygon_footprint_keeps_globe_opt_in() {
        assert!(!demo_geo_polygon_footprint(false).globe_enabled);
        assert!(demo_geo_polygon_footprint(true).globe_enabled);
        assert!(demo_geo_polygon_footprint(false).map_enabled);
    }

    #[test]
    fn demo_globe_link_uses_entity_endpoint() {
        let link = demo_globe_link(Entity::PLACEHOLDER);

        let endpoint = match link.start {
            ferrisium_bevy::prelude::GlobeLinkEndpoint::Entity(endpoint) => Some(endpoint),
            ferrisium_bevy::prelude::GlobeLinkEndpoint::Geodetic { .. }
            | ferrisium_bevy::prelude::GlobeLinkEndpoint::BodyFixed { .. } => None,
        };
        assert_eq!(endpoint, Some(Entity::PLACEHOLDER));
        assert_eq!(link.body.id, ferrisium_core::prelude::BodyId::EARTH);
        assert!(link.width_units > 0.0);
    }

    #[test]
    fn demo_globe_label_uses_geodetic_anchor() {
        let label = demo_globe_label();

        assert!(matches!(
            label.position,
            ferrisium_bevy::prelude::GlobeLabelPosition::Geodetic { .. }
        ));
        assert_eq!(label.body.id, ferrisium_core::prelude::BodyId::EARTH);
        assert!(!label.text.is_empty());
    }

    #[test]
    fn h3_inspection_flag_accepts_browser_truthy_values() {
        assert!(parse_h3_inspection_flag("1"));
        assert!(parse_h3_inspection_flag("true"));
        assert!(parse_h3_inspection_flag("YES"));
        assert!(!parse_h3_inspection_flag("0"));
        assert!(!parse_h3_inspection_flag("false"));
    }

    #[test]
    fn query_param_reads_h3_inspection_flag() {
        assert_eq!(
            query_param("?view=globe&h3_inspect=1", "h3_inspect"),
            Some("1".to_owned())
        );
    }
}
