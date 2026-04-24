use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

use crate::source::TileProjection;

/// Default XYZ tile edge size in pixels.
pub const DEFAULT_TILE_SIZE: f64 = 256.0;
/// Maximum latitude supported by the Web Mercator projection.
pub const WEB_MERCATOR_MAX_LAT_DEG: f64 = 85.051_128_78;

/// Small 2D vector used for pixel-space and world-space map math.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2d {
    /// Horizontal component.
    pub x: f64,
    /// Vertical component.
    pub y: f64,
}

impl Vec2d {
    /// Creates a vector from explicit components.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Creates a vector where both components have the same value.
    #[must_use]
    pub const fn splat(value: f64) -> Self {
        Self { x: value, y: value }
    }
}

impl Add for Vec2d {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2d {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Vec2d {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

/// Geographic longitude and latitude in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLat {
    /// Longitude in degrees, typically normalized to `[-180, 180)`.
    pub lon_deg: f64,
    /// Latitude in degrees, clamped to the Web Mercator-supported range.
    pub lat_deg: f64,
}

impl Default for LonLat {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

impl LonLat {
    /// Creates a geographic coordinate from degree values.
    #[must_use]
    pub const fn new(lon_deg: f64, lat_deg: f64) -> Self {
        Self { lon_deg, lat_deg }
    }

    /// Normalizes longitude and clamps latitude to the Web Mercator range.
    #[must_use]
    pub fn clamped(self) -> Self {
        self.clamped_for_projection(TileProjection::WebMercator)
    }

    /// Normalizes longitude and clamps latitude for a map projection.
    #[must_use]
    pub fn clamped_for_projection(self, projection: TileProjection) -> Self {
        let max_latitude = match projection {
            TileProjection::WebMercator => WEB_MERCATOR_MAX_LAT_DEG,
            TileProjection::Equirectangular => 90.0,
        };

        Self {
            lon_deg: wrap_longitude(self.lon_deg),
            lat_deg: self.lat_deg.clamp(-max_latitude, max_latitude),
        }
    }
}

/// Wraps a longitude into the standard `[-180, 180)` interval.
#[must_use]
pub fn wrap_longitude(lon_deg: f64) -> f64 {
    (lon_deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Returns the full projected world width in pixels at `zoom`.
///
/// The same value is used for the world height because the Web Mercator map
/// pyramid is square in projected pixel space.
#[must_use]
pub fn map_size(tile_size: f64, zoom: f64) -> f64 {
    tile_size * 2_f64.powf(zoom)
}

/// Returns the full projected world size in pixels for a map projection.
///
/// Web Mercator uses a square `1x1` root matrix. Equirectangular planetary
/// sources use a `2x1` root matrix so the projected world has a two-to-one
/// aspect ratio.
#[must_use]
pub fn projected_world_size(projection: TileProjection, tile_size: f64, zoom: f64) -> Vec2d {
    let height = map_size(tile_size, zoom);
    let width = match projection {
        TileProjection::WebMercator => height,
        TileProjection::Equirectangular => height * 2.0,
    };

    Vec2d::new(width, height)
}

/// Projects geographic coordinates into Web Mercator world pixels.
///
/// The returned point is expressed in the infinite repeated world-pixel space
/// for the given `zoom`, where one world copy is `map_size(tile_size, zoom)`
/// pixels wide and tall.
#[must_use]
pub fn lon_lat_to_world_pixel(lon_lat: LonLat, zoom: f64, tile_size: f64) -> Vec2d {
    lon_lat_to_world_pixel_for_projection(lon_lat, TileProjection::WebMercator, zoom, tile_size)
}

/// Projects geographic coordinates into world pixels for a map projection.
#[must_use]
pub fn lon_lat_to_world_pixel_for_projection(
    lon_lat: LonLat,
    projection: TileProjection,
    zoom: f64,
    tile_size: f64,
) -> Vec2d {
    let lon_lat = lon_lat.clamped_for_projection(projection);
    let world_size = projected_world_size(projection, tile_size, zoom);
    let x = (lon_lat.lon_deg + 180.0) / 360.0 * world_size.x;

    let y = match projection {
        TileProjection::WebMercator => web_mercator_lat_to_world_y(lon_lat.lat_deg, world_size.y),
        TileProjection::Equirectangular => (90.0 - lon_lat.lat_deg) / 180.0 * world_size.y,
    };

    Vec2d::new(x, y)
}

fn web_mercator_lat_to_world_y(lat_deg: f64, world_height: f64) -> f64 {
    let latitude_sine = lat_deg.to_radians().sin().clamp(-0.9999, 0.9999);

    (0.5 - ((1.0 + latitude_sine) / (1.0 - latitude_sine)).ln() / (4.0 * PI)) * world_height
}

/// Converts Web Mercator world pixels back into geographic coordinates.
///
/// Longitudes are wrapped into the standard interval and latitudes are clamped
/// to the Web Mercator-supported range.
#[must_use]
pub fn world_pixel_to_lon_lat(world: Vec2d, zoom: f64, tile_size: f64) -> LonLat {
    world_pixel_to_lon_lat_for_projection(world, TileProjection::WebMercator, zoom, tile_size)
}

/// Converts projected world pixels back into geographic coordinates.
#[must_use]
pub fn world_pixel_to_lon_lat_for_projection(
    world: Vec2d,
    projection: TileProjection,
    zoom: f64,
    tile_size: f64,
) -> LonLat {
    let world_size = projected_world_size(projection, tile_size, zoom);
    let normalized_x = world.x / world_size.x;
    let normalized_y = world.y / world_size.y;

    let lon_deg = wrap_longitude(normalized_x * 360.0 - 180.0);
    let lat_deg = match projection {
        TileProjection::WebMercator => {
            let mercator_y = PI * (1.0 - 2.0 * normalized_y);
            mercator_y.sinh().atan().to_degrees()
        }
        TileProjection::Equirectangular => 90.0 - normalized_y * 180.0,
    };

    LonLat::new(lon_deg, lat_deg).clamped_for_projection(projection)
}
