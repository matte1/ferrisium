#[cfg(any(target_arch = "wasm32", test))]
mod browser_params;
#[cfg(target_arch = "wasm32")]
mod custom_geo_draw;
#[cfg(any(target_arch = "wasm32", test))]
mod demo_config;
#[cfg(target_arch = "wasm32")]
mod egui_overlay;
#[cfg(any(target_arch = "wasm32", test))]
mod ephemeris_demo;
#[cfg(any(target_arch = "wasm32", test))]
mod globe_mode;
#[cfg(any(target_arch = "wasm32", test))]
mod h3_demo;
#[cfg(target_arch = "wasm32")]
mod map_mode;
#[cfg(any(target_arch = "wasm32", test))]
mod solar_mode;
#[cfg(target_arch = "wasm32")]
mod test_bridge;

#[cfg(target_arch = "wasm32")]
use crate::demo_config::{browser_demo_mode, DemoMode};
#[cfg(target_arch = "wasm32")]
use bevy::asset::{AssetMetaCheck, AssetPlugin};
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::{ActiveTileSource, FerrisiumPlugin};

#[cfg(target_arch = "wasm32")]
fn main() {
    let demo_mode = browser_demo_mode();
    let mut app = App::new();
    app.insert_resource(ClearColor(demo_mode.clear_color()))
        .insert_resource(ActiveTileSource::from(demo_mode.tile_source()))
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<TransformPlugin>()
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(primary_window()),
                    ..default()
                }),
        )
        .add_plugins(FerrisiumPlugin);

    if egui_overlay::demo_egui_overlay_enabled() {
        egui_overlay::configure_demo_egui_overlay(&mut app);
    }

    match demo_mode {
        DemoMode::Globe => globe_mode::configure_globe_mode(&mut app),
        DemoMode::SolarSystem => solar_mode::configure_solar_system_mode(&mut app),
        DemoMode::Map => map_mode::configure_map_mode(&mut app),
    }
    test_bridge::configure_test_bridge(&mut app, demo_mode);

    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), &'static str> {
    Err("ferrisium_demo is browser-only; use `just web`.")
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "The browser-only demo keeps wasm systems cfg-gated below the shared parser tests."
)]
mod tests {
    use bevy::math::DVec3;
    use bevy::prelude::{Color, GlobalAmbientLight, Visibility};
    use ferrisium_bevy::prelude::{
        DeferredGlobeSkybox, Globe3dState, MetricOrbitCameraState, MetricSceneFocusPivot,
        MetricSceneFocusTarget, MetricSceneId, MetricScenePresentation, TiledBodySurface,
        DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES,
    };

    use crate::browser_params::{
        normalized_mapbox_public_token, parse_bool_flag as parse_demo_bool_flag, parse_finite_f64,
        parse_positive_f32, query_param,
    };
    use crate::demo_config::{
        demo_earth_tile_source, demo_focus_body_id_for_trek_body, demo_focus_metric_target,
        demo_globe_ambient_fill_light, demo_globe_skybox_config, demo_map_tile_source,
        demo_solar_ambient_fill_light, demo_solar_skybox_config, parse_demo_earth_tile_source,
        parse_demo_focus, parse_demo_map_body, parse_demo_mode, parse_solar_focus_target,
        parse_solar_trail_months, solar_demo_epoch, solar_dynamic_tile_source, solar_focus_body,
        solar_focus_metric_target, DemoEarthTileSource, DemoGlobeCameraOverride, DemoMapBody,
        DemoMode, DemoSolarFocusTarget, DemoSolarTexturedBody,
        DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
        DEMO_GLOBE_CAMERA_QUERY_MIN_DISTANCE_RADIUS_FACTOR, SOLAR_TRAIL_DEFAULT_MONTHS,
        SOLAR_TRAIL_MAX_MONTHS, SOLAR_TRAIL_MAX_SAMPLES, SOLAR_TRAIL_MIN_MONTHS,
        SOLAR_TRAIL_MIN_SAMPLES, SOLAR_TRAIL_RESAMPLE_SECONDS,
    };
    use crate::ephemeris_demo::{demo_fallback_state, rotation_about_y};
    use crate::globe_mode::{
        apply_demo_globe_camera_override, apply_demo_globe_camera_override_with_ephemeris,
        body_radius_units, demo_globe_orbit_angles_for_surface, demo_globe_surface_presentations,
        demo_globe_tiled_body_surface, demo_globe_tiled_body_zoom,
        demo_secondary_globe_body_visibility, globe_secondary_body_lod_policy,
        globe_surface_direction_for_lon_lat, orbit_angles_for_direction,
        DEMO_GLOBE_SURFACE_LOD_DISTANCE_RADIUS_FACTOR, DEMO_MARS_TILE_ZOOM, DEMO_MOON_TILE_ZOOM,
        DEMO_REGULAR_BODY_TILE_ZOOM,
    };
    use crate::solar_mode::{
        demo_venus_body, solar_body_lod_policy, solar_body_radius_units,
        solar_camera_min_distance_units, solar_day_side_focus_angles,
        solar_dynamic_body_base_tile_zoom, solar_dynamic_body_focus_distance_units,
        solar_dynamic_body_max_selected_tiles, solar_dynamic_body_radius_units,
        solar_focus_distance_units, solar_focus_hides_overview_aids,
        solar_focus_pivot_needs_recenter, solar_focus_recenter_threshold_units,
        solar_focus_should_recenter_camera, solar_focus_tracks_live_pivot,
        solar_paths_need_resample, solar_reference_ring_path, solar_reference_spoke_path,
        solar_scene_focus_distance_units, solar_sun_light_illuminance_at_distance_units,
        solar_tiled_body_fallback_color, solar_tiled_body_surface, solar_trail_sample_count,
        solar_trail_start_epoch, solar_true_radius_policy, DemoSolarOrbitPathEntities,
        DemoSolarTrailWindow, SOLAR_AU_KM, SOLAR_BODY_FOCUS_DISTANCE_UNITS,
        SOLAR_DYNAMIC_BODY_BASE_SURFACE_DISTANCE_RADIUS_FACTOR,
        SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR, SOLAR_EARTH_TILE_ZOOM,
        SOLAR_MARS_TILE_ZOOM, SOLAR_MERCURY_TILE_ZOOM, SOLAR_MOON_TILE_ZOOM,
        SOLAR_RENDER_KILOMETERS_PER_UNIT, SOLAR_SUN_POINT_LIGHT_RANGE_UNITS,
        SOLAR_SURFACE_LOD_DISTANCE_RADIUS_FACTOR, SOLAR_VENUS_TILE_ZOOM,
    };
    use ferrisium_core::prelude::{
        LonLat, NasaTrekRegularBody, PlanetaryBody, TileId, TileProjection, TileSource,
    };
    use ferrisium_core::space::{
        BodyId, CelestialBody, EphemerisError, EphemerisProvider, EphemerisResult, Epoch, FrameId,
        FrameOrientation, OrientationRequest, RenderScale, StateRequest, StateVector, Vec3d,
    };

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    fn assert_close_f32(lhs: f32, rhs: f32) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-6,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    fn assert_cool_space_fill_light(
        light: &GlobalAmbientLight,
        minimum_brightness: f32,
        maximum_brightness: f32,
    ) {
        let color = light.color.to_srgba();

        assert!(light.affects_lightmapped_meshes);
        assert!(light.brightness >= minimum_brightness);
        assert!(light.brightness <= maximum_brightness);
        assert!(color.blue > color.green);
        assert!(color.green > color.red);
        assert_close_f32(color.alpha, 1.0);
    }

    struct QuarterTurnOrientationProvider;

    impl EphemerisProvider for QuarterTurnOrientationProvider {
        fn state(&self, _request: &StateRequest) -> EphemerisResult<StateVector> {
            Err(EphemerisError::MissingData(
                "test provider only serves orientation".to_owned(),
            ))
        }

        fn orientation(&self, request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
            Ok(FrameOrientation::new(
                request.from.clone(),
                request.to.clone(),
                request.epoch,
                rotation_about_y(std::f64::consts::FRAC_PI_2),
                None,
            ))
        }
    }

    fn html_select_block<'a>(html: &'a str, select_id: &str) -> Option<&'a str> {
        let marker = format!("id=\"{select_id}\"");
        let marker_start = html.find(&marker)?;
        let before_marker = &html[..marker_start];
        let select_start = before_marker.rfind("<select")?;
        let after_select = &html[select_start..];
        let select_end = after_select.find("</select>")? + "</select>".len();

        Some(&after_select[..select_end])
    }

    #[test]
    fn query_param_reads_requested_value() {
        assert_eq!(
            query_param("?mapbox_token=pk.test&other=value", "mapbox_token"),
            Some("pk.test".to_owned())
        );
    }

    #[test]
    fn demo_bool_flag_accepts_browser_truthy_values() {
        assert!(parse_demo_bool_flag("1"));
        assert!(parse_demo_bool_flag("true"));
        assert!(parse_demo_bool_flag("YES"));
        assert!(!parse_demo_bool_flag("0"));
        assert!(!parse_demo_bool_flag("false"));
    }

    #[test]
    fn globe_camera_query_parsers_accept_only_finite_numbers() {
        assert_eq!(parse_finite_f64("-98.5"), Some(-98.5));
        assert_eq!(parse_finite_f64("nan"), None);
        assert_eq!(parse_finite_f64("inf"), None);

        assert_eq!(parse_positive_f32("1.45"), Some(1.45));
        assert_eq!(parse_positive_f32("0"), None);
        assert_eq!(parse_positive_f32("-1"), None);
    }

    #[test]
    fn globe_camera_query_override_targets_requested_surface() {
        let mut state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
        apply_demo_globe_camera_override(
            &mut state,
            BodyId::EARTH,
            DemoGlobeCameraOverride {
                surface_lon_lat: None,
                yaw_deg: Some(-98.0),
                pitch_deg: Some(100.0),
                distance_radius_factor: Some(1.45),
            },
        );

        let expected_distance = body_radius_units(&CelestialBody::earth()) * 1.45;
        assert_close(state.yaw_deg, -98.0);
        assert_close(state.pitch_deg, DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG);
        assert!((state.camera_distance - expected_distance).abs() < 0.001);
        assert_close_f32(state.base_camera_distance, state.camera_distance);
    }

    #[test]
    fn globe_camera_query_distance_factor_stays_outside_the_body() {
        let mut state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
        apply_demo_globe_camera_override(
            &mut state,
            BodyId::EARTH,
            DemoGlobeCameraOverride {
                distance_radius_factor: Some(0.01),
                ..Default::default()
            },
        );

        let min_distance = body_radius_units(&CelestialBody::earth())
            * DEMO_GLOBE_CAMERA_QUERY_MIN_DISTANCE_RADIUS_FACTOR;
        assert!((state.camera_distance - min_distance).abs() < 0.001);
    }

    #[test]
    fn globe_surface_camera_target_maps_lon_lat_to_orbit_angles() {
        let local_direction = globe_surface_direction_for_lon_lat(LonLat::new(-98.0, 39.0));
        let (yaw_deg, pitch_deg) = orbit_angles_for_direction(local_direction);

        assert!((yaw_deg + 98.0).abs() <= 1.0e-9);
        assert!((pitch_deg - 39.0).abs() <= 1.0e-9);
    }

    #[test]
    fn globe_surface_camera_target_uses_body_fixed_orbit_angles() {
        let epoch = Epoch::from_tdb_nanoseconds_since_j2000(1_000_000_000);
        let unrotated = orbit_angles_for_direction(globe_surface_direction_for_lon_lat(
            LonLat::new(-98.0, 39.0),
        ));
        let rotated =
            demo_globe_orbit_angles_for_surface(BodyId::EARTH, LonLat::new(-98.0, 39.0), epoch);

        assert!((rotated.1 - unrotated.1).abs() <= 1.0e-9);
        assert!((rotated.0 - unrotated.0).abs() <= 1.0e-9);
    }

    #[test]
    fn globe_camera_surface_override_is_body_fixed() {
        let mut state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
        let provider = QuarterTurnOrientationProvider;

        apply_demo_globe_camera_override_with_ephemeris(
            &mut state,
            BodyId::EARTH,
            DemoGlobeCameraOverride {
                surface_lon_lat: Some(LonLat::new(0.0, 0.0)),
                ..Default::default()
            },
            Epoch::J2000,
            Some(&provider),
        );

        assert!(state.yaw_deg.abs() <= 1.0e-5);
        assert!(state.pitch_deg.abs() <= 1.0e-5);
    }

    #[test]
    fn mapbox_token_filter_only_accepts_public_tokens() {
        assert_eq!(
            normalized_mapbox_public_token(" pk.public ".to_owned()),
            Some("pk.public".to_owned())
        );
        assert_eq!(normalized_mapbox_public_token("sk.secret".to_owned()), None);
        assert_eq!(normalized_mapbox_public_token(String::new()), None);
    }

    #[test]
    fn demo_mode_parser_accepts_2d_and_3d_aliases() {
        assert_eq!(parse_demo_mode("globe"), Some(DemoMode::Globe));
        assert_eq!(parse_demo_mode("3D"), Some(DemoMode::Globe));
        assert_eq!(parse_demo_mode("solar"), Some(DemoMode::SolarSystem));
        assert_eq!(parse_demo_mode("orrery"), Some(DemoMode::SolarSystem));
        assert_eq!(parse_demo_mode("map"), Some(DemoMode::Map));
        assert_eq!(parse_demo_mode("2d"), Some(DemoMode::Map));
        assert_eq!(parse_demo_mode("terrain"), None);
    }

    #[test]
    fn demo_focus_parser_accepts_named_bodies_and_shortcuts() {
        assert_eq!(parse_demo_focus("earth"), Some(BodyId::EARTH));
        assert_eq!(parse_demo_focus("1"), Some(BodyId::EARTH));
        assert_eq!(
            parse_demo_focus("mercury"),
            Some(BodyId::MERCURY_BARYCENTER)
        );
        assert_eq!(parse_demo_focus("venus"), Some(BodyId::VENUS_BARYCENTER));
        assert_eq!(parse_demo_focus("moon"), Some(BodyId::MOON));
        assert_eq!(parse_demo_focus("2"), Some(BodyId::MOON));
        assert_eq!(parse_demo_focus("mars"), Some(BodyId::MARS_BARYCENTER));
        assert_eq!(parse_demo_focus("3"), Some(BodyId::MARS_BARYCENTER));
        assert_eq!(parse_demo_focus("europa"), Some(BodyId::EUROPA));
        assert_eq!(parse_demo_focus("ceres"), Some(BodyId::CERES));
        assert_eq!(parse_demo_focus("sun"), None);
        assert_eq!(parse_demo_focus("phobos"), None);
    }

    #[test]
    fn demo_globe_focus_uses_metric_scene_focus_targets() {
        assert_eq!(
            demo_focus_metric_target(BodyId::MOON),
            MetricSceneFocusTarget::Object(BodyId::MOON)
        );
        assert_eq!(
            demo_focus_metric_target(BodyId::MARS_BARYCENTER),
            MetricSceneFocusTarget::Object(BodyId::MARS_BARYCENTER)
        );
        assert_eq!(
            demo_focus_metric_target(BodyId::EUROPA),
            MetricSceneFocusTarget::Object(BodyId::EUROPA)
        );
    }

    #[test]
    fn demo_map_body_parser_accepts_supported_projected_bodies() {
        assert_eq!(parse_demo_map_body("earth"), Some(DemoMapBody::Earth));
        assert_eq!(
            parse_demo_map_body("mercury"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Mercury))
        );
        assert_eq!(
            parse_demo_map_body("venus"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Venus))
        );
        assert_eq!(
            parse_demo_map_body("moon"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Moon))
        );
        assert_eq!(
            parse_demo_map_body("mars"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Mars))
        );
        assert_eq!(
            parse_demo_map_body("europa"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Europa))
        );
        assert_eq!(
            parse_demo_map_body("ceres"),
            Some(DemoMapBody::NasaTrek(NasaTrekRegularBody::Ceres))
        );
        assert_eq!(parse_demo_map_body("phobos"), None);
        assert_eq!(parse_demo_map_body("vesta"), None);
    }

    #[test]
    fn solar_focus_parser_accepts_scene_and_planet_targets() {
        assert_eq!(
            parse_solar_focus_target("scene"),
            Some(DemoSolarFocusTarget::Scene)
        );
        assert_eq!(
            parse_solar_focus_target("sun"),
            Some(DemoSolarFocusTarget::Sun)
        );
        assert_eq!(
            parse_solar_focus_target("mercury"),
            Some(DemoSolarFocusTarget::Mercury)
        );
        assert_eq!(
            parse_solar_focus_target("venus"),
            Some(DemoSolarFocusTarget::Venus)
        );
        assert_eq!(
            parse_solar_focus_target("earth"),
            Some(DemoSolarFocusTarget::Earth)
        );
        assert_eq!(
            parse_solar_focus_target("moon"),
            Some(DemoSolarFocusTarget::Moon)
        );
        assert_eq!(
            parse_solar_focus_target("mars"),
            Some(DemoSolarFocusTarget::Mars)
        );
        assert_eq!(parse_solar_focus_target("geo"), None);
        assert_eq!(parse_solar_focus_target("pluto"), None);
    }

    #[test]
    fn solar_focus_targets_map_to_ephemeris_bodies() {
        assert_eq!(solar_focus_body(DemoSolarFocusTarget::Scene), None);
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Sun),
            Some(BodyId::SUN)
        );
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Mercury),
            Some(BodyId::MERCURY_BARYCENTER)
        );
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Venus),
            Some(BodyId::VENUS_BARYCENTER)
        );
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Earth),
            Some(BodyId::EARTH)
        );
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Moon),
            Some(BodyId::MOON)
        );
        assert_eq!(
            solar_focus_body(DemoSolarFocusTarget::Mars),
            Some(BodyId::MARS_BARYCENTER)
        );
    }

    #[test]
    fn solar_focus_targets_convert_to_metric_scene_focus_targets() {
        assert_eq!(
            solar_focus_metric_target(DemoSolarFocusTarget::Scene),
            MetricSceneFocusTarget::Scene
        );
        assert_eq!(
            solar_focus_metric_target(DemoSolarFocusTarget::Earth),
            MetricSceneFocusTarget::Object(BodyId::EARTH)
        );
    }

    #[test]
    fn solar_demo_epoch_is_fixed_at_j2000() {
        assert_eq!(solar_demo_epoch(), Epoch::J2000);
    }

    #[test]
    fn solar_planet_focus_does_not_chase_live_ephemeris_jitter() {
        assert!(!solar_focus_tracks_live_pivot(DemoSolarFocusTarget::Earth));
        assert!(!solar_focus_tracks_live_pivot(DemoSolarFocusTarget::Moon));
        assert!(!solar_focus_tracks_live_pivot(DemoSolarFocusTarget::Mars));
    }

    #[test]
    fn solar_planet_focus_recenters_after_large_ephemeris_correction() {
        let target = DemoSolarFocusTarget::Earth;
        let threshold = solar_focus_recenter_threshold_units(target);
        let camera_state =
            MetricOrbitCameraState::new(DVec3::ZERO, 0.0, 0.0, solar_focus_distance_units(target));
        let small_correction = MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            DVec3::X * threshold * 0.5,
            None,
        );
        let large_correction = MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            DVec3::X * threshold * 2.0,
            None,
        );

        assert!(!solar_focus_pivot_needs_recenter(
            camera_state,
            &small_correction,
            target
        ));
        assert!(solar_focus_pivot_needs_recenter(
            camera_state,
            &large_correction,
            target
        ));
    }

    #[test]
    fn solar_planet_focus_does_not_recenter_user_pan() {
        let target = DemoSolarFocusTarget::Earth;
        let threshold = solar_focus_recenter_threshold_units(target);
        let camera_state = MetricOrbitCameraState::new(
            DVec3::X * threshold * 2.0,
            0.0,
            0.0,
            solar_focus_distance_units(target),
        );
        let focus_pivot = MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            DVec3::ZERO,
            None,
        );

        assert!(solar_focus_should_recenter_camera(
            camera_state,
            &focus_pivot,
            target,
            false,
            false,
        ));
        assert!(!solar_focus_should_recenter_camera(
            camera_state,
            &focus_pivot,
            target,
            false,
            true,
        ));
    }

    #[test]
    fn solar_planet_focus_starts_on_the_sunlit_side() {
        let body_position = DVec3::new(149.0, 0.0, 22.0);
        let angles = solar_day_side_focus_angles(body_position, DemoSolarFocusTarget::Earth);
        assert!(
            angles.is_some(),
            "Earth focus should prefer a day-side camera angle"
        );
        let (yaw_deg, pitch_deg) = angles.unwrap_or((0.0, 0.0));
        let state = MetricOrbitCameraState::new(body_position, yaw_deg, pitch_deg, 1.0);
        let camera_direction = (state.position_units() - body_position).normalize();
        let sun_direction = (-body_position).normalize();

        assert!(
            camera_direction.dot(sun_direction) > 0.999,
            "camera should start on the sunward side for focused planet views"
        );
        assert!(solar_day_side_focus_angles(body_position, DemoSolarFocusTarget::Scene).is_none());
    }

    #[test]
    fn solar_close_body_focus_hides_overview_aids() {
        assert!(!solar_focus_hides_overview_aids(
            DemoSolarFocusTarget::Scene
        ));
        assert!(!solar_focus_hides_overview_aids(DemoSolarFocusTarget::Sun));
        assert!(solar_focus_hides_overview_aids(DemoSolarFocusTarget::Venus));
        assert!(solar_focus_hides_overview_aids(DemoSolarFocusTarget::Earth));
        assert!(solar_focus_hides_overview_aids(DemoSolarFocusTarget::Moon));
        assert!(solar_focus_hides_overview_aids(DemoSolarFocusTarget::Mars));
    }

    #[test]
    fn solar_focus_distance_zooms_into_body_inspection_scale() {
        assert_close(
            solar_focus_distance_units(DemoSolarFocusTarget::Scene),
            solar_scene_focus_distance_units(),
        );
        assert_close(
            solar_focus_distance_units(DemoSolarFocusTarget::Sun),
            f64::from(solar_body_radius_units(&CelestialBody::sun()))
                * SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR,
        );
        assert_close(
            solar_focus_distance_units(DemoSolarFocusTarget::Venus),
            f64::from(solar_body_radius_units(&demo_venus_body()))
                * SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR,
        );
        assert_close(
            solar_focus_distance_units(DemoSolarFocusTarget::Earth),
            solar_dynamic_body_focus_distance_units(DemoSolarFocusTarget::Earth)
                .unwrap_or(SOLAR_BODY_FOCUS_DISTANCE_UNITS),
        );
        assert_close(
            solar_focus_distance_units(DemoSolarFocusTarget::Moon),
            solar_dynamic_body_focus_distance_units(DemoSolarFocusTarget::Moon)
                .unwrap_or(SOLAR_BODY_FOCUS_DISTANCE_UNITS),
        );
    }

    #[test]
    fn solar_dynamic_body_focus_uses_physical_radius_scale() {
        let earth_radius =
            solar_dynamic_body_radius_units(DemoSolarFocusTarget::Earth).unwrap_or_default();
        let moon_radius =
            solar_dynamic_body_radius_units(DemoSolarFocusTarget::Moon).unwrap_or_default();

        assert!(earth_radius > moon_radius);
        assert!(moon_radius > 0.0);
        assert_close(
            solar_dynamic_body_focus_distance_units(DemoSolarFocusTarget::Earth)
                .unwrap_or_default(),
            f64::from(earth_radius) * SOLAR_DYNAMIC_BODY_FOCUS_DISTANCE_RADIUS_FACTOR,
        );
        assert_close(
            solar_camera_min_distance_units(DemoSolarFocusTarget::Earth),
            f64::from(earth_radius) * 1.05,
        );
        let base_surface_distance =
            f64::from(earth_radius) * SOLAR_DYNAMIC_BODY_BASE_SURFACE_DISTANCE_RADIUS_FACTOR;
        assert!(
            base_surface_distance
                < solar_dynamic_body_focus_distance_units(DemoSolarFocusTarget::Earth)
                    .unwrap_or_default()
        );
        assert!(solar_dynamic_body_max_selected_tiles() >= 512);
        let venus_radius =
            solar_dynamic_body_radius_units(DemoSolarFocusTarget::Venus).unwrap_or_default();
        assert!(venus_radius > moon_radius);
        assert_close(
            solar_camera_min_distance_units(DemoSolarFocusTarget::Venus),
            f64::from(venus_radius) * 1.05,
        );
    }

    #[test]
    fn solar_dynamic_tile_source_matches_close_body_focus() {
        let active_earth_source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png")
            .with_body(PlanetaryBody::earth());

        assert_eq!(
            solar_dynamic_tile_source(DemoSolarFocusTarget::Earth, &active_earth_source),
            Some(active_earth_source.clone())
        );
        assert_eq!(
            solar_dynamic_tile_source(DemoSolarFocusTarget::Moon, &active_earth_source)
                .map(|source| source.body),
            Some(PlanetaryBody::moon())
        );
        assert_eq!(
            solar_dynamic_tile_source(DemoSolarFocusTarget::Mars, &active_earth_source)
                .map(|source| source.body),
            Some(PlanetaryBody::mars())
        );
        assert_eq!(
            solar_dynamic_tile_source(DemoSolarFocusTarget::Mercury, &active_earth_source)
                .map(|source| source.body),
            Some(PlanetaryBody::mercury())
        );
        assert_eq!(
            solar_dynamic_tile_source(DemoSolarFocusTarget::Venus, &active_earth_source),
            Some(TileSource::nasa_trek_regular_body(
                NasaTrekRegularBody::Venus
            ))
        );
    }

    #[test]
    fn solar_dynamic_body_tile_zoom_reuses_body_surface_defaults() {
        assert_eq!(
            solar_dynamic_body_base_tile_zoom(DemoSolarFocusTarget::Earth),
            SOLAR_EARTH_TILE_ZOOM
        );
        assert_eq!(
            solar_dynamic_body_base_tile_zoom(DemoSolarFocusTarget::Moon),
            SOLAR_MOON_TILE_ZOOM
        );
        assert_eq!(
            solar_dynamic_body_base_tile_zoom(DemoSolarFocusTarget::Mars),
            SOLAR_MARS_TILE_ZOOM
        );
        assert_eq!(
            solar_dynamic_body_base_tile_zoom(DemoSolarFocusTarget::Venus),
            SOLAR_VENUS_TILE_ZOOM
        );
    }

    #[test]
    fn solar_body_lod_policy_keeps_surface_transition_at_body_focus_distance() {
        let earth = CelestialBody::earth();
        let policy = solar_body_lod_policy(&earth);
        let surface_transition_distance =
            f64::from(solar_body_radius_units(&earth)) * SOLAR_SURFACE_LOD_DISTANCE_RADIUS_FACTOR;

        assert_eq!(
            policy.presentation_for_distance_units(SOLAR_BODY_FOCUS_DISTANCE_UNITS * 4.0),
            MetricScenePresentation::Marker
        );
        assert_eq!(
            policy.presentation_for_distance_units(surface_transition_distance * 2.0),
            MetricScenePresentation::Body
        );
        assert_eq!(
            policy.presentation_for_distance_units(surface_transition_distance * 0.5),
            MetricScenePresentation::Surface
        );
        assert_eq!(
            policy.presentation_for_distance_units(solar_focus_distance_units(
                DemoSolarFocusTarget::Earth
            )),
            MetricScenePresentation::Body
        );
    }

    #[test]
    fn solar_surface_radius_policy_uses_true_body_radius() {
        let policy = solar_true_radius_policy();

        assert_close_f32(policy.minimum_visible_radius_units, 0.0);
        assert_eq!(policy.maximum_visible_radius_units, None);
    }

    #[test]
    fn globe_ambient_fill_lifts_the_dark_side_without_flattening_sunlight() {
        let ambient = demo_globe_ambient_fill_light();

        assert_cool_space_fill_light(&ambient, 240.0, 400.0);
    }

    #[test]
    fn demo_skybox_load_is_deferred_past_the_first_render_frames() {
        let skybox = demo_globe_skybox_config();
        let deferred = DeferredGlobeSkybox::new(skybox.clone());

        assert_eq!(deferred.skybox(), &skybox);
        assert_eq!(
            deferred.frames_remaining(),
            DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES
        );
    }

    #[test]
    fn solar_skybox_reuses_the_globe_asset_with_lower_brightness() {
        let globe = demo_globe_skybox_config();
        let solar = demo_solar_skybox_config();

        assert_eq!(solar.image_path, globe.image_path);
        assert!(solar.brightness < globe.brightness);
    }

    #[test]
    fn solar_presentation_lighting_is_bright_enough_for_planet_readability() {
        let one_au_units = SOLAR_AU_KM / SOLAR_RENDER_KILOMETERS_PER_UNIT;
        let mars_distance_units = one_au_units * 1.6;
        let ambient = demo_solar_ambient_fill_light();

        assert_cool_space_fill_light(&ambient, 280.0, 440.0);
        assert!(f64::from(SOLAR_SUN_POINT_LIGHT_RANGE_UNITS) > mars_distance_units);
        assert!(solar_sun_light_illuminance_at_distance_units(one_au_units) >= 30_000.0);
        assert!(solar_sun_light_illuminance_at_distance_units(mars_distance_units) >= 10_000.0);
    }

    #[test]
    fn demo_fallback_composes_moon_states_relative_to_sun() {
        let request = StateRequest::new(BodyId::MOON, BodyId::SUN, FrameId::j2000(), Epoch::J2000);
        let state = demo_fallback_state(&request).ok();
        assert!(state.is_some());
        let state = state.unwrap_or_else(|| {
            StateVector::relative_to(
                request.target,
                request.origin,
                request.frame,
                request.epoch,
                Vec3d::ZERO,
                Vec3d::ZERO,
            )
        });

        assert_eq!(state.target, BodyId::MOON);
        assert_eq!(state.origin, BodyId::SUN);
        assert_eq!(state.frame, FrameId::j2000());
        assert!(state.position_km.x.is_finite());
        assert!(state.position_km.y.is_finite());
        assert!(state.position_km.z.is_finite());
        let radius_km = (state.position_km.x.powi(2)
            + state.position_km.y.powi(2)
            + state.position_km.z.powi(2))
        .sqrt();
        assert!(radius_km > 0.9 * SOLAR_AU_KM);
    }

    #[test]
    fn demo_earth_tile_source_parser_accepts_mapbox_styles() {
        assert_eq!(
            parse_demo_earth_tile_source("nasa-blue-marble"),
            Some(DemoEarthTileSource::NasaBlueMarble)
        );
        assert_eq!(
            parse_demo_earth_tile_source("mapbox-satellite"),
            Some(DemoEarthTileSource::MapboxSatellite)
        );
        assert_eq!(
            parse_demo_earth_tile_source("streets"),
            Some(DemoEarthTileSource::MapboxStreets)
        );
        assert_eq!(
            parse_demo_earth_tile_source("satellite-streets"),
            Some(DemoEarthTileSource::MapboxSatelliteStreets)
        );
        assert_eq!(parse_demo_earth_tile_source("auto"), None);
    }

    #[test]
    fn demo_earth_tile_source_uses_selected_mapbox_style_with_token() {
        let source = demo_earth_tile_source(
            DemoMode::Globe,
            Some(DemoEarthTileSource::MapboxStreets),
            Some("pk.test"),
        );

        assert_eq!(
            source.tile_url(TileId::new(4, 6, 9)),
            "https://api.mapbox.com/styles/v1/mapbox/streets-v12/tiles/256/4/6/9@2x?access_token=pk.test"
        );
    }

    #[test]
    fn demo_earth_tile_source_falls_back_without_mapbox_token() {
        let source = demo_earth_tile_source(
            DemoMode::Globe,
            Some(DemoEarthTileSource::MapboxSatellite),
            None,
        );

        assert!(source
            .tile_url(TileId::new(1, 0, 0))
            .contains("BlueMarble_NextGeneration"));
    }

    #[test]
    fn demo_map_tile_source_uses_body_specific_projection_sources() {
        let earth = demo_map_tile_source(DemoMapBody::Earth, None, None);
        assert_eq!(earth.body, PlanetaryBody::earth());
        assert_eq!(earth.projection, TileProjection::WebMercator);

        let moon =
            demo_map_tile_source(DemoMapBody::NasaTrek(NasaTrekRegularBody::Moon), None, None);
        assert_eq!(moon.body, PlanetaryBody::moon());
        assert_eq!(moon.projection, TileProjection::Equirectangular);

        let mars =
            demo_map_tile_source(DemoMapBody::NasaTrek(NasaTrekRegularBody::Mars), None, None);
        assert_eq!(mars.body, PlanetaryBody::mars());
        assert_eq!(mars.projection, TileProjection::Equirectangular);

        let venus = demo_map_tile_source(
            DemoMapBody::NasaTrek(NasaTrekRegularBody::Venus),
            None,
            None,
        );
        assert_eq!(venus.body, PlanetaryBody::venus());
        assert_eq!(venus.projection, TileProjection::Equirectangular);

        let europa = demo_map_tile_source(
            DemoMapBody::NasaTrek(NasaTrekRegularBody::Europa),
            None,
            None,
        );
        assert_eq!(europa.body, PlanetaryBody::europa());
        assert_eq!(europa.projection, TileProjection::Equirectangular);
    }

    #[test]
    fn globe_secondary_surfaces_use_shared_body_surface_specs() {
        let moon = demo_globe_tiled_body_surface(BodyId::MOON);
        let mars = demo_globe_tiled_body_surface(BodyId::MARS_BARYCENTER);
        let venus = demo_globe_tiled_body_surface(BodyId::VENUS_BARYCENTER);
        let europa = demo_globe_tiled_body_surface(BodyId::EUROPA);
        let ceres = demo_globe_tiled_body_surface(BodyId::CERES);
        let earth = demo_globe_tiled_body_surface(BodyId::EARTH);

        assert!(moon.is_some());
        let (moon_body, moon_surface) = moon.unwrap_or_else(|| {
            (
                CelestialBody::moon(),
                TiledBodySurface::fixed_zoom(TileSource::nasa_trek_moon_lro_wac(), 0),
            )
        });
        assert_eq!(moon_body.id, BodyId::MOON);
        assert_eq!(moon_surface.source.body, PlanetaryBody::moon());
        assert_eq!(
            moon_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(moon_surface.zoom, DEMO_MOON_TILE_ZOOM);

        assert!(mars.is_some());
        let (mars_body, mars_surface) = mars.unwrap_or_else(|| {
            (
                CelestialBody::mars(),
                TiledBodySurface::fixed_zoom(TileSource::nasa_trek_mars_viking_color(), 0),
            )
        });
        assert_eq!(mars_body.id, BodyId::MARS_BARYCENTER);
        assert_eq!(mars_surface.source.body, PlanetaryBody::mars());
        assert_eq!(
            mars_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(mars_surface.zoom, DEMO_MARS_TILE_ZOOM);

        assert!(venus.is_some());
        let (venus_body, venus_surface) = venus.unwrap_or_else(|| {
            (
                CelestialBody::venus(),
                TiledBodySurface::fixed_zoom(
                    TileSource::nasa_trek_venus_magellan_color_topography(),
                    0,
                ),
            )
        });
        assert_eq!(venus_body.id, BodyId::VENUS_BARYCENTER);
        assert_eq!(venus_surface.source.body, PlanetaryBody::venus());
        assert_eq!(
            venus_surface.zoom,
            demo_globe_tiled_body_zoom(NasaTrekRegularBody::Venus)
        );

        assert!(europa.is_some());
        let (europa_body, europa_surface) = europa.unwrap_or_else(|| {
            (
                CelestialBody::europa(),
                TiledBodySurface::fixed_zoom(
                    TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Europa),
                    0,
                ),
            )
        });
        assert_eq!(europa_body.id, BodyId::EUROPA);
        assert_eq!(europa_surface.source.body, PlanetaryBody::europa());
        assert_eq!(europa_surface.zoom, DEMO_REGULAR_BODY_TILE_ZOOM);

        assert!(ceres.is_some());
        let (ceres_body, ceres_surface) = ceres.unwrap_or_else(|| {
            (
                CelestialBody::ceres(),
                TiledBodySurface::fixed_zoom(
                    TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Ceres),
                    0,
                ),
            )
        });
        assert_eq!(ceres_body.id, BodyId::CERES);
        assert_eq!(ceres_surface.source.body, PlanetaryBody::ceres());
        assert_eq!(ceres_surface.zoom, DEMO_REGULAR_BODY_TILE_ZOOM);

        assert!(earth.is_none());
    }

    #[test]
    fn globe_secondary_lod_keeps_surface_visible_through_zoom_range() {
        let mars = demo_globe_tiled_body_surface(BodyId::MARS_BARYCENTER);
        assert!(mars.is_some());
        let (mars_body, _) = mars.unwrap_or_else(|| {
            (
                CelestialBody::mars(),
                TiledBodySurface::fixed_zoom(TileSource::nasa_trek_mars_viking_color(), 0),
            )
        });
        let radius = f64::from(body_radius_units(&mars_body));
        let policy = globe_secondary_body_lod_policy(&mars_body);
        let surface_lod_distance = radius * DEMO_GLOBE_SURFACE_LOD_DISTANCE_RADIUS_FACTOR;

        assert_eq!(
            policy.presentation_for_distance_units(radius * 3.0),
            MetricScenePresentation::Surface
        );
        assert_eq!(
            policy.presentation_for_distance_units(radius * 100.0),
            MetricScenePresentation::Surface
        );
        assert_eq!(
            policy.presentation_for_distance_units(surface_lod_distance * 1.01),
            MetricScenePresentation::Marker
        );

        let surface_presentations = demo_globe_surface_presentations();
        assert!(surface_presentations.contains(MetricScenePresentation::Surface));
        assert!(!surface_presentations.contains(MetricScenePresentation::Body));
    }

    #[test]
    fn globe_secondary_visibility_shows_only_selected_focus_body() {
        let mercury = demo_focus_body_id_for_trek_body(NasaTrekRegularBody::Mercury);
        let mars = demo_focus_body_id_for_trek_body(NasaTrekRegularBody::Mars);

        assert_eq!(mercury, BodyId::MERCURY_BARYCENTER);
        assert_eq!(mars, BodyId::MARS_BARYCENTER);
        assert_eq!(
            demo_secondary_globe_body_visibility(BodyId::MOON, BodyId::EARTH),
            Visibility::Hidden
        );
        assert_eq!(
            demo_secondary_globe_body_visibility(BodyId::IO, BodyId::IO),
            Visibility::Inherited
        );
        assert_eq!(
            demo_secondary_globe_body_visibility(BodyId::IO, mars),
            Visibility::Hidden
        );
        assert_eq!(
            demo_secondary_globe_body_visibility(mars, BodyId::MARS_BARYCENTER),
            Visibility::Inherited
        );
        assert_eq!(
            demo_secondary_globe_body_visibility(mercury, BodyId::MERCURY_BARYCENTER),
            Visibility::Inherited
        );
        assert_eq!(
            demo_secondary_globe_body_visibility(BodyId::MERCURY, BodyId::MERCURY_BARYCENTER),
            Visibility::Hidden
        );
    }

    #[test]
    fn solar_trail_month_parser_clamps_to_supported_range() {
        assert_eq!(parse_solar_trail_months("0"), Some(SOLAR_TRAIL_MIN_MONTHS));
        assert_eq!(
            parse_solar_trail_months("999"),
            Some(SOLAR_TRAIL_MAX_MONTHS)
        );
        assert_eq!(
            parse_solar_trail_months(&SOLAR_TRAIL_DEFAULT_MONTHS.to_string()),
            Some(SOLAR_TRAIL_DEFAULT_MONTHS)
        );
        assert_eq!(parse_solar_trail_months("months"), None);
    }

    #[test]
    fn solar_trail_start_epoch_builds_history_window() {
        let epoch = Epoch::from_tdb_nanoseconds_since_j2000(100_000_000_000_000);
        let start = solar_trail_start_epoch(epoch, 1);

        let delta_nanos = epoch.tdb_nanoseconds_since_j2000 - start.tdb_nanoseconds_since_j2000;
        assert!(delta_nanos > 2_600_000_i128 * 1_000_000_000);
        assert!(delta_nanos < 2_700_000_i128 * 1_000_000_000);
    }

    #[test]
    fn solar_tiled_body_surface_specs_use_expected_sources_and_radius_policies() {
        let active_earth_source =
            TileSource::mapbox_style_raster("mapbox", "satellite-v9", "pk.test");

        let (mercury_surface, mercury_policy) =
            solar_tiled_body_surface(DemoSolarTexturedBody::Mercury, &active_earth_source);
        assert_eq!(mercury_surface.source.body, PlanetaryBody::mercury());
        assert_eq!(
            mercury_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(mercury_surface.zoom, SOLAR_MERCURY_TILE_ZOOM);
        assert_close_f32(mercury_policy.0.minimum_visible_radius_units, 0.0);

        let (earth_surface, earth_policy) =
            solar_tiled_body_surface(DemoSolarTexturedBody::Earth, &active_earth_source);
        assert_eq!(earth_surface.source, active_earth_source);
        assert_eq!(earth_surface.zoom, SOLAR_EARTH_TILE_ZOOM);
        assert_close_f32(earth_policy.0.minimum_visible_radius_units, 0.0);

        let (venus_surface, venus_policy) =
            solar_tiled_body_surface(DemoSolarTexturedBody::Venus, &active_earth_source);
        assert_eq!(venus_surface.source.body, PlanetaryBody::venus());
        assert_eq!(
            venus_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(venus_surface.zoom, SOLAR_VENUS_TILE_ZOOM);
        assert_close_f32(venus_policy.0.minimum_visible_radius_units, 0.0);

        let (moon_surface, moon_policy) =
            solar_tiled_body_surface(DemoSolarTexturedBody::Moon, &active_earth_source);
        assert_eq!(moon_surface.source.body, PlanetaryBody::moon());
        assert_eq!(
            moon_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(moon_surface.zoom, SOLAR_MOON_TILE_ZOOM);
        assert_close_f32(moon_policy.0.minimum_visible_radius_units, 0.0);

        let (mars_surface, mars_policy) =
            solar_tiled_body_surface(DemoSolarTexturedBody::Mars, &active_earth_source);
        assert_eq!(mars_surface.source.body, PlanetaryBody::mars());
        assert_eq!(
            mars_surface.source.projection,
            TileProjection::Equirectangular
        );
        assert_eq!(mars_surface.zoom, SOLAR_MARS_TILE_ZOOM);
        assert_close_f32(mars_policy.0.minimum_visible_radius_units, 0.0);
    }

    #[test]
    fn solar_tiled_body_fallback_colors_keep_true_radius_bodies_visible() {
        assert_ne!(
            solar_tiled_body_fallback_color(DemoSolarTexturedBody::Earth),
            Color::BLACK
        );
        assert_ne!(
            solar_tiled_body_fallback_color(DemoSolarTexturedBody::Moon),
            Color::BLACK
        );
        assert_ne!(
            solar_tiled_body_fallback_color(DemoSolarTexturedBody::Venus),
            Color::BLACK
        );
        assert_ne!(
            solar_tiled_body_fallback_color(DemoSolarTexturedBody::Mars),
            Color::BLACK
        );
    }

    #[test]
    fn solar_paths_resample_only_when_inputs_become_stale() {
        let epoch = Epoch::from_tdb_nanoseconds_since_j2000(50_000_000_000_000);
        let celestial_epoch = ferrisium_bevy::prelude::CelestialEpoch::new(epoch);
        let trail_window = DemoSolarTrailWindow::new(SOLAR_TRAIL_DEFAULT_MONTHS);
        let mut paths = DemoSolarOrbitPathEntities::default();

        assert!(solar_paths_need_resample(
            &paths,
            celestial_epoch,
            trail_window,
            false
        ));

        paths.initialized = true;
        paths.trail_months = Some(SOLAR_TRAIL_DEFAULT_MONTHS);
        paths.sampled_epoch = Some(epoch);
        assert!(!solar_paths_need_resample(
            &paths,
            celestial_epoch,
            trail_window,
            false
        ));
        assert!(solar_paths_need_resample(
            &paths,
            celestial_epoch,
            DemoSolarTrailWindow::new(SOLAR_TRAIL_DEFAULT_MONTHS + 1),
            false
        ));
        assert!(solar_paths_need_resample(
            &paths,
            celestial_epoch,
            trail_window,
            true
        ));

        assert_close(SOLAR_TRAIL_RESAMPLE_SECONDS, 86_400.0);
        let stale_nanos = 86_400_i128 * 1_000_000_000;
        paths.sampled_epoch = Some(Epoch::from_tdb_nanoseconds_since_j2000(
            epoch.tdb_nanoseconds_since_j2000 - stale_nanos,
        ));
        assert!(solar_paths_need_resample(
            &paths,
            celestial_epoch,
            trail_window,
            false
        ));
    }

    #[test]
    fn solar_reference_ring_path_is_closed_in_the_metric_plane() {
        let radius_km = SOLAR_AU_KM;
        let path = solar_reference_ring_path(radius_km, 8, Epoch::J2000);

        assert_eq!(path.samples.len(), 9);
        let first = &path.samples[0];
        let last = &path.samples[path.samples.len() - 1];
        assert!((first.position_km.x - last.position_km.x).abs() < 1.0e-3);
        assert_close(first.position_km.y, 0.0);
        assert_close(last.position_km.y, 0.0);
        assert_close(first.position_km.z, radius_km);
        assert_close(last.position_km.z, radius_km);
    }

    #[test]
    fn solar_reference_spoke_path_spans_the_grid_diameter() {
        let path = solar_reference_spoke_path(2.0 * SOLAR_AU_KM, 0.0, Epoch::J2000);

        assert_eq!(path.samples.len(), 2);
        let start = path.samples[0].position_km;
        let end = path.samples[1].position_km;
        assert_close(start.x + end.x, 0.0);
        assert_close(start.z + end.z, 0.0);
        assert_close((end.z - start.z).abs(), 4.0 * SOLAR_AU_KM);
    }

    #[test]
    fn solar_trail_sample_count_is_browser_safe_and_period_aware() {
        assert_eq!(
            solar_trail_sample_count(SOLAR_TRAIL_MIN_MONTHS, 1.0e15),
            SOLAR_TRAIL_MIN_SAMPLES
        );
        assert_eq!(
            solar_trail_sample_count(SOLAR_TRAIL_MAX_MONTHS, 1.0),
            SOLAR_TRAIL_MAX_SAMPLES
        );
        assert!(
            solar_trail_sample_count(SOLAR_TRAIL_DEFAULT_MONTHS, 31_557_600.0)
                > SOLAR_TRAIL_MIN_SAMPLES
        );
    }

    #[test]
    fn demo_html_scopes_view_specific_controls_to_their_modes() {
        let html = include_str!("../index.html");
        let solar_focus_select = html_select_block(html, "solar-focus").unwrap_or_default();

        assert!(html.contains("[hidden]"));
        assert!(html.contains("width: min(320px, calc(100vw - 32px))"));
        assert!(html.contains("globeControls.hidden = viewSelect.value !== \"globe\""));
        assert!(html.contains("mapControls.hidden = viewSelect.value !== \"map\""));
        assert!(html.contains("solarControls.hidden = viewSelect.value !== \"solar\""));
        assert!(html.contains("solarMetricNote.hidden = viewSelect.value !== \"solar\""));
        assert!(html.contains("Epoch: J2000 (fixed)"));
        assert!(html.contains("ferrisium.globe_focus"));
        assert!(html.contains("id=\"globe-focus\""));
        assert!(html.contains("<option value=\"mercury\">Mercury</option>"));
        assert!(html.contains("<option value=\"europa\">Europa</option>"));
        assert!(html.contains("<option value=\"ceres\">Ceres</option>"));
        assert!(html.contains("ferrisium.map_body"));
        assert!(html.contains("id=\"map-body\""));
        assert!(html.contains("<option value=\"mercury\">Mercury</option>"));
        assert!(html.contains("<option value=\"venus\">Venus</option>"));
        assert!(html.contains("<option value=\"moon\">Moon</option>"));
        assert!(html.contains("<option value=\"europa\">Europa</option>"));
        assert!(html.contains("<option value=\"ceres\">Ceres</option>"));
        assert!(html.contains("nextUrl.searchParams.set(\"map_body\", mapBody.value)"));
        assert!(html.contains("positions, paths, and body radii are metric"));
        assert!(!html.contains("id=\"solar-body-scale\""));
        assert!(html.contains("ferrisium.solar_focus"));
        assert!(html.contains("id=\"solar-focus\""));
        assert!(html.contains("nextUrl.searchParams.set(\"solar_focus\", solarFocus.value)"));
        assert!(html.contains("nextUrl.searchParams.delete(\"focus\")"));
        assert!(solar_focus_select.contains("<option value=\"mars\">Mars</option>"));
        assert!(html.contains("function storageGet(key)"));
        assert!(html.contains("searchParams.get(\"solar_focus\")"));
        assert!(html.contains("storageGet(solarFocusKey)"));
    }
}

#[cfg(target_arch = "wasm32")]
fn primary_window() -> Window {
    Window {
        title: "Ferrisium Demo".into(),
        resolution: (1280, 720).into(),
        resizable: true,
        canvas: Some("#bevy".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}
