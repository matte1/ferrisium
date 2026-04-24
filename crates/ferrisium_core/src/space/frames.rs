//! Stable celestial body and reference-frame identifiers.
//!
//! The identifiers in this module mirror NAIF/SPICE conventions where useful
//! while remaining plain Rust data, so Bevy and non-Bevy crates can share the
//! same body/frame vocabulary.

/// NAIF-compatible body identifier.
///
/// ANISE and SPICE both use NAIF integer identifiers for natural bodies,
/// barycenters, and spacecraft. Ferrisium stores the same identifier at the
/// core boundary so ephemeris providers can map into engine state without
/// string matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId {
    /// NAIF integer identifier.
    pub naif_id: i32,
}

impl BodyId {
    /// Solar-system barycenter.
    pub const SOLAR_SYSTEM_BARYCENTER: Self = Self::naif(0);
    /// Mercury system barycenter.
    pub const MERCURY_BARYCENTER: Self = Self::naif(1);
    /// Venus system barycenter.
    pub const VENUS_BARYCENTER: Self = Self::naif(2);
    /// Sun body center.
    pub const SUN: Self = Self::naif(10);
    /// Mercury body center.
    pub const MERCURY: Self = Self::naif(199);
    /// Venus body center.
    pub const VENUS: Self = Self::naif(299);
    /// Earth body center.
    pub const EARTH: Self = Self::naif(399);
    /// Moon body center.
    pub const MOON: Self = Self::naif(301);
    /// Mars system barycenter.
    pub const MARS_BARYCENTER: Self = Self::naif(4);
    /// Mars body center.
    pub const MARS: Self = Self::naif(499);
    /// Io body center.
    pub const IO: Self = Self::naif(501);
    /// Europa body center.
    pub const EUROPA: Self = Self::naif(502);
    /// Ganymede body center.
    pub const GANYMEDE: Self = Self::naif(503);
    /// Callisto body center.
    pub const CALLISTO: Self = Self::naif(504);
    /// Mimas body center.
    pub const MIMAS: Self = Self::naif(601);
    /// Enceladus body center.
    pub const ENCELADUS: Self = Self::naif(602);
    /// Tethys body center.
    pub const TETHYS: Self = Self::naif(603);
    /// Dione body center.
    pub const DIONE: Self = Self::naif(604);
    /// Rhea body center.
    pub const RHEA: Self = Self::naif(605);
    /// Titan body center.
    pub const TITAN: Self = Self::naif(606);
    /// Iapetus body center.
    pub const IAPETUS: Self = Self::naif(608);
    /// Ceres body center.
    pub const CERES: Self = Self::naif(2_000_001);

    /// Creates a NAIF-compatible body identifier.
    #[must_use]
    pub const fn naif(naif_id: i32) -> Self {
        Self { naif_id }
    }
}

/// Reference-frame identifier.
///
/// Frame names are kept provider-neutral in `ferrisium_core`. ANISE adapters
/// can translate these names into provider-specific frame handles and may also
/// create additional names for loaded mission kernels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId {
    /// Provider-visible frame name.
    pub name: String,
}

impl FrameId {
    /// Returns the J2000 inertial frame.
    #[must_use]
    pub fn j2000() -> Self {
        Self::named("J2000")
    }

    /// Returns the IAU body-fixed Earth frame.
    #[must_use]
    pub fn iau_earth() -> Self {
        Self::named("IAU_EARTH")
    }

    /// Returns the high-precision Earth-fixed ITRF93 frame.
    #[must_use]
    pub fn itrf93() -> Self {
        Self::named("ITRF93")
    }

    /// Returns the IAU body-fixed Moon frame.
    #[must_use]
    pub fn iau_moon() -> Self {
        Self::named("IAU_MOON")
    }

    /// Returns the IAU body-fixed Mercury frame.
    #[must_use]
    pub fn iau_mercury() -> Self {
        Self::named("IAU_MERCURY")
    }

    /// Returns the IAU body-fixed Venus frame.
    #[must_use]
    pub fn iau_venus() -> Self {
        Self::named("IAU_VENUS")
    }

    /// Returns the high-precision Moon Mean Earth body-fixed frame.
    #[must_use]
    pub fn moon_me() -> Self {
        Self::named("MOON_ME")
    }

    /// Returns the high-precision DE440 Moon Mean Earth body-fixed frame.
    #[must_use]
    pub fn moon_me_de440() -> Self {
        Self::named("MOON_ME_DE440")
    }

    /// Returns the high-precision Moon Principal Axes body-fixed frame.
    #[must_use]
    pub fn moon_pa() -> Self {
        Self::named("MOON_PA")
    }

    /// Returns the high-precision DE440 Moon Principal Axes body-fixed frame.
    #[must_use]
    pub fn moon_pa_de440() -> Self {
        Self::named("MOON_PA_DE440")
    }

    /// Returns the IAU body-fixed Mars frame.
    #[must_use]
    pub fn iau_mars() -> Self {
        Self::named("IAU_MARS")
    }

    /// Returns the IAU body-fixed Io frame.
    #[must_use]
    pub fn iau_io() -> Self {
        Self::named("IAU_IO")
    }

    /// Returns the IAU body-fixed Europa frame.
    #[must_use]
    pub fn iau_europa() -> Self {
        Self::named("IAU_EUROPA")
    }

    /// Returns the IAU body-fixed Ganymede frame.
    #[must_use]
    pub fn iau_ganymede() -> Self {
        Self::named("IAU_GANYMEDE")
    }

    /// Returns the IAU body-fixed Callisto frame.
    #[must_use]
    pub fn iau_callisto() -> Self {
        Self::named("IAU_CALLISTO")
    }

    /// Returns the IAU body-fixed Titan frame.
    #[must_use]
    pub fn iau_titan() -> Self {
        Self::named("IAU_TITAN")
    }

    /// Returns the IAU body-fixed Dione frame.
    #[must_use]
    pub fn iau_dione() -> Self {
        Self::named("IAU_DIONE")
    }

    /// Returns the IAU body-fixed Enceladus frame.
    #[must_use]
    pub fn iau_enceladus() -> Self {
        Self::named("IAU_ENCELADUS")
    }

    /// Returns the IAU body-fixed Iapetus frame.
    #[must_use]
    pub fn iau_iapetus() -> Self {
        Self::named("IAU_IAPETUS")
    }

    /// Returns the IAU body-fixed Mimas frame.
    #[must_use]
    pub fn iau_mimas() -> Self {
        Self::named("IAU_MIMAS")
    }

    /// Returns the IAU body-fixed Rhea frame.
    #[must_use]
    pub fn iau_rhea() -> Self {
        Self::named("IAU_RHEA")
    }

    /// Returns the IAU body-fixed Tethys frame.
    #[must_use]
    pub fn iau_tethys() -> Self {
        Self::named("IAU_TETHYS")
    }

    /// Returns the IAU body-fixed Ceres frame.
    #[must_use]
    pub fn iau_ceres() -> Self {
        Self::named("IAU_CERES")
    }

    /// Creates a frame identifier from a provider-visible name.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Default for FrameId {
    fn default() -> Self {
        Self::j2000()
    }
}

/// Broad category for a reference frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceFrameKind {
    /// Non-rotating frame suitable for ephemeris state vectors.
    Inertial,
    /// Rotating frame fixed to a body's surface.
    BodyFixed,
    /// Observer-local east/north/up or similar tangent frame.
    LocalTangent,
}

/// Describes a reference frame and the body it is centered on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceFrame {
    /// Stable frame identifier.
    pub id: FrameId,
    /// Body or barycenter at the origin of this frame.
    pub center: BodyId,
    /// High-level frame category.
    pub kind: ReferenceFrameKind,
}

impl ReferenceFrame {
    /// Creates an inertial frame centered on the provided body or barycenter.
    #[must_use]
    pub fn inertial(id: FrameId, center: BodyId) -> Self {
        Self {
            id,
            center,
            kind: ReferenceFrameKind::Inertial,
        }
    }

    /// Creates a body-fixed frame centered on the provided body.
    #[must_use]
    pub fn body_fixed(id: FrameId, center: BodyId) -> Self {
        Self {
            id,
            center,
            kind: ReferenceFrameKind::BodyFixed,
        }
    }

    /// Creates an observer-local tangent frame centered on the provided body.
    #[must_use]
    pub fn local_tangent(id: FrameId, center: BodyId) -> Self {
        Self {
            id,
            center,
            kind: ReferenceFrameKind::LocalTangent,
        }
    }
}
