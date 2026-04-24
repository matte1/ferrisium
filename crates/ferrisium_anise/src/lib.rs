//! ANISE-backed ephemeris provider for Ferrisium.
//!
//! This crate is the boundary between ANISE's `Almanac` and
//! `ferrisium_core::space`. It intentionally keeps ANISE types out of renderer
//! crates: callers load or construct an ANISE almanac here, then query through
//! Ferrisium's renderer-agnostic [`EphemerisProvider`] trait.

use anise::almanac::Almanac;
use anise::constants::orientations::{
    IAU_EARTH, IAU_MARS, IAU_MOON, ITRF93, J2000, MOON_ME, MOON_ME_DE440_ME421, MOON_PA,
    MOON_PA_DE440,
};
use anise::frames::Frame;
use anise::math::{Matrix3, Vector3};
use anise::naif::{BPC, SPK};
use anise::structure::dataset::DataSetType;
use anise::structure::metadata::Metadata;
use anise::structure::{
    EulerParameterDataSet, LocationDataSet, PlanetaryDataSet, SpacecraftDataSet,
};
use anise::time::{Duration as AniseDuration, Epoch as AniseEpoch};
use bytes::BytesMut;
use ferrisium_core::prelude::{
    BodyId, EphemerisError, EphemerisProvider, EphemerisResult, Epoch as FerrisiumEpoch, FrameId,
    FrameOrientation, NasaTrekRegularBody, OrientationRequest, QuatD, StateRequest, StateVector,
    Vec3d,
};

/// ANISE-backed implementation of Ferrisium's ephemeris provider trait.
#[derive(Clone)]
pub struct AlmanacProvider {
    almanac: Almanac,
}

impl AlmanacProvider {
    /// Creates a provider from an already-loaded ANISE almanac.
    #[must_use]
    pub const fn from_almanac(almanac: Almanac) -> Self {
        Self { almanac }
    }

    /// Loads a provider from one ANISE-supported kernel path.
    ///
    /// ANISE guesses the file type from the path, so this can load SPK/BSP,
    /// BPC/PCK, PCA, EPA, and other ANISE-supported kernel formats.
    /// Browser wasm applications should use [`Self::load_from_bytes`] with
    /// same-origin fetched assets instead; path-based loading is not a browser
    /// wasm path.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError::ProviderFailure`] when ANISE cannot load the
    /// provided path.
    pub fn load(path: &str) -> EphemerisResult<Self> {
        Almanac::new(path).map(Self::from_almanac).map_err(|error| {
            provider_failure(format!("failed to load ANISE almanac {path}: {error}"))
        })
    }

    /// Loads a provider from multiple ANISE-supported kernel paths.
    ///
    /// The first path creates the almanac and each remaining path is loaded
    /// into that almanac in order.
    /// Browser wasm applications should use [`Self::load_all_from_bytes`] with
    /// same-origin fetched assets instead; path-based loading is not a browser
    /// wasm path.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError::UnsupportedQuery`] when `paths` is empty.
    /// Returns [`EphemerisError::ProviderFailure`] when ANISE cannot load a
    /// provided path.
    pub fn load_all<'a>(paths: impl IntoIterator<Item = &'a str>) -> EphemerisResult<Self> {
        let mut paths = paths.into_iter();
        let Some(first_path) = paths.next() else {
            return Err(EphemerisError::UnsupportedQuery(
                "at least one ANISE kernel path is required".to_owned(),
            ));
        };

        let mut almanac = Almanac::new(first_path).map_err(|error| {
            provider_failure(format!(
                "failed to load initial ANISE almanac {first_path}: {error}"
            ))
        })?;

        for path in paths {
            almanac = almanac.load(path).map_err(|error| {
                provider_failure(format!("failed to load ANISE kernel {path}: {error}"))
            })?;
        }

        Ok(Self::from_almanac(almanac))
    }

    /// Loads a provider from ANISE-supported kernel bytes.
    ///
    /// This is the browser-friendly counterpart to [`Self::load`]. ANISE
    /// inspects the bytes to determine whether they contain SPK/BSP, BPC/PCK,
    /// PCA, EPA, or another supported ANISE dataset.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError::ProviderFailure`] when ANISE cannot inspect or
    /// load the provided bytes.
    pub fn load_from_bytes(name: &str, bytes: &[u8]) -> EphemerisResult<Self> {
        load_named_kernel_bytes(Almanac::default(), name, bytes).map(Self::from_almanac)
    }

    /// Loads a provider from multiple ANISE-supported kernel byte buffers.
    ///
    /// The buffers are loaded in the provided order. Names are also used as
    /// deterministic ANISE aliases so same-tick browser loads cannot collide on
    /// clock-generated aliases.
    ///
    /// # Errors
    ///
    /// Returns [`EphemerisError::UnsupportedQuery`] when `kernels` is empty.
    /// Returns [`EphemerisError::ProviderFailure`] when ANISE cannot load a
    /// provided buffer.
    pub fn load_all_from_bytes<'a>(
        kernels: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    ) -> EphemerisResult<Self> {
        let mut almanac = Almanac::default();
        let mut loaded_count = 0_usize;

        for (name, bytes) in kernels {
            almanac = load_named_kernel_bytes(almanac, name, bytes)?;
            loaded_count = loaded_count.saturating_add(1);
        }

        if loaded_count == 0 {
            return Err(EphemerisError::UnsupportedQuery(
                "at least one ANISE kernel byte buffer is required".to_owned(),
            ));
        }

        Ok(Self::from_almanac(almanac))
    }

    /// Returns the wrapped ANISE almanac.
    #[must_use]
    pub const fn almanac(&self) -> &Almanac {
        &self.almanac
    }
}

fn load_named_kernel_bytes(almanac: Almanac, name: &str, bytes: &[u8]) -> EphemerisResult<Almanac> {
    if bytes.get(..8) == Some(b"version".as_slice()) {
        return Err(provider_failure(format!(
            "failed to load ANISE kernel bytes {name}: file is a git lfs pointer, run `git lfs pull`"
        )));
    }

    if let Some(file_id) = daf_file_id(bytes) {
        return match file_id {
            "PCK" => BPC::parse(BytesMut::from(bytes))
                .map(|bpc| almanac.with_bpc_as(bpc, Some(name.to_owned())))
                .map_err(|error| {
                    provider_failure(format!(
                        "failed to load ANISE BPC kernel bytes {name}: {error}"
                    ))
                }),
            "SPK" => SPK::parse(BytesMut::from(bytes))
                .map(|spk| almanac.with_spk_as(spk, Some(name.to_owned())))
                .map_err(|error| {
                    provider_failure(format!(
                        "failed to load ANISE SPK kernel bytes {name}: {error}"
                    ))
                }),
            other => Err(provider_failure(format!(
                "failed to load ANISE kernel bytes {name}: DAF/{other} is not supported"
            ))),
        };
    }

    let metadata = Metadata::decode_header(bytes).map_err(|error| {
        provider_failure(format!("failed to load ANISE kernel bytes {name}: {error}"))
    })?;
    match metadata.dataset_type {
        DataSetType::NotApplicable => Err(provider_failure(format!(
            "failed to load ANISE kernel bytes {name}: malformed dataset type"
        ))),
        DataSetType::SpacecraftData => SpacecraftDataSet::try_from_bytes(bytes)
            .map(|dataset| almanac.with_spacecraft_data_as(dataset, Some(name.to_owned())))
            .map_err(|error| {
                provider_failure(format!(
                    "failed to load ANISE spacecraft data bytes {name}: {error}"
                ))
            }),
        DataSetType::PlanetaryData => PlanetaryDataSet::try_from_bytes(bytes)
            .map(|dataset| almanac.with_planetary_data_as(dataset, Some(name.to_owned())))
            .map_err(|error| {
                provider_failure(format!(
                    "failed to load ANISE planetary data bytes {name}: {error}"
                ))
            }),
        DataSetType::EulerParameterData => EulerParameterDataSet::try_from_bytes(bytes)
            .map(|dataset| almanac.with_euler_parameters_as(dataset, Some(name.to_owned())))
            .map_err(|error| {
                provider_failure(format!(
                    "failed to load ANISE Euler parameter bytes {name}: {error}"
                ))
            }),
        DataSetType::LocationData => LocationDataSet::try_from_bytes(bytes)
            .map(|dataset| almanac.with_location_data_as(dataset, Some(name.to_owned())))
            .map_err(|error| {
                provider_failure(format!(
                    "failed to load ANISE location data bytes {name}: {error}"
                ))
            }),
    }
}

fn daf_file_id(bytes: &[u8]) -> Option<&str> {
    let id = std::str::from_utf8(bytes.get(..8)?).ok()?;
    id.strip_prefix("DAF/").map(str::trim)
}

impl Default for AlmanacProvider {
    fn default() -> Self {
        Self::from_almanac(Almanac::default())
    }
}

impl EphemerisProvider for AlmanacProvider {
    fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector> {
        let epoch = to_anise_epoch(request.epoch);
        let orientation_id = orientation_id(&request.frame)?;
        let state = self
            .almanac
            .spk_ezr(
                request.target.naif_id,
                epoch,
                orientation_id,
                request.origin.naif_id,
                None,
            )
            .map_err(|error| {
                missing_data(format!(
                    "ANISE could not transform target {} relative to origin {} in frame {} at {:?}: {error}",
                    request.target.naif_id, request.origin.naif_id, request.frame.name, request.epoch
                ))
            })?;

        Ok(StateVector::relative_to(
            request.target,
            request.origin,
            request.frame.clone(),
            request.epoch,
            vector3_to_vec3d(&state.radius_km),
            vector3_to_vec3d(&state.velocity_km_s),
        ))
    }

    fn orientation(&self, request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
        let epoch = to_anise_epoch(request.epoch);
        let from_frame = orientation_frame(&request.from)?;
        let to_frame = orientation_frame(&request.to)?;
        let rotation = self
            .almanac
            .rotate(from_frame, to_frame, epoch)
            .map_err(|error| {
                missing_data(format!(
                    "ANISE could not rotate frame {} into frame {} at {:?}: {error}",
                    request.from.name, request.to.name, request.epoch
                ))
            })?;

        // Angular velocity is useful for body-fixed frame animation, but some
        // orientation datasets only provide rotation. Preserve the rotation
        // result and mark angular velocity absent when ANISE cannot derive it.
        let angular_velocity_rad_s = self
            .almanac
            .angular_velocity_rad_s(
                orientation_frame(&request.from)?,
                orientation_frame(&request.to)?,
                epoch,
            )
            .ok()
            .map(|velocity| vector3_to_vec3d(&velocity));

        Ok(FrameOrientation::new(
            request.from.clone(),
            request.to.clone(),
            request.epoch,
            quaternion_from_rotation_matrix(rotation_matrix_to_array(&rotation.rot_mat)),
            angular_velocity_rad_s,
        ))
    }
}

/// Converts a Ferrisium epoch into ANISE/hifitime's TDB epoch.
#[must_use]
pub fn to_anise_epoch(epoch: FerrisiumEpoch) -> AniseEpoch {
    AniseEpoch::from_tdb_duration(AniseDuration::from_total_nanoseconds(
        epoch.tdb_nanoseconds_since_j2000,
    ))
}

/// Converts an ANISE/hifitime epoch into Ferrisium's TDB nanoseconds from J2000.
#[must_use]
pub fn from_anise_epoch(epoch: AniseEpoch) -> FerrisiumEpoch {
    FerrisiumEpoch::from_tdb_nanoseconds_since_j2000(epoch.to_tdb_duration().total_nanoseconds())
}

/// Returns the current system/browser time as a Ferrisium TDB epoch.
///
/// Browser builds use hifitime's web-time backend under ANISE. This helper
/// keeps applications from depending on ANISE/hifitime types just to seed
/// Ferrisium's renderer-agnostic [`FerrisiumEpoch`].
///
/// # Errors
///
/// Returns [`EphemerisError::ProviderFailure`] when the platform cannot provide
/// a current time.
pub fn current_epoch() -> EphemerisResult<FerrisiumEpoch> {
    AniseEpoch::now()
        .map(from_anise_epoch)
        .map_err(|error| provider_failure(format!("failed to read current epoch: {error}")))
}

/// Resolves a Ferrisium frame name to the corresponding ANISE orientation ID.
///
/// The adapter maps Ferrisium's built-in inertial, Earth/Moon precision, and
/// regular-body IAU frame names. Mission-specific and custom frames can be
/// added once Ferrisium has a registry for provider-loaded frame aliases.
///
/// # Errors
///
/// Returns [`EphemerisError::UnsupportedQuery`] when the frame name is not part
/// of the initial ANISE adapter mapping.
pub fn orientation_id(frame: &FrameId) -> EphemerisResult<i32> {
    let normalized = normalized_frame_name(frame);
    match normalized.as_str() {
        "J2000" | "EME2000" => Ok(J2000),
        "IAU_EARTH" => Ok(IAU_EARTH),
        "ITRF93" | "EARTH_ITRF93" => Ok(ITRF93),
        "IAU_MOON" => Ok(IAU_MOON),
        "MOON_ME" => Ok(MOON_ME),
        "MOON_ME_DE440" => Ok(MOON_ME_DE440_ME421),
        "MOON_PA" => Ok(MOON_PA),
        "MOON_PA_DE440" => Ok(MOON_PA_DE440),
        "IAU_MARS" => Ok(IAU_MARS),
        _ => {
            if let Some(body) = regular_body_from_iau_frame_name(&normalized) {
                Ok(body.naif_id())
            } else {
                Err(unsupported_frame(frame))
            }
        }
    }
}

/// Builds an ANISE frame for a body in a requested orientation frame.
///
/// # Errors
///
/// Returns [`EphemerisError::UnsupportedQuery`] when the frame name is not part
/// of the initial ANISE adapter mapping.
pub fn frame_for_body(body: BodyId, frame: &FrameId) -> EphemerisResult<Frame> {
    Ok(Frame::new(body.naif_id, orientation_id(frame)?))
}

/// Builds an ANISE frame for a pure orientation query.
///
/// Body-fixed orientation names use their natural NAIF body center. Inertial
/// `J2000`/`EME2000` uses the solar-system barycenter because no body-fixed
/// center is implied by the frame name.
///
/// # Errors
///
/// Returns [`EphemerisError::UnsupportedQuery`] when the frame name is not part
/// of the initial ANISE adapter mapping.
pub fn orientation_frame(frame: &FrameId) -> EphemerisResult<Frame> {
    let normalized = normalized_frame_name(frame);
    match normalized.as_str() {
        "J2000" | "EME2000" => frame_for_body(BodyId::SOLAR_SYSTEM_BARYCENTER, frame),
        "IAU_EARTH" | "ITRF93" | "EARTH_ITRF93" => frame_for_body(BodyId::EARTH, frame),
        "IAU_MOON" | "MOON_ME" | "MOON_ME_DE440" | "MOON_PA" | "MOON_PA_DE440" => {
            frame_for_body(BodyId::MOON, frame)
        }
        "IAU_MARS" => frame_for_body(BodyId::MARS, frame),
        _ => {
            if let Some(body) = regular_body_from_iau_frame_name(&normalized) {
                frame_for_body(BodyId::naif(body.naif_id()), frame)
            } else {
                Err(unsupported_frame(frame))
            }
        }
    }
}

/// Converts a row-major rotation matrix into a unit quaternion.
#[must_use]
pub fn quaternion_from_rotation_matrix(matrix: [[f64; 3]; 3]) -> QuatD {
    let trace = matrix[0][0] + matrix[1][1] + matrix[2][2];

    let quaternion = if trace > 0.0 {
        let scale = (trace + 1.0).sqrt() * 2.0;
        QuatD::from_xyzw(
            (matrix[2][1] - matrix[1][2]) / scale,
            (matrix[0][2] - matrix[2][0]) / scale,
            (matrix[1][0] - matrix[0][1]) / scale,
            0.25 * scale,
        )
    } else if matrix[0][0] > matrix[1][1] && matrix[0][0] > matrix[2][2] {
        let scale = (1.0 + matrix[0][0] - matrix[1][1] - matrix[2][2]).sqrt() * 2.0;
        QuatD::from_xyzw(
            0.25 * scale,
            (matrix[0][1] + matrix[1][0]) / scale,
            (matrix[0][2] + matrix[2][0]) / scale,
            (matrix[2][1] - matrix[1][2]) / scale,
        )
    } else if matrix[1][1] > matrix[2][2] {
        let scale = (1.0 + matrix[1][1] - matrix[0][0] - matrix[2][2]).sqrt() * 2.0;
        QuatD::from_xyzw(
            (matrix[0][1] + matrix[1][0]) / scale,
            0.25 * scale,
            (matrix[1][2] + matrix[2][1]) / scale,
            (matrix[0][2] - matrix[2][0]) / scale,
        )
    } else {
        let scale = (1.0 + matrix[2][2] - matrix[0][0] - matrix[1][1]).sqrt() * 2.0;
        QuatD::from_xyzw(
            (matrix[0][2] + matrix[2][0]) / scale,
            (matrix[1][2] + matrix[2][1]) / scale,
            0.25 * scale,
            (matrix[1][0] - matrix[0][1]) / scale,
        )
    };

    quaternion.normalized()
}

fn rotation_matrix_to_array(matrix: &Matrix3) -> [[f64; 3]; 3] {
    [
        [matrix[(0, 0)], matrix[(0, 1)], matrix[(0, 2)]],
        [matrix[(1, 0)], matrix[(1, 1)], matrix[(1, 2)]],
        [matrix[(2, 0)], matrix[(2, 1)], matrix[(2, 2)]],
    ]
}

fn vector3_to_vec3d(vector: &Vector3) -> Vec3d {
    Vec3d::new(vector[0], vector[1], vector[2])
}

fn normalized_frame_name(frame: &FrameId) -> String {
    frame.name.trim().to_ascii_uppercase()
}

fn regular_body_from_iau_frame_name(normalized: &str) -> Option<NasaTrekRegularBody> {
    let body_slug = normalized.strip_prefix("IAU_")?;
    NasaTrekRegularBody::from_slug(body_slug)
}

fn unsupported_frame(frame: &FrameId) -> EphemerisError {
    EphemerisError::UnsupportedQuery(format!(
        "ANISE adapter does not yet map Ferrisium frame {}",
        frame.name
    ))
}

fn missing_data(message: String) -> EphemerisError {
    EphemerisError::MissingData(message)
}

fn provider_failure(message: String) -> EphemerisError {
    EphemerisError::ProviderFailure(message)
}
