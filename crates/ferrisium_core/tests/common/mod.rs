#![allow(
    dead_code,
    reason = "Shared helpers are intentionally consumed by different integration test crates."
)]
#![allow(
    clippy::missing_panics_doc,
    reason = "Test helpers panic by design when invariants fail."
)]

use ferrisium_core::coords::{LonLat, DEFAULT_TILE_SIZE};
use proptest::strategy::{BoxedStrategy, Just, Strategy};

/// Geographic comparison tolerance in degrees for lon/lat round-trip tests.
pub const GEO_EPSILON: f64 = 1.0e-6;
/// Pixel-space pan comparison tolerance for accumulated floating-point math.
pub const PAN_EPSILON: f64 = 1.0e-5;

#[must_use]
pub fn wrapped_lon_distance(lhs: f64, rhs: f64) -> f64 {
    ((lhs - rhs + 180.0).rem_euclid(360.0) - 180.0).abs()
}

pub fn assert_lon_lat_close(lhs: LonLat, rhs: LonLat, epsilon: f64) {
    assert!(
        wrapped_lon_distance(lhs.lon_deg, rhs.lon_deg) <= epsilon,
        "longitude mismatch: lhs={}, rhs={}, epsilon={epsilon}",
        lhs.lon_deg,
        rhs.lon_deg
    );
    assert!(
        (lhs.lat_deg - rhs.lat_deg).abs() <= epsilon,
        "latitude mismatch: lhs={}, rhs={}, epsilon={epsilon}",
        lhs.lat_deg,
        rhs.lat_deg
    );
}

pub fn lon_strategy() -> BoxedStrategy<f64> {
    (-179_999_999_i32..180_000_000_i32)
        .prop_map(|value| f64::from(value) / 1_000_000.0)
        .boxed()
}

pub fn lat_strategy() -> BoxedStrategy<f64> {
    (-85_000_000_i32..85_000_000_i32)
        .prop_map(|value| f64::from(value) / 1_000_000.0)
        .boxed()
}

pub fn tile_size_strategy() -> BoxedStrategy<f64> {
    proptest::prop_oneof![Just(DEFAULT_TILE_SIZE), Just(512.0)].boxed()
}

pub fn zoom_strategy() -> BoxedStrategy<u8> {
    (0_u8..23_u8).boxed()
}
