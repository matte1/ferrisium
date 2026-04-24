//! Sampled trajectory paths in provider-neutral space coordinates.
//!
//! A trajectory path is display- and renderer-agnostic: samples remain
//! authoritative [`StateVector`] values in kilometers, and renderers decide how
//! to draw those samples without changing their physical meaning.

use super::{
    BodyId, EphemerisProvider, EphemerisResult, Epoch, FrameId, StateRequest, StateVector,
};

/// Nanoseconds per second as `f64`, used when converting sampled epoch ranges.
pub const NANOS_PER_SECOND_F64: f64 = 1_000_000_000.0;

/// Average Gregorian month length in seconds.
pub const AVERAGE_GREGORIAN_MONTH_SECONDS: f64 = 30.437_5 * 86_400.0;

/// Sampled translational trajectory for one target relative to one origin.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryPath {
    /// Body, spacecraft, or synthetic object whose path is sampled.
    pub target: BodyId,
    /// Body or barycenter that all sample positions are relative to.
    pub origin: BodyId,
    /// Reference frame shared by all samples.
    pub frame: FrameId,
    /// First epoch covered by this path.
    pub start_epoch: Epoch,
    /// Last epoch covered by this path.
    pub end_epoch: Epoch,
    /// Ordered state samples in kilometers and kilometers per second.
    pub samples: Vec<StateVector>,
}

impl TrajectoryPath {
    /// Creates a sampled path with explicit metadata and samples.
    ///
    /// The constructor intentionally preserves the provided samples verbatim.
    /// Callers can use [`Self::sample_metadata_matches`] when they need to
    /// assert that every sample agrees with the path metadata.
    #[must_use]
    pub fn new(
        target: BodyId,
        origin: BodyId,
        frame: FrameId,
        start_epoch: Epoch,
        end_epoch: Epoch,
        samples: Vec<StateVector>,
    ) -> Self {
        Self {
            target,
            origin,
            frame,
            start_epoch,
            end_epoch,
            samples,
        }
    }

    /// Returns true when every sample has the path's target, origin, and frame.
    #[must_use]
    pub fn sample_metadata_matches(&self) -> bool {
        self.samples.iter().all(|sample| {
            sample.target == self.target
                && sample.origin == self.origin
                && sample.frame == self.frame
        })
    }

    /// Returns the number of state samples in the path.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns whether the path has no samples.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Sampling policy for a trailing trajectory history window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrajectoryTrailConfig {
    /// Smallest allowed history window, in months.
    pub min_months: u32,
    /// Largest allowed history window, in months.
    pub max_months: u32,
    /// Month duration used for fixed-size UI windows.
    pub average_month_seconds: f64,
    /// Minimum number of trajectory segments to sample.
    pub min_samples: usize,
    /// Maximum number of trajectory segments to sample.
    pub max_samples: usize,
    /// Desired sample density per orbital period.
    pub samples_per_period: f64,
}

impl Default for TrajectoryTrailConfig {
    fn default() -> Self {
        Self {
            min_months: 1,
            max_months: 36,
            average_month_seconds: AVERAGE_GREGORIAN_MONTH_SECONDS,
            min_samples: 32,
            max_samples: 512,
            samples_per_period: 128.0,
        }
    }
}

impl TrajectoryTrailConfig {
    /// Returns `months` clamped to the configured supported range.
    #[must_use]
    pub fn clamped_months(self, months: u32) -> u32 {
        let min_months = self.min_months.min(self.max_months);
        let max_months = self.min_months.max(self.max_months);

        months.clamp(min_months, max_months)
    }

    /// Returns the first epoch in a trailing history window ending at `epoch`.
    #[must_use]
    pub fn start_epoch(self, epoch: Epoch, months: u32) -> Epoch {
        let months = self.clamped_months(months);
        let average_month_seconds =
            finite_positive_or(self.average_month_seconds, AVERAGE_GREGORIAN_MONTH_SECONDS);

        offset_epoch_seconds(epoch, -(f64::from(months) * average_month_seconds))
    }

    /// Computes a browser-safe sample count for a trailing history path.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "The value is bounded before conversion into a small segment count."
    )]
    pub fn sample_count(self, months: u32, period_seconds: f64) -> usize {
        let min_samples = self.min_samples.max(2);
        let max_samples = self.max_samples.max(min_samples);
        let average_month_seconds =
            finite_positive_or(self.average_month_seconds, AVERAGE_GREGORIAN_MONTH_SECONDS);
        let period_seconds = finite_positive_or(period_seconds, 1.0);
        let samples_per_period = finite_positive_or(self.samples_per_period, 128.0);
        let trail_seconds = f64::from(self.clamped_months(months)) * average_month_seconds;
        let raw_count = ((trail_seconds / period_seconds) * samples_per_period).ceil();

        if !raw_count.is_finite() || raw_count <= min_samples as f64 {
            min_samples
        } else if raw_count >= max_samples as f64 {
            max_samples
        } else {
            raw_count as usize
        }
    }
}

/// Returns true when a trajectory sample set should be rebuilt for `current_epoch`.
#[must_use]
pub fn trajectory_samples_are_stale(
    sampled_epoch: Option<Epoch>,
    current_epoch: Epoch,
    stale_after_seconds: f64,
) -> bool {
    let Some(sampled_epoch) = sampled_epoch else {
        return true;
    };
    let stale_after_seconds = finite_positive_or(stale_after_seconds, 0.0);

    epoch_delta_seconds(sampled_epoch, current_epoch).abs() >= stale_after_seconds
}

/// Samples a provider-backed trajectory between two epochs, inclusive.
///
/// `sample_count` is a segment count; the returned path contains
/// `sample_count + 1` epochs after enforcing a minimum of two segments.
///
/// # Errors
///
/// Returns the first error produced by the ephemeris provider while sampling
/// the requested path.
pub fn sample_trajectory_between<P>(
    provider: &P,
    target: BodyId,
    origin: BodyId,
    frame: FrameId,
    start_epoch: Epoch,
    end_epoch: Epoch,
    sample_count: usize,
) -> EphemerisResult<TrajectoryPath>
where
    P: EphemerisProvider + ?Sized,
{
    let sample_epochs = sample_epochs_inclusive(start_epoch, end_epoch, sample_count);
    let mut samples = Vec::with_capacity(sample_epochs.len());

    for sample_epoch in sample_epochs {
        let request = StateRequest::new(target, origin, frame.clone(), sample_epoch);
        samples.push(provider.state(&request)?);
    }

    Ok(TrajectoryPath::new(
        target,
        origin,
        frame,
        start_epoch,
        end_epoch,
        samples,
    ))
}

/// Returns evenly spaced epochs from `start_epoch` through `end_epoch`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "Trajectory samples use bounded segment counts; sub-nanosecond interpolation is not represented."
)]
pub fn sample_epochs_inclusive(
    start_epoch: Epoch,
    end_epoch: Epoch,
    sample_count: usize,
) -> Vec<Epoch> {
    let sample_count = sample_count.max(2);
    let duration_seconds = epoch_delta_seconds(start_epoch, end_epoch);

    (0..=sample_count)
        .map(|index| {
            let fraction = index as f64 / sample_count as f64;
            offset_epoch_seconds(start_epoch, duration_seconds * fraction)
        })
        .collect()
}

/// Returns `end - start` in seconds.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "This helper is for trajectory display windows where f64 second precision is sufficient."
)]
pub fn epoch_delta_seconds(start: Epoch, end: Epoch) -> f64 {
    let end = end.tdb_nanoseconds_since_j2000 as f64;
    let start = start.tdb_nanoseconds_since_j2000 as f64;

    (end - start) / NANOS_PER_SECOND_F64
}

/// Offsets an epoch by floating-point seconds with saturating nanosecond math.
#[must_use]
pub fn offset_epoch_seconds(epoch: Epoch, offset_seconds: f64) -> Epoch {
    Epoch::from_tdb_nanoseconds_since_j2000(
        epoch
            .tdb_nanoseconds_since_j2000
            .saturating_add(seconds_to_nanos_i128(offset_seconds)),
    )
}

fn finite_positive_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "The rounded value is clamped to the i128 range before conversion."
)]
fn seconds_to_nanos_i128(seconds: f64) -> i128 {
    if seconds.is_nan() {
        return 0;
    }

    let nanos = seconds * NANOS_PER_SECOND_F64;
    if nanos >= i128::MAX as f64 {
        i128::MAX
    } else if nanos <= i128::MIN as f64 {
        i128::MIN
    } else {
        nanos.round() as i128
    }
}

#[cfg(test)]
mod tests {
    use super::{
        epoch_delta_seconds, offset_epoch_seconds, sample_epochs_inclusive,
        sample_trajectory_between, trajectory_samples_are_stale, TrajectoryTrailConfig,
        AVERAGE_GREGORIAN_MONTH_SECONDS,
    };
    use crate::space::{
        BodyId, EphemerisProvider, EphemerisResult, Epoch, FrameId, FrameOrientation,
        OrientationRequest, StateRequest, StateVector, Vec3d,
    };

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn trail_config_builds_history_window() {
        let epoch = Epoch::from_tdb_nanoseconds_since_j2000(100_000_000_000_000);
        let config = TrajectoryTrailConfig::default();
        let start = config.start_epoch(epoch, 1);

        let delta_nanos = epoch.tdb_nanoseconds_since_j2000 - start.tdb_nanoseconds_since_j2000;
        assert!(delta_nanos > 2_600_000_i128 * 1_000_000_000);
        assert!(delta_nanos < 2_700_000_i128 * 1_000_000_000);
    }

    #[test]
    fn trail_config_clamps_months_and_sample_count() {
        let config = TrajectoryTrailConfig::default();

        assert_eq!(config.clamped_months(0), config.min_months);
        assert_eq!(config.clamped_months(999), config.max_months);
        assert_eq!(
            config.sample_count(config.min_months, 1.0e15),
            config.min_samples
        );
        assert_eq!(
            config.sample_count(config.max_months, 1.0),
            config.max_samples
        );
        assert!(config.sample_count(12, 31_557_600.0) > config.min_samples);
    }

    #[test]
    fn sample_epochs_are_inclusive_and_enforce_minimum_segments() {
        let start = Epoch::from_tdb_nanoseconds_since_j2000(0);
        let end = Epoch::from_tdb_nanoseconds_since_j2000(2_000_000_000);
        let epochs = sample_epochs_inclusive(start, end, 0);

        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs.first().copied(), Some(start));
        assert_eq!(
            epochs[1],
            Epoch::from_tdb_nanoseconds_since_j2000(1_000_000_000)
        );
        assert_eq!(epochs.last().copied(), Some(end));
    }

    #[test]
    fn epoch_offset_and_delta_roundtrip_seconds() {
        let start = Epoch::from_tdb_nanoseconds_since_j2000(42);
        let end = offset_epoch_seconds(start, 12.5);

        assert_close(epoch_delta_seconds(start, end), 12.5);
    }

    #[test]
    fn stale_check_uses_absolute_epoch_delta() {
        let epoch = Epoch::from_tdb_nanoseconds_since_j2000(50_000_000_000_000);
        let stale_epoch = offset_epoch_seconds(epoch, -86_400.0);

        assert!(trajectory_samples_are_stale(None, epoch, 86_400.0));
        assert!(!trajectory_samples_are_stale(Some(epoch), epoch, 86_400.0));
        assert!(trajectory_samples_are_stale(
            Some(stale_epoch),
            epoch,
            AVERAGE_GREGORIAN_MONTH_SECONDS.min(86_400.0)
        ));
    }

    #[test]
    fn sample_trajectory_between_queries_provider_at_inclusive_epochs() {
        let start = Epoch::from_tdb_nanoseconds_since_j2000(0);
        let end = Epoch::from_tdb_nanoseconds_since_j2000(2_000_000_000);

        let result = sample_trajectory_between(
            &LinearProvider,
            BodyId::EARTH,
            BodyId::SUN,
            FrameId::j2000(),
            start,
            end,
            2,
        );
        assert!(result.is_ok(), "linear provider should return all samples");
        let Ok(path) = result else {
            return;
        };

        assert_eq!(path.samples.len(), 3);
        assert_close(path.samples[0].position_km.x, 0.0);
        assert_close(path.samples[1].position_km.x, 1.0);
        assert_close(path.samples[2].position_km.x, 2.0);
        assert!(path.sample_metadata_matches());
    }

    struct LinearProvider;

    impl EphemerisProvider for LinearProvider {
        fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector> {
            Ok(StateVector::relative_to(
                request.target,
                request.origin,
                request.frame.clone(),
                request.epoch,
                Vec3d::new(epoch_delta_seconds(Epoch::J2000, request.epoch), 0.0, 0.0),
                Vec3d::ZERO,
            ))
        }

        fn orientation(&self, request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
            Ok(FrameOrientation::identity(
                request.from.clone(),
                request.to.clone(),
                request.epoch,
            ))
        }
    }
}
