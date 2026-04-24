#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use ferrisium_anise::AlmanacProvider;
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::CelestialEphemeris;
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_core::space::{
    BodyId, EphemerisError, EphemerisResult, Epoch, FrameId, QuatD, StateRequest, StateVector,
    Vec3d,
};
#[cfg(target_arch = "wasm32")]
use ferrisium_core::space::{EphemerisProvider, FrameOrientation, OrientationRequest};

#[cfg(target_arch = "wasm32")]
use crate::demo_config::browser_demo_anise_disabled;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_EPOCH_TDB_NANOSECONDS: i128 = 820_454_400_000_000_000;
#[cfg(any(target_arch = "wasm32", test))]
const MOON_ORBIT_RADIUS_KM: f64 = 384_400.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MOON_ORBIT_PERIOD_SECONDS: f64 = 27.321_661 * 86_400.0;
#[cfg(any(target_arch = "wasm32", test))]
const EARTH_ORBIT_RADIUS_KM: f64 = 149_598_023.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const EARTH_ORBIT_PERIOD_SECONDS: f64 = 365.256_363_004 * 86_400.0;
#[cfg(any(target_arch = "wasm32", test))]
const MARS_ORBIT_RADIUS_KM: f64 = 227_939_200.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MARS_ORBIT_PERIOD_SECONDS: f64 = 686.98 * 86_400.0;
#[cfg(any(target_arch = "wasm32", test))]
const MARS_FALLBACK_PHASE_RAD: f64 = 0.8;
#[cfg(any(target_arch = "wasm32", test))]
const MERCURY_ORBIT_RADIUS_KM: f64 = 57_909_050.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MERCURY_ORBIT_PERIOD_SECONDS: f64 = 87.969_1 * 86_400.0;
#[cfg(any(target_arch = "wasm32", test))]
const MERCURY_FALLBACK_PHASE_RAD: f64 = 1.4;
#[cfg(any(target_arch = "wasm32", test))]
const VENUS_ORBIT_RADIUS_KM: f64 = 108_208_000.0;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const VENUS_ORBIT_PERIOD_SECONDS: f64 = 224.701 * 86_400.0;
#[cfg(any(target_arch = "wasm32", test))]
const VENUS_FALLBACK_PHASE_RAD: f64 = 2.2;
#[cfg(target_arch = "wasm32")]
const EARTH_ROTATION_RAD_S: f64 = 7.292_115_9e-5;
#[cfg(target_arch = "wasm32")]
const MOON_ROTATION_RAD_S: f64 = std::f64::consts::TAU / MOON_ORBIT_PERIOD_SECONDS;
#[cfg(target_arch = "wasm32")]
const MARS_ROTATION_RAD_S: f64 = std::f64::consts::TAU / (24.622_962 * 3_600.0);

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_MERCURY_BARYCENTER: BodyId = BodyId::naif(1);
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_VENUS_BARYCENTER: BodyId = BodyId::naif(2);

#[cfg(target_arch = "wasm32")]
const DEMO_ANISE_KERNELS: &[DemoAniseKernelSpec] = &[
    DemoAniseKernelSpec {
        name: "de440s.bsp",
        url: "assets/kernels/de440s.bsp",
    },
    DemoAniseKernelSpec {
        name: "pck11.pca",
        url: "assets/kernels/pck11.pca",
    },
    DemoAniseKernelSpec {
        name: "moon_fk_de440.epa",
        url: "assets/kernels/moon_fk_de440.epa",
    },
    DemoAniseKernelSpec {
        name: "moon_pa_de440_200625.bpc",
        url: "assets/kernels/moon_pa_de440_200625.bpc",
    },
    DemoAniseKernelSpec {
        name: "earth_latest_high_prec.bpc",
        url: "assets/kernels/earth_latest_high_prec.bpc",
    },
];

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
struct DemoAniseKernelSpec {
    name: &'static str,
    url: &'static str,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
pub(crate) struct DemoAniseKernelLoader {
    receiver: Mutex<Receiver<DemoAniseKernelFetchResult>>,
    sender: Sender<DemoAniseKernelFetchResult>,
    status: DemoAniseKernelStatus,
}

#[cfg(target_arch = "wasm32")]
impl Default for DemoAniseKernelLoader {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver: Mutex::new(receiver),
            sender,
            status: DemoAniseKernelStatus::NotStarted,
        }
    }
}

#[cfg(target_arch = "wasm32")]
enum DemoAniseKernelStatus {
    NotStarted,
    Loading { received: Vec<DemoAniseKernelBytes> },
    Loaded,
    Failed,
}

#[cfg(target_arch = "wasm32")]
struct DemoAniseKernelFetchResult {
    index: usize,
    name: &'static str,
    url: &'static str,
    result: ehttp::Result<ehttp::Response>,
}

#[cfg(target_arch = "wasm32")]
struct DemoAniseKernelBytes {
    index: usize,
    name: &'static str,
    bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn sync_demo_anise_kernel_bundle(
    mut commands: Commands<'_, '_>,
    mut loader: ResMut<'_, DemoAniseKernelLoader>,
) {
    if browser_demo_anise_disabled() {
        return;
    }

    if matches!(loader.status, DemoAniseKernelStatus::NotStarted) {
        queue_demo_anise_kernel_fetches(&loader.sender);
        loader.status = DemoAniseKernelStatus::Loading {
            received: Vec::with_capacity(DEMO_ANISE_KERNELS.len()),
        };
    }

    let fetch_results = drain_demo_anise_kernel_fetches(&loader);
    if fetch_results.is_empty() {
        return;
    }

    let DemoAniseKernelStatus::Loading { received } = &mut loader.status else {
        return;
    };

    for fetch_result in fetch_results {
        let Ok(response) = fetch_result.result else {
            warn!(
                "failed to fetch ANISE demo kernel {} from {}",
                fetch_result.name, fetch_result.url
            );
            loader.status = DemoAniseKernelStatus::Failed;
            return;
        };

        if !response.ok {
            warn!(
                "failed to fetch ANISE demo kernel {} from {}: HTTP {} {}",
                fetch_result.name, response.url, response.status, response.status_text
            );
            loader.status = DemoAniseKernelStatus::Failed;
            return;
        }

        info!(
            "fetched ANISE demo kernel {} ({} bytes)",
            fetch_result.name,
            response.bytes.len()
        );
        received.push(DemoAniseKernelBytes {
            index: fetch_result.index,
            name: fetch_result.name,
            bytes: response.bytes,
        });
    }

    if received.len() != DEMO_ANISE_KERNELS.len() {
        return;
    }

    let mut kernels = std::mem::take(received);
    kernels.sort_by_key(|kernel| kernel.index);
    match AlmanacProvider::load_all_from_bytes(
        kernels
            .iter()
            .map(|kernel| (kernel.name, kernel.bytes.as_slice())),
    ) {
        Ok(provider) => {
            log_demo_anise_diagnostics(&provider);
            commands.insert_resource(CelestialEphemeris::from_provider(
                DemoEphemerisProvider::from_anise(provider),
            ));
            loader.status = DemoAniseKernelStatus::Loaded;
            info!("loaded ANISE demo kernel bundle");
        }
        Err(error) => {
            warn!("failed to build ANISE demo kernel bundle: {error}");
            loader.status = DemoAniseKernelStatus::Failed;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn queue_demo_anise_kernel_fetches(sender: &Sender<DemoAniseKernelFetchResult>) {
    for (index, kernel) in DEMO_ANISE_KERNELS.iter().enumerate() {
        let mut request = ehttp::Request::get(kernel.url);
        request
            .headers
            .insert("Accept", "application/octet-stream,*/*;q=0.8");

        let sender = sender.clone();
        let name = kernel.name;
        let url = kernel.url;
        ehttp::fetch(request, move |result| {
            let _ = sender.send(DemoAniseKernelFetchResult {
                index,
                name,
                url,
                result,
            });
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn drain_demo_anise_kernel_fetches(
    loader: &DemoAniseKernelLoader,
) -> Vec<DemoAniseKernelFetchResult> {
    let Ok(receiver) = loader.receiver.lock() else {
        error!("ANISE demo kernel fetch receiver lock poisoned");
        return Vec::new();
    };

    let mut fetch_results = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(fetch_result) => fetch_results.push(fetch_result),
            Err(TryRecvError::Empty) => return fetch_results,
            Err(TryRecvError::Disconnected) => {
                warn!("ANISE demo kernel fetch channel disconnected");
                return fetch_results;
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn log_demo_anise_diagnostics(provider: &AlmanacProvider) {
    let epoch = Epoch::from_tdb_nanoseconds_since_j2000(DEMO_EPOCH_TDB_NANOSECONDS);

    log_demo_anise_state(provider, "Moon", BodyId::MOON, BodyId::EARTH, epoch);
    log_demo_anise_state(provider, "Sun", BodyId::SUN, BodyId::EARTH, epoch);
    log_demo_anise_state(
        provider,
        "Mercury barycenter",
        BodyId::MERCURY_BARYCENTER,
        BodyId::EARTH,
        epoch,
    );
    log_demo_anise_state(
        provider,
        "Venus barycenter",
        BodyId::VENUS_BARYCENTER,
        BodyId::EARTH,
        epoch,
    );
    log_demo_anise_state(
        provider,
        "Mars barycenter",
        BodyId::MARS_BARYCENTER,
        BodyId::EARTH,
        epoch,
    );
    log_demo_anise_orientation(provider, "Earth ITRF93", FrameId::itrf93(), epoch);
    log_demo_anise_orientation(provider, "Moon ME DE440", FrameId::moon_me_de440(), epoch);
    log_demo_anise_orientation(provider, "Mars IAU", FrameId::iau_mars(), epoch);
}

#[cfg(target_arch = "wasm32")]
fn log_demo_anise_state(
    provider: &AlmanacProvider,
    label: &str,
    target: BodyId,
    origin: BodyId,
    epoch: Epoch,
) {
    let request = StateRequest::new(target, origin, FrameId::j2000(), epoch);
    match provider.state(&request) {
        Ok(state) => info!(
            "ANISE demo state ready for {label}: distance {:.0} km",
            vec3d_length_km(state.position_km)
        ),
        Err(error) => warn!("ANISE demo state unavailable for {label}: {error}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn log_demo_anise_orientation(
    provider: &AlmanacProvider,
    label: &str,
    from: FrameId,
    epoch: Epoch,
) {
    let request = OrientationRequest::new(from, FrameId::j2000(), epoch);
    match provider.orientation(&request) {
        Ok(orientation) => info!(
            "ANISE demo orientation ready for {label}: angular velocity present = {}",
            orientation.angular_velocity_rad_s.is_some()
        ),
        Err(error) => warn!("ANISE demo orientation unavailable for {label}: {error}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn vec3d_length_km(vector: Vec3d) -> f64 {
    vector
        .x
        .mul_add(vector.x, vector.y.mul_add(vector.y, vector.z * vector.z))
        .sqrt()
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub(crate) struct DemoEphemerisProvider {
    anise: AlmanacProvider,
}

#[cfg(target_arch = "wasm32")]
impl DemoEphemerisProvider {
    fn from_anise(anise: AlmanacProvider) -> Self {
        Self { anise }
    }
}

#[cfg(target_arch = "wasm32")]
impl EphemerisProvider for DemoEphemerisProvider {
    fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector> {
        self.anise
            .state(request)
            .or_else(|_| demo_fallback_state(request))
    }

    fn orientation(&self, request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
        self.anise
            .orientation(request)
            .or_else(|_| demo_fallback_orientation(request))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_fallback_state(request: &StateRequest) -> EphemerisResult<StateVector> {
    if request.target == request.origin {
        return Ok(demo_relative_state(request, Vec3d::ZERO, Vec3d::ZERO));
    }
    if request.frame == FrameId::j2000() {
        let Some((target_position, target_velocity)) =
            demo_fallback_solar_state(request.target, request.epoch)
        else {
            return missing_demo_fallback_state();
        };
        let Some((origin_position, origin_velocity)) =
            demo_fallback_solar_state(request.origin, request.epoch)
        else {
            return missing_demo_fallback_state();
        };

        return Ok(demo_relative_state(
            request,
            subtract_vec3d(target_position, origin_position),
            subtract_vec3d(target_velocity, origin_velocity),
        ));
    }

    missing_demo_fallback_state()
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_relative_state(request: &StateRequest, position: Vec3d, velocity: Vec3d) -> StateVector {
    StateVector::relative_to(
        request.target,
        request.origin,
        request.frame.clone(),
        request.epoch,
        position,
        velocity,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_fallback_solar_state(body: BodyId, epoch: Epoch) -> Option<(Vec3d, Vec3d)> {
    if body == BodyId::SUN {
        Some((Vec3d::ZERO, Vec3d::ZERO))
    } else if body == DEMO_MERCURY_BARYCENTER {
        Some(mercury_heliocentric_fallback_state(epoch))
    } else if body == DEMO_VENUS_BARYCENTER {
        Some(venus_heliocentric_fallback_state(epoch))
    } else if body == BodyId::EARTH {
        Some(earth_heliocentric_fallback_state(epoch))
    } else if body == BodyId::MOON {
        let (earth_position, earth_velocity) = earth_heliocentric_fallback_state(epoch);
        let (moon_position, moon_velocity) = moon_geocentric_fallback_state(epoch);
        Some((
            add_vec3d(earth_position, moon_position),
            add_vec3d(earth_velocity, moon_velocity),
        ))
    } else if body == BodyId::MARS_BARYCENTER {
        Some(mars_heliocentric_fallback_state(epoch))
    } else {
        None
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn missing_demo_fallback_state() -> EphemerisResult<StateVector> {
    Err(EphemerisError::MissingData(
        "demo fallback only models Sun, Mercury, Venus, Earth, Moon, and Mars-system barycenter states"
            .to_owned(),
    ))
}

#[cfg(any(target_arch = "wasm32", test))]
fn mercury_heliocentric_fallback_state(epoch: Epoch) -> (Vec3d, Vec3d) {
    circular_orbit_state(
        MERCURY_ORBIT_RADIUS_KM,
        std::f64::consts::TAU / MERCURY_ORBIT_PERIOD_SECONDS,
        epoch_seconds_f64(epoch),
        MERCURY_FALLBACK_PHASE_RAD,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn venus_heliocentric_fallback_state(epoch: Epoch) -> (Vec3d, Vec3d) {
    circular_orbit_state(
        VENUS_ORBIT_RADIUS_KM,
        std::f64::consts::TAU / VENUS_ORBIT_PERIOD_SECONDS,
        epoch_seconds_f64(epoch),
        VENUS_FALLBACK_PHASE_RAD,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn earth_heliocentric_fallback_state(epoch: Epoch) -> (Vec3d, Vec3d) {
    circular_orbit_state(
        EARTH_ORBIT_RADIUS_KM,
        std::f64::consts::TAU / EARTH_ORBIT_PERIOD_SECONDS,
        epoch_seconds_f64(epoch),
        0.0,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn moon_geocentric_fallback_state(epoch: Epoch) -> (Vec3d, Vec3d) {
    circular_orbit_state(
        MOON_ORBIT_RADIUS_KM,
        std::f64::consts::TAU / MOON_ORBIT_PERIOD_SECONDS,
        epoch_seconds_f64(epoch),
        0.0,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn mars_heliocentric_fallback_state(epoch: Epoch) -> (Vec3d, Vec3d) {
    circular_orbit_state(
        MARS_ORBIT_RADIUS_KM,
        std::f64::consts::TAU / MARS_ORBIT_PERIOD_SECONDS,
        epoch_seconds_f64(epoch),
        MARS_FALLBACK_PHASE_RAD,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn circular_orbit_state(
    radius_km: f64,
    angular_rate: f64,
    seconds: f64,
    phase_rad: f64,
) -> (Vec3d, Vec3d) {
    let angle = seconds.mul_add(angular_rate, phase_rad);

    (
        circular_orbit_position(radius_km, angle),
        circular_orbit_velocity(radius_km, angular_rate, angle),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn add_vec3d(left: Vec3d, right: Vec3d) -> Vec3d {
    Vec3d::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

#[cfg(any(target_arch = "wasm32", test))]
fn subtract_vec3d(left: Vec3d, right: Vec3d) -> Vec3d {
    Vec3d::new(left.x - right.x, left.y - right.y, left.z - right.z)
}

#[cfg(any(target_arch = "wasm32", test))]
fn circular_orbit_position(radius_km: f64, angle: f64) -> Vec3d {
    Vec3d::new(radius_km * angle.sin(), 0.0, radius_km * angle.cos())
}

#[cfg(any(target_arch = "wasm32", test))]
fn circular_orbit_velocity(radius_km: f64, angular_rate: f64, angle: f64) -> Vec3d {
    Vec3d::new(
        radius_km * angular_rate * angle.cos(),
        0.0,
        -radius_km * angular_rate * angle.sin(),
    )
}

#[cfg(target_arch = "wasm32")]
fn demo_fallback_orientation(request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
    if request.to != FrameId::j2000() {
        return Err(EphemerisError::UnsupportedQuery(
            "demo fallback only rotates body-fixed frames into J2000".to_owned(),
        ));
    }

    let seconds = epoch_seconds_f64(request.epoch);
    if request.from == FrameId::itrf93() || request.from == FrameId::iau_earth() {
        return Ok(FrameOrientation::new(
            request.from.clone(),
            request.to.clone(),
            request.epoch,
            rotation_about_y(seconds * EARTH_ROTATION_RAD_S),
            Some(Vec3d::new(0.0, EARTH_ROTATION_RAD_S, 0.0)),
        ));
    }
    if request.from == FrameId::moon_me_de440()
        || request.from == FrameId::moon_me()
        || request.from == FrameId::moon_pa_de440()
        || request.from == FrameId::moon_pa()
        || request.from == FrameId::iau_moon()
    {
        return Ok(FrameOrientation::new(
            request.from.clone(),
            request.to.clone(),
            request.epoch,
            rotation_about_y(seconds * MOON_ROTATION_RAD_S),
            Some(Vec3d::new(0.0, MOON_ROTATION_RAD_S, 0.0)),
        ));
    }
    if request.from == FrameId::iau_mars() {
        return Ok(FrameOrientation::new(
            request.from.clone(),
            request.to.clone(),
            request.epoch,
            rotation_about_y(seconds * MARS_ROTATION_RAD_S),
            Some(Vec3d::new(0.0, MARS_ROTATION_RAD_S, 0.0)),
        ));
    }

    Err(EphemerisError::MissingData(
        "demo fallback only models Earth, Moon, and Mars body-fixed rotations".to_owned(),
    ))
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_precision_loss,
    reason = "The fallback ephemeris is display-only; sub-microsecond epoch precision is irrelevant here."
)]
fn epoch_seconds_f64(epoch: Epoch) -> f64 {
    epoch.tdb_nanoseconds_since_j2000 as f64 / 1_000_000_000.0
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn rotation_about_y(angle_rad: f64) -> QuatD {
    let half_angle = angle_rad * 0.5;

    QuatD::from_xyzw(0.0, half_angle.sin(), 0.0, half_angle.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fallback_composes_moon_states_relative_to_sun() {
        let request = StateRequest::new(BodyId::MOON, BodyId::SUN, FrameId::j2000(), Epoch::J2000);
        let state = demo_fallback_state(&request);
        assert!(state.is_ok(), "fallback state failed: {state:?}");
        let Ok(state) = state else {
            return;
        };

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

    const SOLAR_AU_KM: f64 = 149_597_870.7;
}
