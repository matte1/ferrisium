//! Epoch representation for celestial state queries.
//!
//! Time is stored as integer TDB nanoseconds from J2000 so provider adapters can
//! keep deterministic, loss-resistant request keys at the core boundary.

/// TDB epoch represented as integer nanoseconds from J2000.
///
/// The integer representation avoids accumulating precision loss in core time
/// stamps. Provider integrations such as ANISE can convert to and from their
/// richer time types at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Epoch {
    /// Nanoseconds in TDB after J2000.
    pub tdb_nanoseconds_since_j2000: i128,
}

impl Epoch {
    /// J2000 epoch.
    pub const J2000: Self = Self::from_tdb_nanoseconds_since_j2000(0);

    /// Creates an epoch from TDB nanoseconds after J2000.
    #[must_use]
    pub const fn from_tdb_nanoseconds_since_j2000(tdb_nanoseconds_since_j2000: i128) -> Self {
        Self {
            tdb_nanoseconds_since_j2000,
        }
    }
}
