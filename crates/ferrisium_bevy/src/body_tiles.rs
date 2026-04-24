//! Fixed-zoom tiled surfaces for secondary celestial bodies.
//!
//! The primary Earth globe uses the camera-responsive mixed-LOD tile path in
//! `crate::tile_globe`. This module handles simpler secondary body surfaces
//! such as the demo Moon: a body entity declares one raster source and one
//! fixed zoom level, and Ferrisium renders that full global tile matrix with
//! the same curved patch material path used by the primary globe.

mod render;
mod requests;
mod state;
mod systems;

pub use state::{TiledBodySurface, TiledBodySurfaceRadiusPolicy};

pub(crate) use requests::drain_body_surface_tile_fetch_results;
pub(crate) use state::{BodySurfaceTileFetchChannel, BodySurfaceTileState};
pub(crate) use systems::sync_tiled_body_surfaces;
