#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(test)]
use bevy::prelude::Visibility;
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use big_space::prelude::CellCoord;
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::{
    CelestialEphemeris, CelestialEpoch, CelestialFocus, CelestialRoot, FerrisiumSet, Globe3dView,
    Globe3dViewBundle, MetricSceneBodyAnchorBundle, MetricSceneBodySurfaceLayerBundle,
    MetricSceneFocusSelection, PrimaryCelestialBody,
};
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_bevy::prelude::{
    Globe3dState, MetricSceneLodPolicy, MetricScenePresentation, MetricScenePresentationSet,
    TiledBodySurface,
};
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::sync_metric_scene_focus_to_celestial_focus;
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_core::prelude::{LonLat, NasaTrekRegularBody, TileSource};
#[cfg(target_arch = "wasm32")]
use ferrisium_core::space::StateRequest;
#[cfg(any(target_arch = "wasm32", test))]
use ferrisium_core::space::{
    BodyId, CelestialBody, EphemerisProvider, Epoch, FrameId, RenderScale, Vec3d,
};

#[cfg(target_arch = "wasm32")]
use crate::browser_params::browser_local_storage_item;
#[cfg(target_arch = "wasm32")]
use crate::custom_geo_draw;
#[cfg(target_arch = "wasm32")]
use crate::demo_config::{
    browser_demo_focus, browser_demo_focus_token, browser_demo_globe_camera_override,
    browser_demo_metric_focus_selection, demo_focus_body_id_for_trek_body,
    demo_focus_metric_target, demo_globe_ambient_fill_light, demo_globe_skybox_config,
    parse_demo_focus, GLOBE_FOCUS_STORAGE_KEY,
};
#[cfg(any(target_arch = "wasm32", test))]
use crate::demo_config::{
    DemoGlobeCameraOverride, DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
    DEMO_GLOBE_CAMERA_QUERY_MAX_DISTANCE_RADIUS_FACTOR,
    DEMO_GLOBE_CAMERA_QUERY_MIN_DISTANCE_RADIUS_FACTOR,
};
#[cfg(any(target_arch = "wasm32", test))]
use crate::ephemeris_demo::DEMO_EPOCH_TDB_NANOSECONDS;
#[cfg(target_arch = "wasm32")]
use crate::ephemeris_demo::{
    sync_demo_anise_kernel_bundle, DemoAniseKernelLoader, DemoEphemerisProvider,
};
#[cfg(target_arch = "wasm32")]
use crate::h3_demo;

#[cfg(target_arch = "wasm32")]
pub(crate) const EARTH_MARKER_RADIUS_FACTOR: f32 = 0.024;
#[cfg(target_arch = "wasm32")]
pub(crate) const MOON_MARKER_RADIUS_FACTOR: f32 = 0.035;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_MOON_TILE_ZOOM: u8 = 3;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_MARS_TILE_ZOOM: u8 = 2;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_REGULAR_BODY_TILE_ZOOM: u8 = 3;
#[cfg(target_arch = "wasm32")]
pub(crate) const DEMO_SUNLIGHT_ILLUMINANCE: f32 = 32_000.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const DEMO_SUN_LIGHT_DIRECTION_DISTANCE: f32 = 10_000.0;
#[cfg(target_arch = "wasm32")]
pub(crate) const DEMO_EARTH_FOCUS_CAMERA_DISTANCE_RADIUS_FACTOR: f32 = 2.5;
#[cfg(target_arch = "wasm32")]
pub(crate) const DEMO_SECONDARY_FOCUS_CAMERA_DISTANCE_RADIUS_FACTOR: f32 = 3.0;
// The globe camera already caps outward zoom, so keep selected secondary bodies
// in the tiled-surface presentation throughout the reachable globe range.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const DEMO_GLOBE_SURFACE_LOD_DISTANCE_RADIUS_FACTOR: f64 = 1_000_000.0;

#[cfg(target_arch = "wasm32")]
pub(crate) fn configure_globe_mode(app: &mut App) {
    app.insert_resource(demo_globe_skybox_config().deferred())
        .insert_resource(browser_demo_focus())
        .insert_resource(browser_demo_metric_focus_selection())
        .insert_resource(browser_demo_globe_camera_override())
        .insert_resource(DemoGlobeUiState::new(browser_demo_focus_token()))
        .insert_resource(demo_globe_ambient_fill_light())
        .insert_resource(CelestialEpoch::new(
            Epoch::from_tdb_nanoseconds_since_j2000(DEMO_EPOCH_TDB_NANOSECONDS),
        ))
        .insert_resource(CelestialEphemeris::from_provider(
            DemoEphemerisProvider::default(),
        ))
        .init_resource::<DemoAniseKernelLoader>()
        .init_resource::<DemoCelestialVisualsSpawned>()
        .init_resource::<custom_geo_draw::DemoCustomGlobeLayer>()
        .add_systems(
            Startup,
            (setup_globe, h3_demo::spawn_usa_h3_overlay).chain(),
        )
        .add_systems(
            Update,
            (
                sync_demo_anise_kernel_bundle,
                sync_demo_sun_light.after(sync_demo_anise_kernel_bundle),
                spawn_demo_celestial_visuals,
                sync_demo_globe_ui_focus.before(sync_metric_scene_focus_to_celestial_focus),
                handle_demo_focus_shortcuts.before(sync_metric_scene_focus_to_celestial_focus),
                sync_demo_globe_secondary_body_visibility
                    .after(spawn_demo_celestial_visuals)
                    .after(sync_metric_scene_focus_to_celestial_focus),
                sync_demo_globe_camera_distance_for_focus
                    .after(sync_metric_scene_focus_to_celestial_focus),
                h3_demo::log_h3_cell_clicks,
                custom_geo_draw::sync_demo_custom_globe_layer
                    .in_set(FerrisiumSet::ProjectGeoOverlays),
            ),
        );
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Debug, Clone)]
pub(crate) struct DemoGlobeUiState {
    focus_token: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl DemoGlobeUiState {
    pub(crate) fn new(focus_token: Option<String>) -> Self {
        Self { focus_token }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoSunLight;

#[cfg(target_arch = "wasm32")]
fn sync_demo_sun_light(
    celestial_epoch: Res<'_, CelestialEpoch>,
    celestial_ephemeris: Res<'_, CelestialEphemeris>,
    mut sun_lights: Query<'_, '_, &mut Transform, With<DemoSunLight>>,
) {
    let Some(provider) = celestial_ephemeris.provider() else {
        return;
    };
    let request = StateRequest::new(
        BodyId::SUN,
        BodyId::EARTH,
        FrameId::j2000(),
        celestial_epoch.epoch,
    );
    let Ok(state) = provider.state(&request) else {
        return;
    };
    let sun_direction = vec3d_direction_to_vec3(state.position_km);

    for mut transform in &mut sun_lights {
        *transform = sun_light_transform(sun_direction);
    }
}

#[cfg(target_arch = "wasm32")]
fn sun_light_transform(sun_direction: Vec3) -> Transform {
    let sun_direction = sun_direction.normalize_or_zero();
    if sun_direction == Vec3::ZERO {
        return fallback_sun_light_transform();
    }

    let up = if sun_direction.abs().dot(Vec3::Y) > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };

    Transform::from_translation(sun_direction * DEMO_SUN_LIGHT_DIRECTION_DISTANCE)
        .looking_at(Vec3::ZERO, up)
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::cast_possible_truncation,
    reason = "Only the normalized Sun direction is used for Bevy lighting."
)]
fn vec3d_direction_to_vec3(direction: Vec3d) -> Vec3 {
    Vec3::new(direction.x as f32, direction.y as f32, direction.z as f32)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn fallback_sun_light_transform() -> Transform {
    sun_light_transform(Vec3::new(1.0, 0.25, 0.4))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn setup_globe(
    mut commands: Commands<'_, '_>,
    focus: Res<'_, CelestialFocus>,
    epoch: Res<'_, CelestialEpoch>,
    ephemeris: Res<'_, CelestialEphemeris>,
    camera_override: Res<'_, DemoGlobeCameraOverride>,
) {
    let mut globe_state = Globe3dState {
        yaw_deg: -20.0,
        pitch_deg: 24.0,
        tile_zoom: 4,
        max_selected_tiles: 768,
        ..Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL)
    };
    reset_demo_camera_distance_for_focus(&mut globe_state, focus.target);
    apply_demo_globe_camera_override_with_ephemeris(
        &mut globe_state,
        focus.target,
        *camera_override,
        epoch.epoch,
        ephemeris.provider(),
    );

    commands.spawn(Globe3dViewBundle {
        globe_state,
        ..default()
    });
    commands.spawn((
        Name::new("Ferrisium Sun Light"),
        DemoSunLight,
        DirectionalLight {
            illuminance: DEMO_SUNLIGHT_ILLUMINANCE,
            shadows_enabled: false,
            ..default()
        },
        fallback_sun_light_transform(),
    ));
}

#[cfg(target_arch = "wasm32")]
fn handle_demo_focus_shortcuts(
    keyboard: Res<'_, ButtonInput<KeyCode>>,
    mut metric_focus: ResMut<'_, MetricSceneFocusSelection>,
) {
    let Some(next_focus) = demo_focus_shortcut(&keyboard) else {
        return;
    };

    metric_focus.set_target(demo_focus_metric_target(next_focus.target));
}

#[cfg(target_arch = "wasm32")]
fn sync_demo_globe_ui_focus(
    mut ui_state: ResMut<'_, DemoGlobeUiState>,
    mut metric_focus: ResMut<'_, MetricSceneFocusSelection>,
) {
    let next_token = browser_local_storage_item(GLOBE_FOCUS_STORAGE_KEY);
    if next_token == ui_state.focus_token {
        return;
    }
    ui_state.focus_token.clone_from(&next_token);

    let Some(next_focus) = next_token
        .as_deref()
        .and_then(parse_demo_focus)
        .map(demo_focus_metric_target)
    else {
        return;
    };

    metric_focus.set_target(next_focus);
}

#[cfg(target_arch = "wasm32")]
fn sync_demo_globe_camera_distance_for_focus(
    focus: Res<'_, CelestialFocus>,
    epoch: Res<'_, CelestialEpoch>,
    ephemeris: Res<'_, CelestialEphemeris>,
    camera_override: Res<'_, DemoGlobeCameraOverride>,
    mut globe_views: Query<'_, '_, &mut Globe3dState, With<Globe3dView>>,
) {
    let surface_target_needs_reapply =
        camera_override.surface_lon_lat.is_some() && (epoch.is_changed() || ephemeris.is_changed());
    if !focus.is_changed() && !surface_target_needs_reapply {
        return;
    }

    for mut globe_state in &mut globe_views {
        if focus.is_changed() {
            reset_demo_camera_distance_for_focus(&mut globe_state, focus.target);
        }
        apply_demo_globe_camera_override_with_ephemeris(
            &mut globe_state,
            focus.target,
            *camera_override,
            epoch.epoch,
            ephemeris.provider(),
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn demo_focus_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<CelestialFocus> {
    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::Numpad1) {
        return Some(CelestialFocus::earth());
    }
    if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::Numpad2) {
        return Some(CelestialFocus::moon());
    }
    if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::Numpad3) {
        return Some(CelestialFocus::mars_system());
    }

    None
}

#[cfg(target_arch = "wasm32")]
fn reset_demo_camera_distance_for_focus(globe_state: &mut Globe3dState, target: BodyId) {
    let focus_body = demo_focus_body(target);
    let radius = body_radius_units(&focus_body);
    let distance_factor = if target == BodyId::EARTH {
        DEMO_EARTH_FOCUS_CAMERA_DISTANCE_RADIUS_FACTOR
    } else {
        DEMO_SECONDARY_FOCUS_CAMERA_DISTANCE_RADIUS_FACTOR
    };
    let camera_distance = radius * distance_factor;

    globe_state.radius = radius;
    globe_state.camera_distance = camera_distance;
    globe_state.base_camera_distance = camera_distance;
}

#[cfg(test)]
pub(crate) fn apply_demo_globe_camera_override(
    globe_state: &mut Globe3dState,
    target: BodyId,
    camera_override: DemoGlobeCameraOverride,
) {
    apply_demo_globe_camera_override_with_ephemeris(
        globe_state,
        target,
        camera_override,
        Epoch::from_tdb_nanoseconds_since_j2000(DEMO_EPOCH_TDB_NANOSECONDS),
        None,
    );
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn apply_demo_globe_camera_override_with_ephemeris(
    globe_state: &mut Globe3dState,
    target: BodyId,
    camera_override: DemoGlobeCameraOverride,
    epoch: Epoch,
    provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) {
    if let Some(lon_lat) = camera_override.surface_lon_lat {
        let (yaw_deg, pitch_deg) =
            demo_globe_override_orbit_angles(target, lon_lat, epoch, provider);
        globe_state.yaw_deg = yaw_deg;
        globe_state.pitch_deg = pitch_deg;
    }
    if let Some(yaw_deg) = camera_override.yaw_deg {
        globe_state.yaw_deg = yaw_deg;
    }
    if let Some(pitch_deg) = camera_override.pitch_deg {
        globe_state.pitch_deg = pitch_deg.clamp(
            -DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
            DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
        );
    }
    if let Some(distance_factor) = camera_override.distance_radius_factor {
        let focus_body = demo_focus_body(target);
        let radius = body_radius_units(&focus_body);
        let distance = radius
            * distance_factor.clamp(
                DEMO_GLOBE_CAMERA_QUERY_MIN_DISTANCE_RADIUS_FACTOR,
                DEMO_GLOBE_CAMERA_QUERY_MAX_DISTANCE_RADIUS_FACTOR,
            );
        globe_state.camera_distance = distance;
        globe_state.base_camera_distance = distance;
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_globe_override_orbit_angles(
    _target: BodyId,
    lon_lat: LonLat,
    _epoch: Epoch,
    _provider: Option<&(dyn EphemerisProvider + Send + Sync + 'static)>,
) -> (f64, f64) {
    let local_surface = globe_surface_direction_for_lon_lat(lon_lat);
    orbit_angles_for_direction(local_surface)
}

#[cfg(test)]
pub(crate) fn demo_globe_orbit_angles_for_surface(
    target: BodyId,
    lon_lat: LonLat,
    epoch: Epoch,
) -> (f64, f64) {
    demo_globe_override_orbit_angles(target, lon_lat, epoch, None)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn globe_surface_direction_for_lon_lat(lon_lat: LonLat) -> Vec3d {
    let lon_rad = lon_lat.lon_deg.to_radians();
    let lat_rad = lon_lat
        .lat_deg
        .clamp(
            -DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
            DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
        )
        .to_radians();
    let horizontal_radius = lat_rad.cos();

    Vec3d::new(
        horizontal_radius * lon_rad.sin(),
        lat_rad.sin(),
        horizontal_radius * lon_rad.cos(),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn orbit_angles_for_direction(direction: Vec3d) -> (f64, f64) {
    let direction = normalize_vec3d(direction);
    (
        direction.x.atan2(direction.z).to_degrees(),
        direction.y.clamp(-1.0, 1.0).asin().to_degrees().clamp(
            -DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
            DEMO_GLOBE_CAMERA_QUERY_MAX_ABS_PITCH_DEG,
        ),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn normalize_vec3d(direction: Vec3d) -> Vec3d {
    let length =
        (direction.x * direction.x + direction.y * direction.y + direction.z * direction.z).sqrt();
    if length <= f64::EPSILON || !length.is_finite() {
        return Vec3d::ZERO;
    }

    Vec3d::new(
        direction.x / length,
        direction.y / length,
        direction.z / length,
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_focus_body(target: BodyId) -> CelestialBody {
    demo_globe_body(target).unwrap_or_else(CelestialBody::earth)
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_globe_body(target: BodyId) -> Option<CelestialBody> {
    if target == BodyId::EARTH {
        return Some(CelestialBody::earth());
    }

    demo_globe_trek_body(target).map(|body| demo_globe_regular_body(body, target))
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_globe_regular_body(body: NasaTrekRegularBody, target: BodyId) -> CelestialBody {
    let descriptor = body.descriptor();
    CelestialBody::spherical(
        target,
        descriptor.display_name,
        ferrisium_core::prelude::Kilometers::from_meters(f64::from(descriptor.mean_radius_meters)),
        FrameId::j2000(),
        FrameId::named(descriptor.iau_frame_name),
    )
}

#[cfg(any(target_arch = "wasm32", test))]
fn demo_globe_trek_body(target: BodyId) -> Option<NasaTrekRegularBody> {
    NasaTrekRegularBody::from_body_or_system_naif_id(target.naif_id)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn demo_globe_tiled_body_zoom(body: NasaTrekRegularBody) -> u8 {
    match body {
        NasaTrekRegularBody::Mercury | NasaTrekRegularBody::Venus => 2,
        NasaTrekRegularBody::Mars => DEMO_MARS_TILE_ZOOM,
        NasaTrekRegularBody::Moon => DEMO_MOON_TILE_ZOOM,
        _ => DEMO_REGULAR_BODY_TILE_ZOOM,
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
pub(crate) struct DemoCelestialVisualsSpawned(bool);

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoMoonBody;

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct DemoMarsBody;

#[cfg(target_arch = "wasm32")]
#[derive(Component, Debug, Clone, Copy)]
struct DemoSecondaryGlobeBody {
    target: BodyId,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_globe_tiled_body_surface(
    target: BodyId,
) -> Option<(CelestialBody, TiledBodySurface)> {
    let trek_body = demo_globe_trek_body(target)?;
    Some((
        demo_globe_regular_body(trek_body, target),
        TiledBodySurface::fixed_zoom(
            TileSource::nasa_trek_regular_body(trek_body),
            demo_globe_tiled_body_zoom(trek_body),
        ),
    ))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn globe_secondary_focus_distance_units(body: &CelestialBody) -> f64 {
    f64::from(body_radius_units(body) * DEMO_SECONDARY_FOCUS_CAMERA_DISTANCE_RADIUS_FACTOR)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn globe_secondary_body_lod_policy(body: &CelestialBody) -> MetricSceneLodPolicy {
    let radius_units = f64::from(body_radius_units(body));
    let surface_lod_distance = radius_units * DEMO_GLOBE_SURFACE_LOD_DISTANCE_RADIUS_FACTOR;
    MetricSceneLodPolicy::new(surface_lod_distance, surface_lod_distance)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const fn demo_globe_surface_presentations() -> MetricScenePresentationSet {
    MetricScenePresentationSet::only(MetricScenePresentation::Surface)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn demo_secondary_globe_body_visibility(body: BodyId, focus: BodyId) -> Visibility {
    if body == focus {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_demo_globe_secondary_body_visibility(
    focus: Res<'_, CelestialFocus>,
    mut bodies: Query<'_, '_, (&DemoSecondaryGlobeBody, &mut Visibility)>,
) {
    for (body, mut visibility) in &mut bodies {
        *visibility = demo_secondary_globe_body_visibility(body.target, focus.target);
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::too_many_arguments,
    reason = "Demo setup needs explicit ECS access to the generated celestial hierarchy and assets."
)]
pub(crate) fn spawn_demo_celestial_visuals(
    mut commands: Commands<'_, '_>,
    roots: Query<'_, '_, Entity, With<CelestialRoot>>,
    primary_bodies: Query<'_, '_, Entity, With<PrimaryCelestialBody>>,
    mut spawned: ResMut<'_, DemoCelestialVisualsSpawned>,
    focus: Res<'_, CelestialFocus>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    if spawned.0 {
        return;
    }

    let Some(root) = roots.iter().next() else {
        return;
    };
    let Some(earth) = primary_bodies.iter().next() else {
        return;
    };

    let mut moon_marker_target = None;
    for (index, trek_body) in NasaTrekRegularBody::all().iter().copied().enumerate() {
        let target = demo_focus_body_id_for_trek_body(trek_body);
        let Some((body, surface)) = demo_globe_tiled_body_surface(target) else {
            continue;
        };
        let body_entity = commands
            .spawn((
                Name::new(format!("Ferrisium {}", body.name)),
                MetricSceneBodyAnchorBundle::relative_to(
                    body.clone(),
                    BodyId::EARTH,
                    globe_secondary_focus_distance_units(&body),
                ),
                globe_secondary_body_lod_policy(&body),
                DemoSecondaryGlobeBody { target },
                demo_secondary_globe_body_visibility(target, focus.target),
                CellCoord::default(),
                demo_globe_secondary_initial_transform(
                    index,
                    NasaTrekRegularBody::all().len(),
                    &body,
                ),
            ))
            .id();
        commands.entity(root).add_child(body_entity);

        if target == BodyId::MOON {
            commands.entity(body_entity).insert(DemoMoonBody);
            moon_marker_target = Some((body_entity, body_radius_units(&body)));
        } else if target == BodyId::MARS_BARYCENTER {
            commands.entity(body_entity).insert(DemoMarsBody);
        }

        spawn_globe_tiled_body_surface_layer(
            &mut commands,
            body_entity,
            format!("Ferrisium {} Surface Layer", body.name),
            body,
            demo_globe_surface_presentations(),
            surface,
        );
    }

    spawn_body_rotation_markers(
        &mut commands,
        &mut meshes,
        &mut materials,
        earth,
        body_radius_units(&CelestialBody::earth()),
        EARTH_MARKER_RADIUS_FACTOR,
        "Earth",
    );
    if let Some((moon_entity, moon_radius)) = moon_marker_target {
        spawn_body_rotation_markers(
            &mut commands,
            &mut meshes,
            &mut materials,
            moon_entity,
            moon_radius,
            MOON_MARKER_RADIUS_FACTOR,
            "Moon",
        );
    }

    spawned.0 = true;
}

#[cfg(target_arch = "wasm32")]
#[allow(
    clippy::cast_precision_loss,
    reason = "The fallback visual ring is renderer-only placement for unsupported ephemeris bodies."
)]
fn demo_globe_secondary_initial_transform(
    index: usize,
    total: usize,
    body: &CelestialBody,
) -> Transform {
    let angle = (index as f32 / total.max(1) as f32) * std::f32::consts::TAU;
    let distance = (body_radius_units(body) * 16.0).max(80.0);
    Transform::from_translation(Vec3::new(
        angle.cos() * distance,
        0.0,
        angle.sin() * distance,
    ))
}

#[cfg(target_arch = "wasm32")]
fn spawn_globe_tiled_body_surface_layer(
    commands: &mut Commands<'_, '_>,
    parent: Entity,
    name: String,
    celestial_body: CelestialBody,
    presentations: MetricScenePresentationSet,
    surface: TiledBodySurface,
) -> Entity {
    let entity = commands
        .spawn((
            Name::new(name),
            MetricSceneBodySurfaceLayerBundle::for_body_set(celestial_body, presentations),
            surface,
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

#[cfg(target_arch = "wasm32")]
fn spawn_body_rotation_markers(
    commands: &mut Commands<'_, '_>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    body_radius: f32,
    marker_radius_factor: f32,
    label: &str,
) {
    let marker_radius = body_radius * marker_radius_factor;
    let surface_radius = body_radius + marker_radius * 1.25;
    let prime_marker = spawn_surface_marker(
        commands,
        meshes,
        materials,
        format!("{label} body-fixed +Z marker"),
        Vec3::Z * surface_radius,
        marker_radius,
        Color::srgb(1.0, 0.86, 0.30),
    );
    let east_marker = spawn_surface_marker(
        commands,
        meshes,
        materials,
        format!("{label} body-fixed +X marker"),
        Vec3::X * surface_radius,
        marker_radius,
        Color::srgb(0.95, 0.22, 0.18),
    );

    commands.entity(parent).add_child(prime_marker);
    commands.entity(parent).add_child(east_marker);
}

#[cfg(target_arch = "wasm32")]
fn spawn_surface_marker(
    commands: &mut Commands<'_, '_>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: String,
    position: Vec3,
    radius: f32,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            Transform::from_translation(position),
            Mesh3d(meshes.add(Sphere::new(radius).mesh().uv(24, 12))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                emissive: color.into(),
                unlit: true,
                ..default()
            })),
        ))
        .id()
}

#[cfg(any(target_arch = "wasm32", test))]
#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy mesh radii are f32; planetary radii in kilometers fit comfortably for rendering."
)]
pub(crate) fn body_radius_units(body: &CelestialBody) -> f32 {
    body.mean_radius_units(RenderScale::PHYSICAL) as f32
}
