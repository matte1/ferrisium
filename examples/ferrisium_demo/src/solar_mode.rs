#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use crate::browser_params::browser_local_storage_item;
#[cfg(target_arch = "wasm32")]
use crate::demo_config::browser_solar_epoch;
#[cfg(target_arch = "wasm32")]
use crate::demo_config::{
    browser_solar_focus_target, browser_solar_trail_months, demo_solar_ambient_fill_light,
    demo_solar_skybox_config, solar_dynamic_tile_source_for_focus, solar_focus_metric_target,
    DemoMode, SOLAR_CAMERA_RESET_STORAGE_KEY,
};
#[cfg(any(target_arch = "wasm32", test))]
use crate::demo_config::{
    solar_focus_body, DemoSolarFocusTarget, DemoSolarTexturedBody, SOLAR_TRAIL_MAX_MONTHS,
    SOLAR_TRAIL_MAX_SAMPLES, SOLAR_TRAIL_MIN_MONTHS, SOLAR_TRAIL_MIN_SAMPLES,
    SOLAR_TRAIL_RESAMPLE_SECONDS,
};
#[cfg(target_arch = "wasm32")]
use crate::ephemeris_demo::{
    sync_demo_anise_kernel_bundle, DemoAniseKernelLoader, DemoEphemerisProvider,
};
#[cfg(target_arch = "wasm32")]
use crate::ephemeris_demo::{
    EARTH_ORBIT_PERIOD_SECONDS, MARS_ORBIT_PERIOD_SECONDS, MERCURY_ORBIT_PERIOD_SECONDS,
    MOON_ORBIT_PERIOD_SECONDS, VENUS_ORBIT_PERIOD_SECONDS,
};
#[cfg(any(target_arch = "wasm32", test))]
use bevy::asset::Asset;
#[cfg(target_arch = "wasm32")]
use bevy::asset::RenderAssetUsages;
#[cfg(target_arch = "wasm32")]
use bevy::gltf::GltfAssetLabel;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::math::{DVec3, Mat3};
#[cfg(target_arch = "wasm32")]
use bevy::mesh::Indices;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::pbr::Material;
#[cfg(target_arch = "wasm32")]
use bevy::pbr::MaterialPlugin;
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use bevy::prelude::{Alpha, StandardMaterial};
#[cfg(any(target_arch = "wasm32", test))]
use bevy::prelude::{AlphaMode, Color, Entity, Handle, Image, Quat, Resource, Vec3, Vec4};
#[cfg(any(target_arch = "wasm32", test))]
use bevy::reflect::TypePath;
#[cfg(target_arch = "wasm32")]
use bevy::render::render_resource::PrimitiveTopology;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::render::render_resource::{AsBindGroup, ShaderType};
#[cfg(any(target_arch = "wasm32", test))]
use bevy::shader::ShaderRef;
#[cfg(target_arch = "wasm32")]
use bevy::sprite::{Anchor, Text2dShadow};
#[cfg(target_arch = "wasm32")]
use bevy::window::PrimaryWindow;
#[cfg(target_arch = "wasm32")]
use big_space::prelude::{BigSpaceCommands, CellCoord, FloatingOrigin};
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::*;
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_bevy::prelude::{
    CelestialEpoch, MetricOrbitCameraState, MetricSceneFocusPivot, MetricSceneLodPolicy,
    MetricVisualRadiusPolicy, TiledBodySurface, TiledBodySurfaceRadiusPolicy,
};
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::{
    handle_metric_orbit_camera_input, sync_metric_orbit_camera, sync_metric_scene_focus_pivot,
    sync_metric_scene_focus_to_celestial_focus,
};
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_core::prelude::{NasaTrekRegularBody, TileSource};
#[cfg(target_arch = "wasm32")]
use ferrisium_core::space::{offset_epoch_seconds, sample_trajectory_between, EphemerisProvider};
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_core::space::{
    trajectory_samples_are_stale, BodyId, CelestialBody, Epoch, FrameId, RenderScale, StateVector,
    TrajectoryPath, TrajectoryTrailConfig, Vec3d, AVERAGE_GREGORIAN_MONTH_SECONDS,
};

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_MERCURY_TILE_ZOOM: u8 = 2;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_VENUS_TILE_ZOOM: u8 = 2;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_TILE_ZOOM: u8 = 3;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_MOON_TILE_ZOOM: u8 = 3;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_MARS_TILE_ZOOM: u8 = 2;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_RENDER_KILOMETERS_PER_UNIT: f64 = 1_000_000.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_GRID_CELL_SIZE_KM: f64 = 100_000_000.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_AU_KM: f64 = 149_597_870.7;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_CAMERA_DISTANCE_UNITS: f32 = 720.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_CAMERA_HEIGHT_UNITS: f32 = 290.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_CAMERA_MIN_DISTANCE_UNITS: f64 = 0.001;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_MAX_DISTANCE_UNITS: f64 = 1_800.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_ORBIT_DEGREES_PER_PIXEL: f64 = 0.16;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_MAX_ABS_PITCH_DEG: f64 = 82.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_PAN_UNITS_PER_PIXEL_AT_DISTANCE: f64 = 0.0015;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_LINE_SCROLL_ZOOM_STEP: f64 = 0.16;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_CAMERA_PIXEL_SCROLL_ZOOM_STEP: f64 = 0.0016;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_SUN_POINT_LIGHT_INTENSITY: f32 = 1_600_000_000.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_SUN_POINT_LIGHT_RANGE_UNITS: f32 = 1_200.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_BODY_LOCATOR_RADIUS_UNITS: f32 = 1.05;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_BODY_LOCATOR_WIDTH_UNITS: f32 = 0.035;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_ORBIT_GUIDE_WIDTH_UNITS: f32 = 0.18;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_ORBIT_TRAIL_WIDTH_UNITS: f32 = 0.50;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_BODY_FOCUS_DISTANCE_UNITS: f64 = 36.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_SHADER_PATH: &str = "shaders/solar_earth_atmosphere.wgsl";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_RADIUS_FACTOR: f32 = 1.010;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_RAYLEIGH_ALPHA: f32 = 0.34;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_TERMINATOR_ALPHA: f32 = 0.13;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_NIGHT_ALPHA: f32 = 0.012;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_FRESNEL_POWER: f32 = 4.8;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_DISK_ALPHA: f32 = 0.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_ATMOSPHERE_MAX_ALPHA: f32 = 0.34;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_SHADER_PATH: &str = "shaders/solar_earth_clouds.wgsl";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_TEXTURE_PATH: &str = "textures/earth_clouds_2048.jpg";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_RADIUS_FACTOR: f32 = 1.004;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_MAX_ALPHA: f32 = 0.28;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_MASK_THRESHOLD: f32 = 0.30;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_MASK_SOFTNESS: f32 = 0.36;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_CLOUDS_NIGHT_ALPHA_FACTOR: f32 = 0.04;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_SHADER_PATH: &str =
    "shaders/solar_earth_night_lights.wgsl";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_TEXTURE_PATH: &str =
    "textures/earth_night_lights_2048.jpg";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_RADIUS_FACTOR: f32 = 1.002;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_MAX_ALPHA: f32 = 0.95;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_MASK_THRESHOLD: f32 = 0.025;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_MASK_SOFTNESS: f32 = 0.28;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_EARTH_NIGHT_LIGHTS_TERMINATOR_FADE: f32 = 0.18;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_SURFACE_LOD_DISTANCE_RADIUS_FACTOR: f64 = 3.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR: f64 = 6.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_DYNAMIC_BODY_BASE_SURFACE_DISTANCE_RADIUS_FACTOR: f64 = 4.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_DYNAMIC_BODY_MAX_SELECTED_TILES: usize = 768;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_REFERENCE_GRID_WIDTH_UNITS: f32 = 0.08;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_REFERENCE_GRID_SPOKES: usize = 16;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_REFERENCE_GRID_SAMPLES: usize = 240;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_REFERENCE_GRID_RING_RADII_KM: [f64; 4] = [
    0.5 * SOLAR_AU_KM,
    SOLAR_AU_KM,
    1.5 * SOLAR_AU_KM,
    2.0 * SOLAR_AU_KM,
];
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_NASA_SUN_MODEL_ASSET_PATH: &str = "models/nasa_sun.glb";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const SOLAR_NASA_SUN_MODEL_SOURCE_RADIUS_UNITS: f32 = 500.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const SOLAR_ORBIT_SAMPLES: usize = 384;

#[cfg(target_arch = "wasm32")]
pub(crate) fn configure_solar_system_mode(app: &mut App) {
    let epoch = browser_solar_epoch();
    app.add_plugins(MaterialPlugin::<DemoSolarEarthAtmosphereMaterial>::default())
        .add_plugins(MaterialPlugin::<DemoSolarEarthCloudMaterial>::default())
        .add_plugins(MaterialPlugin::<DemoSolarEarthNightLightsMaterial>::default())
        .insert_resource(CelestialSpace::new(
            SOLAR_GRID_CELL_SIZE_KM,
            RenderScale::from_kilometers_per_unit(SOLAR_RENDER_KILOMETERS_PER_UNIT),
        ))
        .insert_resource(CelestialEpoch::new(epoch))
        .insert_resource(default_solar_camera_state())
        .insert_resource(solar_camera_settings())
        .insert_resource(DemoSolarTrailWindow::new(browser_solar_trail_months()))
        .insert_resource(DemoSolarFocusSelection::new(browser_solar_focus_target()))
        .insert_resource(DemoSolarFocusPivotTracker::default())
        .insert_resource(MetricSceneFocusSelection::new(solar_focus_metric_target(
            browser_solar_focus_target(),
        )))
        .insert_resource(DemoSolarUiState::default())
        .insert_resource(CelestialEphemeris::from_provider(
            DemoEphemerisProvider::default(),
        ))
        .insert_resource(demo_solar_skybox_config())
        .insert_resource(demo_solar_ambient_fill_light())
        .init_resource::<DemoAniseKernelLoader>()
        .init_resource::<DemoSolarOrbitPathEntities>()
        .add_systems(Startup, setup_solar_system)
        .add_systems(
            Update,
            (
                sync_solar_ui_controls
                    .before(sync_metric_scene_focus_to_celestial_focus)
                    .before(sync_metric_scene_focus_pivot),
                sync_solar_active_tile_source.after(sync_solar_ui_controls),
                sync_demo_anise_kernel_bundle,
                sync_solar_camera_focus
                    .after(sync_metric_scene_focus_pivot)
                    .after(sync_demo_anise_kernel_bundle)
                    .after(sync_solar_ui_controls),
                sync_solar_camera_constraints
                    .after(sync_metric_scene_focus_pivot)
                    .after(sync_solar_ui_controls)
                    .before(handle_metric_orbit_camera_input),
                handle_metric_orbit_camera_input.after(sync_solar_camera_focus),
                sync_metric_orbit_camera
                    .after(handle_metric_orbit_camera_input)
                    .after(sync_solar_ui_controls),
                sync_solar_dynamic_body_surface_view
                    .after(sync_metric_orbit_camera)
                    .after(sync_metric_scene_focus_pivot)
                    .after(sync_solar_ui_controls),
                sync_demo_solar_orbit_paths
                    .after(sync_demo_anise_kernel_bundle)
                    .after(sync_solar_ui_controls),
                sync_solar_overview_aids_visibility
                    .after(sync_solar_ui_controls)
                    .after(sync_demo_solar_orbit_paths),
                sync_solar_sun_model_material,
                sync_solar_earth_night_lights_material,
                sync_solar_earth_cloud_material,
                sync_solar_earth_atmosphere_material,
                sync_solar_labels.after(sync_metric_orbit_camera),
            ),
        );
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_true_radius_policy() -> MetricVisualRadiusPolicy {
    MetricVisualRadiusPolicy::minimum(0.0)
}

#[cfg(test)]
pub(crate) fn solar_sun_light_illuminance_at_distance_units(distance_units: f64) -> f64 {
    f64::from(SOLAR_SUN_POINT_LIGHT_INTENSITY) / distance_units.max(1.0).powi(2)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_tiled_body_surface(
    body: DemoSolarTexturedBody,
    active_earth_source: &TileSource,
) -> (TiledBodySurface, TiledBodySurfaceRadiusPolicy) {
    let (source, zoom) = match body {
        DemoSolarTexturedBody::Mercury => (
            TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Mercury),
            SOLAR_MERCURY_TILE_ZOOM,
        ),
        DemoSolarTexturedBody::Venus => (
            TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Venus),
            SOLAR_VENUS_TILE_ZOOM,
        ),
        DemoSolarTexturedBody::Earth => (active_earth_source.clone(), SOLAR_EARTH_TILE_ZOOM),
        DemoSolarTexturedBody::Moon => (
            TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Moon),
            SOLAR_MOON_TILE_ZOOM,
        ),
        DemoSolarTexturedBody::Mars => (
            TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Mars),
            SOLAR_MARS_TILE_ZOOM,
        ),
    };

    (
        TiledBodySurface::fixed_zoom(source, zoom),
        TiledBodySurfaceRadiusPolicy(solar_true_radius_policy()),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_tiled_body_fallback_color(body: DemoSolarTexturedBody) -> Color {
    match body {
        DemoSolarTexturedBody::Mercury => Color::srgb(0.54, 0.51, 0.47),
        DemoSolarTexturedBody::Venus => Color::srgb(0.64, 0.52, 0.38),
        DemoSolarTexturedBody::Earth => Color::srgb(0.18, 0.42, 0.70),
        DemoSolarTexturedBody::Moon => Color::srgb(0.52, 0.52, 0.49),
        DemoSolarTexturedBody::Mars => Color::srgb(0.65, 0.38, 0.24),
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarSun;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarEarth;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarEarthAtmosphereLayer;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarEarthCloudLayer;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarEarthNightLightsLayer;

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub(crate) struct DemoSolarEarthAtmosphereMaterial {
    #[uniform(0)]
    pub(crate) params: DemoSolarEarthAtmosphereUniform,
}

#[cfg(any(target_arch = "wasm32", test))]
impl Material for DemoSolarEarthAtmosphereMaterial {
    fn fragment_shader() -> ShaderRef {
        SOLAR_EARTH_ATMOSPHERE_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, ShaderType)]
pub(crate) struct DemoSolarEarthAtmosphereUniform {
    pub(crate) sun_direction_strength: Vec4,
    pub(crate) rayleigh_color_alpha: Vec4,
    pub(crate) terminator_color_alpha: Vec4,
    pub(crate) night_color_alpha: Vec4,
    pub(crate) falloff: Vec4,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub(crate) struct DemoSolarEarthCloudMaterial {
    #[uniform(0)]
    pub(crate) params: DemoSolarEarthCloudUniform,
    #[texture(1)]
    #[sampler(2)]
    pub(crate) cloud_texture: Handle<Image>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl Material for DemoSolarEarthCloudMaterial {
    fn fragment_shader() -> ShaderRef {
        SOLAR_EARTH_CLOUDS_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, ShaderType)]
pub(crate) struct DemoSolarEarthCloudUniform {
    pub(crate) sun_direction_strength: Vec4,
    pub(crate) cloud_color_alpha: Vec4,
    pub(crate) mask_params: Vec4,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub(crate) struct DemoSolarEarthNightLightsMaterial {
    #[uniform(0)]
    pub(crate) params: DemoSolarEarthNightLightsUniform,
    #[texture(1)]
    #[sampler(2)]
    pub(crate) light_texture: Handle<Image>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl Material for DemoSolarEarthNightLightsMaterial {
    fn fragment_shader() -> ShaderRef {
        SOLAR_EARTH_NIGHT_LIGHTS_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, ShaderType)]
pub(crate) struct DemoSolarEarthNightLightsUniform {
    pub(crate) sun_direction_strength: Vec4,
    pub(crate) light_color_alpha: Vec4,
    pub(crate) mask_params: Vec4,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct DemoSolarSunModelMaterial {
    handle: Handle<StandardMaterial>,
    texture: Handle<Image>,
    applied: bool,
}

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarMercury;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarVenus;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarEarthOrbitAnchor;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarMoon;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarMars;

#[cfg(target_arch = "wasm32")]
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct DemoSolarOverviewAid;

#[cfg(target_arch = "wasm32")]
pub(crate) fn default_solar_camera_state() -> MetricOrbitCameraState {
    MetricOrbitCameraState::from_horizontal_distance_and_height(
        f64::from(SOLAR_CAMERA_DISTANCE_UNITS),
        f64::from(SOLAR_CAMERA_HEIGHT_UNITS),
        -34.0,
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn solar_camera_settings() -> MetricOrbitCameraSettings {
    MetricOrbitCameraSettings {
        min_distance_units: SOLAR_CAMERA_MIN_DISTANCE_UNITS,
        max_distance_units: SOLAR_CAMERA_MAX_DISTANCE_UNITS,
        orbit_degrees_per_pixel: SOLAR_CAMERA_ORBIT_DEGREES_PER_PIXEL,
        max_abs_pitch_deg: SOLAR_CAMERA_MAX_ABS_PITCH_DEG,
        pan_units_per_pixel_at_distance: SOLAR_CAMERA_PAN_UNITS_PER_PIXEL_AT_DISTANCE,
        line_scroll_zoom_step: SOLAR_CAMERA_LINE_SCROLL_ZOOM_STEP,
        pixel_scroll_zoom_step: SOLAR_CAMERA_PIXEL_SCROLL_ZOOM_STEP,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DemoSolarTrailWindow {
    pub(crate) months: u32,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DemoSolarTrailWindow {
    pub(crate) const fn new(months: u32) -> Self {
        Self { months }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct DemoSolarFocusSelection {
    target: DemoSolarFocusTarget,
    applied: Option<DemoSolarFocusTarget>,
}

#[cfg(target_arch = "wasm32")]
impl DemoSolarFocusSelection {
    const fn new(target: DemoSolarFocusTarget) -> Self {
        Self {
            target,
            applied: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
struct DemoSolarFocusPivotTracker {
    target: Option<DemoSolarFocusTarget>,
    pivot_units: Option<DVec3>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Debug, Default, Clone)]
struct DemoSolarUiState {
    reset_token: Option<String>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSolarLabel {
    target: Entity,
    offset_px: Vec2,
}

#[cfg(target_arch = "wasm32")]
struct DemoSolarEarthPresentationAssets<'a> {
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    atmosphere_materials: &'a mut Assets<DemoSolarEarthAtmosphereMaterial>,
    cloud_materials: &'a mut Assets<DemoSolarEarthCloudMaterial>,
    night_light_materials: &'a mut Assets<DemoSolarEarthNightLightsMaterial>,
    cloud_texture: Handle<Image>,
    night_light_texture: Handle<Image>,
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "The demo solar scene intentionally lists its fixed visual bodies in one setup system."
)]
pub(crate) fn setup_solar_system(
    mut commands: Commands<'_, '_>,
    active_source: Res<'_, ActiveTileSource>,
    celestial_space: Res<'_, CelestialSpace>,
    camera_state: Res<'_, MetricOrbitCameraState>,
    asset_server: Res<'_, AssetServer>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
    mut atmosphere_materials: ResMut<'_, Assets<DemoSolarEarthAtmosphereMaterial>>,
    mut cloud_materials: ResMut<'_, Assets<DemoSolarEarthCloudMaterial>>,
    mut night_light_materials: ResMut<'_, Assets<DemoSolarEarthNightLightsMaterial>>,
) {
    let camera_position = camera_state.position_units();
    let camera_placement = celestial_space.place_position_units(camera_position);
    let camera_look_direction = (camera_state.pivot_units - camera_position)
        .normalize_or_zero()
        .as_vec3();
    let camera_transform = Transform::from_translation(camera_placement.translation)
        .looking_to(camera_look_direction, Vec3::Y);
    let reference_grid_mesh = meshes.add(solar_reference_grid_mesh(*celestial_space));
    let reference_grid_material = materials.add(solar_reference_grid_material());
    let sun_model_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(SOLAR_NASA_SUN_MODEL_ASSET_PATH));
    let sun_model_material: Handle<StandardMaterial> = asset_server.load(
        GltfAssetLabel::Material {
            index: 0,
            is_scale_inverted: false,
        }
        .from_asset(SOLAR_NASA_SUN_MODEL_ASSET_PATH),
    );
    let sun_model_texture: Handle<Image> =
        asset_server.load(GltfAssetLabel::Texture(0).from_asset(SOLAR_NASA_SUN_MODEL_ASSET_PATH));
    let earth_cloud_texture: Handle<Image> = asset_server.load(SOLAR_EARTH_CLOUDS_TEXTURE_PATH);
    let earth_night_light_texture: Handle<Image> =
        asset_server.load(SOLAR_EARTH_NIGHT_LIGHTS_TEXTURE_PATH);
    commands.insert_resource(DemoSolarSunModelMaterial {
        handle: sun_model_material,
        texture: sun_model_texture,
        applied: false,
    });
    let mut labeled_bodies = Vec::new();
    let mut sun_entity = None;
    let mut earth_entity = None;
    let mut tiled_body_presentations = Vec::new();

    commands.spawn_big_space(celestial_space.grid(), |root| {
        root.insert((Name::new("Ferrisium Solar System Root"), CelestialRoot));

        let sun_body = CelestialBody::sun();
        let sun = root
            .spawn_spatial((
                Name::new("Solar System Sun"),
                DemoSolarSun,
                MetricSceneBodyAnchorBundle::relative_to(
                    sun_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Sun),
                ),
                solar_body_lod_policy(&sun_body),
            ))
            .id();
        sun_entity = Some(sun);
        labeled_bodies.push((
            sun,
            "SUN",
            Color::srgb(1.0, 0.78, 0.34),
            Vec2::new(10.0, -44.0),
        ));

        let mercury_body = demo_mercury_body();
        let mercury = root
            .spawn_spatial((
                Name::new("Solar System Mercury"),
                DemoSolarMercury,
                MetricSceneBodyAnchorBundle::relative_to_without_orientation(
                    mercury_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Mercury),
                ),
                solar_body_lod_policy(&mercury_body),
            ))
            .id();
        tiled_body_presentations.push((
            mercury,
            DemoSolarTexturedBody::Mercury,
            mercury_body.clone(),
        ));
        labeled_bodies.push((
            mercury,
            "MERCURY",
            Color::srgb(0.72, 0.78, 0.86),
            Vec2::new(16.0, 20.0),
        ));

        let venus_body = demo_venus_body();
        let venus = root
            .spawn_spatial((
                Name::new("Solar System Venus"),
                DemoSolarVenus,
                MetricSceneBodyAnchorBundle::relative_to_without_orientation(
                    venus_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Venus),
                ),
                solar_body_lod_policy(&venus_body),
            ))
            .id();
        tiled_body_presentations.push((venus, DemoSolarTexturedBody::Venus, venus_body.clone()));
        labeled_bodies.push((
            venus,
            "VENUS",
            Color::srgb(1.0, 0.78, 0.42),
            Vec2::new(14.0, -30.0),
        ));

        let earth_body = CelestialBody::earth();
        let earth = root
            .spawn_spatial((
                Name::new("Solar System Earth"),
                DemoSolarEarth,
                MetricSceneBodyAnchorBundle::relative_to(
                    earth_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Earth),
                ),
                solar_body_lod_policy(&earth_body),
            ))
            .id();
        earth_entity = Some(earth);
        labeled_bodies.push((
            earth,
            "EARTH",
            Color::srgb(0.44, 0.72, 1.0),
            Vec2::new(16.0, -18.0),
        ));
        root.spawn_spatial((
            Name::new("Solar System Earth Inertial Orbit Anchor"),
            DemoSolarEarthOrbitAnchor,
            MetricObjectPlacement::relative_to(BodyId::EARTH, BodyId::SUN, FrameId::j2000()),
            CellCoord::default(),
            Transform::default(),
        ));

        let moon_body = CelestialBody::moon();
        let moon = root
            .spawn_spatial((
                Name::new("Solar System Moon"),
                DemoSolarMoon,
                MetricSceneBodyAnchorBundle::relative_to(
                    moon_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Moon),
                ),
                solar_body_lod_policy(&moon_body),
            ))
            .id();
        tiled_body_presentations.push((moon, DemoSolarTexturedBody::Moon, moon_body.clone()));
        labeled_bodies.push((
            moon,
            "MOON",
            Color::srgb(0.82, 0.82, 0.78),
            Vec2::new(16.0, 16.0),
        ));

        let mars_body = demo_mars_body();
        let mars = root
            .spawn_spatial((
                Name::new("Solar System Mars"),
                DemoSolarMars,
                MetricSceneBodyAnchorBundle::relative_to(
                    mars_body.clone(),
                    BodyId::SUN,
                    solar_focus_distance_units(DemoSolarFocusTarget::Mars),
                ),
                solar_body_lod_policy(&mars_body),
            ))
            .id();
        tiled_body_presentations.push((mars, DemoSolarTexturedBody::Mars, mars_body.clone()));
        labeled_bodies.push((
            mars,
            "MARS",
            Color::srgb(1.0, 0.43, 0.28),
            Vec2::new(14.0, -16.0),
        ));

        root.spawn_spatial((
            Name::new("Solar System Camera"),
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(DemoMode::SolarSystem.clear_color()),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection::default()),
            camera_placement.cell,
            camera_transform,
            MetricOrbitCamera,
            MetricSceneCamera::primary(),
            FloatingOrigin,
        ));
        root.spawn_spatial((
            Name::new("Solar System Dynamic Focus Surface"),
            DynamicBodySurfaceView::default(),
        ));
    });

    if let Some(sun) = sun_entity {
        let sun_body_layer = spawn_solar_sun_model_layer(
            &mut commands,
            sun,
            sun_model_scene,
            solar_body_radius_units(&CelestialBody::sun()),
        );
        commands.entity(sun_body_layer).insert(DemoSolarSun);
        spawn_solar_reference_grid(
            &mut commands,
            sun,
            reference_grid_mesh,
            reference_grid_material,
        );
        spawn_solar_sun_light(&mut commands, sun);
    }
    if let Some(earth) = earth_entity {
        let mut earth_assets = DemoSolarEarthPresentationAssets {
            meshes: &mut meshes,
            materials: &mut materials,
            atmosphere_materials: &mut atmosphere_materials,
            cloud_materials: &mut cloud_materials,
            night_light_materials: &mut night_light_materials,
            cloud_texture: earth_cloud_texture,
            night_light_texture: earth_night_light_texture,
        };
        spawn_solar_earth_presentation_layers(
            &mut commands,
            earth,
            &active_source.0,
            &mut earth_assets,
        );
    }
    for (parent, body, celestial_body) in tiled_body_presentations {
        spawn_solar_tiled_body_presentation_layers(
            &mut commands,
            parent,
            body,
            celestial_body,
            &active_source.0,
            &mut meshes,
            &mut materials,
        );
    }
    commands.spawn((
        Name::new("Solar System Label Camera"),
        Camera2d,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));

    for (body, label, color, offset_px) in labeled_bodies {
        spawn_solar_label(&mut commands, body, label, color, offset_px);
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_sun_model_layer(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    scene: Handle<Scene>,
    radius_units: f32,
) -> Entity {
    let mut layer = MetricScenePresentationLayerBundle::for_object_set(
        BodyId::SUN,
        MetricScenePresentationSet::all(),
    );
    layer.transform =
        Transform::from_scale(Vec3::splat(solar_nasa_sun_model_scale_units(radius_units)));
    let entity = commands
        .spawn((
            Name::new("Solar System NASA Sun Model Layer"),
            layer,
            SceneRoot(scene),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_locator_layer(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    body: BodyId,
    name: &'static str,
    color: Color,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let entity = commands
        .spawn((
            Name::new(name),
            MetricScenePresentationLayerBundle::for_object(body, MetricScenePresentation::Marker),
            Mesh3d(meshes.add(solar_locator_mesh())),
            MeshMaterial3d(materials.add(solar_locator_material(color))),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_demo_tiled_body_surface_layer(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    name: &'static str,
    celestial_body: CelestialBody,
    presentations: MetricScenePresentationSet,
    surface: TiledBodySurface,
    radius_policy: Option<TiledBodySurfaceRadiusPolicy>,
) -> Entity {
    let entity = commands
        .spawn((
            Name::new(name),
            MetricSceneBodySurfaceLayerBundle::for_body_set(celestial_body, presentations),
            surface,
        ))
        .id();
    if let Some(radius_policy) = radius_policy {
        commands.entity(entity).insert(radius_policy);
    }
    commands.entity(parent).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_tiled_body_presentation_layers(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    body: DemoSolarTexturedBody,
    celestial_body: CelestialBody,
    active_source: &TileSource,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let locator_color = match body {
        DemoSolarTexturedBody::Mercury => Color::srgb(0.72, 0.78, 0.86),
        DemoSolarTexturedBody::Venus => Color::srgb(1.0, 0.78, 0.42),
        DemoSolarTexturedBody::Earth => Color::srgb(0.44, 0.72, 1.0),
        DemoSolarTexturedBody::Moon => Color::srgb(0.82, 0.82, 0.78),
        DemoSolarTexturedBody::Mars => Color::srgb(1.0, 0.43, 0.28),
    };
    spawn_solar_locator_layer(
        commands,
        parent,
        celestial_body.id,
        "Solar System Body Locator Layer",
        locator_color,
        meshes,
        materials,
    );

    spawn_solar_tiled_body_fallback_layer(
        commands,
        parent,
        celestial_body.id,
        "Solar System Tiled Body Fallback Layer",
        solar_body_radius_units(&celestial_body),
        solar_tiled_body_fallback_color(body),
        meshes,
        materials,
    );

    let (surface, radius_policy) = solar_tiled_body_surface(body, active_source);
    spawn_demo_tiled_body_surface_layer(
        commands,
        parent,
        "Solar System Tiled Body Surface Layer",
        celestial_body,
        MetricScenePresentationSet::only(MetricScenePresentation::Body),
        surface,
        Some(radius_policy),
    );
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_earth_presentation_layers(
    commands: &mut Commands<'_, '_>,
    earth: Entity,
    active_source: &TileSource,
    assets: &mut DemoSolarEarthPresentationAssets<'_>,
) {
    spawn_solar_locator_layer(
        commands,
        earth,
        BodyId::EARTH,
        "Solar System Earth Locator Layer",
        Color::srgb(0.44, 0.72, 1.0),
        &mut *assets.meshes,
        &mut *assets.materials,
    );

    spawn_solar_tiled_body_fallback_layer(
        commands,
        earth,
        BodyId::EARTH,
        "Solar System Earth Fallback Layer",
        solar_body_radius_units(&CelestialBody::earth()),
        solar_tiled_body_fallback_color(DemoSolarTexturedBody::Earth),
        &mut *assets.meshes,
        &mut *assets.materials,
    );

    let (earth_surface, earth_radius_policy) =
        solar_tiled_body_surface(DemoSolarTexturedBody::Earth, active_source);
    spawn_demo_tiled_body_surface_layer(
        commands,
        earth,
        "Solar System Earth Surface Layer",
        CelestialBody::earth(),
        MetricScenePresentationSet::only(MetricScenePresentation::Body),
        earth_surface,
        Some(earth_radius_policy),
    );

    spawn_solar_earth_night_lights_layer(
        commands,
        earth,
        &mut *assets.meshes,
        &mut *assets.night_light_materials,
        assets.night_light_texture.clone(),
    );
    spawn_solar_earth_cloud_layer(
        commands,
        earth,
        &mut *assets.meshes,
        &mut *assets.cloud_materials,
        assets.cloud_texture.clone(),
    );
    spawn_solar_earth_atmosphere_layer(
        commands,
        earth,
        &mut *assets.meshes,
        &mut *assets.atmosphere_materials,
    );
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_earth_cloud_layer(
    commands: &mut Commands<'_, '_>,
    earth: Entity,
    meshes: &mut Assets<Mesh>,
    cloud_materials: &mut Assets<DemoSolarEarthCloudMaterial>,
    cloud_texture: Handle<Image>,
) -> Entity {
    let mut layer = MetricScenePresentationLayerBundle::for_object(
        BodyId::EARTH,
        MetricScenePresentation::Body,
    );
    let earth_radius_units = solar_body_radius_units(&CelestialBody::earth());
    layer.transform =
        earth_equirectangular_overlay_transform(solar_earth_cloud_radius_units(earth_radius_units));

    let entity = commands
        .spawn((
            Name::new("Solar System Earth Cloud Layer"),
            DemoSolarEarthCloudLayer,
            layer,
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(96, 48))),
            MeshMaterial3d(cloud_materials.add(solar_earth_cloud_material(cloud_texture))),
        ))
        .id();
    commands.entity(earth).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_earth_night_lights_layer(
    commands: &mut Commands<'_, '_>,
    earth: Entity,
    meshes: &mut Assets<Mesh>,
    night_light_materials: &mut Assets<DemoSolarEarthNightLightsMaterial>,
    night_light_texture: Handle<Image>,
) -> Entity {
    let mut layer = MetricScenePresentationLayerBundle::for_object(
        BodyId::EARTH,
        MetricScenePresentation::Body,
    );
    let earth_radius_units = solar_body_radius_units(&CelestialBody::earth());
    layer.transform = earth_equirectangular_overlay_transform(
        solar_earth_night_lights_radius_units(earth_radius_units),
    );

    let entity = commands
        .spawn((
            Name::new("Solar System Earth Night Lights Layer"),
            DemoSolarEarthNightLightsLayer,
            layer,
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(96, 48))),
            MeshMaterial3d(
                night_light_materials.add(solar_earth_night_lights_material(night_light_texture)),
            ),
        ))
        .id();
    commands.entity(earth).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_earth_atmosphere_layer(
    commands: &mut Commands<'_, '_>,
    earth: Entity,
    meshes: &mut Assets<Mesh>,
    atmosphere_materials: &mut Assets<DemoSolarEarthAtmosphereMaterial>,
) -> Entity {
    let mut layer = MetricScenePresentationLayerBundle::for_object(
        BodyId::EARTH,
        MetricScenePresentation::Body,
    );
    let earth_radius_units = solar_body_radius_units(&CelestialBody::earth());
    layer.transform = Transform::from_scale(Vec3::splat(solar_earth_atmosphere_radius_units(
        earth_radius_units,
    )));

    let entity = commands
        .spawn((
            Name::new("Solar System Earth Atmosphere Layer"),
            DemoSolarEarthAtmosphereLayer,
            layer,
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(64, 32))),
            MeshMaterial3d(atmosphere_materials.add(solar_earth_atmosphere_material())),
        ))
        .id();
    commands.entity(earth).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_arguments,
    reason = "Fallback body layers need explicit ECS asset stores and presentation inputs."
)]
fn spawn_solar_tiled_body_fallback_layer(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    body: BodyId,
    name: &'static str,
    radius_units: f32,
    color: Color,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let mut layer =
        MetricScenePresentationLayerBundle::for_object(body, MetricScenePresentation::Body);
    layer.transform = Transform::from_scale(Vec3::splat(radius_units * 0.985));
    let entity = commands
        .spawn((
            Name::new(name),
            layer,
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(48, 24))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                emissive: color.with_alpha(0.14).into(),
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                cull_mode: None,
                ..default()
            })),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_atmosphere_radius_units(earth_radius_units: f32) -> f32 {
    earth_radius_units * SOLAR_EARTH_ATMOSPHERE_RADIUS_FACTOR
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_cloud_radius_units(earth_radius_units: f32) -> f32 {
    earth_radius_units * SOLAR_EARTH_CLOUDS_RADIUS_FACTOR
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_night_lights_radius_units(earth_radius_units: f32) -> f32 {
    earth_radius_units * SOLAR_EARTH_NIGHT_LIGHTS_RADIUS_FACTOR
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn earth_equirectangular_overlay_rotation() -> Quat {
    Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y))
}

#[cfg(target_arch = "wasm32")]
fn earth_equirectangular_overlay_transform(radius_units: f32) -> Transform {
    Transform {
        rotation: earth_equirectangular_overlay_rotation(),
        scale: Vec3::splat(radius_units),
        ..default()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_atmosphere_material() -> DemoSolarEarthAtmosphereMaterial {
    DemoSolarEarthAtmosphereMaterial {
        params: DemoSolarEarthAtmosphereUniform {
            sun_direction_strength: solar_earth_atmosphere_sun_direction_param(Vec3::X, 1.0),
            rayleigh_color_alpha: Vec4::new(0.30, 0.62, 1.0, SOLAR_EARTH_ATMOSPHERE_RAYLEIGH_ALPHA),
            terminator_color_alpha: Vec4::new(
                1.0,
                0.50,
                0.18,
                SOLAR_EARTH_ATMOSPHERE_TERMINATOR_ALPHA,
            ),
            night_color_alpha: Vec4::new(0.08, 0.25, 0.70, SOLAR_EARTH_ATMOSPHERE_NIGHT_ALPHA),
            falloff: Vec4::new(
                SOLAR_EARTH_ATMOSPHERE_FRESNEL_POWER,
                SOLAR_EARTH_ATMOSPHERE_DISK_ALPHA,
                SOLAR_EARTH_ATMOSPHERE_MAX_ALPHA,
                0.0,
            ),
        },
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_cloud_material(
    cloud_texture: Handle<Image>,
) -> DemoSolarEarthCloudMaterial {
    DemoSolarEarthCloudMaterial {
        params: DemoSolarEarthCloudUniform {
            sun_direction_strength: solar_earth_atmosphere_sun_direction_param(Vec3::X, 1.0),
            cloud_color_alpha: Vec4::new(0.92, 0.96, 1.0, SOLAR_EARTH_CLOUDS_MAX_ALPHA),
            mask_params: Vec4::new(
                SOLAR_EARTH_CLOUDS_MASK_THRESHOLD,
                SOLAR_EARTH_CLOUDS_MASK_SOFTNESS,
                SOLAR_EARTH_CLOUDS_NIGHT_ALPHA_FACTOR,
                0.0,
            ),
        },
        cloud_texture,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_night_lights_material(
    light_texture: Handle<Image>,
) -> DemoSolarEarthNightLightsMaterial {
    DemoSolarEarthNightLightsMaterial {
        params: DemoSolarEarthNightLightsUniform {
            sun_direction_strength: solar_earth_atmosphere_sun_direction_param(Vec3::X, 1.0),
            light_color_alpha: Vec4::new(1.0, 0.68, 0.34, SOLAR_EARTH_NIGHT_LIGHTS_MAX_ALPHA),
            mask_params: Vec4::new(
                SOLAR_EARTH_NIGHT_LIGHTS_MASK_THRESHOLD,
                SOLAR_EARTH_NIGHT_LIGHTS_MASK_SOFTNESS,
                SOLAR_EARTH_NIGHT_LIGHTS_TERMINATOR_FADE,
                0.0,
            ),
        },
        light_texture,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_earth_atmosphere_sun_direction_param(
    sun_direction: Vec3,
    strength: f32,
) -> Vec4 {
    let normalized = if sun_direction.length_squared() > f32::EPSILON {
        sun_direction.normalize()
    } else {
        Vec3::X
    };
    normalized.extend(strength)
}

#[cfg(target_arch = "wasm32")]
fn solar_locator_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color.with_alpha(0.52),
        emissive: color.with_alpha(0.92).into(),
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        unlit: true,
        ..default()
    }
}

#[cfg(target_arch = "wasm32")]
fn solar_locator_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    append_solar_locator_ring(
        SolarLocatorPlane::Xy,
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
    );
    append_solar_locator_ring(
        SolarLocatorPlane::Xz,
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
    );
    append_solar_locator_ring(
        SolarLocatorPlane::Yz,
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
enum SolarLocatorPlane {
    Xy,
    Xz,
    Yz,
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Locator rings are tiny fixed presentation meshes."
)]
fn append_solar_locator_ring(
    plane: SolarLocatorPlane,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    const SEGMENTS: usize = 72;
    let half_width = SOLAR_BODY_LOCATOR_WIDTH_UNITS * 0.5;
    let inner_radius = (SOLAR_BODY_LOCATOR_RADIUS_UNITS - half_width).max(0.01);
    let outer_radius = SOLAR_BODY_LOCATOR_RADIUS_UNITS + half_width;

    for index in 0..SEGMENTS {
        let angle0 = std::f32::consts::TAU * index as f32 / SEGMENTS as f32;
        let angle1 = std::f32::consts::TAU * (index + 1) as f32 / SEGMENTS as f32;
        let Ok(base_index) = u32::try_from(positions.len()) else {
            return;
        };

        positions.extend_from_slice(&[
            solar_locator_point(plane, outer_radius, angle0).to_array(),
            solar_locator_point(plane, inner_radius, angle0).to_array(),
            solar_locator_point(plane, outer_radius, angle1).to_array(),
            solar_locator_point(plane, inner_radius, angle1).to_array(),
        ]);
        normals.extend_from_slice(&[solar_locator_normal(plane).to_array(); 4]);
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[
            base_index,
            base_index + 2,
            base_index + 1,
            base_index + 1,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

#[cfg(target_arch = "wasm32")]
fn solar_locator_point(plane: SolarLocatorPlane, radius: f32, angle_rad: f32) -> Vec3 {
    let (sin, cos) = angle_rad.sin_cos();
    match plane {
        SolarLocatorPlane::Xy => Vec3::new(cos * radius, sin * radius, 0.0),
        SolarLocatorPlane::Xz => Vec3::new(cos * radius, 0.0, sin * radius),
        SolarLocatorPlane::Yz => Vec3::new(0.0, cos * radius, sin * radius),
    }
}

#[cfg(target_arch = "wasm32")]
fn solar_locator_normal(plane: SolarLocatorPlane) -> Vec3 {
    match plane {
        SolarLocatorPlane::Xy => Vec3::Z,
        SolarLocatorPlane::Xz => Vec3::Y,
        SolarLocatorPlane::Yz => Vec3::X,
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_reference_grid(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) {
    let grid = commands
        .spawn((
            Name::new("Solar System Metric Reference Grid"),
            DemoSolarOverviewAid,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Inherited,
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ))
        .id();
    commands.entity(parent).add_child(grid);
}

#[cfg(target_arch = "wasm32")]
fn solar_reference_grid_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.36, 0.58, 0.82, 0.12),
        emissive: Color::srgba(0.36, 0.58, 0.82, 0.12).into(),
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        unlit: true,
        ..default()
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::cast_precision_loss,
    reason = "Reference-grid spoke counts are tiny fixed presentation constants."
)]
fn solar_reference_grid_mesh(celestial_space: CelestialSpace) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for radius_km in SOLAR_REFERENCE_GRID_RING_RADII_KM {
        let path = solar_reference_ring_path(radius_km, SOLAR_REFERENCE_GRID_SAMPLES, Epoch::J2000);
        append_solar_reference_path_mesh(
            celestial_space,
            &path,
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
        );
    }

    let grid_radius_km = SOLAR_REFERENCE_GRID_RING_RADII_KM
        .last()
        .copied()
        .unwrap_or(2.0 * SOLAR_AU_KM);
    for index in 0..SOLAR_REFERENCE_GRID_SPOKES {
        let angle_rad = std::f64::consts::TAU * index as f64 / SOLAR_REFERENCE_GRID_SPOKES as f64;
        let path = solar_reference_spoke_path(grid_radius_km, angle_rad, Epoch::J2000);
        append_solar_reference_path_mesh(
            celestial_space,
            &path,
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
        );
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "Solar reference grid vertices are bounded presentation geometry."
)]
fn append_solar_reference_path_mesh(
    celestial_space: CelestialSpace,
    path: &TrajectoryPath,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    let points = path
        .samples
        .iter()
        .map(|sample| {
            let point = celestial_space
                .render_scale
                .position_to_units(sample.position_km);
            Vec3::new(point.x as f32, point.y as f32, point.z as f32)
        })
        .collect::<Vec<_>>();

    for segment in points.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let direction = (end - start).normalize_or_zero();
        if direction.length_squared() <= f32::EPSILON {
            continue;
        }

        let side =
            direction.cross(Vec3::Y).normalize_or_zero() * (SOLAR_REFERENCE_GRID_WIDTH_UNITS * 0.5);
        if side.length_squared() <= f32::EPSILON {
            continue;
        }

        let Ok(base_index) = u32::try_from(positions.len()) else {
            return;
        };
        positions.extend_from_slice(&[
            (start + side).to_array(),
            (start - side).to_array(),
            (end + side).to_array(),
            (end - side).to_array(),
        ]);
        normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[
            base_index,
            base_index + 2,
            base_index + 1,
            base_index + 1,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_precision_loss,
    reason = "Solar reference paths use bounded sample counts for presentation geometry."
)]
pub(crate) fn solar_reference_ring_path(
    radius_km: f64,
    sample_count: usize,
    epoch: Epoch,
) -> TrajectoryPath {
    let sample_count = sample_count.max(3);
    let samples = (0..=sample_count)
        .map(|index| {
            let angle_rad = std::f64::consts::TAU * index as f64 / sample_count as f64;
            solar_reference_state(solar_reference_point(radius_km, angle_rad), epoch)
        })
        .collect();

    TrajectoryPath::new(
        BodyId::SUN,
        BodyId::SUN,
        FrameId::j2000(),
        epoch,
        epoch,
        samples,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_reference_spoke_path(
    radius_km: f64,
    angle_rad: f64,
    epoch: Epoch,
) -> TrajectoryPath {
    let endpoint = solar_reference_point(radius_km, angle_rad);
    let samples = vec![
        solar_reference_state(Vec3d::new(-endpoint.x, 0.0, -endpoint.z), epoch),
        solar_reference_state(endpoint, epoch),
    ];

    TrajectoryPath::new(
        BodyId::SUN,
        BodyId::SUN,
        FrameId::j2000(),
        epoch,
        epoch,
        samples,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn solar_reference_point(radius_km: f64, angle_rad: f64) -> Vec3d {
    Vec3d::new(
        radius_km * angle_rad.sin(),
        0.0,
        radius_km * angle_rad.cos(),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn solar_reference_state(position_km: Vec3d, epoch: Epoch) -> StateVector {
    StateVector::relative_to(
        BodyId::SUN,
        BodyId::SUN,
        FrameId::j2000(),
        epoch,
        position_km,
        Vec3d::ZERO,
    )
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_sun_light(commands: &mut Commands<'_, '_>, parent: Entity) {
    let sun_radius_units = solar_body_radius_units(&CelestialBody::sun());
    let light = commands
        .spawn((
            Name::new("Solar System Sun Light"),
            Transform::default(),
            PointLight {
                color: Color::srgb(1.0, 0.82, 0.55),
                intensity: SOLAR_SUN_POINT_LIGHT_INTENSITY,
                range: SOLAR_SUN_POINT_LIGHT_RANGE_UNITS,
                radius: sun_radius_units,
                shadows_enabled: false,
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(light);
}

#[cfg(target_arch = "wasm32")]
fn spawn_solar_label(
    commands: &mut Commands<'_, '_>,
    target: Entity,
    label: &'static str,
    color: Color,
    offset_px: Vec2,
) {
    commands.spawn((
        Name::new(format!("Solar System {label} Label")),
        DemoSolarLabel { target, offset_px },
        Text2d::new(label),
        TextFont::from_font_size(13.0),
        TextLayout::new_with_justify(Justify::Center),
        TextColor(color),
        Text2dShadow {
            offset: Vec2::new(1.5, -1.5),
            color: Color::BLACK.with_alpha(0.88),
        },
        Anchor::CENTER_LEFT,
        Transform::from_translation(Vec3::new(0.0, 0.0, 100.0)),
        Visibility::Inherited,
    ));
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_labels(
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    cameras: Query<'_, '_, (&Camera, &GlobalTransform), With<MetricOrbitCamera>>,
    targets: Query<'_, '_, &GlobalTransform, Without<DemoSolarLabel>>,
    mut labels: Query<'_, '_, (&DemoSolarLabel, &mut Transform, &mut Visibility)>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };
    for (label, mut transform, mut visibility) in &mut labels {
        let Ok(target_transform) = targets.get(label.target) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(viewport_position) =
            camera.world_to_viewport(camera_transform, target_transform.translation())
        else {
            *visibility = Visibility::Hidden;
            continue;
        };

        *visibility = Visibility::Inherited;
        transform.translation.x = viewport_position.x + label.offset_px.x - window.width() * 0.5;
        transform.translation.y = window.height() * 0.5 - viewport_position.y + label.offset_px.y;
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_overview_aids_visibility(
    focus_selection: Res<'_, DemoSolarFocusSelection>,
    mut aids: Query<'_, '_, &mut Visibility, With<DemoSolarOverviewAid>>,
) {
    let visibility = if solar_focus_hides_overview_aids(focus_selection.target) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    for mut aid_visibility in &mut aids {
        *aid_visibility = visibility;
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_sun_model_material(
    sun_material: Option<ResMut<'_, DemoSolarSunModelMaterial>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    let Some(mut sun_material) = sun_material else {
        return;
    };
    if sun_material.applied {
        return;
    }
    let Some(material) = materials.get_mut(&sun_material.handle) else {
        return;
    };
    material.base_color = Color::srgb(1.0, 0.55, 0.08);
    material.base_color_texture = Some(sun_material.texture.clone());
    material.unlit = true;
    material.emissive = Color::linear_rgb(0.25, 0.08, 0.01).into();
    material.emissive_texture = Some(sun_material.texture.clone());
    material.perceptual_roughness = 1.0;
    material.reflectance = 0.0;
    material.cull_mode = None;
    sun_material.applied = true;
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_earth_atmosphere_material(
    sun_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarSun>>,
    earth_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarEarth>>,
    atmosphere_layers: Query<
        '_,
        '_,
        &MeshMaterial3d<DemoSolarEarthAtmosphereMaterial>,
        With<DemoSolarEarthAtmosphereLayer>,
    >,
    mut materials: ResMut<'_, Assets<DemoSolarEarthAtmosphereMaterial>>,
) {
    let Some(sun_position) = sun_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let Some(earth_position) = earth_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let sun_direction = sun_position - earth_position;

    for material_handle in &atmosphere_layers {
        let Some(material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.params.sun_direction_strength = solar_earth_atmosphere_sun_direction_param(
            sun_direction,
            material.params.sun_direction_strength.w,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_earth_cloud_material(
    sun_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarSun>>,
    earth_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarEarth>>,
    cloud_layers: Query<
        '_,
        '_,
        &MeshMaterial3d<DemoSolarEarthCloudMaterial>,
        With<DemoSolarEarthCloudLayer>,
    >,
    mut materials: ResMut<'_, Assets<DemoSolarEarthCloudMaterial>>,
) {
    let Some(sun_position) = sun_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let Some(earth_position) = earth_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let sun_direction = sun_position - earth_position;

    for material_handle in &cloud_layers {
        let Some(material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.params.sun_direction_strength = solar_earth_atmosphere_sun_direction_param(
            sun_direction,
            material.params.sun_direction_strength.w,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_earth_night_lights_material(
    sun_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarSun>>,
    earth_transforms: Query<'_, '_, &GlobalTransform, With<DemoSolarEarth>>,
    night_light_layers: Query<
        '_,
        '_,
        &MeshMaterial3d<DemoSolarEarthNightLightsMaterial>,
        With<DemoSolarEarthNightLightsLayer>,
    >,
    mut materials: ResMut<'_, Assets<DemoSolarEarthNightLightsMaterial>>,
) {
    let Some(sun_position) = sun_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let Some(earth_position) = earth_transforms
        .iter()
        .next()
        .map(GlobalTransform::translation)
    else {
        return;
    };
    let sun_direction = sun_position - earth_position;

    for material_handle in &night_light_layers {
        let Some(material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        material.params.sun_direction_strength = solar_earth_atmosphere_sun_direction_param(
            sun_direction,
            material.params.sun_direction_strength.w,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_ui_controls(
    mut celestial_epoch: ResMut<'_, CelestialEpoch>,
    mut camera_state: ResMut<'_, MetricOrbitCameraState>,
    mut trail_window: ResMut<'_, DemoSolarTrailWindow>,
    mut focus_selection: ResMut<'_, DemoSolarFocusSelection>,
    mut metric_focus_selection: ResMut<'_, MetricSceneFocusSelection>,
    mut ui_state: ResMut<'_, DemoSolarUiState>,
) {
    let next_epoch = browser_solar_epoch();
    if celestial_epoch.epoch != next_epoch {
        celestial_epoch.epoch = next_epoch;
    }

    trail_window.months = browser_solar_trail_months();
    let next_focus = browser_solar_focus_target();
    if focus_selection.target != next_focus {
        focus_selection.target = next_focus;
        focus_selection.applied = None;
        metric_focus_selection.set_target(solar_focus_metric_target(next_focus));
    }

    let reset_token = browser_local_storage_item(SOLAR_CAMERA_RESET_STORAGE_KEY);
    if reset_token.is_some() && reset_token != ui_state.reset_token {
        *camera_state = default_solar_camera_state();
        focus_selection.applied = None;
    }
    ui_state.reset_token = reset_token;
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_camera_focus(
    mut camera_state: ResMut<'_, MetricOrbitCameraState>,
    mut focus_selection: ResMut<'_, DemoSolarFocusSelection>,
    mut focus_tracker: ResMut<'_, DemoSolarFocusPivotTracker>,
    focus_pivot: Res<'_, MetricSceneFocusPivot>,
) {
    let target = focus_selection.target;
    let target_changed = focus_selection.applied != Some(target);
    let camera_changed_by_later_system = camera_state.is_changed() && !target_changed;
    if !target_changed && !focus_pivot.is_changed() {
        return;
    }

    let expected_target = solar_focus_metric_target(target);
    if !focus_pivot.resolved || focus_pivot.target != expected_target {
        return;
    }

    let recenter_needed = solar_focus_should_recenter_camera(
        *camera_state,
        focus_pivot.as_ref(),
        target,
        target_changed,
        camera_changed_by_later_system,
    );
    if target_changed || recenter_needed {
        camera_state.pivot_units = focus_pivot.pivot_units;
    } else if let Some(delta) = solar_live_focus_pivot_delta(
        target,
        focus_tracker.target,
        focus_tracker.pivot_units,
        focus_pivot.pivot_units,
    ) {
        camera_state.pivot_units += delta;
    }
    if target_changed {
        camera_state.distance_units = solar_focus_distance_units(target);
    }
    if target_changed || recenter_needed {
        if let Some((yaw_deg, pitch_deg)) =
            solar_day_side_focus_angles(focus_pivot.pivot_units, target)
        {
            camera_state.yaw_deg = yaw_deg;
            camera_state.pitch_deg = pitch_deg;
        }
    }
    if focus_selection.applied != Some(target) {
        focus_selection.applied = Some(target);
    }
    focus_tracker.target = Some(target);
    focus_tracker.pivot_units = Some(focus_pivot.pivot_units);
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_focus_should_recenter_camera(
    camera_state: MetricOrbitCameraState,
    focus_pivot: &MetricSceneFocusPivot,
    target: DemoSolarFocusTarget,
    target_changed: bool,
    camera_state_changed: bool,
) -> bool {
    if target_changed {
        return true;
    }
    if solar_focus_tracks_live_pivot(target) {
        return false;
    }
    !camera_state_changed && solar_focus_pivot_needs_recenter(camera_state, focus_pivot, target)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_focus_tracks_live_pivot(target: DemoSolarFocusTarget) -> bool {
    matches!(
        target,
        DemoSolarFocusTarget::Mercury
            | DemoSolarFocusTarget::Venus
            | DemoSolarFocusTarget::Earth
            | DemoSolarFocusTarget::Moon
            | DemoSolarFocusTarget::Mars
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_live_focus_pivot_delta(
    target: DemoSolarFocusTarget,
    tracked_target: Option<DemoSolarFocusTarget>,
    tracked_pivot_units: Option<DVec3>,
    focus_pivot_units: DVec3,
) -> Option<DVec3> {
    if !solar_focus_tracks_live_pivot(target) || tracked_target != Some(target) {
        return None;
    }

    let delta = focus_pivot_units - tracked_pivot_units?;
    delta.is_finite().then_some(delta)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_focus_pivot_needs_recenter(
    camera_state: MetricOrbitCameraState,
    focus_pivot: &MetricSceneFocusPivot,
    target: DemoSolarFocusTarget,
) -> bool {
    if solar_focus_tracks_live_pivot(target) {
        return false;
    }

    camera_state.pivot_units.distance(focus_pivot.pivot_units)
        > solar_focus_recenter_threshold_units(target)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_focus_recenter_threshold_units(target: DemoSolarFocusTarget) -> f64 {
    (solar_focus_distance_units(target) * 0.25).max(1.0e-6)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_day_side_focus_angles(
    focus_pivot_units: DVec3,
    target: DemoSolarFocusTarget,
) -> Option<(f64, f64)> {
    if !solar_focus_prefers_day_side_camera(target) {
        return None;
    }

    metric_orbit_angles_from_direction((-focus_pivot_units).normalize_or_zero())
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_focus_prefers_day_side_camera(target: DemoSolarFocusTarget) -> bool {
    matches!(
        target,
        DemoSolarFocusTarget::Mercury
            | DemoSolarFocusTarget::Venus
            | DemoSolarFocusTarget::Earth
            | DemoSolarFocusTarget::Moon
            | DemoSolarFocusTarget::Mars
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn metric_orbit_angles_from_direction(direction: DVec3) -> Option<(f64, f64)> {
    let direction = direction.normalize_or_zero();
    if direction.length_squared() <= f64::EPSILON {
        return None;
    }

    let pitch_deg = direction.y.asin().to_degrees();
    let yaw_deg = direction.x.atan2(direction.z).to_degrees();
    Some((yaw_deg, pitch_deg))
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_active_tile_source(
    focus_selection: Res<'_, DemoSolarFocusSelection>,
    mut active_source: ResMut<'_, ActiveTileSource>,
) {
    let Some(next_source) = solar_dynamic_tile_source_for_focus(focus_selection.target) else {
        return;
    };

    if active_source.0 != next_source {
        active_source.0 = next_source;
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_camera_constraints(
    focus_selection: Res<'_, DemoSolarFocusSelection>,
    mut settings: ResMut<'_, MetricOrbitCameraSettings>,
) {
    let min_distance_units = solar_camera_min_distance_units(focus_selection.target);
    if (settings.min_distance_units - min_distance_units).abs() > f64::EPSILON {
        settings.min_distance_units = min_distance_units;
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_solar_dynamic_body_surface_view(
    camera_state: Res<'_, MetricOrbitCameraState>,
    focus_selection: Res<'_, DemoSolarFocusSelection>,
    focus_pivot: Res<'_, MetricSceneFocusPivot>,
    body_anchors: Query<'_, '_, (&MetricSceneObject, &Transform)>,
    mut surface_views: Query<'_, '_, &mut DynamicBodySurfaceView>,
) {
    let Some(mut surface_view) = surface_views.iter_mut().next() else {
        return;
    };
    let target = focus_selection.target;
    let Some(body) = solar_focus_body(target) else {
        *surface_view = DynamicBodySurfaceView::disabled(BodyId::EARTH);
        return;
    };
    if solar_dynamic_tile_source_for_focus(target).is_none() {
        *surface_view = DynamicBodySurfaceView::disabled(body);
        return;
    }
    let expected_target = MetricSceneFocusTarget::Object(body);
    if !focus_pivot.resolved || focus_pivot.target != expected_target {
        *surface_view = DynamicBodySurfaceView::disabled(body);
        return;
    }
    let Some((_, body_transform)) = body_anchors.iter().find(|(object, _)| object.id == body)
    else {
        *surface_view = DynamicBodySurfaceView::disabled(body);
        return;
    };

    let radius = focus_pivot
        .physical_radius_units
        .or_else(|| solar_dynamic_body_radius_units(target))
        .unwrap_or(1.0);
    let camera_vector = camera_state.position_units() - focus_pivot.pivot_units;
    let camera_distance = camera_vector.length();
    if camera_distance <= f64::EPSILON {
        *surface_view = DynamicBodySurfaceView::disabled(body);
        return;
    }
    let surface_transition_distance = f64::from(radius) * SOLAR_SURFACE_LOD_DISTANCE_RADIUS_FACTOR;
    if camera_distance > surface_transition_distance {
        *surface_view = DynamicBodySurfaceView::disabled(body);
        return;
    }
    let camera_direction_world = camera_vector.normalize().as_vec3();
    let camera_surface_direction = body_transform
        .rotation
        .inverse()
        .mul_vec3(camera_direction_world)
        .normalize_or_zero();

    *surface_view = DynamicBodySurfaceView::enabled(
        body,
        radius,
        f64::from(radius) * SOLAR_DYNAMIC_BODY_BASE_SURFACE_DISTANCE_RADIUS_FACTOR,
        (camera_distance - f64::from(radius)).max(0.0),
        camera_surface_direction,
        solar_dynamic_body_base_tile_zoom(target),
        solar_dynamic_body_max_selected_tiles(),
    );
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_focus_hides_overview_aids(target: DemoSolarFocusTarget) -> bool {
    matches!(
        target,
        DemoSolarFocusTarget::Mercury
            | DemoSolarFocusTarget::Venus
            | DemoSolarFocusTarget::Earth
            | DemoSolarFocusTarget::Moon
            | DemoSolarFocusTarget::Mars
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_scene_focus_distance_units() -> f64 {
    f64::from(SOLAR_CAMERA_DISTANCE_UNITS).hypot(f64::from(SOLAR_CAMERA_HEIGHT_UNITS))
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_body_lod_policy(body: &CelestialBody) -> MetricSceneLodPolicy {
    MetricSceneLodPolicy::new(
        SOLAR_BODY_FOCUS_DISTANCE_UNITS,
        f64::from(solar_body_radius_units(body)) * SOLAR_SURFACE_LOD_DISTANCE_RADIUS_FACTOR,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_focus_distance_units(target: DemoSolarFocusTarget) -> f64 {
    match target {
        DemoSolarFocusTarget::Scene => solar_scene_focus_distance_units(),
        DemoSolarFocusTarget::Sun
        | DemoSolarFocusTarget::Mercury
        | DemoSolarFocusTarget::Venus
        | DemoSolarFocusTarget::Earth
        | DemoSolarFocusTarget::Moon
        | DemoSolarFocusTarget::Mars => {
            solar_body_focus_distance_units(target).unwrap_or(SOLAR_BODY_FOCUS_DISTANCE_UNITS)
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_body_focus_distance_units(target: DemoSolarFocusTarget) -> Option<f64> {
    solar_focus_body_radius_units(target)
        .map(|radius| f64::from(radius) * SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR)
}

#[cfg(test)]
pub(crate) fn solar_dynamic_body_focus_distance_units(target: DemoSolarFocusTarget) -> Option<f64> {
    solar_dynamic_body_radius_units(target)
        .map(|radius| f64::from(radius) * SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_camera_min_distance_units(target: DemoSolarFocusTarget) -> f64 {
    solar_focus_body_radius_units(target).map_or(SOLAR_CAMERA_MIN_DISTANCE_UNITS, |radius| {
        SOLAR_CAMERA_MIN_DISTANCE_UNITS.max(f64::from(radius) * 1.05)
    })
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_focus_body_radius_units(target: DemoSolarFocusTarget) -> Option<f32> {
    let body = match target {
        DemoSolarFocusTarget::Sun => CelestialBody::sun(),
        DemoSolarFocusTarget::Mercury
        | DemoSolarFocusTarget::Venus
        | DemoSolarFocusTarget::Earth
        | DemoSolarFocusTarget::Moon
        | DemoSolarFocusTarget::Mars => solar_focus_body(target).and_then(solar_body_for_focus)?,
        DemoSolarFocusTarget::Scene => return None,
    };

    Some(solar_body_radius_units(&body))
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_body_for_focus(body: BodyId) -> Option<CelestialBody> {
    match body {
        BodyId::SUN => Some(CelestialBody::sun()),
        BodyId::MERCURY | BodyId::MERCURY_BARYCENTER => Some(demo_mercury_body()),
        BodyId::VENUS | BodyId::VENUS_BARYCENTER => Some(demo_venus_body()),
        BodyId::EARTH => Some(CelestialBody::earth()),
        BodyId::MOON => Some(CelestialBody::moon()),
        BodyId::MARS | BodyId::MARS_BARYCENTER => Some(demo_mars_body()),
        _ => None,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_possible_truncation,
    reason = "Solar body radii are small render-unit f32 values for Bevy meshes and camera clamps."
)]
pub(crate) fn solar_body_radius_units(body: &CelestialBody) -> f32 {
    body.mean_radius_units(RenderScale::from_kilometers_per_unit(
        SOLAR_RENDER_KILOMETERS_PER_UNIT,
    )) as f32
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn solar_nasa_sun_model_scale_units(radius_units: f32) -> f32 {
    radius_units / SOLAR_NASA_SUN_MODEL_SOURCE_RADIUS_UNITS
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_possible_truncation,
    reason = "Solar body radii are small render-unit f32 values for Bevy camera clamps."
)]
pub(crate) fn solar_dynamic_body_radius_units(target: DemoSolarFocusTarget) -> Option<f32> {
    let body = match target {
        DemoSolarFocusTarget::Mercury => demo_mercury_body(),
        DemoSolarFocusTarget::Venus => demo_venus_body(),
        DemoSolarFocusTarget::Earth => CelestialBody::earth(),
        DemoSolarFocusTarget::Moon => CelestialBody::moon(),
        DemoSolarFocusTarget::Mars => demo_mars_body(),
        DemoSolarFocusTarget::Scene | DemoSolarFocusTarget::Sun => return None,
    };

    Some(solar_body_radius_units(&body))
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_dynamic_body_max_selected_tiles() -> usize {
    SOLAR_DYNAMIC_BODY_MAX_SELECTED_TILES
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_dynamic_body_base_tile_zoom(target: DemoSolarFocusTarget) -> u8 {
    match target {
        DemoSolarFocusTarget::Mercury => SOLAR_MERCURY_TILE_ZOOM,
        DemoSolarFocusTarget::Venus => SOLAR_VENUS_TILE_ZOOM,
        DemoSolarFocusTarget::Earth => SOLAR_EARTH_TILE_ZOOM,
        DemoSolarFocusTarget::Moon => SOLAR_MOON_TILE_ZOOM,
        DemoSolarFocusTarget::Mars => SOLAR_MARS_TILE_ZOOM,
        DemoSolarFocusTarget::Scene | DemoSolarFocusTarget::Sun => 0,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    dead_code,
    reason = "Host tests exercise the resampling metadata while wasm uses every entity slot."
)]
#[derive(Resource, Default)]
pub(crate) struct DemoSolarOrbitPathEntities {
    pub(crate) initialized: bool,
    pub(crate) trail_months: Option<u32>,
    pub(crate) sampled_epoch: Option<Epoch>,
    pub(crate) mercury_guide: Option<Entity>,
    pub(crate) mercury_trail: Option<Entity>,
    pub(crate) venus_guide: Option<Entity>,
    pub(crate) venus_trail: Option<Entity>,
    pub(crate) earth_guide: Option<Entity>,
    pub(crate) earth_trail: Option<Entity>,
    pub(crate) moon_guide: Option<Entity>,
    pub(crate) moon_trail: Option<Entity>,
    pub(crate) mars_guide: Option<Entity>,
    pub(crate) mars_trail: Option<Entity>,
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_lines,
    reason = "The demo solar mode samples a fixed set of named orbit paths for readability."
)]
fn sync_demo_solar_orbit_paths(
    mut commands: Commands<'_, '_>,
    celestial_epoch: Res<'_, CelestialEpoch>,
    celestial_ephemeris: Res<'_, CelestialEphemeris>,
    trail_window: Res<'_, DemoSolarTrailWindow>,
    mut path_entities: ResMut<'_, DemoSolarOrbitPathEntities>,
    roots: Query<'_, '_, Entity, With<CelestialRoot>>,
    earth_orbit_anchors: Query<'_, '_, Entity, With<DemoSolarEarthOrbitAnchor>>,
) {
    if !solar_paths_need_resample(
        &path_entities,
        *celestial_epoch,
        *trail_window,
        celestial_ephemeris.is_changed(),
    ) {
        return;
    }

    let Some(root) = roots.iter().next() else {
        return;
    };
    let Some(earth_orbit_anchor) = earth_orbit_anchors.iter().next() else {
        return;
    };
    let Some(provider) = celestial_ephemeris.provider() else {
        return;
    };

    let (mercury_guide, mercury_trail) = sync_demo_solar_body_paths(
        &mut commands,
        root,
        provider,
        *celestial_epoch,
        *trail_window,
        BodyId::MERCURY_BARYCENTER,
        BodyId::SUN,
        MERCURY_ORBIT_PERIOD_SECONDS,
        "Mercury",
        Color::srgba(0.55, 0.66, 0.76, 0.18),
        Color::srgba(0.72, 0.82, 0.95, 0.70),
        true,
        path_entities.mercury_guide,
        path_entities.mercury_trail,
        SOLAR_ORBIT_GUIDE_WIDTH_UNITS,
        SOLAR_ORBIT_TRAIL_WIDTH_UNITS,
    );
    path_entities.mercury_guide = mercury_guide;
    path_entities.mercury_trail = mercury_trail;

    let (venus_guide, venus_trail) = sync_demo_solar_body_paths(
        &mut commands,
        root,
        provider,
        *celestial_epoch,
        *trail_window,
        BodyId::VENUS_BARYCENTER,
        BodyId::SUN,
        VENUS_ORBIT_PERIOD_SECONDS,
        "Venus",
        Color::srgba(1.0, 0.78, 0.42, 0.16),
        Color::srgba(1.0, 0.84, 0.48, 0.68),
        true,
        path_entities.venus_guide,
        path_entities.venus_trail,
        SOLAR_ORBIT_GUIDE_WIDTH_UNITS,
        SOLAR_ORBIT_TRAIL_WIDTH_UNITS,
    );
    path_entities.venus_guide = venus_guide;
    path_entities.venus_trail = venus_trail;

    let (earth_guide, earth_trail) = sync_demo_solar_body_paths(
        &mut commands,
        root,
        provider,
        *celestial_epoch,
        *trail_window,
        BodyId::EARTH,
        BodyId::SUN,
        EARTH_ORBIT_PERIOD_SECONDS,
        "Earth",
        Color::srgba(0.45, 0.72, 1.0, 0.20),
        Color::srgba(0.52, 0.78, 1.0, 0.76),
        true,
        path_entities.earth_guide,
        path_entities.earth_trail,
        SOLAR_ORBIT_GUIDE_WIDTH_UNITS,
        SOLAR_ORBIT_TRAIL_WIDTH_UNITS,
    );
    path_entities.earth_guide = earth_guide;
    path_entities.earth_trail = earth_trail;

    let (moon_guide, moon_trail) = sync_demo_solar_body_paths(
        &mut commands,
        earth_orbit_anchor,
        provider,
        *celestial_epoch,
        *trail_window,
        BodyId::MOON,
        BodyId::EARTH,
        MOON_ORBIT_PERIOD_SECONDS,
        "Moon",
        Color::srgba(0.82, 0.82, 0.78, 0.16),
        Color::srgba(0.88, 0.88, 0.84, 0.58),
        false,
        path_entities.moon_guide,
        path_entities.moon_trail,
        SOLAR_ORBIT_GUIDE_WIDTH_UNITS,
        SOLAR_ORBIT_TRAIL_WIDTH_UNITS,
    );
    path_entities.moon_guide = moon_guide;
    path_entities.moon_trail = moon_trail;

    let (mars_guide, mars_trail) = sync_demo_solar_body_paths(
        &mut commands,
        root,
        provider,
        *celestial_epoch,
        *trail_window,
        BodyId::MARS_BARYCENTER,
        BodyId::SUN,
        MARS_ORBIT_PERIOD_SECONDS,
        "Mars",
        Color::srgba(1.0, 0.38, 0.22, 0.18),
        Color::srgba(1.0, 0.46, 0.28, 0.72),
        true,
        path_entities.mars_guide,
        path_entities.mars_trail,
        SOLAR_ORBIT_GUIDE_WIDTH_UNITS,
        SOLAR_ORBIT_TRAIL_WIDTH_UNITS,
    );
    path_entities.mars_guide = mars_guide;
    path_entities.mars_trail = mars_trail;

    path_entities.sampled_epoch = Some(celestial_epoch.epoch);
    path_entities.trail_months = Some(trail_window.months);
    path_entities.initialized = true;
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_arguments,
    reason = "Each solar path has explicit body, style, parent, and entity-slot inputs."
)]
fn sync_demo_solar_body_paths(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    provider: &(dyn EphemerisProvider + Send + Sync + 'static),
    epoch: CelestialEpoch,
    trail_window: DemoSolarTrailWindow,
    target: BodyId,
    origin: BodyId,
    period_seconds: f64,
    label: &str,
    guide_color: Color,
    trail_color: Color,
    high_precision_root_child: bool,
    mut guide_slot: Option<Entity>,
    mut trail_slot: Option<Entity>,
    guide_width_units: f32,
    trail_width_units: f32,
) -> (Option<Entity>, Option<Entity>) {
    if let Ok(path) = sample_trajectory_between(
        provider,
        target,
        origin,
        FrameId::j2000(),
        epoch.epoch,
        offset_epoch_seconds(epoch.epoch, period_seconds),
        SOLAR_ORBIT_SAMPLES,
    ) {
        upsert_demo_trajectory(
            commands,
            parent,
            &mut guide_slot,
            &format!("{label} Metric Orbit Guide"),
            path,
            guide_color,
            guide_width_units,
            high_precision_root_child,
        );
    }

    let trail_start = solar_trail_start_epoch(epoch.epoch, trail_window.months);
    let trail_samples = solar_trail_sample_count(trail_window.months, period_seconds);
    if let Ok(path) = sample_trajectory_between(
        provider,
        target,
        origin,
        FrameId::j2000(),
        trail_start,
        epoch.epoch,
        trail_samples,
    ) {
        upsert_demo_trajectory(
            commands,
            parent,
            &mut trail_slot,
            &format!("{label} Metric Trail"),
            path,
            trail_color,
            trail_width_units,
            high_precision_root_child,
        );
    }

    (guide_slot, trail_slot)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_paths_need_resample(
    path_entities: &DemoSolarOrbitPathEntities,
    epoch: CelestialEpoch,
    trail_window: DemoSolarTrailWindow,
    ephemeris_changed: bool,
) -> bool {
    if !path_entities.initialized
        || ephemeris_changed
        || path_entities.trail_months != Some(trail_window.months)
    {
        return true;
    }

    trajectory_samples_are_stale(
        path_entities.sampled_epoch,
        epoch.epoch,
        SOLAR_TRAIL_RESAMPLE_SECONDS,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_trail_start_epoch(epoch: Epoch, months: u32) -> Epoch {
    solar_trail_config().start_epoch(epoch, months)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_trail_sample_count(months: u32, period_seconds: f64) -> usize {
    solar_trail_config().sample_count(months, period_seconds)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn solar_trail_config() -> TrajectoryTrailConfig {
    TrajectoryTrailConfig {
        min_months: SOLAR_TRAIL_MIN_MONTHS,
        max_months: SOLAR_TRAIL_MAX_MONTHS,
        average_month_seconds: AVERAGE_GREGORIAN_MONTH_SECONDS,
        min_samples: SOLAR_TRAIL_MIN_SAMPLES,
        max_samples: SOLAR_TRAIL_MAX_SAMPLES,
        samples_per_period: 128.0,
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_arguments,
    reason = "Trajectory upsert needs explicit parent, slot, style, and placement parameters."
)]
fn upsert_demo_trajectory(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    entity_slot: &mut Option<Entity>,
    name: &str,
    path: TrajectoryPath,
    color: Color,
    width_units: f32,
    high_precision_root_child: bool,
) {
    let trajectory = MetricTrajectory::new(path, MetricTrajectoryVisual { width_units, color });

    if let Some(entity) = *entity_slot {
        commands.entity(entity).insert(trajectory);
        return;
    }

    let entity = commands
        .spawn((
            Name::new(name.to_owned()),
            DemoSolarOverviewAid,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Inherited,
            trajectory,
        ))
        .id();
    if high_precision_root_child {
        commands.entity(entity).insert(CellCoord::default());
    }
    commands.entity(parent).add_child(entity);
    *entity_slot = Some(entity);
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_mercury_body() -> CelestialBody {
    CelestialBody::spherical(
        BodyId::MERCURY_BARYCENTER,
        "Mercury",
        CelestialBody::mercury().mean_radius_km,
        FrameId::j2000(),
        FrameId::named("IAU_MERCURY"),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_venus_body() -> CelestialBody {
    CelestialBody::spherical(
        BodyId::VENUS_BARYCENTER,
        "Venus",
        CelestialBody::venus().mean_radius_km,
        FrameId::j2000(),
        FrameId::named("IAU_VENUS"),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_mars_body() -> CelestialBody {
    CelestialBody::spherical(
        BodyId::MARS_BARYCENTER,
        "Mars",
        CelestialBody::mars().mean_radius_km,
        FrameId::j2000(),
        FrameId::iau_mars(),
    )
}
