#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use bevy::prelude::{Color, GlobalAmbientLight, Resource};
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::{CelestialFocus, MetricSceneFocusSelection};
use ferrisium_bevy::prelude::{GlobeSkybox, MetricSceneFocusTarget, MilkyWaySkyboxResolution};
use ferrisium_core::prelude::{LonLat, NasaTrekRegularBody, TileSource};
use ferrisium_core::space::{BodyId, Epoch};

#[cfg(target_arch = "wasm32")]
use crate::browser_params::{
    browser_local_storage_item, browser_query_param, normalized_mapbox_public_token,
    parse_bool_flag, parse_finite_f64, parse_positive_f32,
};

const DEMO_SPACE_FILL_LIGHT_COLOR: Color = Color::srgb(0.68, 0.76, 0.95);
const DEMO_GLOBE_SPACE_FILL_BRIGHTNESS: f32 = 320.0;
const SOLAR_SPACE_FILL_BRIGHTNESS: f32 = 360.0;

pub(crate) const DEMO_GLOBE_CAMERA_QUERY_MIN_DISTANCE_RADIUS_FACTOR: f32 = 1.02;
pub(crate) const DEMO_GLOBE_CAMERA_QUERY_MAX_DISTANCE_RADIUS_FACTOR: f32 = 20.0;
pub(crate) const DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG: f64 = 89.0;

pub(crate) const SOLAR_TRAIL_MIN_SAMPLES: usize = 32;
pub(crate) const SOLAR_TRAIL_MAX_SAMPLES: usize = 512;
pub(crate) const SOLAR_TRAIL_DEFAULT_MONTHS: u32 = 12;
pub(crate) const SOLAR_TRAIL_MIN_MONTHS: u32 = 1;
pub(crate) const SOLAR_TRAIL_MAX_MONTHS: u32 = 36;
pub(crate) const SOLAR_TRAIL_RESAMPLE_SECONDS: f64 = 86_400.0;

#[cfg(target_arch = "wasm32")]
pub(crate) const MAPBOX_TOKEN_STORAGE_KEY: &str = "ferrisium.mapbox_token";
#[cfg(target_arch = "wasm32")]
pub(crate) const EARTH_TILE_SOURCE_STORAGE_KEY: &str = "ferrisium.earth_tile_source";
#[cfg(target_arch = "wasm32")]
pub(crate) const GLOBE_FOCUS_STORAGE_KEY: &str = "ferrisium.globe_focus";
#[cfg(target_arch = "wasm32")]
pub(crate) const MAP_BODY_STORAGE_KEY: &str = "ferrisium.map_body";
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_TRAIL_MONTHS_STORAGE_KEY: &str = "ferrisium.solar_trail_months";
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_RESET_STORAGE_KEY: &str = "ferrisium.solar_camera_reset";
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_FOCUS_STORAGE_KEY: &str = "ferrisium.solar_focus";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DemoMode {
    Globe,
    SolarSystem,
    Map,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DemoSolarFocusTarget {
    Scene,
    Sun,
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DemoMapBody {
    Earth,
    NasaTrek(NasaTrekRegularBody),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DemoEarthTileSource {
    NasaBlueMarble,
    OpenStreetMap,
    MapboxSatellite,
    MapboxStreets,
    MapboxOutdoors,
    MapboxLight,
    MapboxDark,
    MapboxSatelliteStreets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DemoSolarTexturedBody {
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct DemoGlobeCameraOverride {
    pub(crate) surface_lon_lat: Option<LonLat>,
    pub(crate) yaw_deg: Option<f64>,
    pub(crate) pitch_deg: Option<f64>,
    pub(crate) distance_radius_factor: Option<f32>,
}

impl DemoMode {
    pub(crate) fn clear_color(self) -> Color {
        match self {
            Self::Globe => Color::srgb(0.02, 0.03, 0.06),
            Self::SolarSystem => Color::srgb(0.01, 0.015, 0.028),
            Self::Map => Color::srgb(0.05, 0.07, 0.1),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn tile_source(self) -> TileSource {
        match self {
            Self::Map => browser_map_tile_source(),
            Self::Globe | Self::SolarSystem => browser_earth_tile_source(self),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_mode() -> DemoMode {
    browser_query_param("view")
        .or_else(|| browser_query_param("mode"))
        .as_deref()
        .and_then(parse_demo_mode)
        .unwrap_or(DemoMode::Globe)
}

pub(crate) const fn solar_demo_epoch() -> Epoch {
    Epoch::J2000
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_focus() -> CelestialFocus {
    browser_demo_focus_token()
        .as_deref()
        .and_then(parse_demo_focus)
        .map(CelestialFocus::target)
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_metric_focus_selection() -> MetricSceneFocusSelection {
    MetricSceneFocusSelection::new(demo_focus_metric_target(browser_demo_focus().target))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_globe_camera_override() -> DemoGlobeCameraOverride {
    let globe_lon = browser_query_param("globe_lon")
        .as_deref()
        .and_then(parse_finite_f64);
    let globe_lat = browser_query_param("globe_lat")
        .as_deref()
        .and_then(parse_finite_f64);

    DemoGlobeCameraOverride {
        surface_lon_lat: globe_lon
            .zip(globe_lat)
            .map(|(lon_deg, lat_deg)| LonLat::new(lon_deg, lat_deg)),
        yaw_deg: browser_query_param("globe_yaw")
            .as_deref()
            .and_then(parse_finite_f64),
        pitch_deg: browser_query_param("globe_pitch")
            .as_deref()
            .and_then(parse_finite_f64),
        distance_radius_factor: browser_query_param("globe_distance_factor")
            .as_deref()
            .and_then(parse_positive_f32),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_focus_token() -> Option<String> {
    browser_query_param("focus").or_else(|| browser_local_storage_item(GLOBE_FOCUS_STORAGE_KEY))
}

#[cfg(target_arch = "wasm32")]
fn browser_earth_tile_source(mode: DemoMode) -> TileSource {
    let selected_source = browser_demo_earth_tile_source();
    let access_token = browser_mapbox_access_token();

    demo_earth_tile_source(mode, selected_source, access_token.as_deref())
}

#[cfg(target_arch = "wasm32")]
fn browser_map_tile_source() -> TileSource {
    demo_map_tile_source(
        browser_demo_map_body(),
        browser_demo_earth_tile_source(),
        browser_mapbox_access_token().as_deref(),
    )
}

pub(crate) fn demo_map_tile_source(
    body: DemoMapBody,
    selected_earth_source: Option<DemoEarthTileSource>,
    mapbox_access_token: Option<&str>,
) -> TileSource {
    match body {
        DemoMapBody::Earth => {
            demo_earth_tile_source(DemoMode::Map, selected_earth_source, mapbox_access_token)
        }
        DemoMapBody::NasaTrek(body) => TileSource::nasa_trek_regular_body(body),
    }
}

pub(crate) fn demo_earth_tile_source(
    mode: DemoMode,
    selected_source: Option<DemoEarthTileSource>,
    mapbox_access_token: Option<&str>,
) -> TileSource {
    let selected_source = selected_source
        .unwrap_or_else(|| default_demo_earth_tile_source(mode, mapbox_access_token.is_some()));

    match selected_source {
        DemoEarthTileSource::NasaBlueMarble => TileSource::nasa_gibs_blue_marble(),
        DemoEarthTileSource::OpenStreetMap => open_street_map_tile_source(),
        DemoEarthTileSource::MapboxSatellite => mapbox_access_token.map_or_else(
            || fallback_demo_earth_tile_source(mode),
            TileSource::mapbox_satellite,
        ),
        DemoEarthTileSource::MapboxStreets => {
            mapbox_style_tile_source(mode, mapbox_access_token, "streets-v12")
        }
        DemoEarthTileSource::MapboxOutdoors => {
            mapbox_style_tile_source(mode, mapbox_access_token, "outdoors-v12")
        }
        DemoEarthTileSource::MapboxLight => {
            mapbox_style_tile_source(mode, mapbox_access_token, "light-v11")
        }
        DemoEarthTileSource::MapboxDark => {
            mapbox_style_tile_source(mode, mapbox_access_token, "dark-v11")
        }
        DemoEarthTileSource::MapboxSatelliteStreets => {
            mapbox_style_tile_source(mode, mapbox_access_token, "satellite-streets-v12")
        }
    }
}

fn default_demo_earth_tile_source(
    mode: DemoMode,
    has_mapbox_access_token: bool,
) -> DemoEarthTileSource {
    match (mode, has_mapbox_access_token) {
        (DemoMode::Globe, true) => DemoEarthTileSource::MapboxSatellite,
        (DemoMode::Globe, false) | (DemoMode::SolarSystem, _) => {
            DemoEarthTileSource::NasaBlueMarble
        }
        (DemoMode::Map, _) => DemoEarthTileSource::OpenStreetMap,
    }
}

fn fallback_demo_earth_tile_source(mode: DemoMode) -> TileSource {
    match default_demo_earth_tile_source(mode, false) {
        DemoEarthTileSource::NasaBlueMarble => TileSource::nasa_gibs_blue_marble(),
        DemoEarthTileSource::OpenStreetMap => open_street_map_tile_source(),
        _ => unreachable!("fallback source must not require a Mapbox token"),
    }
}

fn mapbox_style_tile_source(
    mode: DemoMode,
    access_token: Option<&str>,
    style_id: &str,
) -> TileSource {
    access_token.map_or_else(
        || fallback_demo_earth_tile_source(mode),
        |access_token| TileSource::mapbox_style_raster("mapbox", style_id, access_token),
    )
}

fn open_street_map_tile_source() -> TileSource {
    TileSource::xyz("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
        .with_attribution("© OpenStreetMap contributors")
}

#[cfg(target_arch = "wasm32")]
fn browser_demo_earth_tile_source() -> Option<DemoEarthTileSource> {
    browser_query_param("tile_source")
        .or_else(|| browser_query_param("earth_tiles"))
        .or_else(|| browser_local_storage_item(EARTH_TILE_SOURCE_STORAGE_KEY))
        .as_deref()
        .and_then(parse_demo_earth_tile_source)
}

#[cfg(target_arch = "wasm32")]
fn browser_mapbox_access_token() -> Option<String> {
    let query_token = browser_query_param("mapbox_token").and_then(normalized_mapbox_public_token);
    let storage_token = browser_local_storage_item(MAPBOX_TOKEN_STORAGE_KEY)
        .and_then(normalized_mapbox_public_token);

    query_token.or(storage_token)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_demo_anise_disabled() -> bool {
    browser_query_param("no_anise")
        .as_deref()
        .is_some_and(parse_bool_flag)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_solar_trail_months() -> u32 {
    browser_query_param("trail_months")
        .or_else(|| browser_local_storage_item(SOLAR_TRAIL_MONTHS_STORAGE_KEY))
        .as_deref()
        .and_then(parse_solar_trail_months)
        .unwrap_or(SOLAR_TRAIL_DEFAULT_MONTHS)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_solar_focus_target() -> DemoSolarFocusTarget {
    browser_query_param("solar_focus")
        .or_else(|| browser_local_storage_item(SOLAR_FOCUS_STORAGE_KEY))
        .or_else(|| browser_query_param("focus"))
        .as_deref()
        .and_then(parse_solar_focus_target)
        .unwrap_or(DemoSolarFocusTarget::Scene)
}

#[cfg(target_arch = "wasm32")]
fn browser_demo_map_body() -> DemoMapBody {
    browser_query_param("map_body")
        .or_else(|| browser_query_param("body"))
        .or_else(|| browser_local_storage_item(MAP_BODY_STORAGE_KEY))
        .as_deref()
        .and_then(parse_demo_map_body)
        .unwrap_or(DemoMapBody::Earth)
}

pub(crate) fn parse_demo_mode(value: &str) -> Option<DemoMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "globe" | "3d" => Some(DemoMode::Globe),
        "solar" | "solar-system" | "orrery" => Some(DemoMode::SolarSystem),
        "map" | "2d" => Some(DemoMode::Map),
        _ => None,
    }
}

pub(crate) fn parse_demo_focus(value: &str) -> Option<BodyId> {
    match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "earth" | "terra" | "1" => Some(BodyId::EARTH),
        "mercury" | "mercury-system" | "mercury-barycenter" => Some(BodyId::MERCURY_BARYCENTER),
        "venus" | "venus-system" | "venus-barycenter" => Some(BodyId::VENUS_BARYCENTER),
        "moon" | "luna" | "2" => Some(BodyId::MOON),
        "mars" | "mars-system" | "mars-barycenter" | "4" | "3" => Some(BodyId::MARS_BARYCENTER),
        value => NasaTrekRegularBody::from_slug(value).map(demo_focus_body_id_for_trek_body),
    }
}

pub(crate) const fn demo_focus_body_id_for_trek_body(body: NasaTrekRegularBody) -> BodyId {
    match body {
        NasaTrekRegularBody::Mercury => BodyId::MERCURY_BARYCENTER,
        NasaTrekRegularBody::Venus => BodyId::VENUS_BARYCENTER,
        NasaTrekRegularBody::Mars => BodyId::MARS_BARYCENTER,
        _ => BodyId::naif(body.naif_id()),
    }
}

pub(crate) fn demo_focus_metric_target(target: BodyId) -> MetricSceneFocusTarget {
    MetricSceneFocusTarget::Object(target)
}

pub(crate) fn parse_demo_map_body(value: &str) -> Option<DemoMapBody> {
    match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "earth" | "terra" | "1" => Some(DemoMapBody::Earth),
        "2" => Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Moon)),
        "3" | "4" => Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Mars)),
        value => NasaTrekRegularBody::from_slug(value).map(DemoMapBody::NasaTrek),
    }
}

pub(crate) fn parse_solar_focus_target(value: &str) -> Option<DemoSolarFocusTarget> {
    match value.trim().to_ascii_lowercase().as_str() {
        "scene" | "center" | "system" | "solar-system" => Some(DemoSolarFocusTarget::Scene),
        "sun" | "sol" => Some(DemoSolarFocusTarget::Sun),
        "mercury" | "1" => Some(DemoSolarFocusTarget::Mercury),
        "venus" | "2" => Some(DemoSolarFocusTarget::Venus),
        "earth" | "terra" | "3" => Some(DemoSolarFocusTarget::Earth),
        "moon" | "luna" => Some(DemoSolarFocusTarget::Moon),
        "mars" | "mars-system" | "mars_barycenter" | "4" => Some(DemoSolarFocusTarget::Mars),
        _ => None,
    }
}

pub(crate) fn parse_demo_earth_tile_source(value: &str) -> Option<DemoEarthTileSource> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nasa" | "nasa-blue-marble" | "blue-marble" => Some(DemoEarthTileSource::NasaBlueMarble),
        "osm" | "openstreetmap" | "open-street-map" => Some(DemoEarthTileSource::OpenStreetMap),
        "mapbox-satellite" | "satellite" => Some(DemoEarthTileSource::MapboxSatellite),
        "mapbox-streets" | "streets" => Some(DemoEarthTileSource::MapboxStreets),
        "mapbox-outdoors" | "outdoors" => Some(DemoEarthTileSource::MapboxOutdoors),
        "mapbox-light" | "light" => Some(DemoEarthTileSource::MapboxLight),
        "mapbox-dark" | "dark" => Some(DemoEarthTileSource::MapboxDark),
        "mapbox-satellite-streets" | "satellite-streets" => {
            Some(DemoEarthTileSource::MapboxSatelliteStreets)
        }
        _ => None,
    }
}

pub(crate) fn parse_solar_trail_months(value: &str) -> Option<u32> {
    let months = value.trim().parse::<u32>().ok()?;
    Some(months.clamp(SOLAR_TRAIL_MIN_MONTHS, SOLAR_TRAIL_MAX_MONTHS))
}

pub(crate) fn demo_globe_skybox_config() -> GlobeSkybox {
    GlobeSkybox::milky_way(MilkyWaySkyboxResolution::Face4096)
}

pub(crate) fn demo_solar_skybox_config() -> GlobeSkybox {
    demo_globe_skybox_config().with_brightness(420.0)
}

pub(crate) const fn demo_globe_ambient_fill_light() -> GlobalAmbientLight {
    GlobalAmbientLight {
        color: DEMO_SPACE_FILL_LIGHT_COLOR,
        brightness: DEMO_GLOBE_SPACE_FILL_BRIGHTNESS,
        affects_lightmapped_meshes: true,
    }
}

pub(crate) const fn demo_solar_ambient_fill_light() -> GlobalAmbientLight {
    GlobalAmbientLight {
        color: DEMO_SPACE_FILL_LIGHT_COLOR,
        brightness: SOLAR_SPACE_FILL_BRIGHTNESS,
        affects_lightmapped_meshes: true,
    }
}

pub(crate) fn solar_focus_metric_target(target: DemoSolarFocusTarget) -> MetricSceneFocusTarget {
    solar_focus_body(target).map_or(
        MetricSceneFocusTarget::Scene,
        MetricSceneFocusTarget::Object,
    )
}

pub(crate) fn solar_focus_body(target: DemoSolarFocusTarget) -> Option<BodyId> {
    match target {
        DemoSolarFocusTarget::Scene => None,
        DemoSolarFocusTarget::Sun => Some(BodyId::SUN),
        DemoSolarFocusTarget::Mercury => Some(BodyId::MERCURY_BARYCENTER),
        DemoSolarFocusTarget::Venus => Some(BodyId::VENUS_BARYCENTER),
        DemoSolarFocusTarget::Earth => Some(BodyId::EARTH),
        DemoSolarFocusTarget::Moon => Some(BodyId::MOON),
        DemoSolarFocusTarget::Mars => Some(BodyId::MARS_BARYCENTER),
    }
}

pub(crate) fn solar_dynamic_tile_source(
    target: DemoSolarFocusTarget,
    active_earth_source: &TileSource,
) -> Option<TileSource> {
    match target {
        DemoSolarFocusTarget::Mercury => Some(TileSource::nasa_trek_regular_body(
            NasaTrekRegularBody::Mercury,
        )),
        DemoSolarFocusTarget::Venus => Some(TileSource::nasa_trek_regular_body(
            NasaTrekRegularBody::Venus,
        )),
        DemoSolarFocusTarget::Earth => Some(active_earth_source.clone()),
        DemoSolarFocusTarget::Moon => Some(TileSource::nasa_trek_regular_body(
            NasaTrekRegularBody::Moon,
        )),
        DemoSolarFocusTarget::Mars => Some(TileSource::nasa_trek_regular_body(
            NasaTrekRegularBody::Mars,
        )),
        DemoSolarFocusTarget::Scene | DemoSolarFocusTarget::Sun => None,
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn solar_dynamic_tile_source_for_focus(
    target: DemoSolarFocusTarget,
) -> Option<TileSource> {
    solar_dynamic_tile_source(target, &browser_earth_tile_source(DemoMode::SolarSystem))
}
