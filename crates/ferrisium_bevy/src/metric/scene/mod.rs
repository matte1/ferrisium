//! Higher-level object vocabulary for metric scenes.
//!
//! The lower-level metric visual module knows how to place markers and
//! trajectories. This module names the reusable scene contract above that:
//! arbitrary bodies, spacecraft, waypoints, and barycenters can share the same
//! identity, focus, placement, and marker vocabulary across solar overview,
//! close-body globe inspection, and other metric-aware views.

use std::collections::BTreeMap;

use bevy::math::DVec3;
use bevy::prelude::*;
use big_space::prelude::CellCoord;
use ferrisium_core::coords::LonLat;
use ferrisium_core::space::{
    spherical_surface_position, BodyId, CelestialBody, EphemerisProvider, Epoch, FrameId,
    Kilometers, OrientationRequest, StateRequest, Vec3d,
};

use crate::celestial::{CelestialBodyPlacement, CelestialFocus, CelestialSpace};
use crate::metric_camera::MetricOrbitCameraState;
use crate::metric_visual::{
    metric_position_units, MetricObjectPlacement, MetricVisualObject, MetricVisualRadiusPolicy,
    MetricVisualRadiusSource,
};

/// Stable identifier for an independent metric scene.
///
/// Scene ids let applications run more than one metric object graph without
/// presentation state, duplicate-id reporting, or camera-distance LOD decisions
/// leaking between them. Existing unscoped APIs use [`MetricSceneId::PRIMARY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetricSceneId(u64);

impl MetricSceneId {
    /// Default scene used by existing constructors and unscoped registry access.
    pub const PRIMARY: Self = Self(0);

    /// Creates an application-defined scene id.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw scene id value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for MetricSceneId {
    fn default() -> Self {
        Self::PRIMARY
    }
}

/// Assigns an entity to a metric scene.
///
/// Entities without this component are treated as members of
/// [`MetricSceneId::PRIMARY`] so existing applications continue to work.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSceneMember {
    /// Scene containing this object, camera, or render layer.
    pub scene: MetricSceneId,
}

impl MetricSceneMember {
    /// Creates scene membership for `scene`.
    #[must_use]
    pub const fn new(scene: MetricSceneId) -> Self {
        Self { scene }
    }

    /// Creates membership in [`MetricSceneId::PRIMARY`].
    #[must_use]
    pub const fn primary() -> Self {
        Self::new(MetricSceneId::PRIMARY)
    }
}

impl Default for MetricSceneMember {
    fn default() -> Self {
        Self::primary()
    }
}

/// Marks a `Camera3d` as the LOD camera for one metric scene.
///
/// If a camera omits this component, its [`MetricSceneMember`] scene is used;
/// if both are absent, it belongs to [`MetricSceneId::PRIMARY`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSceneCamera {
    /// Scene whose object presentation distances should use this camera.
    pub scene: MetricSceneId,
}

impl MetricSceneCamera {
    /// Creates a scene camera marker for `scene`.
    #[must_use]
    pub const fn new(scene: MetricSceneId) -> Self {
        Self { scene }
    }

    /// Creates a scene camera marker for [`MetricSceneId::PRIMARY`].
    #[must_use]
    pub const fn primary() -> Self {
        Self::new(MetricSceneId::PRIMARY)
    }
}

impl Default for MetricSceneCamera {
    fn default() -> Self {
        Self::primary()
    }
}

type MetricSceneCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static MetricSceneMember>,
        Option<&'static MetricSceneCamera>,
        Option<&'static CellCoord>,
        &'static Transform,
    ),
    With<Camera3d>,
>;

type MetricSceneFocusObjectQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static MetricSceneMember>,
        &'static MetricSceneObject,
        Option<&'static CellCoord>,
        &'static Transform,
        Option<&'static CelestialBodyPlacement>,
    ),
>;

/// Role an object plays in a metric scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricSceneObjectRole {
    /// Physical body with a meaningful radius and optional surface renderer.
    Body,
    /// Point-like spacecraft, probe, vehicle, or other tracked asset.
    Spacecraft,
    /// Planning or annotation point with an ephemeris state.
    Waypoint,
    /// Dynamical center that may not have a visible surface.
    Barycenter,
    /// Application-defined object that still uses metric placement.
    Other,
}

/// Stable identity and label for an object in a metric scene.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct MetricSceneObject {
    /// Body, spacecraft, barycenter, or synthetic object identifier.
    pub id: BodyId,
    /// Short display label used by UI and optional labels.
    pub label: String,
    /// Semantic role used by presentation policies.
    pub role: MetricSceneObjectRole,
}

impl MetricSceneObject {
    /// Creates object metadata for an ephemeris-backed scene object.
    #[must_use]
    pub fn new(id: BodyId, label: impl Into<String>, role: MetricSceneObjectRole) -> Self {
        Self {
            id,
            label: label.into(),
            role,
        }
    }

    /// Creates object metadata from built-in or application-provided body metadata.
    #[must_use]
    pub fn body(body: &CelestialBody) -> Self {
        Self::new(body.id, body.name.clone(), MetricSceneObjectRole::Body)
    }
}

/// Registry of known metric scene objects keyed by scene and stable body/object id.
#[derive(Resource, Debug, Clone, Default)]
pub struct MetricSceneObjectRegistry {
    objects: BTreeMap<(MetricSceneId, BodyId), MetricSceneObject>,
    duplicate_ids: BTreeMap<MetricSceneId, Vec<BodyId>>,
}

impl MetricSceneObjectRegistry {
    /// Returns the registered primary-scene object for `id`, if one exists.
    #[must_use]
    pub fn get(&self, id: BodyId) -> Option<&MetricSceneObject> {
        self.get_in_scene(MetricSceneId::PRIMARY, id)
    }

    /// Returns the registered object for `id` in `scene`, if one exists.
    #[must_use]
    pub fn get_in_scene(&self, scene: MetricSceneId, id: BodyId) -> Option<&MetricSceneObject> {
        self.objects.get(&(scene, id))
    }

    /// Returns whether the primary scene currently contains `id`.
    #[must_use]
    pub fn contains(&self, id: BodyId) -> bool {
        self.contains_in_scene(MetricSceneId::PRIMARY, id)
    }

    /// Returns whether `scene` currently contains `id`.
    #[must_use]
    pub fn contains_in_scene(&self, scene: MetricSceneId, id: BodyId) -> bool {
        self.objects.contains_key(&(scene, id))
    }

    /// Returns the number of registered objects in the primary scene.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len_in_scene(MetricSceneId::PRIMARY)
    }

    /// Returns the number of registered objects in `scene`.
    #[must_use]
    pub fn len_in_scene(&self, scene: MetricSceneId) -> usize {
        self.objects
            .keys()
            .filter(|(object_scene, _)| *object_scene == scene)
            .count()
    }

    /// Returns whether the primary scene currently has no objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.is_empty_in_scene(MetricSceneId::PRIMARY)
    }

    /// Returns whether `scene` currently has no objects.
    #[must_use]
    pub fn is_empty_in_scene(&self, scene: MetricSceneId) -> bool {
        self.len_in_scene(scene) == 0
    }

    /// Iterates primary-scene objects in stable `BodyId` order.
    pub fn objects(&self) -> impl Iterator<Item = &MetricSceneObject> {
        self.objects_in_scene(MetricSceneId::PRIMARY)
    }

    /// Iterates objects in `scene` in stable `BodyId` order.
    pub fn objects_in_scene(
        &self,
        scene: MetricSceneId,
    ) -> impl Iterator<Item = &MetricSceneObject> {
        self.objects
            .iter()
            .filter_map(move |((object_scene, _), object)| {
                (*object_scene == scene).then_some(object)
            })
    }

    /// Returns primary-scene object ids registered by more than one entity last rebuild.
    ///
    /// Duplicate ids usually mean a render layer accidentally carries object
    /// identity instead of referencing an object anchor through
    /// [`MetricScenePresentationLayer`].
    #[must_use]
    pub fn duplicate_ids(&self) -> &[BodyId] {
        self.duplicate_ids_in_scene(MetricSceneId::PRIMARY)
    }

    /// Returns object ids registered by more than one entity in `scene`.
    #[must_use]
    pub fn duplicate_ids_in_scene(&self, scene: MetricSceneId) -> &[BodyId] {
        self.duplicate_ids.get(&scene).map_or(&[], Vec::as_slice)
    }

    fn rebuild(&mut self, objects: impl Iterator<Item = (MetricSceneId, MetricSceneObject)>) {
        self.objects.clear();
        self.duplicate_ids.clear();
        for (scene, object) in objects {
            let id = object.id;
            if self.objects.insert((scene, id), object).is_some() {
                let duplicate_ids = self.duplicate_ids.entry(scene).or_default();
                if !duplicate_ids.contains(&id) {
                    duplicate_ids.push(id);
                }
            }
        }
    }
}

/// Suggested camera behavior when focusing a metric scene object.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MetricSceneFocusPolicy {
    /// Camera-to-pivot distance in metric render units.
    pub distance_units: f64,
}

impl MetricSceneFocusPolicy {
    /// Creates a focus policy with an explicit camera distance.
    #[must_use]
    pub const fn new(distance_units: f64) -> Self {
        Self { distance_units }
    }
}

/// A concrete surface point on a spherical body.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricSceneSurfaceTarget {
    /// Body whose surface is being targeted.
    pub body: BodyId,
    /// Surface longitude and latitude in the body's fixed frame.
    pub lon_lat: LonLat,
    /// Mean or local spherical reference radius used for this focus point.
    pub reference_radius_km: Kilometers,
    /// Height above the spherical reference radius.
    pub altitude_km: Kilometers,
    /// Body-fixed frame that `lon_lat` is expressed in.
    pub body_fixed_frame: FrameId,
}

impl MetricSceneSurfaceTarget {
    /// Creates a spherical body-surface focus target from body metadata.
    #[must_use]
    pub fn from_body(body: &CelestialBody, lon_lat: LonLat, altitude_km: Kilometers) -> Self {
        Self {
            body: body.id,
            lon_lat,
            reference_radius_km: body.mean_radius_km,
            altitude_km,
            body_fixed_frame: body.body_fixed_frame.clone(),
        }
    }

    /// Radius from the body center to the requested focus point.
    #[must_use]
    pub fn radius_from_center_km(&self) -> Kilometers {
        Kilometers::new(self.reference_radius_km.as_f64() + self.altitude_km.as_f64())
    }
}

/// Focus target for a metric scene camera.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricSceneFocusTarget {
    /// Focus the scene origin or overview pivot.
    Scene,
    /// Focus a named metric object.
    Object(BodyId),
    /// Focus a concrete surface point on a spherical body.
    BodySurface(MetricSceneSurfaceTarget),
}

impl MetricSceneFocusTarget {
    /// Returns the focused object id, if this target names one.
    #[must_use]
    pub fn object_id(&self) -> Option<BodyId> {
        match self {
            Self::Scene => None,
            Self::Object(id) => Some(*id),
            Self::BodySurface(surface) => Some(surface.body),
        }
    }
}

/// Resource storing the requested and last-applied metric scene focus.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct MetricSceneFocusSelection {
    /// Scene containing the focused object.
    pub scene: MetricSceneId,
    /// Requested focus target.
    pub target: MetricSceneFocusTarget,
    /// Target whose one-time focus behavior has already been applied.
    pub applied: Option<MetricSceneFocusTarget>,
}

impl Default for MetricSceneFocusSelection {
    fn default() -> Self {
        Self::new(MetricSceneFocusTarget::Scene)
    }
}

impl MetricSceneFocusSelection {
    /// Creates a focus selection in [`MetricSceneId::PRIMARY`] for `target`.
    #[must_use]
    pub fn new(target: MetricSceneFocusTarget) -> Self {
        Self::in_scene(MetricSceneId::PRIMARY, target)
    }

    /// Creates a focus selection in `scene` for `target`.
    #[must_use]
    pub fn in_scene(scene: MetricSceneId, target: MetricSceneFocusTarget) -> Self {
        Self {
            scene,
            target,
            applied: None,
        }
    }

    /// Updates the requested target and clears the applied marker when changed.
    pub fn set_target(&mut self, target: MetricSceneFocusTarget) {
        if self.target != target {
            self.target = target;
            self.applied = None;
        }
    }
}

/// Resolved render-space pivot for the active metric scene focus.
///
/// The requested target remains in [`MetricSceneFocusSelection`]. This resource
/// is the post-placement result that cameras and controls can consume without
/// re-querying ephemeris state or reconstructing `big_space` positions
/// themselves. `pivot_units` is always in the active [`CelestialSpace`] render
/// units; `physical_radius_units` is metadata about the focused body, not a
/// visual proxy radius.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct MetricSceneFocusPivot {
    /// Scene containing the resolved target.
    pub scene: MetricSceneId,
    /// Focus target this pivot was resolved for.
    pub target: MetricSceneFocusTarget,
    /// Whether the target had enough scene data to resolve this frame.
    pub resolved: bool,
    /// Target center or surface point in metric render units.
    pub pivot_units: DVec3,
    /// Physical focused-body radius in metric render units when known.
    pub physical_radius_units: Option<f32>,
}

impl Default for MetricSceneFocusPivot {
    fn default() -> Self {
        Self::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Scene,
            DVec3::ZERO,
            None,
        )
    }
}

impl MetricSceneFocusPivot {
    /// Creates a resolved focus pivot.
    #[must_use]
    pub fn resolved(
        scene: MetricSceneId,
        target: MetricSceneFocusTarget,
        pivot_units: DVec3,
        physical_radius_units: Option<f32>,
    ) -> Self {
        Self {
            scene,
            target,
            resolved: true,
            pivot_units,
            physical_radius_units,
        }
    }

    /// Creates an unresolved pivot placeholder for a requested target.
    #[must_use]
    pub fn unresolved(scene: MetricSceneId, target: MetricSceneFocusTarget) -> Self {
        Self {
            scene,
            target,
            resolved: false,
            pivot_units: DVec3::ZERO,
            physical_radius_units: None,
        }
    }

    /// Returns the focused object or body id if this target names one.
    #[must_use]
    pub fn object_id(&self) -> Option<BodyId> {
        self.target.object_id()
    }
}

/// Coarse presentation used for an object at the current camera distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricScenePresentation {
    /// Draw a simple marker or proxy for distant objects.
    Marker,
    /// Draw a resolved body mesh or static textured sphere.
    Body,
    /// Draw the near/focused tiled surface renderer.
    Surface,
}

/// Distance-based presentation policy for a metric scene object.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MetricSceneLodPolicy {
    /// At or below this object distance, body rendering can replace a marker.
    pub marker_to_body_distance_units: f64,
    /// At or below this object distance, tiled/near-surface rendering can replace the body mesh.
    pub body_to_surface_distance_units: f64,
}

impl MetricSceneLodPolicy {
    /// Creates a normalized distance-based presentation policy.
    #[must_use]
    pub fn new(marker_to_body_distance_units: f64, body_to_surface_distance_units: f64) -> Self {
        let body_to_surface_distance_units = finite_non_negative(body_to_surface_distance_units);
        let marker_to_body_distance_units =
            finite_non_negative(marker_to_body_distance_units).max(body_to_surface_distance_units);

        Self {
            marker_to_body_distance_units,
            body_to_surface_distance_units,
        }
    }

    /// Returns the desired presentation at `distance_units` from the object.
    #[must_use]
    pub fn presentation_for_distance_units(self, distance_units: f64) -> MetricScenePresentation {
        let distance_units = finite_non_negative(distance_units);
        if distance_units <= self.body_to_surface_distance_units {
            MetricScenePresentation::Surface
        } else if distance_units <= self.marker_to_body_distance_units {
            MetricScenePresentation::Body
        } else {
            MetricScenePresentation::Marker
        }
    }
}

/// Current presentation chosen for a metric scene object by its LOD policy.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MetricScenePresentationState {
    /// Camera distance from the object's center in render units.
    pub distance_units: f64,
    /// Presentation selected for that distance.
    pub presentation: MetricScenePresentation,
}

impl MetricScenePresentationState {
    /// Creates a presentation state from camera distance and selected presentation.
    #[must_use]
    pub const fn new(distance_units: f64, presentation: MetricScenePresentation) -> Self {
        Self {
            distance_units,
            presentation,
        }
    }
}

/// Set of presentations for which a render layer should be visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricScenePresentationSet {
    marker: bool,
    body: bool,
    surface: bool,
}

impl MetricScenePresentationSet {
    /// Creates a set containing only `presentation`.
    #[must_use]
    pub const fn only(presentation: MetricScenePresentation) -> Self {
        match presentation {
            MetricScenePresentation::Marker => Self {
                marker: true,
                body: false,
                surface: false,
            },
            MetricScenePresentation::Body => Self {
                marker: false,
                body: true,
                surface: false,
            },
            MetricScenePresentation::Surface => Self {
                marker: false,
                body: false,
                surface: true,
            },
        }
    }

    /// Creates a set that is visible for body and surface presentations.
    ///
    /// Use this only for geometry that intentionally remains visible in close
    /// surface mode and does not occupy the same shell as a surface renderer.
    /// For physical body meshes that would overlap a tiled or draped surface,
    /// prefer [`MetricScenePresentationSet::only`] with
    /// [`MetricScenePresentation::Body`].
    #[must_use]
    pub const fn body_and_surface() -> Self {
        Self {
            marker: false,
            body: true,
            surface: true,
        }
    }

    /// Creates a set that is visible for every presentation.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            marker: true,
            body: true,
            surface: true,
        }
    }

    /// Returns whether `presentation` is included in this set.
    #[must_use]
    pub const fn contains(self, presentation: MetricScenePresentation) -> bool {
        match presentation {
            MetricScenePresentation::Marker => self.marker,
            MetricScenePresentation::Body => self.body,
            MetricScenePresentation::Surface => self.surface,
        }
    }
}

/// Visibility layer controlled by a metric scene object's presentation state.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricScenePresentationLayer {
    /// Scene containing the controlling object.
    pub scene: MetricSceneId,
    /// Object whose presentation state controls this layer.
    pub object: BodyId,
    /// Presentations for which this layer should be visible.
    pub presentations: MetricScenePresentationSet,
}

impl MetricScenePresentationLayer {
    /// Creates a layer visible only for `presentation`.
    #[must_use]
    pub const fn for_object(object: BodyId, presentation: MetricScenePresentation) -> Self {
        Self::for_object_in_scene(MetricSceneId::PRIMARY, object, presentation)
    }

    /// Creates a layer in `scene` visible only for `presentation`.
    #[must_use]
    pub const fn for_object_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentation: MetricScenePresentation,
    ) -> Self {
        Self {
            scene,
            object,
            presentations: MetricScenePresentationSet::only(presentation),
        }
    }

    /// Creates a layer visible for any presentation in `presentations`.
    #[must_use]
    pub const fn for_object_set(object: BodyId, presentations: MetricScenePresentationSet) -> Self {
        Self::for_object_set_in_scene(MetricSceneId::PRIMARY, object, presentations)
    }

    /// Creates a layer in `scene` visible for any presentation in `presentations`.
    #[must_use]
    pub const fn for_object_set_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentations: MetricScenePresentationSet,
    ) -> Self {
        Self {
            scene,
            object,
            presentations,
        }
    }
}

/// Bundle for a spherical body that acts as the authoritative metric scene anchor.
///
/// Presentation-specific render entities should usually be children of this
/// anchor and should reference the anchor's object id with
/// [`MetricScenePresentationLayer`], rather than carrying their own
/// [`MetricSceneObject`]. This keeps UI/focus identity and ephemeris placement
/// tied to one entity while allowing marker/body/surface render layers to swap
/// visibility independently.
#[derive(Bundle)]
pub struct MetricSceneBodyAnchorBundle {
    /// Scene containing this object anchor.
    pub scene: MetricSceneMember,
    /// Stable body identity.
    pub object: MetricSceneObject,
    /// Suggested camera focus behavior for this body.
    pub focus_policy: MetricSceneFocusPolicy,
    /// Ephemeris-backed body placement.
    pub placement: CelestialBodyPlacement,
}

impl MetricSceneBodyAnchorBundle {
    /// Creates an anchor for `body` positioned relative to `origin`.
    #[must_use]
    pub fn relative_to(body: CelestialBody, origin: BodyId, focus_distance_units: f64) -> Self {
        Self::relative_to_in_scene(MetricSceneId::PRIMARY, body, origin, focus_distance_units)
    }

    /// Creates an anchor for `body` and leaves surface orientation to render children.
    #[must_use]
    pub fn relative_to_without_orientation(
        body: CelestialBody,
        origin: BodyId,
        focus_distance_units: f64,
    ) -> Self {
        Self::relative_to_in_scene_without_orientation(
            MetricSceneId::PRIMARY,
            body,
            origin,
            focus_distance_units,
        )
    }

    /// Creates an anchor in `scene` for `body` positioned relative to `origin`.
    #[must_use]
    pub fn relative_to_in_scene(
        scene: MetricSceneId,
        body: CelestialBody,
        origin: BodyId,
        focus_distance_units: f64,
    ) -> Self {
        Self {
            scene: MetricSceneMember::new(scene),
            object: MetricSceneObject::body(&body),
            focus_policy: MetricSceneFocusPolicy::new(focus_distance_units),
            placement: CelestialBodyPlacement::for_body_relative_to(body, origin),
        }
    }

    /// Creates an anchor in `scene` and disables provider-driven body orientation.
    #[must_use]
    pub fn relative_to_in_scene_without_orientation(
        scene: MetricSceneId,
        body: CelestialBody,
        origin: BodyId,
        focus_distance_units: f64,
    ) -> Self {
        let mut bundle = Self::relative_to_in_scene(scene, body, origin, focus_distance_units);
        bundle.placement = bundle.placement.without_orientation();
        bundle
    }
}

/// Bundle for an entity whose visibility is controlled by an object anchor's LOD state.
#[derive(Bundle)]
pub struct MetricScenePresentationLayerBundle {
    /// Presentation state selector.
    pub layer: MetricScenePresentationLayer,
    /// Local transform relative to the object anchor.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Hidden until the presentation-layer sync system observes matching state.
    pub visibility: Visibility,
}

impl MetricScenePresentationLayerBundle {
    /// Creates a hidden layer visible only for `presentation`.
    #[must_use]
    pub fn for_object(object: BodyId, presentation: MetricScenePresentation) -> Self {
        Self::for_object_in_scene(MetricSceneId::PRIMARY, object, presentation)
    }

    /// Creates a hidden layer in `scene` visible only for `presentation`.
    #[must_use]
    pub fn for_object_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentation: MetricScenePresentation,
    ) -> Self {
        Self {
            layer: MetricScenePresentationLayer::for_object_in_scene(scene, object, presentation),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Hidden,
        }
    }

    /// Creates a hidden layer visible for any presentation in `presentations`.
    #[must_use]
    pub fn for_object_set(object: BodyId, presentations: MetricScenePresentationSet) -> Self {
        Self::for_object_set_in_scene(MetricSceneId::PRIMARY, object, presentations)
    }

    /// Creates a hidden layer in `scene` visible for any presentation in `presentations`.
    #[must_use]
    pub fn for_object_set_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentations: MetricScenePresentationSet,
    ) -> Self {
        Self {
            layer: MetricScenePresentationLayer::for_object_set_in_scene(
                scene,
                object,
                presentations,
            ),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Hidden,
        }
    }
}

/// Metric visual render layer controlled by an object anchor's presentation state.
#[derive(Bundle)]
pub struct MetricSceneBodyVisualLayerBundle {
    /// Presentation state selector.
    pub layer: MetricScenePresentationLayer,
    /// Display marker or proxy sphere rendered by metric visual systems.
    pub visual: MetricVisualObject,
    /// Local transform relative to the object anchor.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Hidden until the presentation-layer sync system observes matching state.
    pub visibility: Visibility,
}

impl MetricSceneBodyVisualLayerBundle {
    /// Creates a hidden primary-scene visual layer for `object`.
    #[must_use]
    pub fn for_object(
        object: BodyId,
        presentation: MetricScenePresentation,
        visual: MetricVisualObject,
    ) -> Self {
        Self::for_object_in_scene(MetricSceneId::PRIMARY, object, presentation, visual)
    }

    /// Creates a hidden visual layer in `scene` for `object`.
    #[must_use]
    pub fn for_object_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentation: MetricScenePresentation,
        visual: MetricVisualObject,
    ) -> Self {
        Self::for_object_set_in_scene(
            scene,
            object,
            MetricScenePresentationSet::only(presentation),
            visual,
        )
    }

    /// Creates a hidden primary-scene visual layer for any presentation in `presentations`.
    #[must_use]
    pub fn for_object_set(
        object: BodyId,
        presentations: MetricScenePresentationSet,
        visual: MetricVisualObject,
    ) -> Self {
        Self::for_object_set_in_scene(MetricSceneId::PRIMARY, object, presentations, visual)
    }

    /// Creates a hidden visual layer in `scene` for any presentation in `presentations`.
    #[must_use]
    pub fn for_object_set_in_scene(
        scene: MetricSceneId,
        object: BodyId,
        presentations: MetricScenePresentationSet,
        visual: MetricVisualObject,
    ) -> Self {
        Self {
            layer: MetricScenePresentationLayer::for_object_set_in_scene(
                scene,
                object,
                presentations,
            ),
            visual,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Hidden,
        }
    }
}

/// Body-relative surface render layer controlled by an object anchor's presentation state.
#[derive(Bundle)]
pub struct MetricSceneBodySurfaceLayerBundle {
    /// Presentation state selector.
    pub layer: MetricScenePresentationLayer,
    /// Body-relative placement for surface renderers parented under a body anchor.
    pub placement: CelestialBodyPlacement,
    /// Local transform relative to the object anchor.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Hidden until the presentation-layer sync system observes matching state.
    pub visibility: Visibility,
}

impl MetricSceneBodySurfaceLayerBundle {
    /// Creates a hidden primary-scene surface layer for `body`.
    #[must_use]
    pub fn for_body(body: CelestialBody, presentation: MetricScenePresentation) -> Self {
        Self::for_body_in_scene(MetricSceneId::PRIMARY, body, presentation)
    }

    /// Creates a hidden surface layer in `scene` for `body`.
    #[must_use]
    pub fn for_body_in_scene(
        scene: MetricSceneId,
        body: CelestialBody,
        presentation: MetricScenePresentation,
    ) -> Self {
        Self::for_body_set_in_scene(scene, body, MetricScenePresentationSet::only(presentation))
    }

    /// Creates a hidden primary-scene surface layer for any presentation in `presentations`.
    #[must_use]
    pub fn for_body_set(body: CelestialBody, presentations: MetricScenePresentationSet) -> Self {
        Self::for_body_set_in_scene(MetricSceneId::PRIMARY, body, presentations)
    }

    /// Creates a hidden surface layer in `scene` for any presentation in `presentations`.
    #[must_use]
    pub fn for_body_set_in_scene(
        scene: MetricSceneId,
        body: CelestialBody,
        presentations: MetricScenePresentationSet,
    ) -> Self {
        let body_id = body.id;
        Self {
            layer: MetricScenePresentationLayer::for_object_set_in_scene(
                scene,
                body_id,
                presentations,
            ),
            placement: CelestialBodyPlacement::for_body_relative_to(body, body_id)
                .without_orientation(),
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Hidden,
        }
    }
}

/// Bundle for point-like metric scene markers.
#[derive(Bundle)]
pub struct MetricSceneMarkerBundle {
    /// Scene containing this object marker.
    pub scene: MetricSceneMember,
    /// Stable object metadata.
    pub object: MetricSceneObject,
    /// Ephemeris-backed metric placement.
    pub placement: MetricObjectPlacement,
    /// Display marker rendered by Ferrisium's metric visual systems.
    pub visual: MetricVisualObject,
    /// Suggested camera focus behavior for this object.
    pub focus_policy: MetricSceneFocusPolicy,
    /// Local transform updated by metric placement systems.
    pub transform: Transform,
    /// Root-visible marker visibility that still inherits hidden parents.
    pub visibility: Visibility,
}

impl MetricSceneMarkerBundle {
    /// Creates a point-like marker bundle for a spacecraft or waypoint.
    #[must_use]
    pub fn point(
        object: MetricSceneObject,
        placement: MetricObjectPlacement,
        radius_policy: MetricVisualRadiusPolicy,
        color: Color,
        focus_policy: MetricSceneFocusPolicy,
    ) -> Self {
        Self::point_in_scene(
            MetricSceneId::PRIMARY,
            object,
            placement,
            radius_policy,
            color,
            focus_policy,
        )
    }

    /// Creates a point-like marker bundle in `scene` for a spacecraft or waypoint.
    #[must_use]
    pub fn point_in_scene(
        scene: MetricSceneId,
        object: MetricSceneObject,
        placement: MetricObjectPlacement,
        radius_policy: MetricVisualRadiusPolicy,
        color: Color,
        focus_policy: MetricSceneFocusPolicy,
    ) -> Self {
        Self {
            scene: MetricSceneMember::new(scene),
            object,
            placement,
            visual: MetricVisualObject {
                radius_source: MetricVisualRadiusSource::Point,
                radius_policy,
                color,
                emissive: true,
            },
            focus_policy,
            transform: Transform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Computes a metric scene focus pivot in render units.
///
/// Positions are always queried relative to the scene origin and frame supplied
/// by the caller. This keeps focus behavior tied to the same authoritative
/// metric state as object placement and trajectory rendering.
#[must_use]
pub fn metric_scene_focus_pivot_units(
    target: MetricSceneFocusTarget,
    scene_origin: BodyId,
    frame: FrameId,
    epoch: Epoch,
    celestial_space: CelestialSpace,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<DVec3> {
    let position_km =
        metric_scene_focus_position_km(&target, scene_origin, &frame, epoch, provider)?;
    Some(metric_position_units(celestial_space, position_km))
}

/// Converts a metric scene focus target into a celestial body focus when possible.
///
/// Object-center focus only maps to [`CelestialFocus`] when the object is
/// registered as a body or barycenter. Spacecraft and waypoints remain metric
/// camera targets without changing the body used by globe/tile systems.
#[must_use]
pub fn metric_scene_celestial_focus_target(
    target: &MetricSceneFocusTarget,
    scene: MetricSceneId,
    registry: &MetricSceneObjectRegistry,
) -> Option<BodyId> {
    match target {
        MetricSceneFocusTarget::Scene => None,
        MetricSceneFocusTarget::BodySurface(surface) => Some(surface.body),
        MetricSceneFocusTarget::Object(object) => registry
            .get_in_scene(scene, *object)
            .filter(|object| {
                matches!(
                    object.role,
                    MetricSceneObjectRole::Body | MetricSceneObjectRole::Barycenter
                )
            })
            .map(|_| *object),
    }
}

/// Rebuilds the active metric scene object registry from object components.
pub(crate) fn sync_metric_scene_object_registry(
    mut registry: ResMut<'_, MetricSceneObjectRegistry>,
    objects: Query<'_, '_, (Option<&MetricSceneMember>, &MetricSceneObject)>,
) {
    registry.rebuild(
        objects
            .iter()
            .map(|(member, object)| (scene_from_member(member), object.clone())),
    );
}

/// Synchronizes legacy celestial-body focus from the active metric scene focus.
pub fn sync_metric_scene_focus_to_celestial_focus(
    selection: Res<'_, MetricSceneFocusSelection>,
    registry: Res<'_, MetricSceneObjectRegistry>,
    mut celestial_focus: ResMut<'_, CelestialFocus>,
) {
    if !selection.is_changed() && !registry.is_changed() {
        return;
    }

    let Some(target) =
        metric_scene_celestial_focus_target(&selection.target, selection.scene, &registry)
    else {
        return;
    };
    if celestial_focus.target != target {
        celestial_focus.target = target;
    }
}

/// Resolves the active metric focus into the scene's current render-space pivot.
///
/// This runs after object/body placement and intentionally reads actual entity
/// positions. Cameras therefore share the same focus point as rendered objects,
/// including `big_space` cell reconstruction and render-scale changes.
pub fn sync_metric_scene_focus_pivot(
    selection: Res<'_, MetricSceneFocusSelection>,
    celestial_space: Res<'_, CelestialSpace>,
    objects: MetricSceneFocusObjectQuery<'_, '_>,
    mut focus_pivot: ResMut<'_, MetricSceneFocusPivot>,
) {
    let next = metric_scene_focus_pivot_from_objects(&selection, *celestial_space, &objects);
    if *focus_pivot != next {
        *focus_pivot = next;
    }
}

/// Evaluates metric scene LOD policies from the active camera position.
#[allow(
    clippy::type_complexity,
    reason = "The LOD system needs both placement components and optional previous state."
)]
pub(crate) fn sync_metric_scene_lod_from_camera(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    metric_camera_state: Option<Res<'_, MetricOrbitCameraState>>,
    cameras: MetricSceneCameraQuery<'_, '_>,
    objects: Query<
        '_,
        '_,
        (
            Entity,
            Option<&MetricSceneMember>,
            &MetricSceneLodPolicy,
            Option<&CellCoord>,
            &Transform,
            Option<&MetricScenePresentationState>,
        ),
    >,
) {
    let positions_by_scene = metric_scene_camera_positions_units(
        *celestial_space,
        metric_camera_state.as_deref(),
        &cameras,
    );
    if positions_by_scene.is_empty() {
        return;
    }

    for (entity, member, policy, cell, transform, current_state) in &objects {
        let scene = scene_from_member(member);
        let Some(camera_position) = positions_by_scene.get(&scene).copied() else {
            continue;
        };
        let object_position_units = absolute_position_units(*celestial_space, cell, transform);
        let distance_units = camera_position.distance(object_position_units);
        let next_state = MetricScenePresentationState::new(
            distance_units,
            policy.presentation_for_distance_units(distance_units),
        );

        if current_state.is_none_or(|current| *current != next_state) {
            commands.entity(entity).insert(next_state);
        }
    }
}

/// Applies presentation state to opt-in metric scene render layers.
pub(crate) fn sync_metric_scene_presentation_layers(
    objects: Query<
        '_,
        '_,
        (
            Option<&MetricSceneMember>,
            &MetricSceneObject,
            &MetricScenePresentationState,
        ),
    >,
    mut layers: Query<'_, '_, (&MetricScenePresentationLayer, &mut Visibility)>,
) {
    let states = objects
        .iter()
        .map(|(member, object, state)| ((scene_from_member(member), object.id), state.presentation))
        .collect::<BTreeMap<_, _>>();

    for (layer, mut visibility) in &mut layers {
        *visibility = if states
            .get(&(layer.scene, layer.object))
            .is_some_and(|presentation| layer.presentations.contains(*presentation))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn metric_scene_camera_positions_units(
    celestial_space: CelestialSpace,
    metric_camera_state: Option<&MetricOrbitCameraState>,
    cameras: &MetricSceneCameraQuery<'_, '_>,
) -> BTreeMap<MetricSceneId, DVec3> {
    let mut positions = BTreeMap::new();
    if let Some(metric_camera_state) = metric_camera_state {
        positions.insert(MetricSceneId::PRIMARY, metric_camera_state.position_units());
    }

    for (member, camera, cell, transform) in cameras.iter() {
        let scene = camera.map_or_else(|| scene_from_member(member), |camera| camera.scene);
        positions
            .entry(scene)
            .or_insert_with(|| absolute_position_units(celestial_space, cell, transform));
    }

    positions
}

fn metric_scene_focus_pivot_from_objects(
    selection: &MetricSceneFocusSelection,
    celestial_space: CelestialSpace,
    objects: &MetricSceneFocusObjectQuery<'_, '_>,
) -> MetricSceneFocusPivot {
    match &selection.target {
        MetricSceneFocusTarget::Scene => MetricSceneFocusPivot::resolved(
            selection.scene,
            MetricSceneFocusTarget::Scene,
            DVec3::ZERO,
            None,
        ),
        MetricSceneFocusTarget::Object(object) => {
            let Some((center_units, physical_radius_units)) =
                metric_scene_object_pivot(*object, selection.scene, celestial_space, objects)
            else {
                return MetricSceneFocusPivot::unresolved(
                    selection.scene,
                    selection.target.clone(),
                );
            };

            MetricSceneFocusPivot::resolved(
                selection.scene,
                selection.target.clone(),
                center_units,
                physical_radius_units,
            )
        }
        MetricSceneFocusTarget::BodySurface(surface) => {
            let Some((center_units, rotation, physical_radius_units)) =
                metric_scene_body_anchor(surface.body, selection.scene, celestial_space, objects)
            else {
                return MetricSceneFocusPivot::unresolved(
                    selection.scene,
                    selection.target.clone(),
                );
            };
            let surface_offset_units = metric_position_units(
                celestial_space,
                spherical_surface_position(surface.lon_lat, surface.radius_from_center_km()),
            );
            let pivot_units =
                center_units + rotation.mul_vec3(surface_offset_units.as_vec3()).as_dvec3();

            MetricSceneFocusPivot::resolved(
                selection.scene,
                selection.target.clone(),
                pivot_units,
                physical_radius_units.or_else(|| {
                    Some(radius_km_to_units(
                        celestial_space,
                        surface.reference_radius_km,
                    ))
                }),
            )
        }
    }
}

fn metric_scene_object_pivot(
    object_id: BodyId,
    scene: MetricSceneId,
    celestial_space: CelestialSpace,
    objects: &MetricSceneFocusObjectQuery<'_, '_>,
) -> Option<(DVec3, Option<f32>)> {
    let (center_units, _, physical_radius_units) =
        metric_scene_body_anchor(object_id, scene, celestial_space, objects)?;
    Some((center_units, physical_radius_units))
}

fn metric_scene_body_anchor(
    object_id: BodyId,
    scene: MetricSceneId,
    celestial_space: CelestialSpace,
    objects: &MetricSceneFocusObjectQuery<'_, '_>,
) -> Option<(DVec3, Quat, Option<f32>)> {
    objects
        .iter()
        .find_map(|(member, object, cell, transform, placement)| {
            (scene_from_member(member) == scene && object.id == object_id).then(|| {
                (
                    absolute_position_units(celestial_space, cell, transform),
                    transform.rotation,
                    placement.map(|placement| {
                        radius_km_to_units(celestial_space, placement.body.mean_radius_km)
                    }),
                )
            })
        })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Focused-body radius is renderer metadata consumed by Bevy cameras."
)]
fn radius_km_to_units(celestial_space: CelestialSpace, radius: Kilometers) -> f32 {
    celestial_space.render_scale.distance_to_units(radius) as f32
}

fn scene_from_member(member: Option<&MetricSceneMember>) -> MetricSceneId {
    member.map_or(MetricSceneId::PRIMARY, |member| member.scene)
}

fn absolute_position_units(
    celestial_space: CelestialSpace,
    cell: Option<&CellCoord>,
    transform: &Transform,
) -> DVec3 {
    if let Some(cell) = cell {
        return celestial_space.grid().grid_position_double(cell, transform);
    }

    transform.translation.as_dvec3()
}

fn metric_scene_focus_position_km(
    target: &MetricSceneFocusTarget,
    scene_origin: BodyId,
    frame: &FrameId,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<Vec3d> {
    match target {
        MetricSceneFocusTarget::Scene => Some(Vec3d::ZERO),
        MetricSceneFocusTarget::Object(target) => {
            object_position_km(*target, scene_origin, frame, epoch, provider)
        }
        MetricSceneFocusTarget::BodySurface(surface) => {
            let body_position_km =
                object_position_km(surface.body, scene_origin, frame, epoch, provider)?;
            let surface_offset_km =
                spherical_surface_position(surface.lon_lat, surface.radius_from_center_km());
            let surface_offset_km =
                orient_surface_offset(surface, surface_offset_km, frame, epoch, provider)?;

            Some(add_vec3d(body_position_km, surface_offset_km))
        }
    }
}

fn object_position_km(
    target: BodyId,
    scene_origin: BodyId,
    frame: &FrameId,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<Vec3d> {
    if target == scene_origin {
        return Some(Vec3d::ZERO);
    }

    Some(
        provider?
            .state(&StateRequest::new(
                target,
                scene_origin,
                frame.clone(),
                epoch,
            ))
            .ok()?
            .position_km,
    )
}

fn orient_surface_offset(
    surface: &MetricSceneSurfaceTarget,
    surface_offset_km: Vec3d,
    frame: &FrameId,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> Option<Vec3d> {
    if surface.body_fixed_frame == *frame {
        return Some(surface_offset_km);
    }

    let orientation = provider?
        .orientation(&OrientationRequest::new(
            surface.body_fixed_frame.clone(),
            frame.clone(),
            epoch,
        ))
        .ok()?;

    Some(orientation.rotation.rotate_vector(surface_offset_km))
}

fn add_vec3d(left: Vec3d, right: Vec3d) -> Vec3d {
    Vec3d::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        metric_scene_celestial_focus_target, metric_scene_focus_pivot_units,
        sync_metric_scene_focus_pivot, MetricSceneBodyAnchorBundle,
        MetricSceneBodySurfaceLayerBundle, MetricSceneBodyVisualLayerBundle, MetricSceneFocusPivot,
        MetricSceneFocusSelection, MetricSceneFocusTarget, MetricSceneId, MetricSceneLodPolicy,
        MetricSceneMarkerBundle, MetricSceneMember, MetricSceneObject, MetricSceneObjectRegistry,
        MetricSceneObjectRole, MetricScenePresentation, MetricScenePresentationLayer,
        MetricScenePresentationLayerBundle, MetricScenePresentationSet,
        MetricScenePresentationState, MetricSceneSurfaceTarget,
    };
    use crate::{
        metric_scene::{
            sync_metric_scene_focus_to_celestial_focus, sync_metric_scene_lod_from_camera,
            sync_metric_scene_presentation_layers, MetricSceneCamera,
        },
        metric_visual::{MetricVisualObject, MetricVisualRadiusPolicy},
        CelestialBodyPlacement, CelestialFocus, CelestialSpace, MetricObjectPlacement,
        MetricOrbitCameraState, MetricSceneFocusPolicy,
    };
    use bevy::math::DVec3;
    use bevy::prelude::{App, Camera3d, Color, Transform, Update, Visibility};
    use ferrisium_core::coords::LonLat;
    use ferrisium_core::space::{
        BodyId, CelestialBody, EphemerisError, EphemerisProvider, EphemerisResult, Epoch, FrameId,
        FrameOrientation, Kilometers, OrientationRequest, RenderScale, StateRequest, StateVector,
        Vec3d,
    };

    struct FixedProvider;

    impl EphemerisProvider for FixedProvider {
        fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector> {
            Ok(StateVector::relative_to(
                request.target,
                request.origin,
                request.frame.clone(),
                request.epoch,
                Vec3d::new(2.0, 0.0, 0.0),
                Vec3d::ZERO,
            ))
        }

        fn orientation(&self, _request: &OrientationRequest) -> EphemerisResult<FrameOrientation> {
            Err(EphemerisError::MissingData("unused".to_owned()))
        }
    }

    #[test]
    fn focus_selection_marks_target_changes_unapplied() {
        let mut selection = MetricSceneFocusSelection::default();
        assert_eq!(selection.target, MetricSceneFocusTarget::Scene);
        assert_eq!(selection.scene, MetricSceneId::PRIMARY);

        selection.applied = Some(MetricSceneFocusTarget::Scene);
        selection.set_target(MetricSceneFocusTarget::Object(BodyId::EARTH));

        assert_eq!(
            selection.target,
            MetricSceneFocusTarget::Object(BodyId::EARTH)
        );
        assert_eq!(selection.applied, None);
    }

    #[test]
    fn metric_focus_maps_only_bodies_and_surfaces_to_celestial_focus() {
        let mut registry = MetricSceneObjectRegistry::default();
        registry.rebuild(
            [
                (
                    MetricSceneId::PRIMARY,
                    MetricSceneObject::body(&CelestialBody::earth()),
                ),
                (
                    MetricSceneId::PRIMARY,
                    MetricSceneObject::new(
                        BodyId::naif(-42),
                        "SAT",
                        MetricSceneObjectRole::Spacecraft,
                    ),
                ),
            ]
            .into_iter(),
        );

        assert_eq!(
            metric_scene_celestial_focus_target(
                &MetricSceneFocusTarget::Object(BodyId::EARTH),
                MetricSceneId::PRIMARY,
                &registry,
            ),
            Some(BodyId::EARTH)
        );
        assert_eq!(
            metric_scene_celestial_focus_target(
                &MetricSceneFocusTarget::Object(BodyId::naif(-42)),
                MetricSceneId::PRIMARY,
                &registry,
            ),
            None
        );

        let surface = MetricSceneSurfaceTarget::from_body(
            &CelestialBody::moon(),
            LonLat::new(0.0, 0.0),
            Kilometers::new(0.0),
        );
        assert_eq!(
            metric_scene_celestial_focus_target(
                &MetricSceneFocusTarget::BodySurface(surface),
                MetricSceneId::PRIMARY,
                &registry,
            ),
            Some(BodyId::MOON)
        );
    }

    #[test]
    fn metric_focus_bridge_updates_celestial_focus_for_registered_body() {
        let mut app = App::new();
        app.insert_resource(MetricSceneFocusSelection::new(
            MetricSceneFocusTarget::Object(BodyId::MOON),
        ));
        app.insert_resource(CelestialFocus::earth());
        app.insert_resource(MetricSceneObjectRegistry::default());
        app.add_systems(Update, sync_metric_scene_focus_to_celestial_focus);

        app.world_mut()
            .resource_mut::<MetricSceneObjectRegistry>()
            .rebuild(
                [(
                    MetricSceneId::PRIMARY,
                    MetricSceneObject::body(&CelestialBody::moon()),
                )]
                .into_iter(),
            );

        app.update();

        assert_eq!(
            app.world().resource::<CelestialFocus>().target,
            BodyId::MOON
        );
    }

    #[test]
    fn metric_focus_bridge_keeps_celestial_focus_for_spacecraft() {
        let mut app = App::new();
        let spacecraft = BodyId::naif(-42);
        app.insert_resource(MetricSceneFocusSelection::new(
            MetricSceneFocusTarget::Object(spacecraft),
        ));
        app.insert_resource(CelestialFocus::earth());
        app.insert_resource(MetricSceneObjectRegistry::default());
        app.add_systems(Update, sync_metric_scene_focus_to_celestial_focus);

        app.world_mut()
            .resource_mut::<MetricSceneObjectRegistry>()
            .rebuild(
                [(
                    MetricSceneId::PRIMARY,
                    MetricSceneObject::new(spacecraft, "SC", MetricSceneObjectRole::Spacecraft),
                )]
                .into_iter(),
            );

        app.update();

        assert_eq!(
            app.world().resource::<CelestialFocus>().target,
            BodyId::EARTH
        );
    }

    #[test]
    fn focus_pivot_resolves_scene_origin() {
        let mut app = App::new();
        app.insert_resource(MetricSceneFocusSelection::default());
        app.insert_resource(CelestialSpace::new(
            1_000.0,
            RenderScale::from_kilometers_per_unit(2.0),
        ));
        app.init_resource::<MetricSceneFocusPivot>();
        app.add_systems(Update, sync_metric_scene_focus_pivot);

        app.update();

        let pivot = app.world().resource::<MetricSceneFocusPivot>();
        assert!(pivot.resolved);
        assert_eq!(pivot.target, MetricSceneFocusTarget::Scene);
        assert_eq!(pivot.pivot_units, DVec3::ZERO);
        assert_eq!(pivot.physical_radius_units, None);
    }

    #[test]
    fn focus_pivot_resolves_object_from_rendered_anchor() {
        let mut app = App::new();
        let moon = CelestialBody::moon();
        app.insert_resource(MetricSceneFocusSelection::new(
            MetricSceneFocusTarget::Object(BodyId::MOON),
        ));
        app.insert_resource(CelestialSpace::new(
            1_000.0,
            RenderScale::from_kilometers_per_unit(2.0),
        ));
        app.init_resource::<MetricSceneFocusPivot>();
        app.add_systems(Update, sync_metric_scene_focus_pivot);
        app.world_mut().spawn((
            MetricSceneObject::body(&moon),
            CelestialBodyPlacement::for_body_relative_to(moon.clone(), BodyId::EARTH),
            Transform::from_xyz(3.0, 4.0, 5.0),
        ));

        app.update();

        let pivot = app.world().resource::<MetricSceneFocusPivot>();
        assert!(pivot.resolved);
        assert_eq!(pivot.target, MetricSceneFocusTarget::Object(BodyId::MOON));
        assert_eq!(pivot.pivot_units, DVec3::new(3.0, 4.0, 5.0));
        let radius_units = pivot.physical_radius_units.unwrap_or(f32::NAN);
        assert!(
            (f64::from(radius_units)
                - moon.mean_radius_units(RenderScale::from_kilometers_per_unit(2.0)))
            .abs()
                <= 1.0e-4
        );
    }

    #[test]
    fn focus_pivot_marks_missing_object_unresolved() {
        let mut app = App::new();
        app.insert_resource(MetricSceneFocusSelection::new(
            MetricSceneFocusTarget::Object(BodyId::MOON),
        ));
        app.insert_resource(CelestialSpace::default());
        app.init_resource::<MetricSceneFocusPivot>();
        app.add_systems(Update, sync_metric_scene_focus_pivot);

        app.update();

        let pivot = app.world().resource::<MetricSceneFocusPivot>();
        assert!(!pivot.resolved);
        assert_eq!(pivot.target, MetricSceneFocusTarget::Object(BodyId::MOON));
        assert_eq!(pivot.pivot_units, DVec3::ZERO);
    }

    #[test]
    fn focus_pivot_resolves_surface_point_from_body_anchor() {
        let mut app = App::new();
        let surface = MetricSceneSurfaceTarget {
            body: BodyId::EARTH,
            lon_lat: LonLat::new(90.0, 0.0),
            reference_radius_km: Kilometers::new(2.0),
            altitude_km: Kilometers::new(1.0),
            body_fixed_frame: FrameId::itrf93(),
        };
        app.insert_resource(MetricSceneFocusSelection::new(
            MetricSceneFocusTarget::BodySurface(surface.clone()),
        ));
        app.insert_resource(CelestialSpace::new(
            1_000.0,
            RenderScale::from_kilometers_per_unit(1.0),
        ));
        app.init_resource::<MetricSceneFocusPivot>();
        app.add_systems(Update, sync_metric_scene_focus_pivot);
        app.world_mut().spawn((
            MetricSceneObject::body(&CelestialBody::earth()),
            Transform::from_xyz(10.0, 20.0, 30.0),
        ));

        app.update();

        let pivot = app.world().resource::<MetricSceneFocusPivot>();
        assert!(pivot.resolved);
        assert_eq!(pivot.target, MetricSceneFocusTarget::BodySurface(surface));
        assert!((pivot.pivot_units.x - 13.0).abs() <= 1.0e-9);
        assert!((pivot.pivot_units.y - 20.0).abs() <= 1.0e-9);
        assert!((pivot.pivot_units.z - 30.0).abs() <= 1.0e-9);
        assert_eq!(pivot.physical_radius_units, Some(2.0));
    }

    #[test]
    fn focus_pivot_uses_scene_origin_and_render_scale() {
        let provider = FixedProvider;
        let space = CelestialSpace::new(1_000.0, RenderScale::from_kilometers_per_unit(2.0));
        let pivot = metric_scene_focus_pivot_units(
            MetricSceneFocusTarget::Object(BodyId::EARTH),
            BodyId::SUN,
            FrameId::j2000(),
            Epoch::J2000,
            space,
            Some(&provider),
        );

        assert!(pivot.is_some());
        let pivot = pivot.unwrap_or(DVec3::splat(f64::NAN));
        assert!((pivot.x - 1.0).abs() <= f64::EPSILON);
        assert!(pivot.y.abs() <= f64::EPSILON);
        assert!(pivot.z.abs() <= f64::EPSILON);
    }

    #[test]
    fn surface_focus_pivot_adds_body_fixed_surface_offset() {
        struct SurfaceProvider;

        impl EphemerisProvider for SurfaceProvider {
            fn state(&self, request: &StateRequest) -> EphemerisResult<StateVector> {
                Ok(StateVector::relative_to(
                    request.target,
                    request.origin,
                    request.frame.clone(),
                    request.epoch,
                    Vec3d::new(100.0, 0.0, 0.0),
                    Vec3d::ZERO,
                ))
            }

            fn orientation(
                &self,
                request: &OrientationRequest,
            ) -> EphemerisResult<FrameOrientation> {
                Ok(FrameOrientation::identity(
                    request.from.clone(),
                    request.to.clone(),
                    request.epoch,
                ))
            }
        }

        let surface = MetricSceneSurfaceTarget {
            body: BodyId::EARTH,
            lon_lat: LonLat::new(90.0, 0.0),
            reference_radius_km: Kilometers::new(10.0),
            altitude_km: Kilometers::new(2.0),
            body_fixed_frame: FrameId::itrf93(),
        };
        let pivot = metric_scene_focus_pivot_units(
            MetricSceneFocusTarget::BodySurface(surface),
            BodyId::SUN,
            FrameId::j2000(),
            Epoch::J2000,
            CelestialSpace::new(1_000.0, RenderScale::from_kilometers_per_unit(2.0)),
            Some(&SurfaceProvider),
        );

        assert!(pivot.is_some());
        let pivot = pivot.unwrap_or(DVec3::splat(f64::NAN));
        assert!((pivot.x - 56.0).abs() <= 1.0e-9);
        assert!(pivot.y.abs() <= 1.0e-9);
        assert!(pivot.z.abs() <= 1.0e-9);
    }

    #[test]
    fn surface_target_uses_body_radius_and_fixed_frame() {
        let earth = CelestialBody::earth();
        let surface = MetricSceneSurfaceTarget::from_body(
            &earth,
            LonLat::new(-122.3, 47.6),
            Kilometers::new(0.4),
        );

        assert_eq!(surface.body, BodyId::EARTH);
        assert_eq!(surface.body_fixed_frame, earth.body_fixed_frame);
        assert!((surface.radius_from_center_km().as_f64() - 6_371.408).abs() <= 1.0e-4);
    }

    #[test]
    fn object_registry_rebuilds_from_scene_objects() {
        let mut registry = MetricSceneObjectRegistry::default();
        registry.rebuild(
            [
                MetricSceneObject::body(&CelestialBody::earth()),
                MetricSceneObject::new(BodyId::naif(-42), "SAT", MetricSceneObjectRole::Spacecraft),
            ]
            .into_iter()
            .map(|object| (MetricSceneId::PRIMARY, object)),
        );

        assert_eq!(registry.len(), 2);
        assert!(registry.contains(BodyId::EARTH));
        assert!(registry.duplicate_ids().is_empty());
        assert_eq!(
            registry
                .get(BodyId::naif(-42))
                .map(|object| object.label.as_str()),
            Some("SAT")
        );

        registry.rebuild(
            [MetricSceneObject::body(&CelestialBody::moon())]
                .into_iter()
                .map(|object| (MetricSceneId::PRIMARY, object)),
        );
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains(BodyId::EARTH));
        assert!(registry.contains(BodyId::MOON));
    }

    #[test]
    fn object_registry_reports_duplicate_object_ids() {
        let mut registry = MetricSceneObjectRegistry::default();
        registry.rebuild(
            [
                MetricSceneObject::body(&CelestialBody::earth()),
                MetricSceneObject::new(
                    BodyId::EARTH,
                    "Duplicate Earth",
                    MetricSceneObjectRole::Other,
                ),
            ]
            .into_iter()
            .map(|object| (MetricSceneId::PRIMARY, object)),
        );

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.duplicate_ids(), &[BodyId::EARTH]);
    }

    #[test]
    fn object_registry_scopes_objects_and_duplicates_by_scene() {
        let secondary = MetricSceneId::new(7);
        let mut registry = MetricSceneObjectRegistry::default();
        registry.rebuild(
            [
                (
                    MetricSceneId::PRIMARY,
                    MetricSceneObject::body(&CelestialBody::earth()),
                ),
                (
                    secondary,
                    MetricSceneObject::new(BodyId::EARTH, "Earth alt", MetricSceneObjectRole::Body),
                ),
                (
                    secondary,
                    MetricSceneObject::new(
                        BodyId::EARTH,
                        "Earth duplicate",
                        MetricSceneObjectRole::Other,
                    ),
                ),
            ]
            .into_iter(),
        );

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.len_in_scene(secondary), 1);
        assert_eq!(
            registry
                .get_in_scene(secondary, BodyId::EARTH)
                .map(|object| object.label.as_str()),
            Some("Earth duplicate")
        );
        assert!(registry.duplicate_ids().is_empty());
        assert_eq!(registry.duplicate_ids_in_scene(secondary), &[BodyId::EARTH]);
    }

    #[test]
    fn lod_policy_selects_marker_body_and_surface_presentations() {
        let policy = MetricSceneLodPolicy::new(100.0, 10.0);

        assert_eq!(
            policy.presentation_for_distance_units(150.0),
            MetricScenePresentation::Marker
        );
        assert_eq!(
            policy.presentation_for_distance_units(50.0),
            MetricScenePresentation::Body
        );
        assert_eq!(
            policy.presentation_for_distance_units(5.0),
            MetricScenePresentation::Surface
        );

        let normalized = MetricSceneLodPolicy::new(2.0, 10.0);
        assert!((normalized.marker_to_body_distance_units - 10.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn lod_system_writes_presentation_state_from_metric_camera_distance() {
        let mut app = App::new();
        app.insert_resource(CelestialSpace::new(
            1_000.0,
            RenderScale::from_kilometers_per_unit(1.0),
        ));
        app.insert_resource(MetricOrbitCameraState::new(
            DVec3::new(0.0, 0.0, 20.0),
            0.0,
            0.0,
            0.0,
        ));
        app.add_systems(Update, sync_metric_scene_lod_from_camera);

        let object = app
            .world_mut()
            .spawn((
                MetricSceneObject::body(&CelestialBody::earth()),
                MetricSceneLodPolicy::new(30.0, 10.0),
                Transform::default(),
            ))
            .id();

        app.update();

        let state = app.world().get::<MetricScenePresentationState>(object);
        assert!(state.is_some());
        let state = state.copied().unwrap_or(MetricScenePresentationState::new(
            f64::NAN,
            MetricScenePresentation::Marker,
        ));
        assert!((state.distance_units - 20.0).abs() <= 1.0e-9);
        assert_eq!(state.presentation, MetricScenePresentation::Body);
    }

    #[test]
    fn lod_system_uses_scene_specific_camera_positions() {
        let mut app = App::new();
        let secondary = MetricSceneId::new(3);
        app.insert_resource(CelestialSpace::new(
            1_000.0,
            RenderScale::from_kilometers_per_unit(1.0),
        ));
        app.add_systems(Update, sync_metric_scene_lod_from_camera);

        app.world_mut().spawn((
            Camera3d::default(),
            MetricSceneCamera::primary(),
            Transform::from_xyz(0.0, 0.0, 20.0),
        ));
        app.world_mut().spawn((
            Camera3d::default(),
            MetricSceneCamera::new(secondary),
            Transform::from_xyz(0.0, 0.0, 120.0),
        ));

        let primary_object = app
            .world_mut()
            .spawn((
                MetricSceneObject::body(&CelestialBody::earth()),
                MetricSceneLodPolicy::new(60.0, 10.0),
                Transform::default(),
            ))
            .id();
        let secondary_object = app
            .world_mut()
            .spawn((
                MetricSceneMember::new(secondary),
                MetricSceneObject::new(BodyId::EARTH, "Earth alt", MetricSceneObjectRole::Body),
                MetricSceneLodPolicy::new(60.0, 10.0),
                Transform::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<MetricScenePresentationState>(primary_object)
                .map(|state| state.presentation),
            Some(MetricScenePresentation::Body)
        );
        assert_eq!(
            app.world()
                .get::<MetricScenePresentationState>(secondary_object)
                .map(|state| state.presentation),
            Some(MetricScenePresentation::Marker)
        );
    }

    #[test]
    fn presentation_layers_follow_object_presentation_state() {
        let mut app = App::new();
        app.add_systems(Update, sync_metric_scene_presentation_layers);

        app.world_mut().spawn((
            MetricSceneObject::body(&CelestialBody::earth()),
            MetricScenePresentationState::new(4.0, MetricScenePresentation::Surface),
        ));
        let body_layer = app
            .world_mut()
            .spawn((
                MetricScenePresentationLayer::for_object(
                    BodyId::EARTH,
                    MetricScenePresentation::Body,
                ),
                Visibility::Visible,
            ))
            .id();
        let surface_layer = app
            .world_mut()
            .spawn((
                MetricScenePresentationLayer::for_object_set(
                    BodyId::EARTH,
                    MetricScenePresentationSet::body_and_surface(),
                ),
                Visibility::Hidden,
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Visibility>(body_layer),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(surface_layer),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn presentation_layers_are_scoped_to_their_scene() {
        let mut app = App::new();
        let secondary = MetricSceneId::new(11);
        app.add_systems(Update, sync_metric_scene_presentation_layers);

        app.world_mut().spawn((
            MetricSceneObject::body(&CelestialBody::earth()),
            MetricScenePresentationState::new(4.0, MetricScenePresentation::Marker),
        ));
        app.world_mut().spawn((
            MetricSceneMember::new(secondary),
            MetricSceneObject::new(BodyId::EARTH, "Earth alt", MetricSceneObjectRole::Body),
            MetricScenePresentationState::new(4.0, MetricScenePresentation::Surface),
        ));
        let primary_surface_layer = app
            .world_mut()
            .spawn((
                MetricScenePresentationLayer::for_object(
                    BodyId::EARTH,
                    MetricScenePresentation::Surface,
                ),
                Visibility::Visible,
            ))
            .id();
        let secondary_surface_layer = app
            .world_mut()
            .spawn((
                MetricScenePresentationLayer::for_object_in_scene(
                    secondary,
                    BodyId::EARTH,
                    MetricScenePresentation::Surface,
                ),
                Visibility::Hidden,
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Visibility>(primary_surface_layer),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(secondary_surface_layer),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn presentation_layers_hide_without_object_presentation_state() {
        let mut app = App::new();
        app.add_systems(Update, sync_metric_scene_presentation_layers);

        let layer = app
            .world_mut()
            .spawn((
                MetricScenePresentationLayer::for_object(
                    BodyId::EARTH,
                    MetricScenePresentation::Marker,
                ),
                Visibility::Visible,
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Visibility>(layer),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn body_anchor_bundle_carries_one_scene_object_and_body_placement() {
        let earth = CelestialBody::earth();
        let bundle = MetricSceneBodyAnchorBundle::relative_to(earth.clone(), BodyId::SUN, 42.0);

        assert_eq!(bundle.scene.scene, MetricSceneId::PRIMARY);
        assert_eq!(bundle.object.id, BodyId::EARTH);
        assert!((bundle.focus_policy.distance_units - 42.0).abs() <= f64::EPSILON);
        assert_eq!(bundle.placement.body, earth);
        assert_eq!(bundle.placement.origin, BodyId::SUN);
    }

    #[test]
    fn presentation_layer_bundle_starts_hidden() {
        let bundle = MetricScenePresentationLayerBundle::for_object(
            BodyId::EARTH,
            MetricScenePresentation::Surface,
        );

        assert_eq!(bundle.layer.object, BodyId::EARTH);
        assert_eq!(bundle.layer.scene, MetricSceneId::PRIMARY);
        assert!(bundle
            .layer
            .presentations
            .contains(MetricScenePresentation::Surface));
        assert_eq!(bundle.visibility, Visibility::Hidden);
    }

    #[test]
    fn body_visual_layer_bundle_carries_visual_and_presentation_set() {
        let bundle = MetricSceneBodyVisualLayerBundle::for_object_set(
            BodyId::EARTH,
            MetricScenePresentationSet::body_and_surface(),
            MetricVisualObject::physical_radius(
                Kilometers::new(1.0),
                MetricVisualRadiusPolicy::minimum(0.25),
                Color::srgb(0.0, 0.0, 1.0),
            ),
        );

        assert_eq!(bundle.layer.object, BodyId::EARTH);
        assert_eq!(bundle.layer.scene, MetricSceneId::PRIMARY);
        assert!(bundle
            .layer
            .presentations
            .contains(MetricScenePresentation::Body));
        assert!(bundle
            .layer
            .presentations
            .contains(MetricScenePresentation::Surface));
        assert_eq!(bundle.visibility, Visibility::Hidden);
    }

    #[test]
    fn body_surface_layer_bundle_is_body_relative_without_orientation() {
        let moon = CelestialBody::moon();
        let bundle = MetricSceneBodySurfaceLayerBundle::for_body(
            moon.clone(),
            MetricScenePresentation::Surface,
        );

        assert_eq!(bundle.layer.object, BodyId::MOON);
        assert_eq!(bundle.placement.body, moon);
        assert_eq!(bundle.placement.origin, BodyId::MOON);
        assert!(!bundle.placement.apply_orientation);
        assert_eq!(bundle.visibility, Visibility::Hidden);
    }

    #[test]
    fn marker_bundle_carries_shared_scene_object_and_marker_components() {
        let object =
            MetricSceneObject::new(BodyId::naif(-42), "SAT", MetricSceneObjectRole::Spacecraft);
        let bundle = MetricSceneMarkerBundle::point(
            object,
            MetricObjectPlacement::relative_to(BodyId::naif(-42), BodyId::EARTH, FrameId::j2000()),
            MetricVisualRadiusPolicy::minimum(0.25),
            Color::srgb(1.0, 1.0, 0.0),
            MetricSceneFocusPolicy::new(4.0),
        );

        assert_eq!(bundle.scene.scene, MetricSceneId::PRIMARY);
        assert_eq!(bundle.object.label, "SAT");
        assert_eq!(bundle.placement.origin, BodyId::EARTH);
        assert!(bundle.visual.emissive);
        assert!((bundle.focus_policy.distance_units - 4.0).abs() <= f64::EPSILON);
        assert_eq!(bundle.visibility, Visibility::Inherited);
    }
}
