use std::f64::consts::FRAC_1_SQRT_2;
use std::fs;
use std::path::PathBuf;

use anise::constants::orientations::{
    IAU_EARTH, IAU_MARS, IAU_MOON, ITRF93, J2000, MOON_ME, MOON_ME_DE440_ME421, MOON_PA,
    MOON_PA_DE440,
};
use ferrisium_anise::{
    current_epoch, frame_for_body, from_anise_epoch, orientation_frame, orientation_id,
    quaternion_from_rotation_matrix, to_anise_epoch, AlmanacProvider,
};
use ferrisium_core::prelude::{
    BodyId, EphemerisError, EphemerisProvider, Epoch, FrameId, OrientationRequest, StateRequest,
};

fn assert_close(lhs: f64, rhs: f64) {
    assert!(
        (lhs - rhs).abs() <= 1.0e-12,
        "float mismatch: lhs={lhs}, rhs={rhs}"
    );
}

#[test]
fn epochs_roundtrip_through_anise_tdb_duration() {
    let ferrisium_epoch = Epoch::from_tdb_nanoseconds_since_j2000(12_345_678_901_234);
    let anise_epoch = to_anise_epoch(ferrisium_epoch);
    let roundtrip = from_anise_epoch(anise_epoch);

    assert_eq!(roundtrip, ferrisium_epoch);
}

#[test]
fn current_epoch_returns_time_after_j2000() {
    let epoch = current_epoch();

    assert!(epoch.is_ok());
    let Ok(epoch) = epoch else {
        return;
    };
    assert!(epoch.tdb_nanoseconds_since_j2000 > 0);
}

#[test]
fn frame_names_map_to_anise_orientation_ids() {
    let cases = [
        (FrameId::j2000(), J2000),
        (FrameId::named("EME2000"), J2000),
        (FrameId::iau_earth(), IAU_EARTH),
        (FrameId::named("ITRF93"), ITRF93),
        (FrameId::named("EARTH_ITRF93"), ITRF93),
        (FrameId::iau_mercury(), BodyId::MERCURY.naif_id),
        (FrameId::iau_venus(), BodyId::VENUS.naif_id),
        (FrameId::iau_moon(), IAU_MOON),
        (FrameId::moon_me(), MOON_ME),
        (FrameId::moon_me_de440(), MOON_ME_DE440_ME421),
        (FrameId::moon_pa(), MOON_PA),
        (FrameId::moon_pa_de440(), MOON_PA_DE440),
        (FrameId::iau_mars(), IAU_MARS),
        (FrameId::iau_europa(), BodyId::EUROPA.naif_id),
        (FrameId::iau_ceres(), BodyId::CERES.naif_id),
    ];

    for (frame, expected_id) in cases {
        assert_eq!(orientation_id(&frame), Ok(expected_id));
    }
}

#[test]
fn unsupported_frame_names_are_rejected_before_anise_queries() {
    let result = orientation_id(&FrameId::named("CUSTOM_SPACECRAFT_FRAME"));

    assert!(matches!(result, Err(EphemerisError::UnsupportedQuery(_))));
}

#[test]
fn state_frames_keep_requested_body_and_orientation_ids() {
    let frame = FrameId::j2000();
    let anise_frame = frame_for_body(BodyId::MOON, &frame);

    assert!(anise_frame.is_ok());
    let Ok(anise_frame) = anise_frame else {
        return;
    };

    assert_eq!(anise_frame.ephemeris_id, BodyId::MOON.naif_id);
    assert_eq!(
        anise_frame.orientation_id,
        orientation_id(&frame).unwrap_or_default()
    );
}

#[test]
fn orientation_frames_use_natural_body_centers_for_body_fixed_frames() {
    let earth = orientation_frame(&FrameId::itrf93());
    let moon = orientation_frame(&FrameId::moon_me_de440());
    let europa = orientation_frame(&FrameId::iau_europa());
    let ceres = orientation_frame(&FrameId::iau_ceres());
    let inertial = orientation_frame(&FrameId::j2000());

    assert!(earth.is_ok());
    assert!(moon.is_ok());
    assert!(europa.is_ok());
    assert!(ceres.is_ok());
    assert!(inertial.is_ok());

    let Ok(earth) = earth else {
        return;
    };
    let Ok(inertial) = inertial else {
        return;
    };

    assert_eq!(earth.ephemeris_id, BodyId::EARTH.naif_id);
    let Ok(moon) = moon else {
        return;
    };
    assert_eq!(moon.ephemeris_id, BodyId::MOON.naif_id);
    let Ok(europa) = europa else {
        return;
    };
    assert_eq!(europa.ephemeris_id, BodyId::EUROPA.naif_id);
    let Ok(ceres) = ceres else {
        return;
    };
    assert_eq!(ceres.ephemeris_id, BodyId::CERES.naif_id);
    assert_eq!(
        inertial.ephemeris_id,
        BodyId::SOLAR_SYSTEM_BARYCENTER.naif_id
    );
}

#[test]
fn rotation_matrices_convert_to_normalized_quaternions() {
    let identity =
        quaternion_from_rotation_matrix([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let quarter_turn_about_z =
        quaternion_from_rotation_matrix([[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);

    assert_eq!(identity, ferrisium_core::prelude::QuatD::IDENTITY);
    assert_close(quarter_turn_about_z.x, 0.0);
    assert_close(quarter_turn_about_z.y, 0.0);
    assert_close(quarter_turn_about_z.z, FRAC_1_SQRT_2);
    assert_close(quarter_turn_about_z.w, FRAC_1_SQRT_2);
}

#[test]
fn empty_provider_reports_missing_data_for_state_queries() {
    let provider = AlmanacProvider::default();
    let request = StateRequest::new(BodyId::MOON, BodyId::EARTH, FrameId::j2000(), Epoch::J2000);
    let result = provider.state(&request);

    assert!(matches!(result, Err(EphemerisError::MissingData(_))));
}

#[test]
fn empty_kernel_list_is_rejected() {
    let result = AlmanacProvider::load_all([]);

    assert!(matches!(result, Err(EphemerisError::UnsupportedQuery(_))));
}

#[test]
fn empty_kernel_byte_list_is_rejected() {
    let result = AlmanacProvider::load_all_from_bytes([]);

    assert!(matches!(result, Err(EphemerisError::UnsupportedQuery(_))));
}

#[test]
fn invalid_kernel_bytes_are_rejected() {
    let result = AlmanacProvider::load_from_bytes("invalid.kernel", b"not an anise kernel");

    assert!(matches!(result, Err(EphemerisError::ProviderFailure(_))));
}

#[test]
fn staged_demo_kernel_bundle_supports_demo_queries() {
    let Some(kernel_paths) = staged_demo_kernel_paths() else {
        return;
    };
    let kernel_path_strings = kernel_paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let provider = AlmanacProvider::load_all(kernel_path_strings.iter().map(String::as_str));

    assert!(provider.is_ok());
    let Ok(provider) = provider else {
        return;
    };

    assert_demo_queries(&provider);
}

#[test]
fn staged_demo_kernel_bytes_support_demo_queries() {
    let Some(kernel_paths) = staged_demo_kernel_paths() else {
        return;
    };
    let kernel_bytes = kernel_paths
        .iter()
        .map(|path| fs::read(path).map(|bytes| (path.to_string_lossy().into_owned(), bytes)))
        .collect::<Result<Vec<_>, _>>();

    assert!(kernel_bytes.is_ok());
    let Ok(kernel_bytes) = kernel_bytes else {
        return;
    };

    let provider = AlmanacProvider::load_all_from_bytes(
        kernel_bytes
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice())),
    );

    assert!(provider.is_ok());
    let Ok(provider) = provider else {
        return;
    };

    assert_demo_queries(&provider);
}

fn staged_demo_kernel_paths() -> Option<[PathBuf; 5]> {
    let kernel_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/ferrisium_demo/assets/kernels");
    let paths = [
        kernel_dir.join("de440s.bsp"),
        kernel_dir.join("pck11.pca"),
        kernel_dir.join("moon_fk_de440.epa"),
        kernel_dir.join("moon_pa_de440_200625.bpc"),
        kernel_dir.join("earth_latest_high_prec.bpc"),
    ];

    paths.iter().all(|path| path.is_file()).then_some(paths)
}

fn assert_demo_queries(provider: &AlmanacProvider) {
    let epoch = Epoch::from_tdb_nanoseconds_since_j2000(820_454_400_000_000_000);
    let moon_j2000 = provider.state(&StateRequest::new(
        BodyId::MOON,
        BodyId::EARTH,
        FrameId::j2000(),
        epoch,
    ));
    let moon_itrf93 = provider.state(&StateRequest::new(
        BodyId::MOON,
        BodyId::EARTH,
        FrameId::itrf93(),
        epoch,
    ));
    assert!(moon_j2000.is_ok());
    assert!(moon_itrf93.is_ok());
    if let (Ok(j2000), Ok(itrf93)) = (moon_j2000, moon_itrf93) {
        assert_ne!(j2000.position_km, itrf93.position_km);
        assert_eq!(itrf93.frame, FrameId::itrf93());
    }
    assert!(provider
        .state(&StateRequest::new(
            BodyId::SUN,
            BodyId::EARTH,
            FrameId::j2000(),
            epoch
        ))
        .is_ok());
    assert!(provider
        .state(&StateRequest::new(
            BodyId::MARS_BARYCENTER,
            BodyId::EARTH,
            FrameId::j2000(),
            epoch
        ))
        .is_ok());
    assert!(provider
        .orientation(&OrientationRequest::new(
            FrameId::itrf93(),
            FrameId::j2000(),
            epoch
        ))
        .is_ok());
    assert!(provider
        .orientation(&OrientationRequest::new(
            FrameId::moon_me_de440(),
            FrameId::j2000(),
            epoch
        ))
        .is_ok());
}
