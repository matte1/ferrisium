//! H3-facing types for Ferrisium.
//!
//! These types back Ferrisium's Bevy H3 overlay while keeping raw H3 index
//! validation, boundary conversion, and color normalization out of rendering
//! systems.

use std::borrow::Cow;

use ferrisium_core::coords::LonLat;
use geo::{MultiPolygon, Polygon};
use h3o::error::InvalidCellIndex;
use h3o::{CellIndex, LatLng, Resolution};

/// Logical description of an H3 layer in an application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H3Layer {
    name: Cow<'static, str>,
}

impl H3Layer {
    /// Creates a named H3 layer descriptor.
    #[must_use]
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the human-readable layer name used by the application.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Validated H3 cell index.
///
/// This wraps `h3o::CellIndex` so Ferrisium APIs do not accept arbitrary raw
/// index bits as cells. Raw `u64` values can still be imported with
/// [`TryFrom<u64>`], but invalid H3 modes and malformed cells are rejected at
/// the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct H3Cell {
    index: CellIndex,
}

impl H3Cell {
    /// Creates a Ferrisium cell from an already validated `h3o` cell index.
    #[must_use]
    pub const fn new(index: CellIndex) -> Self {
        Self { index }
    }

    /// Returns the wrapped `h3o` cell index.
    #[must_use]
    pub const fn index(self) -> CellIndex {
        self.index
    }

    /// Returns the raw H3 index bits.
    #[must_use]
    pub fn bits(self) -> u64 {
        self.index.into()
    }

    /// Returns the H3 resolution of this cell.
    #[must_use]
    pub const fn resolution(self) -> Resolution {
        self.index.resolution()
    }

    /// Returns the projected-map boundary polygons for this H3 cell.
    ///
    /// Most cells produce one polygon. Cells split by the antimeridian can
    /// produce multiple polygons so projected map renderers do not draw a long
    /// wraparound edge across the world.
    #[must_use]
    pub fn boundary_polygons(self) -> Vec<H3CellPolygon> {
        self.map_boundary_polygons()
    }

    /// Returns the projected-map boundary polygons for this H3 cell.
    ///
    /// This representation is clipped/split at the antimeridian when needed.
    /// Use [`Self::spherical_boundary`] for globe rendering, where artificial
    /// antimeridian clip edges would be visible.
    #[must_use]
    pub fn map_boundary_polygons(self) -> Vec<H3CellPolygon> {
        h3_cell_map_boundary_polygons(self)
    }

    /// Returns the unsplit spherical boundary ring for this H3 cell.
    ///
    /// The vertices are the raw H3 cell boundary in longitude/latitude order,
    /// without antimeridian clipping. Globe renderers should tessellate these
    /// edges as great-circle arcs on the sphere.
    #[must_use]
    pub fn spherical_boundary(self) -> H3CellBoundary {
        h3_cell_spherical_boundary(self)
    }

    /// Returns true when H3 assigns `lon_lat` to this cell's resolution and index.
    #[must_use]
    pub fn contains_lon_lat(self, lon_lat: LonLat) -> bool {
        h3_lat_lng(lon_lat).is_some_and(|point| point.to_cell(self.resolution()) == self.index)
    }

    /// Returns the spherical center point of this H3 cell.
    #[must_use]
    pub fn center_lon_lat(self) -> LonLat {
        let center = LatLng::from(self.index);
        LonLat::new(center.lng(), center.lat())
    }
}

impl From<CellIndex> for H3Cell {
    fn from(index: CellIndex) -> Self {
        Self::new(index)
    }
}

impl From<H3Cell> for CellIndex {
    fn from(cell: H3Cell) -> Self {
        cell.index
    }
}

impl From<H3Cell> for u64 {
    fn from(cell: H3Cell) -> Self {
        cell.bits()
    }
}

impl TryFrom<u64> for H3Cell {
    type Error = InvalidCellIndex;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        CellIndex::try_from(value).map(Self::new)
    }
}

/// Collection of H3 cell identifiers.
///
/// Collections preserve insertion order and do not deduplicate cells. Mixed
/// resolutions are also allowed. Renderer-facing code can layer stricter
/// validation on top when it needs set-like or single-resolution coverage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct H3Cells {
    cells: Vec<H3Cell>,
}

impl H3Cells {
    /// Builds a collection from an iterator of validated H3 cells.
    #[must_use]
    pub fn from_cells(iter: impl IntoIterator<Item = H3Cell>) -> Self {
        Self {
            cells: iter.into_iter().collect(),
        }
    }

    /// Builds a collection from already validated `h3o` cell indexes.
    #[must_use]
    pub fn from_indexes(iter: impl IntoIterator<Item = CellIndex>) -> Self {
        Self::from_cells(iter.into_iter().map(H3Cell::from))
    }

    /// Validates raw H3 cell IDs and builds a collection.
    ///
    /// # Errors
    ///
    /// Returns the first [`InvalidCellIndex`] reported by `h3o` when a raw
    /// value is not a valid H3 cell.
    pub fn try_from_raw(iter: impl IntoIterator<Item = u64>) -> Result<Self, InvalidCellIndex> {
        iter.into_iter()
            .map(H3Cell::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map(|cells| Self { cells })
    }

    /// Returns the cells in insertion order.
    #[must_use]
    pub fn cells(&self) -> &[H3Cell] {
        &self.cells
    }

    /// Iterates over the cells in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = H3Cell> + '_ {
        self.cells.iter().copied()
    }

    /// Returns the first collection cell that contains `lon_lat` by H3 membership.
    ///
    /// Mixed-resolution collections are supported by encoding the coordinate at
    /// each cell's own resolution. Duplicate cells preserve insertion-order
    /// behavior and return the first matching entry.
    #[must_use]
    pub fn cell_at_lon_lat(&self, lon_lat: LonLat) -> Option<(usize, H3Cell)> {
        self.cells
            .iter()
            .copied()
            .enumerate()
            .find(|(_, cell)| cell.contains_lon_lat(lon_lat))
    }

    /// Returns true when the collection has no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns the number of cells in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }
}

impl FromIterator<H3Cell> for H3Cells {
    fn from_iter<T: IntoIterator<Item = H3Cell>>(iter: T) -> Self {
        Self::from_cells(iter)
    }
}

impl FromIterator<CellIndex> for H3Cells {
    fn from_iter<T: IntoIterator<Item = CellIndex>>(iter: T) -> Self {
        Self::from_indexes(iter)
    }
}

/// Ordered per-cell fill colors for an [`H3Cells`] collection.
///
/// Colors are indexed by the matching cell's insertion order. A renderer that
/// receives fewer colors than cells should use its layer fill color for the
/// remaining cells.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct H3CellColors {
    colors: Vec<[f32; 4]>,
}

impl H3CellColors {
    /// Builds per-cell colors from normalized, non-premultiplied sRGBA values.
    ///
    /// Non-finite values are converted to `0.0`; finite values are clamped into
    /// `[0, 1]` so invalid colors cannot reach WebGL vertex attributes.
    #[must_use]
    pub fn from_rgba(iter: impl IntoIterator<Item = [f32; 4]>) -> Self {
        Self {
            colors: iter
                .into_iter()
                .map(|rgba| rgba.map(normalized_color_component))
                .collect(),
        }
    }

    /// Returns the stored colors in cell insertion order.
    #[must_use]
    pub fn colors(&self) -> &[[f32; 4]] {
        &self.colors
    }

    /// Returns the color for one cell index, falling back to the supplied style color.
    #[must_use]
    pub fn color_for_cell(&self, cell_index: usize, fallback: [f32; 4]) -> [f32; 4] {
        self.colors.get(cell_index).copied().unwrap_or(fallback)
    }

    /// Returns true when no per-cell colors are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Returns the number of per-cell colors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.colors.len()
    }
}

impl FromIterator<[f32; 4]> for H3CellColors {
    fn from_iter<T: IntoIterator<Item = [f32; 4]>>(iter: T) -> Self {
        Self::from_rgba(iter)
    }
}

/// One geographic polygon that bounds an H3 cell.
///
/// The vertices are longitude/latitude pairs in degrees. The closing vertex is
/// omitted; renderers should close the ring themselves when they need an outline.
#[derive(Debug, Clone, PartialEq)]
pub struct H3CellPolygon {
    vertices: Vec<LonLat>,
}

impl H3CellPolygon {
    /// Creates a polygon from an unclosed lon/lat vertex ring.
    #[must_use]
    pub fn new(vertices: Vec<LonLat>) -> Self {
        Self { vertices }
    }

    /// Returns the polygon vertices in boundary order.
    #[must_use]
    pub fn vertices(&self) -> &[LonLat] {
        &self.vertices
    }

    /// Consumes the polygon and returns its vertex ring.
    #[must_use]
    pub fn into_vertices(self) -> Vec<LonLat> {
        self.vertices
    }
}

/// One unsplit geographic boundary ring for an H3 cell.
///
/// The vertices are longitude/latitude pairs in degrees. The closing vertex is
/// omitted; globe renderers should close the ring themselves when drawing an
/// outline. Unlike [`H3CellPolygon`], this ring is not split at the
/// antimeridian and should be interpreted on the sphere.
#[derive(Debug, Clone, PartialEq)]
pub struct H3CellBoundary {
    vertices: Vec<LonLat>,
}

impl H3CellBoundary {
    /// Creates an unsplit boundary from an unclosed lon/lat vertex ring.
    #[must_use]
    pub fn new(vertices: Vec<LonLat>) -> Self {
        Self { vertices }
    }

    /// Returns the boundary vertices in H3 boundary order.
    #[must_use]
    pub fn vertices(&self) -> &[LonLat] {
        &self.vertices
    }

    /// Consumes the boundary and returns its vertex ring.
    #[must_use]
    pub fn into_vertices(self) -> Vec<LonLat> {
        self.vertices
    }
}

/// Returns projected-map boundary polygons for an H3 cell.
///
/// H3 cells crossing the antimeridian may return multiple polygons clipped to
/// `[-180, 180]`. This is the correct representation for projected map fills
/// and outlines. Use [`h3_cell_spherical_boundary`] for globe rendering.
#[must_use]
pub fn h3_cell_boundary_polygons(cell: H3Cell) -> Vec<H3CellPolygon> {
    h3_cell_map_boundary_polygons(cell)
}

/// Returns projected-map boundary polygons for an H3 cell.
///
/// H3 cells crossing the antimeridian may return multiple polygons clipped to
/// `[-180, 180]`. This is the correct representation for projected map fills
/// and outlines. Use [`h3_cell_spherical_boundary`] for globe rendering.
#[must_use]
pub fn h3_cell_map_boundary_polygons(cell: H3Cell) -> Vec<H3CellPolygon> {
    let polygons = MultiPolygon::<f64>::from(cell.index());

    polygons
        .0
        .iter()
        .filter_map(|polygon| {
            let vertices = polygon_vertices(polygon);
            (vertices.len() >= 3).then(|| H3CellPolygon::new(vertices))
        })
        .collect()
}

/// Returns the unsplit spherical boundary ring for an H3 cell.
#[must_use]
pub fn h3_cell_spherical_boundary(cell: H3Cell) -> H3CellBoundary {
    let vertices = cell
        .index()
        .boundary()
        .iter()
        .map(|point| LonLat::new(point.lng(), point.lat()))
        .collect();

    H3CellBoundary::new(vertices)
}

/// Minimal fill style for an H3 layer.
#[derive(Debug, Clone, PartialEq)]
pub struct H3Style {
    fill_rgba: [f32; 4],
    outline_rgba: [f32; 4],
}

impl H3Style {
    /// Creates a constant solid fill style without an outline.
    ///
    /// Components are interpreted as normalized, non-premultiplied sRGBA in
    /// `[0, 1]`. Non-finite values are converted to `0.0`; finite values are
    /// clamped into range so invalid colors cannot reach WebGL materials.
    #[must_use]
    pub fn solid(fill_rgba: [f32; 4]) -> Self {
        Self {
            fill_rgba: fill_rgba.map(normalized_color_component),
            outline_rgba: [0.0; 4],
        }
    }

    /// Sets a constant outline color for H3 cell boundaries.
    ///
    /// The outline is intended as a lightweight tiling guide. Renderer backends
    /// may draw it with platform-native line width limitations.
    #[must_use]
    pub fn with_outline(mut self, outline_rgba: [f32; 4]) -> Self {
        self.outline_rgba = outline_rgba.map(normalized_color_component);
        self
    }

    /// Returns the normalized sRGBA fill components.
    #[must_use]
    pub const fn fill_rgba(&self) -> [f32; 4] {
        self.fill_rgba
    }

    /// Returns the normalized sRGBA outline components.
    #[must_use]
    pub const fn outline_rgba(&self) -> [f32; 4] {
        self.outline_rgba
    }
}

fn normalized_color_component(component: f32) -> f32 {
    if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn polygon_vertices(polygon: &Polygon<f64>) -> Vec<LonLat> {
    let mut vertices = polygon
        .exterior()
        .0
        .iter()
        .map(|coord| LonLat::new(coord.x, coord.y))
        .collect::<Vec<_>>();

    if vertices.first() == vertices.last() {
        let _ = vertices.pop();
    }

    vertices
}

fn h3_lat_lng(lon_lat: LonLat) -> Option<LatLng> {
    LatLng::new(lon_lat.lat_deg, lon_lat.lon_deg).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        h3_cell_boundary_polygons, h3_cell_map_boundary_polygons, h3_cell_spherical_boundary,
        H3Cell, H3CellColors, H3Cells, H3Layer, H3Style,
    };
    use h3o::{CellIndex, Resolution};

    const RAW_TEST_CELL: u64 = 0x0837_506f_ffff_ffff_u64;
    const RAW_TRANSMERIDIAN_HEX: u64 = 0x0840_d9ed_ffff_ffff_u64;
    const RAW_TRANSMERIDIAN_PENTAGON: u64 = 0x0807_ffff_ffff_ffff_u64;

    fn assert_rgba_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "rgba mismatch: actual={actual}, expected={expected}"
            );
        }
    }

    fn is_antimeridian_lon(lon_deg: f64) -> bool {
        (lon_deg.abs() - 180.0).abs() <= f64::EPSILON
    }

    #[test]
    fn layer_name_is_exposed_without_public_mutability() {
        let layer = H3Layer::new("demo");

        assert_eq!(layer.name(), "demo");
    }

    #[test]
    fn raw_cells_are_validated_before_collection() {
        let cells = H3Cells::try_from_raw([RAW_TEST_CELL]);

        assert!(cells.is_ok());
        let cells = cells.unwrap_or_default();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells.cells()[0].bits(), RAW_TEST_CELL);
        assert_eq!(cells.cells()[0].resolution(), Resolution::Three);
    }

    #[test]
    fn invalid_raw_cells_are_rejected() {
        let cells = H3Cells::try_from_raw([0]);

        assert!(cells.is_err());
    }

    #[test]
    fn raw_cell_import_is_all_or_nothing() {
        let cells = H3Cells::try_from_raw([RAW_TEST_CELL, 0]);

        assert!(cells.is_err());
    }

    #[test]
    fn cells_can_be_built_from_validated_indexes() {
        let Ok(index) = CellIndex::try_from(RAW_TEST_CELL) else {
            return;
        };
        let cell = H3Cell::from(index);
        let cells = [cell].into_iter().collect::<H3Cells>();

        assert_eq!(cells.iter().collect::<Vec<_>>(), vec![cell]);
        assert!(!cells.is_empty());
    }

    #[test]
    fn cell_collections_can_be_built_from_h3o_indexes() {
        let Ok(index) = CellIndex::try_from(RAW_TEST_CELL) else {
            return;
        };

        let cells = H3Cells::from_indexes([index]);

        assert_eq!(cells.len(), 1);
        assert_eq!(cells.cells()[0].index(), index);
    }

    #[test]
    fn cell_collections_preserve_duplicates_and_order() {
        let Ok(index) = CellIndex::try_from(RAW_TEST_CELL) else {
            return;
        };
        let cell = H3Cell::from(index);
        let cells = H3Cells::from_cells([cell, cell]);

        assert_eq!(cells.len(), 2);
        assert_eq!(cells.iter().collect::<Vec<_>>(), vec![cell, cell]);
    }

    #[test]
    fn solid_style_normalizes_components() {
        let style = H3Style::solid([-1.0, 0.5, 2.0, f32::NAN]).with_outline([
            f32::INFINITY,
            0.25,
            0.75,
            2.0,
        ]);

        for (actual, expected) in style.fill_rgba().into_iter().zip([0.0, 0.5, 1.0, 0.0]) {
            assert!((actual - expected).abs() <= f32::EPSILON);
        }
        for (actual, expected) in style.outline_rgba().into_iter().zip([0.0, 0.25, 0.75, 1.0]) {
            assert!((actual - expected).abs() <= f32::EPSILON);
        }
    }

    #[test]
    fn solid_style_defaults_to_transparent_outline() {
        let style = H3Style::solid([1.0, 0.0, 0.0, 0.5]);

        assert!(style
            .outline_rgba()
            .into_iter()
            .all(|actual| actual.abs() <= f32::EPSILON));
    }

    #[test]
    fn cell_colors_normalize_components_and_fallback_by_cell_index() {
        let colors = H3CellColors::from_rgba([[1.2, 0.5, f32::NAN, -1.0]]);

        assert_eq!(colors.len(), 1);
        assert_eq!(colors.colors(), &[[1.0, 0.5, 0.0, 0.0]]);
        assert_rgba_close(
            colors.color_for_cell(0, [0.0, 0.0, 0.0, 1.0]),
            [1.0, 0.5, 0.0, 0.0],
        );
        assert_rgba_close(
            colors.color_for_cell(1, [0.1, 0.2, 0.3, 0.4]),
            [0.1, 0.2, 0.3, 0.4],
        );
    }

    #[test]
    fn cell_boundary_polygons_return_unclosed_lon_lat_rings() {
        let Ok(index) = CellIndex::try_from(RAW_TEST_CELL) else {
            return;
        };
        let polygons = h3_cell_boundary_polygons(H3Cell::from(index));

        assert!(!polygons.is_empty());
        assert!(polygons.iter().all(|polygon| polygon.vertices().len() >= 3));
        assert!(polygons
            .iter()
            .all(|polygon| polygon.vertices().first() != polygon.vertices().last()));
    }

    #[test]
    fn map_boundary_polygons_split_transmeridian_hexes() {
        let Ok(index) = CellIndex::try_from(RAW_TRANSMERIDIAN_HEX) else {
            return;
        };

        let polygons = h3_cell_map_boundary_polygons(H3Cell::from(index));

        assert_eq!(polygons.len(), 2);
        assert!(polygons.iter().all(|polygon| {
            polygon
                .vertices()
                .iter()
                .any(|vertex| is_antimeridian_lon(vertex.lon_deg))
        }));
        assert!(polygons
            .iter()
            .all(|polygon| polygon.vertices().first() != polygon.vertices().last()));
    }

    #[test]
    fn map_boundary_polygons_split_transmeridian_pentagons() {
        let Ok(index) = CellIndex::try_from(RAW_TRANSMERIDIAN_PENTAGON) else {
            return;
        };

        let polygons = h3_cell_map_boundary_polygons(H3Cell::from(index));
        let vertex_counts = polygons
            .iter()
            .map(|polygon| polygon.vertices().len())
            .collect::<Vec<_>>();

        assert_eq!(polygons.len(), 2);
        assert_eq!(vertex_counts, vec![3, 6]);
        assert!(polygons.iter().all(|polygon| {
            polygon
                .vertices()
                .iter()
                .any(|vertex| is_antimeridian_lon(vertex.lon_deg))
        }));
    }

    #[test]
    fn spherical_boundary_keeps_transmeridian_cells_unsplit() {
        let Ok(index) = CellIndex::try_from(RAW_TRANSMERIDIAN_HEX) else {
            return;
        };
        let cell = H3Cell::from(index);

        let boundary = h3_cell_spherical_boundary(cell);

        assert_eq!(boundary.vertices().len(), 6);
        assert_eq!(cell.spherical_boundary(), boundary);
        assert!(boundary
            .vertices()
            .windows(2)
            .any(|pair| (pair[0].lon_deg - pair[1].lon_deg).abs() > 180.0));
        assert!(boundary.vertices().first() != boundary.vertices().last());
    }

    #[test]
    fn h3_membership_resolves_cell_centers() {
        let Ok(index) = CellIndex::try_from(RAW_TRANSMERIDIAN_HEX) else {
            return;
        };
        let cell = H3Cell::from(index);
        let center = cell.center_lon_lat();
        let cells = H3Cells::from_cells([cell]);

        assert!(cell.contains_lon_lat(center));
        assert_eq!(cells.cell_at_lon_lat(center), Some((0, cell)));
    }
}
