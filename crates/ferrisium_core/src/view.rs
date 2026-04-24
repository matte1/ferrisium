use crate::coords::{
    lon_lat_to_world_pixel_for_projection, projected_world_size,
    world_pixel_to_lon_lat_for_projection, LonLat, Vec2d, DEFAULT_TILE_SIZE,
};
use crate::source::TileProjection;
use crate::tiles::MAX_TILE_ZOOM;

/// Shared view state for a projected map view.
///
/// This is the renderer-agnostic camera model used to translate between screen
/// coordinates, world pixels, and lon/lat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewState {
    /// Geographic center of the current view.
    pub center: LonLat,
    /// Continuous zoom value. Tile selection uses [`Self::discrete_zoom`].
    pub zoom: f64,
    /// Current viewport size in pixels.
    pub viewport_size: Vec2d,
    /// Tile size in pixels for the active raster source.
    pub tile_size: f64,
    /// Native projection used by the active map source.
    pub projection: TileProjection,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            center: LonLat::default(),
            zoom: 2.0,
            viewport_size: Vec2d::new(1280.0, 720.0),
            tile_size: DEFAULT_TILE_SIZE,
            projection: TileProjection::WebMercator,
        }
    }
}

impl ViewState {
    // Keep all public zoom operations on the same bounded range so the map
    // math and tile pyramid never diverge.
    fn clamped_zoom(&self) -> f64 {
        self.zoom.clamp(0.0, f64::from(MAX_TILE_ZOOM))
    }

    /// Returns a finite positive tile size for view and tile math.
    ///
    /// `tile_size` is public so applications can build a `ViewState` directly.
    /// Normalizing at each math boundary prevents invalid manual values from
    /// producing infinities or unbounded tile ranges.
    #[must_use]
    pub fn normalized_tile_size(&self) -> f64 {
        if self.tile_size.is_finite() && self.tile_size > 0.0 {
            self.tile_size
        } else {
            1.0
        }
    }

    // Convert an absolute screen position into a signed offset from the
    // viewport center so pan/zoom math can stay symmetric around the view.
    fn screen_offset_from_center(&self, screen_position: Vec2d) -> Vec2d {
        Vec2d::new(
            screen_position.x - self.viewport_size.x * 0.5,
            screen_position.y - self.viewport_size.y * 0.5,
        )
    }

    /// Returns the integer zoom level used for tile selection.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Zoom is clamped to the supported tile pyramid range before conversion."
    )]
    pub fn discrete_zoom(&self) -> u8 {
        self.clamped_zoom().round() as u8
    }

    /// Returns the minimum zoom that keeps one world copy at least as large as
    /// the current viewport in both dimensions.
    ///
    /// Ferrisium uses this to keep the map covering the window instead of
    /// allowing the entire projected world to shrink into a movable sprite.
    #[must_use]
    pub fn min_covering_zoom(&self) -> f64 {
        let safe_tile_size = self.normalized_tile_size();
        let root_world_size = projected_world_size(self.projection, safe_tile_size, 0.0);
        let required_x = (self.viewport_size.x.max(1.0) / root_world_size.x.max(1.0)).log2();
        let required_y = (self.viewport_size.y.max(1.0) / root_world_size.y.max(1.0)).log2();

        required_x
            .max(required_y)
            .clamp(0.0, f64::from(MAX_TILE_ZOOM))
    }

    /// Converts a screen position in pixels to a geographic lon/lat.
    ///
    /// This uses the view's continuous zoom value rather than the rounded tile
    /// zoom so pointer interactions remain stable while tiles crossfade between
    /// discrete pyramid levels.
    #[must_use]
    pub fn screen_to_lon_lat(&self, screen_position: Vec2d) -> LonLat {
        let zoom = self.clamped_zoom();
        let tile_size = self.normalized_tile_size();
        let center_world =
            lon_lat_to_world_pixel_for_projection(self.center, self.projection, zoom, tile_size);
        let world = center_world + self.screen_offset_from_center(screen_position);

        world_pixel_to_lon_lat_for_projection(world, self.projection, zoom, tile_size)
    }

    /// Pans the view by a pixel delta in screen space.
    ///
    /// Positive `delta_pixels.x` corresponds to the pointer moving to the
    /// right on screen, which shifts the geographic center west in map space.
    pub fn pan_by_pixels(&mut self, delta_pixels: Vec2d) {
        let zoom = self.clamped_zoom();
        let tile_size = self.normalized_tile_size();
        let center_world =
            lon_lat_to_world_pixel_for_projection(self.center, self.projection, zoom, tile_size);
        let next_center = world_pixel_to_lon_lat_for_projection(
            center_world - delta_pixels,
            self.projection,
            zoom,
            tile_size,
        );
        self.center = next_center;
    }

    /// Applies a zoom delta and clamps it to the supported tile pyramid range.
    ///
    /// The stored zoom remains continuous; only tile selection snaps to
    /// [`Self::discrete_zoom`].
    pub fn zoom_by_steps(&mut self, zoom_delta: f64) {
        self.zoom = (self.zoom + zoom_delta).clamp(0.0, f64::from(MAX_TILE_ZOOM));
    }

    /// Enforces the current viewport constraints on the view.
    ///
    /// This raises zoom to at least [`Self::min_covering_zoom`] and clamps the
    /// vertical center so the map stays pinned to the window rather than
    /// exposing empty space beyond the poles.
    pub fn constrain_to_viewport(&mut self) {
        self.zoom = self
            .zoom
            .clamp(self.min_covering_zoom(), f64::from(MAX_TILE_ZOOM));

        let zoom = self.clamped_zoom();
        let tile_size = self.normalized_tile_size();
        let map_world_size = projected_world_size(self.projection, tile_size, zoom);
        let half_viewport_height = self.viewport_size.y * 0.5;
        let mut center_world =
            lon_lat_to_world_pixel_for_projection(self.center, self.projection, zoom, tile_size);

        center_world.y = if self.viewport_size.y >= map_world_size.y {
            map_world_size.y * 0.5
        } else {
            center_world.y.clamp(
                half_viewport_height,
                map_world_size.y - half_viewport_height,
            )
        };

        self.center =
            world_pixel_to_lon_lat_for_projection(center_world, self.projection, zoom, tile_size);
    }

    /// Applies a zoom delta while keeping the geographic point under
    /// `screen_position` anchored to that same screen position.
    ///
    /// This is the map interaction used for cursor-centered wheel zooming.
    /// If `screen_position` is the viewport center, this is equivalent to a
    /// center-based zoom.
    pub fn zoom_about_screen_position(&mut self, screen_position: Vec2d, zoom_delta: f64) {
        let anchored_lon_lat = self.screen_to_lon_lat(screen_position);
        self.zoom =
            (self.zoom + zoom_delta).clamp(self.min_covering_zoom(), f64::from(MAX_TILE_ZOOM));
        let zoom = self.zoom;
        let tile_size = self.normalized_tile_size();
        let anchored_world = lon_lat_to_world_pixel_for_projection(
            anchored_lon_lat,
            self.projection,
            zoom,
            tile_size,
        );
        let center_world = anchored_world - self.screen_offset_from_center(screen_position);

        self.center =
            world_pixel_to_lon_lat_for_projection(center_world, self.projection, zoom, tile_size);
        self.constrain_to_viewport();
    }
}
