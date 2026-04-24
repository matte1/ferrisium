//! Focus-target resolution for globe orbit and camera placement.
//!
//! The camera can orbit Earth, Moon, Mars-system barycenter, or any body with a
//! matching `CelestialBodyPlacement`. This module converts that focused body
//! into absolute render-space center/radius data used by input and projection.

use bevy::ecs::query::QueryFilter;
use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::space::BodyId;

use crate::celestial::{CelestialBodyPlacement, CelestialFocus, CelestialSpace};
use crate::globe::state::{globe_radius_from_render_units, Globe3dState};
use crate::metric_scene::{MetricSceneFocusPivot, MetricSceneFocusTarget, MetricSceneId};

/// Render-space body target currently used by globe controls.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlobeFocusTarget {
    pub(crate) body: BodyId,
    pub(crate) center_units: DVec3,
    pub(crate) radius: f32,
    pub(crate) body_rotation: Quat,
}

/// Resolves the active celestial focus into center/radius data.
pub(super) fn globe_focus_target<F>(
    focus: CelestialFocus,
    globe_state: Globe3dState,
    celestial_space: CelestialSpace,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
) -> GlobeFocusTarget
where
    F: QueryFilter,
{
    bodies
        .iter()
        .find_map(|(placement, cell, transform)| {
            (placement.body.id == focus.target).then(|| GlobeFocusTarget {
                body: placement.body.id,
                center_units: body_position_units(celestial_space, cell, transform),
                radius: globe_radius_from_render_units(
                    placement
                        .body
                        .mean_radius_units(celestial_space.render_scale),
                ),
                body_rotation: transform.rotation,
            })
        })
        .unwrap_or(GlobeFocusTarget {
            body: BodyId::EARTH,
            center_units: DVec3::ZERO,
            radius: globe_state.radius,
            body_rotation: Quat::IDENTITY,
        })
}

/// Resolves globe focus, preferring the shared metric-scene pivot when it
/// matches the focused body and falling back to the legacy body query.
pub(crate) fn globe_focus_target_with_metric_pivot<F>(
    focus: CelestialFocus,
    globe_state: Globe3dState,
    celestial_space: CelestialSpace,
    focus_pivot: &MetricSceneFocusPivot,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
) -> GlobeFocusTarget
where
    F: QueryFilter,
{
    let fallback = globe_focus_target(focus, globe_state, celestial_space, bodies);
    let matching_object_pivot = focus_pivot.resolved
        && focus_pivot.scene == MetricSceneId::PRIMARY
        && matches!(
            &focus_pivot.target,
            MetricSceneFocusTarget::Object(body) if *body == focus.target
        );
    if matching_object_pivot {
        return GlobeFocusTarget {
            body: focus.target,
            center_units: focus_pivot.pivot_units,
            radius: focus_pivot
                .physical_radius_units
                .filter(|radius| radius.is_finite() && *radius > 0.0)
                .map_or(fallback.radius, |radius| {
                    globe_radius_from_render_units(f64::from(radius))
                }),
            body_rotation: fallback.body_rotation,
        };
    }

    fallback
}

/// Reconstructs a body position from `BigSpace` cell data plus local transform.
pub(super) fn body_position_units(
    celestial_space: CelestialSpace,
    cell: Option<&CellCoord>,
    transform: &Transform,
) -> DVec3 {
    cell.map_or_else(
        || transform.translation.as_dvec3(),
        |cell| celestial_space.grid().grid_position_double(cell, transform),
    )
}
