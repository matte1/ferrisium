use ferrisium_core::prelude::{
    BodyId, CelestialBody, EphemerisError, EphemerisProvider, Epoch, FrameId, FrameOrientation,
    Kilometers, NasaTrekRegularBody, OrientationRequest, QuatD, ReferenceFrame, ReferenceFrameKind,
    RenderScale, StateRequest, StateVector, TrajectoryPath, TrajectoryTrailConfig, Vec3d,
    EARTH_MEAN_RADIUS_METERS, METERS_PER_KILOMETER, WGS84_EQUATORIAL_RADIUS_KILOMETERS,
    WGS84_INVERSE_FLATTENING,
};
use ferrisium_core::space::epoch_delta_seconds;

fn assert_close(lhs: f64, rhs: f64) {
    assert!(
        (lhs - rhs).abs() <= f64::EPSILON,
        "float mismatch: lhs={lhs}, rhs={rhs}"
    );
}

#[test]
fn kilometers_convert_to_and_from_meters() {
    let earth_radius = Kilometers::from_meters(f64::from(EARTH_MEAN_RADIUS_METERS));

    assert_close(earth_radius.as_f64(), 6_371.008);
    assert_close(
        earth_radius.as_meters(),
        f64::from(EARTH_MEAN_RADIUS_METERS),
    );
    assert_close(METERS_PER_KILOMETER, 1_000.0);
}

#[test]
fn body_ids_use_naif_compatible_identifiers() {
    assert_eq!(BodyId::SOLAR_SYSTEM_BARYCENTER.naif_id, 0);
    assert_eq!(BodyId::SUN.naif_id, 10);
    assert_eq!(BodyId::EARTH.naif_id, 399);
    assert_eq!(BodyId::MOON.naif_id, 301);
    assert_eq!(BodyId::MARS_BARYCENTER.naif_id, 4);
    assert_eq!(BodyId::MARS.naif_id, 499);
    assert_eq!(BodyId::IO.naif_id, 501);
    assert_eq!(BodyId::EUROPA.naif_id, 502);
    assert_eq!(BodyId::TITAN.naif_id, 606);
    assert_eq!(BodyId::CERES.naif_id, 2_000_001);
}

#[test]
fn built_in_body_metadata_uses_expected_radii_and_frames() {
    let earth = CelestialBody::earth();
    let moon = CelestialBody::moon();
    let mars = CelestialBody::mars();
    let europa = CelestialBody::europa();
    let ceres = CelestialBody::nasa_trek_regular_body(NasaTrekRegularBody::Ceres);

    assert_eq!(earth.id, BodyId::EARTH);
    assert_close(earth.mean_radius_km.as_f64(), 6_371.008);
    assert_close(
        earth.reference_ellipsoid.equatorial_radius_km.as_f64(),
        WGS84_EQUATORIAL_RADIUS_KILOMETERS,
    );
    assert_close(
        earth.reference_ellipsoid.flattening,
        WGS84_INVERSE_FLATTENING.recip(),
    );
    assert_eq!(earth.inertial_frame, FrameId::j2000());
    assert_eq!(earth.body_fixed_frame, FrameId::itrf93());
    assert_close(
        moon.reference_ellipsoid.equatorial_radius_km.as_f64(),
        moon.mean_radius_km.as_f64(),
    );
    assert_eq!(moon.body_fixed_frame, FrameId::moon_me_de440());
    assert_eq!(mars.body_fixed_frame, FrameId::iau_mars());
    assert_eq!(europa.id, BodyId::EUROPA);
    assert_close(europa.mean_radius_km.as_f64(), 1_560.8);
    assert_eq!(europa.body_fixed_frame, FrameId::iau_europa());
    assert_eq!(ceres.id, BodyId::CERES);
    assert_close(ceres.mean_radius_km.as_f64(), 469.7);
    assert_eq!(ceres.body_fixed_frame, FrameId::iau_ceres());
}

#[test]
fn geodetic_position_uses_ferrisium_body_fixed_axes() {
    let earth = CelestialBody::earth();
    let equator_prime_meridian = earth.geodetic_position_km(0.0, 0.0, 0.0);
    let equator_east = earth.geodetic_position_km(0.0, 90.0, 0.0);
    let north_pole = earth.geodetic_position_km(90.0, 42.0, 0.0);
    let expected_polar_radius =
        WGS84_EQUATORIAL_RADIUS_KILOMETERS * (1.0 - WGS84_INVERSE_FLATTENING.recip());

    assert!(equator_prime_meridian.x.abs() <= 1.0e-9);
    assert!(equator_prime_meridian.y.abs() <= 1.0e-9);
    assert_close(equator_prime_meridian.z, WGS84_EQUATORIAL_RADIUS_KILOMETERS);
    assert_close(equator_east.x, WGS84_EQUATORIAL_RADIUS_KILOMETERS);
    assert!(equator_east.y.abs() <= 1.0e-9);
    assert!(equator_east.z.abs() <= 1.0e-9);
    assert!(north_pole.x.abs() <= 1.0e-9);
    assert_close(north_pole.y, expected_polar_radius);
    assert!(north_pole.z.abs() <= 1.0e-9);
}

#[test]
fn reference_frames_record_center_and_kind() {
    let inertial = ReferenceFrame::inertial(FrameId::j2000(), BodyId::SOLAR_SYSTEM_BARYCENTER);
    let fixed = ReferenceFrame::body_fixed(FrameId::iau_earth(), BodyId::EARTH);
    let local = ReferenceFrame::local_tangent(FrameId::named("ENU_DEMO"), BodyId::EARTH);

    assert_eq!(inertial.kind, ReferenceFrameKind::Inertial);
    assert_eq!(inertial.center, BodyId::SOLAR_SYSTEM_BARYCENTER);
    assert_eq!(fixed.kind, ReferenceFrameKind::BodyFixed);
    assert_eq!(fixed.center, BodyId::EARTH);
    assert_eq!(local.kind, ReferenceFrameKind::LocalTangent);
}

#[test]
fn epoch_defaults_to_j2000_tdb_nanoseconds() {
    assert_eq!(Epoch::default(), Epoch::J2000);
    assert_eq!(
        Epoch::from_tdb_nanoseconds_since_j2000(42).tdb_nanoseconds_since_j2000,
        42
    );
}

#[test]
fn epoch_delta_seconds_handles_extreme_public_epoch_values_without_overflow() {
    let start = Epoch::from_tdb_nanoseconds_since_j2000(i128::MIN);
    let end = Epoch::from_tdb_nanoseconds_since_j2000(i128::MAX);
    let seconds = epoch_delta_seconds(start, end);

    assert!(seconds.is_finite());
    assert!(seconds > 0.0);
}

#[test]
fn state_vectors_keep_origin_frame_and_epoch_explicit() {
    let state = StateVector::relative_to(
        BodyId::MOON,
        BodyId::EARTH,
        FrameId::j2000(),
        Epoch::J2000,
        Vec3d::new(384_400.0, 0.0, 0.0),
        Vec3d::new(0.0, 1.022, 0.0),
    );

    assert_eq!(state.target, BodyId::MOON);
    assert_eq!(state.origin, BodyId::EARTH);
    assert_eq!(state.frame, FrameId::j2000());
    assert_eq!(state.epoch, Epoch::J2000);
    assert_close(state.position_km.x, 384_400.0);
    assert_close(state.velocity_km_s.y, 1.022);
}

#[test]
fn state_requests_describe_target_origin_frame_and_epoch() {
    let request = StateRequest::new(BodyId::MOON, BodyId::EARTH, FrameId::j2000(), Epoch::J2000);

    assert_eq!(request.target, BodyId::MOON);
    assert_eq!(request.origin, BodyId::EARTH);
    assert_eq!(request.frame, FrameId::j2000());
    assert_eq!(request.epoch, Epoch::J2000);
}

#[test]
fn trajectory_paths_preserve_authoritative_state_samples() {
    let start_epoch = Epoch::J2000;
    let end_epoch = Epoch::from_tdb_nanoseconds_since_j2000(1_000_000_000);
    let samples = vec![
        StateVector::relative_to(
            BodyId::MOON,
            BodyId::EARTH,
            FrameId::j2000(),
            start_epoch,
            Vec3d::new(384_400.0, 0.0, 0.0),
            Vec3d::ZERO,
        ),
        StateVector::relative_to(
            BodyId::MOON,
            BodyId::EARTH,
            FrameId::j2000(),
            end_epoch,
            Vec3d::new(384_399.0, 1.0, 0.0),
            Vec3d::new(0.0, 1.022, 0.0),
        ),
    ];

    let path = TrajectoryPath::new(
        BodyId::MOON,
        BodyId::EARTH,
        FrameId::j2000(),
        start_epoch,
        end_epoch,
        samples.clone(),
    );

    assert_eq!(path.len(), 2);
    assert!(!path.is_empty());
    assert!(path.sample_metadata_matches());
    assert_eq!(path.samples, samples);
    assert_eq!(path.start_epoch, start_epoch);
    assert_eq!(path.end_epoch, end_epoch);
}

#[test]
fn trajectory_trail_config_normalizes_reversed_month_bounds() {
    let config = TrajectoryTrailConfig {
        min_months: 36,
        max_months: 1,
        ..TrajectoryTrailConfig::default()
    };

    assert_eq!(config.clamped_months(0), 1);
    assert_eq!(config.clamped_months(12), 12);
    assert_eq!(config.clamped_months(99), 36);
}

#[test]
fn orientation_requests_describe_frame_rotation_and_epoch() {
    let request = OrientationRequest::new(FrameId::iau_earth(), FrameId::j2000(), Epoch::J2000);

    assert_eq!(request.from, FrameId::iau_earth());
    assert_eq!(request.to, FrameId::j2000());
    assert_eq!(request.epoch, Epoch::J2000);
}

#[test]
fn quaternions_normalize_or_fall_back_to_identity() {
    let normalized = QuatD::from_xyzw(0.0, 0.0, 0.0, 2.0).normalized();
    let invalid = QuatD::from_xyzw(f64::NAN, 0.0, 0.0, 1.0).normalized();

    assert_eq!(normalized, QuatD::IDENTITY);
    assert_eq!(invalid, QuatD::IDENTITY);
}

#[test]
fn frame_orientations_normalize_rotation_and_preserve_angular_velocity() {
    let orientation = FrameOrientation::new(
        FrameId::iau_earth(),
        FrameId::j2000(),
        Epoch::J2000,
        QuatD::from_xyzw(0.0, 0.0, 0.0, 2.0),
        Some(Vec3d::new(0.0, 0.0, 7.292_115_9e-5)),
    );

    assert_eq!(orientation.from, FrameId::iau_earth());
    assert_eq!(orientation.to, FrameId::j2000());
    assert_eq!(orientation.epoch, Epoch::J2000);
    assert_eq!(orientation.rotation, QuatD::IDENTITY);
    assert_eq!(
        orientation.angular_velocity_rad_s,
        Some(Vec3d::new(0.0, 0.0, 7.292_115_9e-5))
    );
}

#[test]
fn fake_ephemeris_provider_returns_state_vectors() {
    let provider = FakeEphemerisProvider;
    let request = StateRequest::new(BodyId::MOON, BodyId::EARTH, FrameId::j2000(), Epoch::J2000);
    let state = provider.state(&request);

    assert!(state.is_ok());
    let Ok(state) = state else {
        return;
    };

    assert_eq!(state.target, request.target);
    assert_eq!(state.origin, request.origin);
    assert_eq!(state.frame, request.frame);
    assert_eq!(state.epoch, request.epoch);
    assert_close(state.position_km.x, 384_400.0);
    assert_close(state.velocity_km_s.y, 1.022);
}

#[test]
fn fake_ephemeris_provider_returns_frame_orientations() {
    let provider = FakeEphemerisProvider;
    let request = OrientationRequest::new(FrameId::iau_earth(), FrameId::j2000(), Epoch::J2000);
    let orientation = provider.orientation(&request);

    assert!(orientation.is_ok());
    let Ok(orientation) = orientation else {
        return;
    };

    assert_eq!(orientation.from, request.from);
    assert_eq!(orientation.to, request.to);
    assert_eq!(orientation.epoch, request.epoch);
    assert_eq!(orientation.rotation, QuatD::IDENTITY);
    assert_eq!(
        orientation.angular_velocity_rad_s,
        Some(Vec3d::new(0.0, 0.0, 7.292_115_9e-5))
    );
}

#[test]
fn fake_ephemeris_provider_reports_missing_data() {
    let provider = FakeEphemerisProvider;
    let request = StateRequest::new(BodyId::MARS, BodyId::EARTH, FrameId::j2000(), Epoch::J2000);
    let error = provider.state(&request);

    assert!(matches!(error, Err(EphemerisError::MissingData(_))));
}

#[test]
fn physical_render_scale_maps_one_bevy_unit_to_one_kilometer() {
    let scale = RenderScale::PHYSICAL;
    let earth = CelestialBody::earth();

    assert_close(scale.distance_to_units(Kilometers::new(1.0)), 1.0);
    assert_close(scale.units_to_distance(1.0).as_f64(), 1.0);
    assert_close(earth.mean_radius_units(scale), 6_371.008);
}

#[test]
fn render_scale_can_intentionally_compress_distances() {
    let scale = RenderScale::from_kilometers_per_unit(100.0);
    let position_units = scale.position_to_units(Vec3d::new(10_000.0, -5_000.0, 1_000.0));

    assert_close(scale.distance_to_units(Kilometers::new(1_000.0)), 10.0);
    assert_close(scale.units_to_distance(10.0).as_f64(), 1_000.0);
    assert_close(position_units.x, 100.0);
    assert_close(position_units.y, -50.0);
    assert_close(position_units.z, 10.0);
}

#[test]
fn invalid_render_scales_fall_back_to_physical_units() {
    let zero = RenderScale::from_kilometers_per_unit(0.0);
    let negative = RenderScale::from_kilometers_per_unit(-1.0);
    let infinite = RenderScale::from_kilometers_per_unit(f64::INFINITY);

    assert_close(
        zero.normalized().kilometers_per_unit,
        RenderScale::PHYSICAL.kilometers_per_unit,
    );
    assert_close(
        negative.normalized().kilometers_per_unit,
        RenderScale::PHYSICAL.kilometers_per_unit,
    );
    assert_close(
        infinite.normalized().kilometers_per_unit,
        RenderScale::PHYSICAL.kilometers_per_unit,
    );
}

struct FakeEphemerisProvider;

impl EphemerisProvider for FakeEphemerisProvider {
    fn state(
        &self,
        request: &StateRequest,
    ) -> ferrisium_core::prelude::EphemerisResult<StateVector> {
        if request.target == BodyId::MOON
            && request.origin == BodyId::EARTH
            && request.frame == FrameId::j2000()
            && request.epoch == Epoch::J2000
        {
            return Ok(StateVector::relative_to(
                request.target,
                request.origin,
                request.frame.clone(),
                request.epoch,
                Vec3d::new(384_400.0, 0.0, 0.0),
                Vec3d::new(0.0, 1.022, 0.0),
            ));
        }

        Err(EphemerisError::MissingData(
            "fake provider only contains Moon relative to Earth at J2000".to_owned(),
        ))
    }

    fn orientation(
        &self,
        request: &OrientationRequest,
    ) -> ferrisium_core::prelude::EphemerisResult<FrameOrientation> {
        if request.from == FrameId::iau_earth()
            && request.to == FrameId::j2000()
            && request.epoch == Epoch::J2000
        {
            return Ok(FrameOrientation::new(
                request.from.clone(),
                request.to.clone(),
                request.epoch,
                QuatD::IDENTITY,
                Some(Vec3d::new(0.0, 0.0, 7.292_115_9e-5)),
            ));
        }

        Err(EphemerisError::MissingData(
            "fake provider only contains IAU_EARTH to J2000 at J2000".to_owned(),
        ))
    }
}
