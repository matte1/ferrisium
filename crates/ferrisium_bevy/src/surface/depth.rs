//! Shared visual-depth policy for spherical body surfaces.
//!
//! Surface renderers should not invent independent offsets for raster patches,
//! polar fillers, and draped decals. This module keeps those layers ordered in
//! one place so browser depth precision and visual-only decal lift evolve
//! together.

use bevy::prelude::{AlphaMode, Color, StandardMaterial};

/// Ordered visual layers on a spherical surface.
///
/// Custom globe-surface drawing systems can use these layers to share
/// Ferrisium's depth-spacing convention with raster tiles, polar caps, filled
/// decals, and outline decals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SphericalSurfaceLayer {
    /// Body raster/color surface.
    RasterSurface,
    /// Projection filler geometry such as Web Mercator polar caps.
    PolarCap,
    /// Filled draped data decals such as H3 coverage polygons.
    DrapedFill,
    /// Line/outline decals drawn above filled draped data.
    DrapedOutline,
}

// Browser WebGL depth buffers need a render-unit lift that scales with physical
// body radius. The absolute floor keeps compact demo spheres stable without
// making their overlays visibly float.
const MIN_DRAPED_FILL_OFFSET_UNITS: f32 = 0.01;
const MIN_DRAPED_OUTLINE_GAP_UNITS: f32 = 0.005;
const DRAPED_FILL_RADIUS_RATIO: f32 = 0.000_7;
const DRAPED_OUTLINE_GAP_RADIUS_RATIO: f32 = 0.000_18;

// Material bias is deliberately secondary to geometry separation. It remains
// useful for transparent decals over tessellated raster patches but should not
// be the only protection against z-fighting.
const DRAPED_FILL_DEPTH_BIAS: f32 = 4.0;
const DRAPED_OUTLINE_DEPTH_BIAS: f32 = 8.0;

const MIN_GLOBE_NEAR_PLANE: f32 = 0.000_01;
const GLOBE_NEAR_PLANE_SURFACE_CLEARANCE_RATIO: f32 = 0.2;

/// Returns the default visual lift for one spherical surface layer.
#[must_use]
pub fn spherical_surface_layer_offset(base_radius: f32, layer: SphericalSurfaceLayer) -> f32 {
    match layer {
        SphericalSurfaceLayer::RasterSurface | SphericalSurfaceLayer::PolarCap => 0.0,
        SphericalSurfaceLayer::DrapedFill => draped_fill_offset(base_radius, 0.0),
        SphericalSurfaceLayer::DrapedOutline => {
            draped_fill_offset(base_radius, 0.0) + draped_outline_gap(base_radius)
        }
    }
}

/// Returns the radius for a non-user-offset surface layer.
#[must_use]
pub fn spherical_surface_layer_radius(base_radius: f32, layer: SphericalSurfaceLayer) -> f32 {
    stable_surface_radius(base_radius) + spherical_surface_layer_offset(base_radius, layer)
}

/// Returns the radius for a draped decal fill.
#[must_use]
pub fn draped_fill_radius(base_radius: f32, requested_offset_units: f32) -> f32 {
    stable_surface_radius(base_radius) + draped_fill_offset(base_radius, requested_offset_units)
}

/// Returns the radius for a draped decal outline.
#[must_use]
pub fn draped_outline_radius(base_radius: f32, requested_offset_units: f32) -> f32 {
    draped_fill_radius(base_radius, requested_offset_units) + draped_outline_gap(base_radius)
}

/// Returns the material depth bias for a spherical surface layer.
#[must_use]
pub const fn spherical_surface_depth_bias(layer: SphericalSurfaceLayer) -> f32 {
    match layer {
        SphericalSurfaceLayer::RasterSurface | SphericalSurfaceLayer::PolarCap => 0.0,
        SphericalSurfaceLayer::DrapedFill => DRAPED_FILL_DEPTH_BIAS,
        SphericalSurfaceLayer::DrapedOutline => DRAPED_OUTLINE_DEPTH_BIAS,
    }
}

/// Applies the shared layer bias to a material.
pub fn apply_spherical_surface_depth_bias(
    material: &mut StandardMaterial,
    layer: SphericalSurfaceLayer,
) {
    material.depth_bias = spherical_surface_depth_bias(layer);
}

/// Applies the shared transparent-decal material contract for draped surface layers.
pub fn apply_spherical_decal_material(
    material: &mut StandardMaterial,
    color: Color,
    layer: SphericalSurfaceLayer,
) {
    debug_assert!(matches!(
        layer,
        SphericalSurfaceLayer::DrapedFill | SphericalSurfaceLayer::DrapedOutline
    ));
    material.base_color = color;
    material.emissive = color.into();
    material.alpha_mode = AlphaMode::Blend;
    material.cull_mode = None;
    material.unlit = true;
    apply_spherical_surface_depth_bias(material, layer);
}

/// Returns a near plane that preserves depth precision without clipping the
/// visible spherical surface layers.
#[must_use]
pub(crate) fn globe_surface_near_plane(
    camera_surface_distance: f32,
    base_radius: f32,
    far_plane: f32,
) -> f32 {
    let camera_surface_distance = camera_surface_distance.max(0.0);
    let layer_lift =
        spherical_surface_layer_offset(base_radius, SphericalSurfaceLayer::DrapedOutline);
    let available_clearance = (camera_surface_distance - layer_lift)
        .max(camera_surface_distance * GLOBE_NEAR_PLANE_SURFACE_CLEARANCE_RATIO);
    let near =
        (available_clearance * GLOBE_NEAR_PLANE_SURFACE_CLEARANCE_RATIO).max(MIN_GLOBE_NEAR_PLANE);

    near.min(far_plane * 0.5).max(MIN_GLOBE_NEAR_PLANE)
}

/// Maximum default visual lift used by built-in surface decals.
#[cfg(test)]
#[must_use]
pub(crate) fn max_default_surface_visual_lift(base_radius: f32) -> f32 {
    spherical_surface_layer_offset(base_radius, SphericalSurfaceLayer::DrapedOutline)
}

fn draped_fill_offset(base_radius: f32, requested_offset_units: f32) -> f32 {
    requested_offset_units
        .max(stable_surface_radius(base_radius) * DRAPED_FILL_RADIUS_RATIO)
        .max(MIN_DRAPED_FILL_OFFSET_UNITS)
}

fn draped_outline_gap(base_radius: f32) -> f32 {
    (stable_surface_radius(base_radius) * DRAPED_OUTLINE_GAP_RADIUS_RATIO)
        .max(MIN_DRAPED_OUTLINE_GAP_UNITS)
}

fn stable_surface_radius(radius: f32) -> f32 {
    radius.max(f32::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_spherical_decal_material, draped_fill_radius, draped_outline_radius,
        globe_surface_near_plane, max_default_surface_visual_lift, spherical_surface_depth_bias,
        spherical_surface_layer_radius, SphericalSurfaceLayer,
    };
    use bevy::prelude::{AlphaMode, Color, StandardMaterial};

    #[test]
    fn spherical_surface_layers_are_ordered() {
        let radius = 6_371.0;
        let raster = spherical_surface_layer_radius(radius, SphericalSurfaceLayer::RasterSurface);
        let cap = spherical_surface_layer_radius(radius, SphericalSurfaceLayer::PolarCap);
        let fill = draped_fill_radius(radius, 0.0);
        let outline = draped_outline_radius(radius, 0.0);

        assert!((raster - radius).abs() <= f32::EPSILON);
        assert!((cap - radius).abs() <= f32::EPSILON);
        assert!(fill > raster);
        assert!(outline > fill);
    }

    #[test]
    fn requested_draped_offset_is_a_minimum() {
        let radius = 6_371.0;
        let requested = 8.0;
        let fill = draped_fill_radius(radius, requested);

        assert!(fill >= radius + requested);
    }

    #[test]
    fn depth_bias_keeps_decals_ordered_after_geometry_lift() {
        assert!(
            spherical_surface_depth_bias(SphericalSurfaceLayer::RasterSurface).abs()
                <= f32::EPSILON
        );
        assert!(
            spherical_surface_depth_bias(SphericalSurfaceLayer::DrapedOutline)
                > spherical_surface_depth_bias(SphericalSurfaceLayer::DrapedFill)
        );
    }

    #[test]
    fn decal_material_uses_transparent_unlit_depth_contract() {
        let color = Color::srgba(0.2, 0.6, 0.9, 0.55);
        let mut material = StandardMaterial::default();

        apply_spherical_decal_material(&mut material, color, SphericalSurfaceLayer::DrapedFill);

        assert_eq!(material.base_color, color);
        assert_eq!(material.alpha_mode, AlphaMode::Blend);
        assert!(material.cull_mode.is_none());
        assert!(material.unlit);
        assert!(
            (material.depth_bias - spherical_surface_depth_bias(SphericalSurfaceLayer::DrapedFill))
                .abs()
                <= f32::EPSILON
        );
    }

    #[test]
    fn globe_near_plane_uses_available_surface_clearance() {
        let radius = 6_371.0;
        let surface_distance = 3_000.0;
        let near = globe_surface_near_plane(surface_distance, radius, 650_000.0);

        assert!(near > 0.1);
        assert!(near < surface_distance - max_default_surface_visual_lift(radius));
    }
}
