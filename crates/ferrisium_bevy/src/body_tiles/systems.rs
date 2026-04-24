//! ECS orchestration for fixed-zoom secondary body surfaces.
//!
//! The system in this module validates each surface against its body placement,
//! resets stale per-body state when inputs change, queues missing tiles, and
//! delegates patch rendering to `render`.

use std::collections::HashSet;

use bevy::prelude::*;
use ferrisium_core::prelude::{NasaTrekRegularBody, PlanetaryBody};
use ferrisium_core::space::BodyId;

use crate::body_tiles::render::{despawn_body_surface, sync_body_surface_patches};
use crate::body_tiles::requests::{clamped_surface_zoom, queue_body_surface_tiles};
use crate::body_tiles::state::{
    BodySurfaceState, BodySurfaceTileFetchChannel, BodySurfaceTileState, TiledBodySurface,
    TiledBodySurfaceRadiusPolicy,
};
use crate::celestial::{CelestialBodyPlacement, CelestialFocus, CelestialSpace};
use crate::metric_visual::{MetricVisualRadius, MetricVisualRadiusPolicy};
use crate::source::supports_globe_renderer;

/// Synchronizes all entities carrying `TiledBodySurface` and `CelestialBodyPlacement`.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Bevy systems expose each ECS input as an explicit system parameter."
)]
pub(crate) fn sync_tiled_body_surfaces(
    mut commands: Commands<'_, '_>,
    celestial_focus: Res<'_, CelestialFocus>,
    celestial_space: Res<'_, CelestialSpace>,
    fetch_channel: Res<'_, BodySurfaceTileFetchChannel>,
    surfaces_query: Query<
        '_,
        '_,
        (
            Entity,
            &TiledBodySurface,
            &CelestialBodyPlacement,
            Option<&TiledBodySurfaceRadiusPolicy>,
            Option<&MetricVisualRadius>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
    >,
    mut tile_state: ResMut<'_, BodySurfaceTileState>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    tile_state.frame_index = tile_state.frame_index.saturating_add(1);

    let mut surfaces = surfaces_query
        .iter()
        .filter(|(_, _, _, _, _, visibility, inherited_visibility)| {
            body_surface_is_active(*visibility, *inherited_visibility)
        })
        .collect::<Vec<_>>();
    surfaces.sort_by_key(|(_, _, placement, _, _, _, _)| {
        body_focus_priority(placement.body.id, *celestial_focus)
    });

    let live_bodies = surfaces
        .iter()
        .map(|(entity, _, _, _, _, _, _)| *entity)
        .collect::<HashSet<_>>();
    let stale_bodies = tile_state
        .bodies
        .keys()
        .copied()
        .filter(|entity| !live_bodies.contains(entity))
        .collect::<Vec<_>>();
    for body in stale_bodies {
        despawn_body_surface(&mut commands, &mut tile_state, body);
    }

    for (body, surface, placement, radius_policy, existing_radius, _, _) in surfaces {
        if !supports_globe_renderer(&surface.source) {
            warn!(
                "tiled body source projection {:?} is not supported for {}",
                surface.source.projection, placement.body.name
            );
            despawn_body_surface(&mut commands, &mut tile_state, body);
            continue;
        }
        if !source_matches_body(&surface.source, placement) {
            warn!(
                "tiled body source for {} cannot be applied to {}",
                surface.source.body.name, placement.body.name
            );
            despawn_body_surface(&mut commands, &mut tile_state, body);
            continue;
        }

        let zoom = clamped_surface_zoom(surface);
        let visual_radius =
            body_surface_visual_radius(placement, *celestial_space, radius_policy.copied());
        if existing_radius != Some(&visual_radius) {
            commands.entity(body).insert(visual_radius);
        }
        let radius = visual_radius.visual_radius_units;
        let reset_needed = tile_state.bodies.get(&body).is_some_and(|state| {
            state.source != surface.source
                || state.zoom != zoom
                || (state.radius - radius).abs() > f32::EPSILON
        });
        if reset_needed {
            despawn_body_surface(&mut commands, &mut tile_state, body);
        }
        tile_state
            .bodies
            .entry(body)
            .or_insert_with(|| BodySurfaceState::new(surface.source.clone(), zoom, radius));

        queue_body_surface_tiles(
            body,
            &surface.source,
            zoom,
            placement.body.id == celestial_focus.target,
            &mut tile_state,
            &fetch_channel,
        );
        sync_body_surface_patches(
            &mut commands,
            body,
            &surface.source,
            zoom,
            radius,
            &mut tile_state,
            &mut meshes,
            &mut materials,
        );
    }
}

/// Returns whether a body surface should queue tiles and keep patch entities alive.
fn body_surface_is_active(
    visibility: Option<&Visibility>,
    inherited_visibility: Option<&InheritedVisibility>,
) -> bool {
    match visibility {
        Some(Visibility::Hidden) => false,
        Some(Visibility::Visible) => true,
        Some(Visibility::Inherited) | None => {
            inherited_visibility.is_none_or(|visibility| visibility.get())
        }
    }
}

/// Sorts focused body surfaces before background body surfaces.
fn body_focus_priority(body: BodyId, focus: CelestialFocus) -> u8 {
    u8::from(body != focus.target)
}

/// Ensures a planetary tile source is applied only to the matching body.
fn source_matches_body(
    source: &ferrisium_core::prelude::TileSource,
    placement: &CelestialBodyPlacement,
) -> bool {
    let expected = if placement.body.id == BodyId::EARTH {
        PlanetaryBody::earth()
    } else {
        let Some(trek_body) =
            NasaTrekRegularBody::from_body_or_system_naif_id(placement.body.id.naif_id)
        else {
            return false;
        };
        trek_body.planetary_body()
    };

    source.body == expected
}

/// Converts physical body radius into the current render unit convention.
#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy mesh radii are f32; built-in body radii fit comfortably for rendering."
)]
fn body_surface_visual_radius(
    placement: &CelestialBodyPlacement,
    celestial_space: CelestialSpace,
    radius_policy: Option<TiledBodySurfaceRadiusPolicy>,
) -> MetricVisualRadius {
    let physical_radius_units = celestial_space
        .render_scale
        .distance_to_units(placement.body.mean_radius_km) as f32;
    radius_policy
        .map_or_else(|| MetricVisualRadiusPolicy::minimum(0.0), |policy| policy.0)
        .visual_radius(physical_radius_units)
}

#[cfg(test)]
mod tests {
    use super::{
        body_focus_priority, body_surface_is_active, body_surface_visual_radius,
        source_matches_body,
    };
    use crate::celestial::{CelestialBodyPlacement, CelestialFocus};
    use crate::{CelestialSpace, MetricVisualRadiusPolicy, TiledBodySurfaceRadiusPolicy};
    use bevy::prelude::{InheritedVisibility, Visibility};
    use ferrisium_core::prelude::{CelestialBody, NasaTrekRegularBody, RenderScale, TileSource};
    use ferrisium_core::space::BodyId;

    #[test]
    fn focused_body_surface_gets_request_priority() {
        let focus = CelestialFocus::mars_system();

        assert_eq!(body_focus_priority(BodyId::MARS_BARYCENTER, focus), 0);
        assert_eq!(body_focus_priority(BodyId::MOON, focus), 1);
    }

    #[test]
    fn hidden_body_surfaces_do_not_queue_or_render_tiles() {
        assert!(body_surface_is_active(None, None));
        assert!(body_surface_is_active(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(body_surface_is_active(
            Some(&Visibility::Inherited),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(!body_surface_is_active(
            Some(&Visibility::Inherited),
            Some(&InheritedVisibility::HIDDEN)
        ));
        assert!(!body_surface_is_active(
            Some(&Visibility::Hidden),
            Some(&InheritedVisibility::VISIBLE)
        ));
        assert!(body_surface_is_active(
            Some(&Visibility::Visible),
            Some(&InheritedVisibility::HIDDEN)
        ));
    }

    #[test]
    fn body_surface_source_must_match_placement_body() {
        let moon =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::moon(), BodyId::EARTH);
        let mars =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::mars(), BodyId::EARTH);
        let mars_barycenter = CelestialBodyPlacement::for_body_relative_to(
            CelestialBody::spherical(
                BodyId::MARS_BARYCENTER,
                "Mars",
                CelestialBody::mars().mean_radius_km,
                ferrisium_core::space::FrameId::j2000(),
                ferrisium_core::space::FrameId::iau_mars(),
            ),
            BodyId::EARTH,
        );
        let mercury_barycenter = CelestialBodyPlacement::for_body_relative_to(
            CelestialBody::spherical(
                BodyId::MERCURY_BARYCENTER,
                "Mercury",
                CelestialBody::mercury().mean_radius_km,
                ferrisium_core::space::FrameId::j2000(),
                ferrisium_core::space::FrameId::named("IAU_MERCURY"),
            ),
            BodyId::EARTH,
        );
        let venus_barycenter = CelestialBodyPlacement::for_body_relative_to(
            CelestialBody::spherical(
                BodyId::VENUS_BARYCENTER,
                "Venus",
                CelestialBody::venus().mean_radius_km,
                ferrisium_core::space::FrameId::j2000(),
                ferrisium_core::space::FrameId::named("IAU_VENUS"),
            ),
            BodyId::EARTH,
        );
        let europa =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::europa(), BodyId::SUN);
        let ceres =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::ceres(), BodyId::SUN);
        let sun = CelestialBodyPlacement::for_body_relative_to(CelestialBody::sun(), BodyId::EARTH);

        assert!(source_matches_body(
            &TileSource::nasa_trek_moon_lro_wac(),
            &moon
        ));
        assert!(!source_matches_body(
            &TileSource::nasa_trek_moon_lro_wac(),
            &mars
        ));
        assert!(source_matches_body(
            &TileSource::nasa_trek_mars_viking_color(),
            &mars_barycenter
        ));
        assert!(source_matches_body(
            &TileSource::nasa_trek_mercury_mdis_enhanced_color(),
            &mercury_barycenter
        ));
        assert!(source_matches_body(
            &TileSource::nasa_trek_venus_magellan_color_topography(),
            &venus_barycenter
        ));
        assert!(source_matches_body(
            &TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Europa),
            &europa
        ));
        assert!(source_matches_body(
            &TileSource::nasa_trek_regular_body(NasaTrekRegularBody::Ceres),
            &ceres
        ));
        assert!(!source_matches_body(
            &TileSource::nasa_gibs_blue_marble(),
            &sun
        ));
    }

    #[test]
    fn body_surface_radius_without_policy_uses_physical_render_scale() {
        let moon =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::moon(), BodyId::EARTH);
        let radius = body_surface_visual_radius(&moon, CelestialSpace::default(), None);

        assert!((radius.physical_radius_units - 1_737.4).abs() <= 1.0e-3);
        assert!((radius.physical_radius_units - radius.visual_radius_units).abs() <= f32::EPSILON);
        assert!(!radius.enlarged);
    }

    #[test]
    fn body_surface_radius_policy_uses_active_metric_render_scale() {
        let earth =
            CelestialBodyPlacement::for_body_relative_to(CelestialBody::earth(), BodyId::SUN);
        let celestial_space = CelestialSpace::new(
            100_000_000.0,
            RenderScale::from_kilometers_per_unit(1_000_000.0),
        );
        let radius = body_surface_visual_radius(
            &earth,
            celestial_space,
            Some(TiledBodySurfaceRadiusPolicy(
                MetricVisualRadiusPolicy::minimum(2.0),
            )),
        );

        assert!((radius.physical_radius_units - 0.006_371).abs() <= 1.0e-6);
        assert!((radius.visual_radius_units - 2.0).abs() <= 1.0e-6);
        assert!(radius.enlarged);
    }
}
