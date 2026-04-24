use crate::tiles::{TileId, MAX_TILE_ZOOM};

/// Mean Earth radius used by Ferrisium body metadata, in meters.
pub const EARTH_MEAN_RADIUS_METERS: u32 = 6_371_008;
/// Mean Moon radius used by Ferrisium body metadata, in meters.
pub const MOON_MEAN_RADIUS_METERS: u32 = 1_737_400;
/// Mean Mercury radius used by Ferrisium body metadata, in meters.
pub const MERCURY_MEAN_RADIUS_METERS: u32 = 2_439_700;
/// Mean Venus radius used by Ferrisium body metadata, in meters.
pub const VENUS_MEAN_RADIUS_METERS: u32 = 6_051_800;
/// Mean Mars radius used by Ferrisium body metadata, in meters.
pub const MARS_MEAN_RADIUS_METERS: u32 = 3_389_500;
/// Mean Io radius used by Ferrisium body metadata, in meters.
pub const IO_MEAN_RADIUS_METERS: u32 = 1_821_600;
/// Mean Europa radius used by Ferrisium body metadata, in meters.
pub const EUROPA_MEAN_RADIUS_METERS: u32 = 1_560_800;
/// Mean Ganymede radius used by Ferrisium body metadata, in meters.
pub const GANYMEDE_MEAN_RADIUS_METERS: u32 = 2_631_200;
/// Mean Callisto radius used by Ferrisium body metadata, in meters.
pub const CALLISTO_MEAN_RADIUS_METERS: u32 = 2_410_300;
/// Mean Titan radius used by Ferrisium body metadata, in meters.
pub const TITAN_MEAN_RADIUS_METERS: u32 = 2_574_730;
/// Mean Dione radius used by Ferrisium body metadata, in meters.
pub const DIONE_MEAN_RADIUS_METERS: u32 = 561_400;
/// Mean Enceladus radius used by Ferrisium body metadata, in meters.
pub const ENCELADUS_MEAN_RADIUS_METERS: u32 = 252_100;
/// Mean Iapetus radius used by Ferrisium body metadata, in meters.
pub const IAPETUS_MEAN_RADIUS_METERS: u32 = 734_500;
/// Mean Mimas radius used by Ferrisium body metadata, in meters.
pub const MIMAS_MEAN_RADIUS_METERS: u32 = 198_200;
/// Mean Rhea radius used by Ferrisium body metadata, in meters.
pub const RHEA_MEAN_RADIUS_METERS: u32 = 763_800;
/// Mean Tethys radius used by Ferrisium body metadata, in meters.
pub const TETHYS_MEAN_RADIUS_METERS: u32 = 531_100;
/// Mean Ceres radius used by Ferrisium body metadata, in meters.
pub const CERES_MEAN_RADIUS_METERS: u32 = 469_700;
/// Conservative Mapbox Satellite max zoom with documented global coverage.
pub const MAPBOX_SATELLITE_GLOBAL_MAX_ZOOM: u8 = 16;
/// Maximum zoom accepted by Mapbox's classic raster tile endpoint.
pub const MAPBOX_RASTER_TILE_MAX_ZOOM: u8 = 30;
/// Maximum zoom accepted by Mapbox's style-raster static tile endpoint.
pub const MAPBOX_STYLE_RASTER_MAX_ZOOM: u8 = 22;
/// Default max zoom for Mapbox style-raster tiles in Ferrisium examples.
pub const MAPBOX_STYLE_RASTER_DEFAULT_MAX_ZOOM: u8 = 16;

// Attribution is stored with each source so examples and apps can expose the
// provider credit alongside rendered tiles.
const NASA_GIBS_ATTRIBUTION: &str = "Imagery provided by NASA Global Imagery Browse Services (GIBS), part of NASA's Earth Science Data and Information System (ESDIS).";
// Keep the Mapbox string generic because exact attribution presentation is
// application-specific and may need a UI treatment outside the tile source.
const MAPBOX_ATTRIBUTION: &str = "Mapbox and OpenStreetMap contributors";
// NASA Trek serves public planetary WMTS imagery with CORS-enabled tile URLs.
const NASA_TREK_ATTRIBUTION: &str = "Imagery provided by NASA Solar System Treks.";

/// Regular spherical bodies with built-in NASA Solar System Treks raster sources.
///
/// Ferrisium exposes only Trek products that fit its current spherical
/// equirectangular tile path. Irregular targets that need shape models, such
/// as Phobos, Phoebe, Vesta, Bennu, and Ryugu, are intentionally excluded until
/// renderer support can represent their non-spherical geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NasaTrekRegularBody {
    /// Mercury, using Mercury Trek global MESSENGER MDIS imagery.
    Mercury,
    /// Venus, using Venus Trek Magellan-derived topography imagery.
    Venus,
    /// Earth's Moon, using Moon Trek LRO WAC imagery.
    Moon,
    /// Mars, using Mars Trek global Viking imagery.
    Mars,
    /// Jupiter's moon Io.
    Io,
    /// Jupiter's moon Europa.
    Europa,
    /// Jupiter's moon Ganymede.
    Ganymede,
    /// Jupiter's moon Callisto.
    Callisto,
    /// Saturn's moon Titan.
    Titan,
    /// Saturn's moon Dione.
    Dione,
    /// Saturn's moon Enceladus.
    Enceladus,
    /// Saturn's moon Iapetus.
    Iapetus,
    /// Saturn's moon Mimas.
    Mimas,
    /// Saturn's moon Rhea.
    Rhea,
    /// Saturn's moon Tethys.
    Tethys,
    /// The dwarf planet Ceres.
    Ceres,
}

/// Metadata for one built-in regular-body NASA Trek raster source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NasaTrekRegularBodyDescriptor {
    /// Registry value represented by this descriptor.
    pub body: NasaTrekRegularBody,
    /// Stable lowercase token suitable for URLs and demo query parameters.
    pub slug: &'static str,
    /// Human-readable body name.
    pub display_name: &'static str,
    /// NAIF-compatible body center identifier.
    pub naif_id: i32,
    /// IAU body-fixed frame name used by SPICE/ANISE kernels.
    pub iau_frame_name: &'static str,
    /// Mean spherical radius used by Ferrisium, in meters.
    pub mean_radius_meters: u32,
    /// NASA Trek XYZ-style WMTS URL template for the default global layer.
    pub url_template: &'static str,
    /// Inclusive maximum Trek zoom level for the default global layer.
    pub max_zoom: u8,
}

/// All regular spherical bodies currently exposed through built-in NASA Trek sources.
pub const NASA_TREK_REGULAR_BODIES: &[NasaTrekRegularBody] = &[
    NasaTrekRegularBody::Mercury,
    NasaTrekRegularBody::Venus,
    NasaTrekRegularBody::Moon,
    NasaTrekRegularBody::Mars,
    NasaTrekRegularBody::Io,
    NasaTrekRegularBody::Europa,
    NasaTrekRegularBody::Ganymede,
    NasaTrekRegularBody::Callisto,
    NasaTrekRegularBody::Titan,
    NasaTrekRegularBody::Dione,
    NasaTrekRegularBody::Enceladus,
    NasaTrekRegularBody::Iapetus,
    NasaTrekRegularBody::Mimas,
    NasaTrekRegularBody::Rhea,
    NasaTrekRegularBody::Tethys,
    NasaTrekRegularBody::Ceres,
];

impl NasaTrekRegularBody {
    /// Returns every built-in regular-body Trek source in deterministic UI order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        NASA_TREK_REGULAR_BODIES
    }

    /// Returns the descriptor for this body.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "The built-in Trek registry is intentionally colocated so source metadata stays auditable."
    )]
    pub const fn descriptor(self) -> NasaTrekRegularBodyDescriptor {
        match self {
            Self::Mercury => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "mercury",
                display_name: "Mercury",
                naif_id: 199,
                iau_frame_name: "IAU_MERCURY",
                mean_radius_meters: MERCURY_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Mercury/EQ/Mercury_MESSENGER_MDIS_Basemap_EnhancedColor_Mosaic_Global_665m/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 7,
            },
            Self::Venus => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "venus",
                display_name: "Venus",
                naif_id: 299,
                iau_frame_name: "IAU_VENUS",
                mean_radius_meters: VENUS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Venus/EQ/Clrtopogrd/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 2,
            },
            Self::Moon => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "moon",
                display_name: "Moon",
                naif_id: 301,
                iau_frame_name: "IAU_MOON",
                mean_radius_meters: MOON_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Moon/EQ/LRO_WAC_Mosaic_Global_303ppd_v02/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 8,
            },
            Self::Mars => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "mars",
                display_name: "Mars",
                naif_id: 499,
                iau_frame_name: "IAU_MARS",
                mean_radius_meters: MARS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Mars/EQ/Mars_Viking_MDIM21_ClrMosaic_global_232m/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 7,
            },
            Self::Io => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "io",
                display_name: "Io",
                naif_id: 501,
                iau_frame_name: "IAU_IO",
                mean_radius_meters: IO_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Io/EQ/Io_GalileoSSI_Voyager_Global_Mosaic_ClrMerge_1km/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 4,
            },
            Self::Europa => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "europa",
                display_name: "Europa",
                naif_id: 502,
                iau_frame_name: "IAU_EUROPA",
                mean_radius_meters: EUROPA_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Europa/EQ/20150218_europa_global_map_20000x10000/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 5,
            },
            Self::Ganymede => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "ganymede",
                display_name: "Ganymede",
                naif_id: 503,
                iau_frame_name: "IAU_GANYMEDE",
                mean_radius_meters: GANYMEDE_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Ganymede/EQ/Ganymede_Voyager_GalileoSSI_global_mosaic_1km/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 5,
            },
            Self::Callisto => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "callisto",
                display_name: "Callisto",
                naif_id: 504,
                iau_frame_name: "IAU_CALLISTO",
                mean_radius_meters: CALLISTO_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Callisto/EQ/Callisto_Voyager_GalileoSSI_global_mosaic_1km/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 5,
            },
            Self::Titan => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "titan",
                display_name: "Titan",
                naif_id: 606,
                iau_frame_name: "IAU_TITAN",
                mean_radius_meters: TITAN_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Titan/EQ/Titan_global_32ppd_ColorRatio_v2/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 4,
            },
            Self::Dione => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "dione",
                display_name: "Dione",
                naif_id: 604,
                iau_frame_name: "IAU_DIONE",
                mean_radius_meters: DIONE_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Dione/EQ/DI_120723_DLR_basemap/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 5,
            },
            Self::Enceladus => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "enceladus",
                display_name: "Enceladus",
                naif_id: 602,
                iau_frame_name: "IAU_ENCELADUS",
                mean_radius_meters: ENCELADUS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Enceladus/EQ/Enceladus_Cassini_ISS_Global_Mosaic_100m_HPF/1.0.0/default/default028mm/{z}/{y}/{x}.png",
                max_zoom: 5,
            },
            Self::Iapetus => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "iapetus",
                display_name: "Iapetus",
                naif_id: 608,
                iau_frame_name: "IAU_IAPETUS",
                mean_radius_meters: IAPETUS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Iapetus/EQ/IA_080529_DLR_basemap/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 4,
            },
            Self::Mimas => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "mimas",
                display_name: "Mimas",
                naif_id: 601,
                iau_frame_name: "IAU_MIMAS",
                mean_radius_meters: MIMAS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Mimas/EQ/MI_170630_DLR_basemap/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 4,
            },
            Self::Rhea => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "rhea",
                display_name: "Rhea",
                naif_id: 605,
                iau_frame_name: "IAU_RHEA",
                mean_radius_meters: RHEA_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Rhea/EQ/RH_120803_DLR_basemap/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 4,
            },
            Self::Tethys => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "tethys",
                display_name: "Tethys",
                naif_id: 603,
                iau_frame_name: "IAU_TETHYS",
                mean_radius_meters: TETHYS_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Tethys/EQ/TE_131203_DLR_basemap/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 4,
            },
            Self::Ceres => NasaTrekRegularBodyDescriptor {
                body: self,
                slug: "ceres",
                display_name: "Ceres",
                naif_id: 2_000_001,
                iau_frame_name: "IAU_CERES",
                mean_radius_meters: CERES_MEAN_RADIUS_METERS,
                url_template: "https://trek.nasa.gov/tiles/Ceres/EQ/Ceres_Dawn_FC_HAMO_ClrShade_DLR_Global_60ppd_Oct2016/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
                max_zoom: 5,
            },
        }
    }

    /// Parses a stable lowercase body token such as `europa` or `ceres`.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "mercury" => Some(Self::Mercury),
            "venus" => Some(Self::Venus),
            "moon" | "luna" => Some(Self::Moon),
            "mars" => Some(Self::Mars),
            "io" => Some(Self::Io),
            "europa" => Some(Self::Europa),
            "ganymede" => Some(Self::Ganymede),
            "callisto" => Some(Self::Callisto),
            "titan" => Some(Self::Titan),
            "dione" => Some(Self::Dione),
            "enceladus" => Some(Self::Enceladus),
            "iapetus" => Some(Self::Iapetus),
            "mimas" => Some(Self::Mimas),
            "rhea" => Some(Self::Rhea),
            "tethys" => Some(Self::Tethys),
            "ceres" => Some(Self::Ceres),
            _ => None,
        }
    }

    /// Resolves a NAIF body-center identifier to a built-in regular Trek body.
    #[must_use]
    pub const fn from_naif_id(naif_id: i32) -> Option<Self> {
        match naif_id {
            199 => Some(Self::Mercury),
            299 => Some(Self::Venus),
            301 => Some(Self::Moon),
            499 => Some(Self::Mars),
            501 => Some(Self::Io),
            502 => Some(Self::Europa),
            503 => Some(Self::Ganymede),
            504 => Some(Self::Callisto),
            606 => Some(Self::Titan),
            604 => Some(Self::Dione),
            602 => Some(Self::Enceladus),
            608 => Some(Self::Iapetus),
            601 => Some(Self::Mimas),
            605 => Some(Self::Rhea),
            603 => Some(Self::Tethys),
            2_000_001 => Some(Self::Ceres),
            _ => None,
        }
    }

    /// Resolves a body-center or supported planet-system barycenter NAIF ID.
    ///
    /// Some higher-level demo focus targets use planet-system barycenters for
    /// ephemeris paths while rendering the corresponding regular body surface.
    #[must_use]
    pub const fn from_body_or_system_naif_id(naif_id: i32) -> Option<Self> {
        match naif_id {
            1 | 199 => Some(Self::Mercury),
            2 | 299 => Some(Self::Venus),
            4 | 499 => Some(Self::Mars),
            _ => Self::from_naif_id(naif_id),
        }
    }

    /// Returns this body's stable lowercase token.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        self.descriptor().slug
    }

    /// Returns this body's display name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        self.descriptor().display_name
    }

    /// Returns this body's NAIF-compatible body-center ID.
    #[must_use]
    pub const fn naif_id(self) -> i32 {
        self.descriptor().naif_id
    }

    /// Returns this body's IAU body-fixed frame name.
    #[must_use]
    pub const fn iau_frame_name(self) -> &'static str {
        self.descriptor().iau_frame_name
    }

    /// Returns the mean spherical radius used by Ferrisium, in meters.
    #[must_use]
    pub const fn mean_radius_meters(self) -> u32 {
        self.descriptor().mean_radius_meters
    }

    /// Returns the planetary-body metadata for this Trek body.
    #[must_use]
    pub fn planetary_body(self) -> PlanetaryBody {
        PlanetaryBody::nasa_trek_regular_body(self)
    }
}

/// Declares the coordinate projection used by a raster tile source.
///
/// Ferrisium's 2D and globe tile-selection math supports Web Mercator Earth
/// sources and equirectangular planetary sources. Renderers may still choose to
/// reject a source if the active mode does not support that body or projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileProjection {
    /// Spherical Web Mercator / EPSG:3857 tiles, usually addressed as XYZ.
    WebMercator,
    /// Geographic/equirectangular tiles with linear longitude and latitude.
    Equirectangular,
}

/// Describes the planetary body a tile source is mapped onto.
///
/// Current renderers keep camera and globe radius in renderer state rather
/// than deriving them from this metadata. The metadata exists so future
/// planetary providers can be selected explicitly without overloading source
/// names or URL templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryBody {
    /// Human-readable body name.
    pub name: String,
    /// Mean body radius in meters.
    pub mean_radius_meters: u32,
}

impl PlanetaryBody {
    /// Returns Earth body metadata.
    #[must_use]
    pub fn earth() -> Self {
        Self::new("Earth", EARTH_MEAN_RADIUS_METERS)
    }

    /// Returns Moon body metadata.
    #[must_use]
    pub fn moon() -> Self {
        Self::new("Moon", MOON_MEAN_RADIUS_METERS)
    }

    /// Returns Mercury body metadata.
    #[must_use]
    pub fn mercury() -> Self {
        Self::new("Mercury", MERCURY_MEAN_RADIUS_METERS)
    }

    /// Returns Venus body metadata.
    #[must_use]
    pub fn venus() -> Self {
        Self::new("Venus", VENUS_MEAN_RADIUS_METERS)
    }

    /// Returns Mars body metadata.
    #[must_use]
    pub fn mars() -> Self {
        Self::new("Mars", MARS_MEAN_RADIUS_METERS)
    }

    /// Returns Io body metadata.
    #[must_use]
    pub fn io() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Io)
    }

    /// Returns Europa body metadata.
    #[must_use]
    pub fn europa() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Europa)
    }

    /// Returns Ganymede body metadata.
    #[must_use]
    pub fn ganymede() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Ganymede)
    }

    /// Returns Callisto body metadata.
    #[must_use]
    pub fn callisto() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Callisto)
    }

    /// Returns Titan body metadata.
    #[must_use]
    pub fn titan() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Titan)
    }

    /// Returns Dione body metadata.
    #[must_use]
    pub fn dione() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Dione)
    }

    /// Returns Enceladus body metadata.
    #[must_use]
    pub fn enceladus() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Enceladus)
    }

    /// Returns Iapetus body metadata.
    #[must_use]
    pub fn iapetus() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Iapetus)
    }

    /// Returns Mimas body metadata.
    #[must_use]
    pub fn mimas() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Mimas)
    }

    /// Returns Rhea body metadata.
    #[must_use]
    pub fn rhea() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Rhea)
    }

    /// Returns Tethys body metadata.
    #[must_use]
    pub fn tethys() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Tethys)
    }

    /// Returns Ceres body metadata.
    #[must_use]
    pub fn ceres() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Ceres)
    }

    /// Returns body metadata from the regular-body NASA Trek registry.
    #[must_use]
    pub fn nasa_trek_regular_body(body: NasaTrekRegularBody) -> Self {
        let descriptor = body.descriptor();
        Self::new(descriptor.display_name, descriptor.mean_radius_meters)
    }

    /// Creates metadata for an arbitrary spherical body.
    #[must_use]
    pub fn new(name: impl Into<String>, mean_radius_meters: u32) -> Self {
        Self {
            name: name.into(),
            mean_radius_meters,
        }
    }
}

/// Describes a raster tile source shared by Ferrisium renderers.
///
/// The current implementation targets standard XYZ-style URL templates with
/// `{z}`, `{x}`, and `{y}` placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSource {
    /// URL template used to produce tile requests.
    pub url_template: String,
    /// Attribution text that applications should surface to end users.
    pub attribution: String,
    /// Minimum supported zoom level.
    pub min_zoom: u8,
    /// Maximum supported zoom level.
    pub max_zoom: u8,
    /// Logical tile edge size used by projection math.
    pub tile_size: u16,
    /// Fetched raster edge size in physical image pixels.
    ///
    /// High-DPI sources may return more pixels than the logical tile extent
    /// covers. For example, a 256-pixel Web Mercator tile requested at `@2x`
    /// has `tile_size == 256` and `raster_tile_size == 512`.
    pub raster_tile_size: u16,
    /// Native projection used by this tile source.
    pub projection: TileProjection,
    /// Planetary body the tile source represents.
    pub body: PlanetaryBody,
}

impl TileSource {
    /// Creates a tile source from an XYZ URL template.
    #[must_use]
    pub fn xyz(url_template: impl Into<String>) -> Self {
        Self {
            url_template: url_template.into(),
            attribution: String::new(),
            min_zoom: 0,
            max_zoom: 19,
            tile_size: 256,
            raster_tile_size: 256,
            projection: TileProjection::WebMercator,
            body: PlanetaryBody::earth(),
        }
    }

    /// Creates a public NASA GIBS Blue Marble source in Web Mercator.
    ///
    /// This source is useful as a no-key default for globe demos. It is a
    /// beautiful global raster, but its maximum zoom is intentionally low
    /// compared with commercial satellite basemaps.
    #[must_use]
    pub fn nasa_gibs_blue_marble() -> Self {
        Self::xyz(
            "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/BlueMarble_NextGeneration/default/GoogleMapsCompatible_Level8/{z}/{y}/{x}.jpeg",
        )
        .with_attribution(NASA_GIBS_ATTRIBUTION)
        .with_zoom_range(0, 8)
    }

    /// Creates a NASA Moon Trek LRO WAC global mosaic source.
    ///
    /// NASA Trek uses a two-to-one equirectangular WMTS matrix where zoom 0 has
    /// two columns and one row. This source therefore requires a globe renderer
    /// that supports [`TileProjection::Equirectangular`].
    #[must_use]
    pub fn nasa_trek_moon_lro_wac() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Moon)
    }

    /// Creates a NASA Mercury Trek MESSENGER MDIS enhanced-color global source.
    ///
    /// NASA Trek exposes this product through the same two-to-one
    /// equirectangular WMTS matrix used by the Moon and Mars sources. The
    /// colors are enhanced to emphasize surface differences and should not be
    /// treated as human-eye true color.
    #[must_use]
    pub fn nasa_trek_mercury_mdis_enhanced_color() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Mercury)
    }

    /// Creates a NASA Venus Trek Magellan colorized topography source.
    ///
    /// Venus surface imagery is radar-derived because its dense atmosphere
    /// hides the surface from conventional optical cameras. This Magellan RSS
    /// product is a low-resolution false-color topography mosaic intended for
    /// visual context, not human-eye surface color.
    #[must_use]
    pub fn nasa_trek_venus_magellan_color_topography() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Venus)
    }

    /// Creates a NASA Venus Trek Magellan gap-filled hillshade source.
    ///
    /// This full-globe grayscale hillshade is radar/topography-derived, not an
    /// optical image. It is useful when an app wants a less stylized Venus
    /// surface than the colorized topography helper.
    #[must_use]
    pub fn nasa_trek_venus_magellan_hillshade() -> Self {
        Self::xyz(
            "https://trek.nasa.gov/tiles/Venus/EQ/Venus_Magellan_Hillshade_Global_4641m_gapfilled_v02/1.0.0/default/default028mm/{z}/{y}/{x}.png",
        )
        .with_attribution(NASA_TREK_ATTRIBUTION)
        .with_zoom_range(0, 4)
        .with_projection(TileProjection::Equirectangular)
        .with_body(PlanetaryBody::venus())
    }

    /// Creates a NASA Mars Trek Viking VIS global color mosaic source.
    ///
    /// NASA Trek uses a two-to-one equirectangular WMTS matrix where zoom 0 has
    /// two columns and one row. This optical color mosaic is a better visual
    /// Mars default than topographic color-shaded products, but it is still a
    /// processed/colorized global product rather than raw true-color sensor
    /// data.
    #[must_use]
    pub fn nasa_trek_mars_viking_color() -> Self {
        Self::nasa_trek_regular_body(NasaTrekRegularBody::Mars)
    }

    /// Creates a built-in regular-body NASA Trek equirectangular source.
    ///
    /// The registry is limited to global Trek products that fit Ferrisium's
    /// current spherical tile renderers. Irregular bodies are deliberately not
    /// returned by this helper.
    #[must_use]
    pub fn nasa_trek_regular_body(body: NasaTrekRegularBody) -> Self {
        let descriptor = body.descriptor();
        Self::xyz(descriptor.url_template)
            .with_attribution(NASA_TREK_ATTRIBUTION)
            .with_zoom_range(0, descriptor.max_zoom)
            .with_projection(TileProjection::Equirectangular)
            .with_body(PlanetaryBody::nasa_trek_regular_body(body))
    }

    /// Creates a NASA Mars Trek MGS MOLA color-shaded global source.
    ///
    /// Like other Trek equirectangular products, zoom 0 has two columns and one
    /// row, then each zoom level doubles both axes. This source emphasizes
    /// terrain through color hillshading and should not be treated as optical
    /// surface color.
    #[must_use]
    pub fn nasa_trek_mars_mola_color() -> Self {
        Self::xyz(
            "https://trek.nasa.gov/tiles/Mars/EQ/Mars_MGS_MOLA_ClrShade_merge_global_463m/1.0.0/default/default028mm/{z}/{y}/{x}.jpg",
        )
        .with_attribution(NASA_TREK_ATTRIBUTION)
        .with_zoom_range(0, 7)
        .with_projection(TileProjection::Equirectangular)
        .with_body(PlanetaryBody::mars())
    }

    /// Creates a Mapbox Satellite source using a caller-provided public token.
    ///
    /// Browser applications should pass a Mapbox `pk` token supplied by the
    /// end user or application environment. Do not embed secret `sk` tokens in
    /// browser-distributed wasm. The default max zoom is the conservative
    /// global-coverage zoom; use [`Self::mapbox_satellite_with_max_zoom`] when
    /// an app wants to opt into regional higher zooms.
    #[must_use]
    pub fn mapbox_satellite(access_token: impl AsRef<str>) -> Self {
        Self::mapbox_satellite_with_max_zoom(access_token, MAPBOX_SATELLITE_GLOBAL_MAX_ZOOM)
    }

    /// Creates a Mapbox Satellite source with an explicit maximum zoom.
    ///
    /// Mapbox has global satellite coverage through zoom 16, with higher
    /// coverage in some regions. This helper makes that tradeoff explicit
    /// instead of hiding it in a single hard-coded source constructor.
    #[must_use]
    pub fn mapbox_satellite_with_max_zoom(access_token: impl AsRef<str>, max_zoom: u8) -> Self {
        let access_token = access_token.as_ref();
        Self::xyz(format!(
            "https://api.mapbox.com/v4/mapbox.satellite/{{z}}/{{x}}/{{y}}@2x.jpg90?access_token={access_token}"
        ))
        .with_attribution(MAPBOX_ATTRIBUTION)
        .with_zoom_range(0, max_zoom.min(MAPBOX_RASTER_TILE_MAX_ZOOM))
        .with_raster_tile_size(512)
    }

    /// Creates raster tiles from a Mapbox Studio style.
    ///
    /// This uses the Mapbox Static Tiles API with 256 logical tiles and `@2x`
    /// imagery, which keeps Ferrisium's XYZ tile math aligned while fetching
    /// 512-pixel rasters for sharper display.
    #[must_use]
    pub fn mapbox_style_raster(
        username: impl AsRef<str>,
        style_id: impl AsRef<str>,
        access_token: impl AsRef<str>,
    ) -> Self {
        Self::mapbox_style_raster_with_max_zoom(
            username,
            style_id,
            access_token,
            MAPBOX_STYLE_RASTER_DEFAULT_MAX_ZOOM,
        )
    }

    /// Creates raster tiles from a Mapbox Studio style with an explicit maximum zoom.
    #[must_use]
    pub fn mapbox_style_raster_with_max_zoom(
        username: impl AsRef<str>,
        style_id: impl AsRef<str>,
        access_token: impl AsRef<str>,
        max_zoom: u8,
    ) -> Self {
        let username = username.as_ref();
        let style_id = style_id.as_ref();
        let access_token = access_token.as_ref();

        Self::xyz(format!(
            "https://api.mapbox.com/styles/v1/{username}/{style_id}/tiles/256/{{z}}/{{x}}/{{y}}@2x?access_token={access_token}"
        ))
        .with_attribution(MAPBOX_ATTRIBUTION)
        .with_zoom_range(0, max_zoom.min(MAPBOX_STYLE_RASTER_MAX_ZOOM))
        .with_raster_tile_size(512)
    }

    /// Sets the attribution string associated with this source.
    #[must_use]
    pub fn with_attribution(mut self, attribution: impl Into<String>) -> Self {
        self.attribution = attribution.into();
        self
    }

    /// Sets the supported zoom range for this source.
    ///
    /// Both bounds are inclusive. Bounds are normalized into ascending order
    /// and clamped to Ferrisium's addressable tile pyramid.
    #[must_use]
    pub fn with_zoom_range(mut self, min_zoom: u8, max_zoom: u8) -> Self {
        (self.min_zoom, self.max_zoom) = normalized_zoom_range(min_zoom, max_zoom);
        self
    }

    /// Returns this source's supported zoom range after normalizing invalid
    /// field values into Ferrisium's addressable tile pyramid.
    #[must_use]
    pub fn normalized_zoom_range(&self) -> (u8, u8) {
        normalized_zoom_range(self.min_zoom, self.max_zoom)
    }

    /// Sets the logical source tile edge size used by projection math.
    ///
    /// This also updates the raster tile size because non-HiDPI sources use
    /// the same value for logical and physical tile size. Use
    /// [`Self::with_raster_tile_size`] afterwards for `@2x` style sources.
    #[must_use]
    pub fn with_tile_size(mut self, tile_size: u16) -> Self {
        let tile_size = tile_size.max(1);
        self.tile_size = tile_size;
        self.raster_tile_size = tile_size;
        self
    }

    /// Returns the source tile edge size with invalid manual field values
    /// normalized away.
    #[must_use]
    pub fn normalized_tile_size(&self) -> u16 {
        self.tile_size.max(1)
    }

    /// Sets the fetched raster tile edge size in physical image pixels.
    #[must_use]
    pub fn with_raster_tile_size(mut self, raster_tile_size: u16) -> Self {
        self.raster_tile_size = raster_tile_size.max(1);
        self
    }

    /// Returns the fetched raster tile edge size with invalid manual field
    /// values normalized away.
    #[must_use]
    pub fn normalized_raster_tile_size(&self) -> u16 {
        self.raster_tile_size.max(1)
    }

    /// Sets the native projection metadata for this source.
    #[must_use]
    pub fn with_projection(mut self, projection: TileProjection) -> Self {
        self.projection = projection;
        self
    }

    /// Sets the planetary body metadata for this source.
    #[must_use]
    pub fn with_body(mut self, body: PlanetaryBody) -> Self {
        self.body = body;
        self
    }

    /// Returns the expanded URL if the tile is inside the source zoom range.
    #[must_use]
    pub fn checked_tile_url(&self, tile: TileId) -> Option<String> {
        let (min_zoom, max_zoom) = self.normalized_zoom_range();
        (min_zoom..=max_zoom)
            .contains(&tile.z)
            .then(|| self.tile_url(tile))
    }

    /// Expands the URL template for a specific tile request.
    ///
    /// This is a pure string expander. Use [`Self::checked_tile_url`] when
    /// source zoom limits should be enforced.
    #[must_use]
    pub fn tile_url(&self, tile: TileId) -> String {
        self.url_template
            .replace("{z}", &tile.z.to_string())
            .replace("{x}", &tile.x.to_string())
            .replace("{y}", &tile.y.to_string())
    }
}

fn normalized_zoom_range(min_zoom: u8, max_zoom: u8) -> (u8, u8) {
    let min_zoom = min_zoom.min(MAX_TILE_ZOOM);
    let max_zoom = max_zoom.min(MAX_TILE_ZOOM);

    (min_zoom.min(max_zoom), min_zoom.max(max_zoom))
}
