//! Camera-facing labels anchored to positions around a globe body.
//!
//! `GlobeLabel` resolves its anchor through the same body-frame path as
//! [`crate::GlobePosition`], then projects that point into a dedicated 2D
//! overlay camera. The resulting text stays orthogonal to the active globe
//! camera view while the anchor remains a physical globe-space position.

use std::collections::{HashMap, HashSet};

use bevy::camera::visibility::RenderLayers;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::sprite::{Anchor, Text2dShadow};
use bevy::window::PrimaryWindow;
use big_space::prelude::CellCoord;
use ferrisium_core::space::CelestialBody;

use crate::celestial::{CelestialBodyPlacement, CelestialSpace};
use crate::globe::{current_globe_camera_transform, Globe3dCamera};
use crate::globe_anchor::{dvec3_is_finite, resolve_globe_anchor_position, GlobeAnchor};
use crate::globe_position::{globe_position_scene_position_units, GlobePosition};
use crate::overlay_common::{color_from_rgba, overlay_is_visible, OverlayColorKey};
use crate::view::{FerrisiumViewKind, FerrisiumViewMode, Map2dView};
use crate::Globe3dView;

/// Render layer reserved for Ferrisium's built-in globe label overlay camera.
pub const GLOBE_LABEL_RENDER_LAYER: usize = 31;
/// Default label font size in logical pixels.
pub const DEFAULT_GLOBE_LABEL_FONT_SIZE: f32 = 14.0;
/// Default label z value in the label overlay camera.
pub const DEFAULT_GLOBE_LABEL_Z: f32 = 100.0;

/// Marker for the 2D camera Ferrisium uses to draw built-in globe labels.
#[derive(Component, Debug, Default)]
pub struct GlobeLabelCamera;

/// A physical anchor for a camera-facing [`GlobeLabel`].
pub type GlobeLabelPosition = GlobeAnchor;

/// Text label anchored to a position around one globe body.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GlobeLabel {
    /// Body used for fixed geodetic/body-fixed anchor conversion.
    pub body: CelestialBody,
    /// Physical label anchor.
    pub position: GlobeLabelPosition,
    /// Text displayed by the label.
    pub text: String,
    /// Constant non-premultiplied sRGBA text color.
    pub color_rgba: [f32; 4],
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Screen-space offset from the projected anchor, in logical pixels.
    pub screen_offset_px: Vec2,
    /// Z value used by the label overlay camera.
    pub z: f32,
    /// Whether labels behind the owning body should be hidden.
    pub hide_behind_body: bool,
}

impl GlobeLabel {
    /// Creates a label for `body`.
    #[must_use]
    pub fn for_body(
        body: CelestialBody,
        position: GlobeLabelPosition,
        text: impl Into<String>,
        color_rgba: [f32; 4],
    ) -> Self {
        Self {
            body,
            position,
            text: text.into(),
            color_rgba: color_rgba.map(normalized_color_component),
            font_size: DEFAULT_GLOBE_LABEL_FONT_SIZE,
            screen_offset_px: Vec2::ZERO,
            z: DEFAULT_GLOBE_LABEL_Z,
            hide_behind_body: true,
        }
    }

    /// Creates an Earth-centered label.
    #[must_use]
    pub fn earth(
        position: GlobeLabelPosition,
        text: impl Into<String>,
        color_rgba: [f32; 4],
    ) -> Self {
        Self::for_body(CelestialBody::earth(), position, text, color_rgba)
    }

    /// Sets the font size in logical pixels.
    #[must_use]
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = normalized_font_size(font_size);
        self
    }

    /// Sets a screen-space offset in logical pixels.
    #[must_use]
    pub const fn with_screen_offset_px(mut self, offset_px: Vec2) -> Self {
        self.screen_offset_px = offset_px;
        self
    }

    /// Sets whether the owning body hides this label when the anchor is behind it.
    #[must_use]
    pub const fn with_hide_behind_body(mut self, hide: bool) -> Self {
        self.hide_behind_body = hide;
        self
    }
}

/// Convenience bundle for spawning a globe label.
#[derive(Bundle)]
pub struct GlobeLabelBundle {
    /// Label content and style.
    pub label: GlobeLabel,
    /// Local transform for the marker entity.
    pub transform: Transform,
    /// Global transform storage for Bevy hierarchy propagation.
    pub global_transform: GlobalTransform,
    /// Local visibility gate; inherited hierarchy visibility is also honored.
    pub visibility: Visibility,
}

impl GlobeLabelBundle {
    /// Creates a root-visible label bundle that inherits parent visibility.
    #[must_use]
    pub fn new(label: GlobeLabel) -> Self {
        Self {
            label,
            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::Inherited,
        }
    }
}

/// Spawned text entities for globe labels.
#[derive(Resource, Default)]
pub(crate) struct GlobeLabelEntities {
    labels: HashMap<Entity, GlobeLabelHandles>,
}

#[derive(Debug, Clone)]
struct GlobeLabelHandles {
    entity: Entity,
    text: String,
    color_key: GlobeLabelColorKey,
    font_size_bits: u32,
}

type GlobeLabelColorKey = OverlayColorKey;

#[derive(Component, Debug, Default)]
pub(crate) struct GlobeLabelRenderEntity;

type GlobeLabelSourceQueryItem<'a> = (
    Entity,
    Ref<'a, GlobeLabel>,
    Option<&'a Visibility>,
    Option<&'a InheritedVisibility>,
);
type GlobeLabelBodyQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CelestialBodyPlacement,
        Option<&'static CellCoord>,
        &'static Transform,
    ),
    (Without<GlobePosition>, Without<GlobeLabelRenderEntity>),
>;
type GlobeLabelRenderQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Text2d,
        &'static mut TextFont,
        &'static mut TextColor,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (With<GlobeLabelRenderEntity>, Without<GlobeLabel>),
>;
type GlobeLabelCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        &'static GlobalTransform,
        &'static Transform,
        Option<&'static CellCoord>,
    ),
    (With<Globe3dCamera>, Without<GlobeLabelRenderEntity>),
>;

/// Synchronizes globe labels into a camera-facing 2D overlay.
#[allow(
    clippy::too_many_arguments,
    reason = "Bevy systems expose view, camera, anchor, text, and asset state explicitly."
)]
pub(crate) fn sync_globe_labels(
    mut commands: Commands<'_, '_>,
    celestial_space: Res<'_, CelestialSpace>,
    view_mode: Res<'_, FerrisiumViewMode>,
    map_views: Query<'_, '_, (), With<Map2dView>>,
    globe_views: Query<'_, '_, (), With<Globe3dView>>,
    label_cameras: Query<'_, '_, (), With<GlobeLabelCamera>>,
    windows: Query<'_, '_, &Window, With<PrimaryWindow>>,
    cameras: GlobeLabelCameraQuery<'_, '_>,
    bodies: GlobeLabelBodyQuery<'_, '_>,
    endpoint_positions: Query<'_, '_, &GlobePosition>,
    labels: Query<'_, '_, GlobeLabelSourceQueryItem<'_>>,
    mut label_entities: ResMut<'_, GlobeLabelEntities>,
    mut render_labels: GlobeLabelRenderQuery<'_, '_>,
) {
    let live_labels = labels
        .iter()
        .map(|(entity, _, _, _)| entity)
        .collect::<HashSet<_>>();
    retain_live_globe_labels(&mut commands, &mut label_entities, &live_labels);

    if labels.is_empty() {
        return;
    }
    if label_cameras.is_empty() {
        spawn_globe_label_camera(&mut commands);
    }

    if view_mode.active_kind(!map_views.is_empty(), !globe_views.is_empty())
        != Some(FerrisiumViewKind::Globe3d)
    {
        hide_all_globe_labels(&mut label_entities, &mut render_labels);
        return;
    }

    let Some(window) = windows.iter().next() else {
        hide_all_globe_labels(&mut label_entities, &mut render_labels);
        return;
    };
    let Some((camera, camera_transform, camera_origin_units)) =
        active_globe_label_camera(*celestial_space, cameras.iter())
    else {
        hide_all_globe_labels(&mut label_entities, &mut render_labels);
        return;
    };

    for (label_entity, label, visibility, inherited_visibility) in &labels {
        let render_entity = ensure_globe_label_render_entity(
            &mut commands,
            &mut label_entities,
            label_entity,
            &label,
        );
        let Ok((mut text, mut font, mut color, mut transform, mut render_visibility)) =
            render_labels.get_mut(render_entity)
        else {
            continue;
        };

        if !overlay_is_visible(visibility, inherited_visibility) || label.text.is_empty() {
            *render_visibility = Visibility::Hidden;
            continue;
        }

        update_globe_label_text_components(
            &mut text,
            &mut font,
            &mut color,
            &mut label_entities,
            label_entity,
            &label,
        );

        let Some(position_units) = resolve_globe_label_position(
            *celestial_space,
            &label.body,
            label.position,
            &bodies,
            &endpoint_positions,
        ) else {
            *render_visibility = Visibility::Hidden;
            continue;
        };

        if label.hide_behind_body
            && globe_label_is_occluded_by_body(
                *celestial_space,
                &label.body,
                camera_origin_units,
                position_units,
                &bodies,
            )
        {
            *render_visibility = Visibility::Hidden;
            continue;
        }

        let Some(world_position) =
            camera_relative_world_position(&camera_transform, camera_origin_units, position_units)
        else {
            *render_visibility = Visibility::Hidden;
            continue;
        };
        let Ok(viewport_position) = camera.world_to_viewport(&camera_transform, world_position)
        else {
            *render_visibility = Visibility::Hidden;
            continue;
        };

        *render_visibility = Visibility::Inherited;
        transform.translation = globe_label_screen_translation(
            window,
            viewport_position,
            label.screen_offset_px,
            label.z,
        );
    }
}

fn spawn_globe_label_camera(commands: &mut Commands<'_, '_>) {
    commands.spawn((
        Name::new("Ferrisium Globe Label Camera"),
        GlobeLabelCamera,
        Camera2d,
        Camera {
            order: 30,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(GLOBE_LABEL_RENDER_LAYER),
    ));
}

fn retain_live_globe_labels(
    commands: &mut Commands<'_, '_>,
    label_entities: &mut GlobeLabelEntities,
    live_labels: &HashSet<Entity>,
) {
    label_entities.labels.retain(|label, handles| {
        if live_labels.contains(label) {
            true
        } else {
            commands.entity(handles.entity).despawn();
            false
        }
    });
}

fn hide_all_globe_labels(
    label_entities: &mut GlobeLabelEntities,
    render_labels: &mut GlobeLabelRenderQuery<'_, '_>,
) {
    for handles in label_entities.labels.values() {
        if let Ok((_, _, _, _, mut visibility)) = render_labels.get_mut(handles.entity) {
            *visibility = Visibility::Hidden;
        }
    }
}

fn ensure_globe_label_render_entity(
    commands: &mut Commands<'_, '_>,
    label_entities: &mut GlobeLabelEntities,
    label_entity: Entity,
    label: &GlobeLabel,
) -> Entity {
    if let Some(handles) = label_entities.labels.get(&label_entity) {
        return handles.entity;
    }

    let color_key = GlobeLabelColorKey::from_rgba(label.color_rgba);
    let font_size = normalized_font_size(label.font_size);
    let render_entity = commands
        .spawn((
            Name::new("Globe Label"),
            GlobeLabelRenderEntity,
            Text2d::new(label.text.clone()),
            TextFont::from_font_size(font_size),
            TextLayout::new_with_justify(Justify::Center),
            TextColor(color_from_rgba(label.color_rgba)),
            Text2dShadow {
                offset: Vec2::new(1.5, -1.5),
                color: Color::BLACK.with_alpha(0.86),
            },
            Anchor::CENTER,
            Transform::from_translation(Vec3::new(0.0, 0.0, label.z)),
            Visibility::Hidden,
            RenderLayers::layer(GLOBE_LABEL_RENDER_LAYER),
        ))
        .id();
    label_entities.labels.insert(
        label_entity,
        GlobeLabelHandles {
            entity: render_entity,
            text: label.text.clone(),
            color_key,
            font_size_bits: font_size.to_bits(),
        },
    );

    render_entity
}

fn update_globe_label_text_components(
    text: &mut Text2d,
    font: &mut TextFont,
    color: &mut TextColor,
    label_entities: &mut GlobeLabelEntities,
    label_entity: Entity,
    label: &GlobeLabel,
) {
    let Some(handles) = label_entities.labels.get_mut(&label_entity) else {
        return;
    };
    if handles.text != label.text {
        (**text).clone_from(&label.text);
        handles.text.clone_from(&label.text);
    }

    let font_size = normalized_font_size(label.font_size);
    let font_size_bits = font_size.to_bits();
    if handles.font_size_bits != font_size_bits {
        font.font_size = font_size;
        handles.font_size_bits = font_size_bits;
    }

    let color_key = GlobeLabelColorKey::from_rgba(label.color_rgba);
    if handles.color_key != color_key {
        color.0 = color_from_rgba(label.color_rgba);
        handles.color_key = color_key;
    }
}

fn resolve_globe_label_position<F>(
    celestial_space: CelestialSpace,
    body: &CelestialBody,
    position: GlobeLabelPosition,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
    endpoint_positions: &Query<'_, '_, &GlobePosition>,
) -> Option<DVec3>
where
    F: bevy::ecs::query::QueryFilter,
{
    resolve_globe_anchor_position(celestial_space, body, position, bodies, endpoint_positions)
}

fn globe_label_is_occluded_by_body<F>(
    celestial_space: CelestialSpace,
    body: &CelestialBody,
    camera_origin_units: DVec3,
    label_position_units: DVec3,
    bodies: &Query<'_, '_, (&CelestialBodyPlacement, Option<&CellCoord>, &Transform), F>,
) -> bool
where
    F: bevy::ecs::query::QueryFilter,
{
    let center_position = GlobePosition::body_fixed(body, 0.0, 0.0, 0.0);
    let Some(center_units) =
        globe_position_scene_position_units(celestial_space, &center_position, bodies)
    else {
        return false;
    };
    let radius_units = body.mean_radius_units(celestial_space.render_scale);
    let Some(occlusion_target_units) =
        globe_label_occlusion_target_units(label_position_units, center_units, radius_units)
    else {
        return false;
    };
    segment_hits_sphere_before_target(
        camera_origin_units,
        occlusion_target_units,
        center_units,
        radius_units,
    )
}

fn globe_label_occlusion_target_units(
    label_position_units: DVec3,
    center_units: DVec3,
    radius_units: f64,
) -> Option<DVec3> {
    if !radius_units.is_finite() || radius_units <= 0.0 {
        return None;
    }
    let center_to_label = label_position_units - center_units;
    let distance_from_center = center_to_label.length();
    if !distance_from_center.is_finite() || distance_from_center <= f64::EPSILON {
        return None;
    }

    let occlusion_distance = distance_from_center.max(radius_units);
    Some(center_units + center_to_label / distance_from_center * occlusion_distance)
}

fn segment_hits_sphere_before_target(
    start_units: DVec3,
    target_units: DVec3,
    center_units: DVec3,
    radius_units: f64,
) -> bool {
    let target_delta = target_units - start_units;
    let target_distance = target_delta.length();
    if target_distance <= f64::EPSILON || !radius_units.is_finite() || radius_units <= 0.0 {
        return false;
    }
    let direction = target_delta / target_distance;
    let origin = start_units - center_units;
    let b = 2.0 * origin.dot(direction);
    let c = origin.length_squared() - radius_units * radius_units;
    let discriminant = b.mul_add(b, -4.0 * c);
    if discriminant < 0.0 {
        return false;
    }

    let near_distance = (-b - discriminant.sqrt()) * 0.5;
    near_distance > f64::EPSILON && near_distance < target_distance - 1.0e-4
}

fn active_globe_label_camera<'a>(
    celestial_space: CelestialSpace,
    cameras: impl Iterator<
        Item = (
            &'a Camera,
            &'a GlobalTransform,
            &'a Transform,
            Option<&'a CellCoord>,
        ),
    >,
) -> Option<(&'a Camera, GlobalTransform, DVec3)> {
    let mut cameras = cameras;
    let (camera, camera_global_transform, camera_local_transform, camera_cell) = cameras.next()?;
    let camera_transform = current_globe_camera_transform(
        camera_global_transform,
        camera_local_transform,
        camera_cell,
    );
    let camera_origin_units =
        camera_origin_units(celestial_space, camera_cell, camera_transform.translation());

    Some((camera, camera_transform, camera_origin_units))
}

fn camera_origin_units(
    celestial_space: CelestialSpace,
    camera_cell: Option<&CellCoord>,
    camera_translation: Vec3,
) -> DVec3 {
    camera_cell.map_or_else(
        || camera_translation.as_dvec3(),
        |cell| {
            celestial_space
                .grid()
                .grid_position_double(cell, &Transform::from_translation(camera_translation))
        },
    )
}

fn camera_relative_world_position(
    camera_transform: &GlobalTransform,
    camera_origin_units: DVec3,
    position_units: DVec3,
) -> Option<Vec3> {
    let relative = position_units - camera_origin_units;
    dvec3_fits_f32(relative).then(|| camera_transform.translation() + relative.as_vec3())
}

fn globe_label_screen_translation(
    window: &Window,
    viewport_position: Vec2,
    offset_px: Vec2,
    z: f32,
) -> Vec3 {
    Vec3::new(
        viewport_position.x + offset_px.x - window.width() * 0.5,
        window.height() * 0.5 - viewport_position.y + offset_px.y,
        z,
    )
}

fn normalized_color_component(component: f32) -> f32 {
    if component.is_finite() {
        component.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn normalized_font_size(font_size: f32) -> f32 {
    if font_size.is_finite() && font_size > 0.0 {
        font_size
    } else {
        DEFAULT_GLOBE_LABEL_FONT_SIZE
    }
}

fn dvec3_fits_f32(position: DVec3) -> bool {
    dvec3_is_finite(position)
        && position.x.abs() <= f64::from(f32::MAX)
        && position.y.abs() <= f64::from(f32::MAX)
        && position.z.abs() <= f64::from(f32::MAX)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{
        App, Camera, GlobalTransform, Text2d, TextColor, TextFont, Transform, Update, Visibility,
        Window, With,
    };
    use bevy::window::PrimaryWindow;
    use ferrisium_core::space::BodyId;

    use super::{
        camera_origin_units, camera_relative_world_position, globe_label_occlusion_target_units,
        globe_label_screen_translation, segment_hits_sphere_before_target, sync_globe_labels,
        GlobeLabelEntities, GlobeLabelPosition, GLOBE_LABEL_RENDER_LAYER,
    };
    use crate::{
        CelestialSpace, FerrisiumViewMode, Globe3dCamera, Globe3dViewBundle, GlobeLabel,
        GlobeLabelBundle, GlobeLabelCamera, DEFAULT_GLOBE_LABEL_FONT_SIZE,
    };

    fn assert_close_f32(lhs: f32, rhs: f32) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-6,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    fn assert_rgba_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= f32::EPSILON,
                "rgba mismatch: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn earth_label_defaults_to_earth_body_and_normalizes_style() {
        let label = GlobeLabel::earth(
            GlobeLabelPosition::geodetic(39.0, -105.0, 0.0),
            "Denver",
            [-1.0, 0.5, 2.0, f32::NAN],
        )
        .with_font_size(f32::NAN);

        assert_eq!(label.body.id, BodyId::EARTH);
        assert_rgba_close(label.color_rgba, [0.0, 0.5, 1.0, 0.0]);
        assert_close_f32(label.font_size, DEFAULT_GLOBE_LABEL_FONT_SIZE);
        assert!(label.hide_behind_body);
        assert_eq!(
            GlobeLabelBundle::new(label).visibility,
            Visibility::Inherited
        );
    }

    #[test]
    fn camera_relative_position_keeps_large_absolute_values_out_of_view_projection() {
        let celestial_space = CelestialSpace::default();
        let camera_origin = camera_origin_units(
            celestial_space,
            None,
            bevy::prelude::Vec3::new(1.0, 2.0, 3.0),
        );
        let camera_transform = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
        let position = camera_origin + bevy::math::DVec3::new(10.0, 20.0, 30.0);
        let world_position =
            camera_relative_world_position(&camera_transform, camera_origin, position);

        assert_eq!(
            world_position,
            Some(bevy::prelude::Vec3::new(11.0, 22.0, 33.0))
        );
    }

    #[test]
    fn label_screen_translation_uses_centered_overlay_coordinates() {
        let window = Window {
            resolution: (1000, 800).into(),
            ..Default::default()
        };
        let translation = globe_label_screen_translation(
            &window,
            bevy::prelude::Vec2::new(550.0, 300.0),
            bevy::prelude::Vec2::new(4.0, -8.0),
            12.0,
        );

        assert_eq!(translation, bevy::prelude::Vec3::new(54.0, 92.0, 12.0));
    }

    #[test]
    fn body_occlusion_hides_far_side_labels_only() {
        let camera = bevy::math::DVec3::new(0.0, 0.0, 10.0);
        let center = bevy::math::DVec3::ZERO;
        let near_label = bevy::math::DVec3::new(0.0, 0.0, 1.0);
        let far_label = bevy::math::DVec3::new(0.0, 0.0, -1.0);

        assert!(!segment_hits_sphere_before_target(
            camera, near_label, center, 1.0
        ));
        assert!(segment_hits_sphere_before_target(
            camera, far_label, center, 1.0
        ));
    }

    #[test]
    fn body_occlusion_treats_inside_surface_labels_as_surface_directions() {
        let camera = bevy::math::DVec3::new(0.0, 0.0, 10.0);
        let center = bevy::math::DVec3::ZERO;
        let near_label = bevy::math::DVec3::new(0.0, 0.0, 0.99);
        let far_label = bevy::math::DVec3::new(0.0, 0.0, -0.99);
        let near_target = bevy::math::DVec3::new(0.0, 0.0, 1.0);
        let far_target = bevy::math::DVec3::new(0.0, 0.0, -1.0);

        assert_eq!(
            globe_label_occlusion_target_units(near_label, center, 1.0),
            Some(near_target)
        );
        assert_eq!(
            globe_label_occlusion_target_units(far_label, center, 1.0),
            Some(far_target)
        );
        assert!(!segment_hits_sphere_before_target(
            camera,
            near_target,
            center,
            1.0
        ));
        assert!(segment_hits_sphere_before_target(
            camera, far_target, center, 1.0
        ));
    }

    #[test]
    fn globe_label_sync_spawns_overlay_camera_and_text_entity() {
        let mut app = App::new();
        app.init_resource::<CelestialSpace>()
            .init_resource::<FerrisiumViewMode>()
            .init_resource::<GlobeLabelEntities>()
            .add_systems(Update, sync_globe_labels);
        app.world_mut().spawn((
            Window {
                resolution: (1000, 800).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ));
        app.world_mut().spawn(Globe3dViewBundle::default());
        let camera_transform = Transform::from_xyz(0.0, 0.0, 10_000.0);
        app.world_mut().spawn((
            Globe3dCamera,
            Camera::default(),
            camera_transform,
            GlobalTransform::from(camera_transform),
        ));
        let label = app
            .world_mut()
            .spawn(GlobeLabelBundle::new(GlobeLabel::earth(
                GlobeLabelPosition::geodetic(0.0, 0.0, 0.0),
                "Origin",
                [1.0, 1.0, 1.0, 1.0],
            )))
            .id();

        app.update();

        let label_camera_count = {
            let mut query = app
                .world_mut()
                .query_filtered::<(), With<GlobeLabelCamera>>();
            query.iter(app.world()).count()
        };
        assert_eq!(label_camera_count, 1);
        let render_entity = app
            .world()
            .resource::<GlobeLabelEntities>()
            .labels
            .get(&label)
            .map(|handles| handles.entity);
        assert!(render_entity.is_some(), "label render entity should exist");
        let Some(render_entity) = render_entity else {
            return;
        };
        let world = app.world();
        assert!(world.get::<Text2d>(render_entity).is_some());
        assert!(world.get::<TextFont>(render_entity).is_some());
        assert!(world.get::<TextColor>(render_entity).is_some());
        assert_eq!(
            world
                .get::<bevy::camera::visibility::RenderLayers>(render_entity)
                .map(|layers| layers.iter().collect::<Vec<_>>()),
            Some(vec![GLOBE_LABEL_RENDER_LAYER])
        );
        assert!(world.get::<Visibility>(render_entity).is_some());
    }
}
