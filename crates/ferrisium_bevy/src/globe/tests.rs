//! Regression tests for globe camera placement, projection, picking, and input helpers.

use super::{
    camera_direction_for_surface_grab_pan, camera_frame_surface_direction, clamp_scroll_delta,
    distance_control_scale, globe_camera_grid_transform, globe_camera_position,
    globe_camera_preferred_up, globe_orbit_angles, globe_radius_from_render_units,
    globe_surface_pan_degrees_per_pixel, max_camera_distance_for_radius,
    min_camera_distance_for_source, min_globe_far_plane, orbit_angles_for_surface_direction,
    pinch_zoom_delta, projected_camera_up, ray_origin_units, ray_sphere_intersection,
    ray_sphere_intersection_from_origin, source_resolution_surface_distance, spawn_globe_camera,
    sync_globe_camera, sync_globe_projection, wheel_zoom_delta, zoom_globe_camera, Globe3dCamera,
    Globe3dState, Globe3dViewBundle, GlobeCameraUpMode, GlobeFocusTarget,
    MAX_SCROLL_DELTA_PER_FRAME, ZOOM_EXP_FACTOR,
};
use bevy::math::{DVec3, Dir3, Quat, Vec2, Vec3};
use bevy::prelude::{
    App, Camera3d, ChildOf, Entity, PerspectiveProjection, Projection, Ray3d, Transform, Update,
    With,
};
use big_space::prelude::{BigSpace, CellCoord, FloatingOrigin, Grid};
use ferrisium_core::coords::LonLat;
use ferrisium_core::source::TileSource;
use ferrisium_core::space::{BodyId, CelestialBody, Kilometers, RenderScale};

use crate::celestial::{
    CelestialBodyPlacement, CelestialFocus, CelestialSpace, CelestialSpaceEntities,
};
use crate::metric_scene::{
    MetricSceneFocusPivot, MetricSceneFocusTarget, MetricSceneId, MetricSceneSurfaceTarget,
};
use crate::source::ActiveTileSource;
use crate::surface_depth::max_default_surface_visual_lift;

fn assert_dvec3_close(lhs: DVec3, rhs: DVec3) {
    assert!(
        lhs.distance(rhs) <= 0.001,
        "vector mismatch: lhs={lhs:?}, rhs={rhs:?}"
    );
}

fn assert_vec3_close(lhs: Vec3, rhs: Vec3) {
    assert!(
        lhs.distance(rhs) <= 1.0e-5,
        "vector mismatch: lhs={lhs:?}, rhs={rhs:?}"
    );
}

fn earth_focus_target(state: Globe3dState) -> GlobeFocusTarget {
    GlobeFocusTarget {
        body: BodyId::EARTH,
        center_units: DVec3::ZERO,
        radius: state.radius,
        body_rotation: Quat::IDENTITY,
    }
}

#[test]
fn automatic_globe_camera_spawns_big_space_hierarchy() {
    let mut app = App::new();
    app.init_resource::<CelestialSpace>()
        .init_resource::<CelestialSpaceEntities>()
        .add_systems(Update, spawn_globe_camera);
    app.world_mut().spawn(Globe3dViewBundle::default());

    app.update();

    let entities = *app.world().resource::<CelestialSpaceEntities>();
    assert!(entities.root.is_some());
    assert!(entities.primary_body.is_some());
    let Some(root) = entities.root else {
        return;
    };
    let Some(primary_body) = entities.primary_body else {
        return;
    };

    let world = app.world_mut();
    assert!(world.get::<BigSpace>(root).is_some());
    assert!(world.get::<Grid>(root).is_some());
    assert!(world.get::<CellCoord>(primary_body).is_some());
    assert_eq!(
        world.get::<ChildOf>(primary_body).map(ChildOf::parent),
        Some(root)
    );

    let mut cameras = world.query_filtered::<Entity, (With<Globe3dCamera>, With<FloatingOrigin>)>();
    let cameras = cameras.iter(world).collect::<Vec<_>>();
    assert_eq!(cameras.len(), 1);
    let Some(camera) = cameras.first().copied() else {
        return;
    };

    assert!(world.get::<Camera3d>(camera).is_some());
    assert_eq!(
        world.get::<ChildOf>(camera).map(ChildOf::parent),
        Some(root)
    );
}

#[test]
fn sync_globe_camera_resets_big_space_cell_instead_of_accumulating() {
    let celestial_space = CelestialSpace::default();
    let state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
    let (expected_cell, _) =
        globe_camera_grid_transform(state, celestial_space, DVec3::ZERO, Quat::IDENTITY);
    let mut app = App::new();
    app.insert_resource(celestial_space)
        .insert_resource(CelestialFocus::earth())
        .insert_resource(ActiveTileSource::from(TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png",
        )))
        .init_resource::<MetricSceneFocusPivot>()
        .add_systems(Update, sync_globe_camera);
    app.world_mut().spawn(Globe3dViewBundle {
        globe_state: state,
        ..Globe3dViewBundle::default()
    });
    let camera = app
        .world_mut()
        .spawn((
            Globe3dCamera,
            Camera3d::default(),
            CellCoord::new(expected_cell.x + 3, expected_cell.y, expected_cell.z),
            Transform::from_translation(Vec3::splat(12_345.0)),
        ))
        .id();

    app.update();
    app.update();

    let world = app.world();
    let Some(actual_cell) = world.get::<CellCoord>(camera).copied() else {
        return;
    };
    let Some(actual_transform) = world.get::<Transform>(camera) else {
        return;
    };

    assert_eq!(actual_cell, expected_cell);
    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&actual_cell, actual_transform),
        globe_camera_position(state),
    );
}

#[test]
fn sync_globe_camera_orbits_the_selected_focus_body() {
    let celestial_space = CelestialSpace::default();
    let moon = CelestialBody::moon();
    let moon_radius = globe_radius_from_render_units(moon.mean_radius_units(RenderScale::PHYSICAL));
    let moon_center = DVec3::new(384_400.0, 0.0, 0.0);
    let moon_placement = celestial_space.place_position_units(moon_center);
    let state = Globe3dState {
        camera_distance: moon_radius * 3.0,
        base_camera_distance: moon_radius * 3.0,
        ..Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL)
    };
    let mut app = App::new();
    app.insert_resource(celestial_space)
        .insert_resource(CelestialFocus::moon())
        .insert_resource(ActiveTileSource::from(TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png",
        )))
        .init_resource::<MetricSceneFocusPivot>()
        .add_systems(Update, sync_globe_camera);
    let globe_view = app
        .world_mut()
        .spawn(Globe3dViewBundle {
            globe_state: state,
            ..Globe3dViewBundle::default()
        })
        .id();
    app.world_mut().spawn((
        CelestialBodyPlacement::for_body_relative_to(moon, BodyId::EARTH),
        moon_placement.cell,
        Transform::from_translation(moon_placement.translation),
    ));
    let camera = app
        .world_mut()
        .spawn((
            Globe3dCamera,
            Camera3d::default(),
            CellCoord::default(),
            Transform::default(),
        ))
        .id();

    app.update();

    let world = app.world();
    let Some(actual_cell) = world.get::<CellCoord>(camera).copied() else {
        return;
    };
    let Some(actual_transform) = world.get::<Transform>(camera) else {
        return;
    };

    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&actual_cell, actual_transform),
        moon_center + globe_camera_position(state),
    );
    let Some(globe_state) = world.get::<Globe3dState>(globe_view) else {
        return;
    };
    assert!((globe_state.radius - moon_radius).abs() < 0.001);
}

#[test]
fn body_north_globe_camera_places_orbit_in_body_fixed_frame() {
    let celestial_space = CelestialSpace::default();
    let state = Globe3dState {
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        camera_distance: 10.0,
        ..Globe3dState::default()
    };
    let body_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);

    let (cell, transform) =
        globe_camera_grid_transform(state, celestial_space, DVec3::ZERO, body_rotation);

    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&cell, &transform),
        DVec3::X * 10.0,
    );
}

#[test]
fn scene_up_globe_camera_keeps_orbit_in_scene_frame() {
    let celestial_space = CelestialSpace::default();
    let state = Globe3dState {
        camera_up_mode: GlobeCameraUpMode::SceneUp,
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        camera_distance: 10.0,
        ..Globe3dState::default()
    };
    let body_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);

    let (cell, transform) =
        globe_camera_grid_transform(state, celestial_space, DVec3::ZERO, body_rotation);

    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&cell, &transform),
        DVec3::Z * 10.0,
    );
}

#[test]
fn globe_camera_preferred_up_uses_focused_body_north() {
    let body_rotation = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let preferred_up = globe_camera_preferred_up(Globe3dState::default(), body_rotation);

    assert_vec3_close(preferred_up, Vec3::X);
}

#[test]
fn globe_camera_preferred_up_can_use_scene_up() {
    let state = Globe3dState {
        camera_up_mode: GlobeCameraUpMode::SceneUp,
        ..Globe3dState::default()
    };
    let body_rotation = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let preferred_up = globe_camera_preferred_up(state, body_rotation);

    assert_vec3_close(preferred_up, Vec3::Y);
}

#[test]
fn projected_camera_up_removes_forward_component() {
    let projected_up = projected_camera_up(Vec3::NEG_Z, Vec3::new(1.0, 0.0, 1.0), Vec3::X);

    assert!(projected_up.dot(Vec3::NEG_Z).abs() <= 1.0e-6);
    assert_vec3_close(projected_up, Vec3::X);
}

#[test]
fn projected_camera_up_falls_back_near_the_body_poles() {
    let projected_up = projected_camera_up(Vec3::NEG_Y, Vec3::Y, Vec3::X);

    assert!(projected_up.dot(Vec3::NEG_Y).abs() <= 1.0e-6);
    assert_vec3_close(projected_up, Vec3::X);
}

#[test]
fn projected_camera_up_uses_body_local_fallback_near_rotated_poles() {
    let body_rotation = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let preferred_up = body_rotation.mul_vec3(Vec3::Y);
    let fallback_up = body_rotation.mul_vec3(Vec3::X);
    let projected_up = projected_camera_up(-preferred_up, preferred_up, fallback_up);

    assert!(projected_up.dot(-preferred_up).abs() <= 1.0e-6);
    assert_vec3_close(projected_up, Vec3::NEG_Y);
}

#[test]
fn sync_globe_camera_uses_focused_body_north_as_camera_up() {
    let celestial_space = CelestialSpace::default();
    let earth = CelestialBody::earth();
    let body_rotation = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
    let state = Globe3dState {
        yaw_deg: 0.0,
        pitch_deg: 0.0,
        ..Globe3dState::for_body(&earth, RenderScale::PHYSICAL)
    };
    let mut app = App::new();
    app.insert_resource(celestial_space)
        .insert_resource(CelestialFocus::earth())
        .insert_resource(ActiveTileSource::from(TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png",
        )))
        .init_resource::<MetricSceneFocusPivot>()
        .add_systems(Update, sync_globe_camera);
    app.world_mut().spawn(Globe3dViewBundle {
        globe_state: state,
        ..Globe3dViewBundle::default()
    });
    app.world_mut().spawn((
        CelestialBodyPlacement::for_body_relative_to(earth, BodyId::EARTH),
        Transform::from_rotation(body_rotation),
    ));
    let camera = app
        .world_mut()
        .spawn((Globe3dCamera, Camera3d::default(), Transform::default()))
        .id();

    app.update();

    let world = app.world();
    let Some(actual_transform) = world.get::<Transform>(camera) else {
        return;
    };
    let actual_up = actual_transform.rotation.mul_vec3(Vec3::Y).normalize();

    assert_vec3_close(actual_up, Vec3::X);
}

#[test]
fn body_north_input_converts_world_surface_hits_to_body_frame() {
    let body_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let surface_direction =
        camera_frame_surface_direction(Globe3dState::default(), body_rotation, DVec3::X);

    assert_dvec3_close(surface_direction, DVec3::Z);
}

#[test]
fn body_north_input_uses_identity_for_invalid_body_rotation() {
    let surface_direction = camera_frame_surface_direction(
        Globe3dState::default(),
        Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
        DVec3::X,
    );

    assert_dvec3_close(surface_direction, DVec3::X);
}

#[test]
fn scene_up_input_keeps_world_surface_hits_in_scene_frame() {
    let state = Globe3dState {
        camera_up_mode: GlobeCameraUpMode::SceneUp,
        ..Globe3dState::default()
    };
    let body_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let surface_direction = camera_frame_surface_direction(state, body_rotation, DVec3::X);

    assert_dvec3_close(surface_direction, DVec3::X);
}

#[test]
fn sync_globe_camera_uses_matching_metric_focus_pivot() {
    let celestial_space = CelestialSpace::default();
    let moon = CelestialBody::moon();
    let legacy_moon_center = DVec3::new(384_400.0, 0.0, 0.0);
    let shared_pivot = DVec3::new(12_000.0, 30.0, -40.0);
    let moon_placement = celestial_space.place_position_units(legacy_moon_center);
    let state = Globe3dState {
        camera_distance: 6.0,
        base_camera_distance: 6.0,
        ..Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL)
    };
    let mut app = App::new();
    app.insert_resource(celestial_space)
        .insert_resource(CelestialFocus::moon())
        .insert_resource(MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::Object(BodyId::MOON),
            shared_pivot,
            Some(2.0),
        ))
        .insert_resource(ActiveTileSource::from(TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png",
        )))
        .add_systems(Update, sync_globe_camera);
    app.world_mut().spawn(Globe3dViewBundle {
        globe_state: state,
        ..Globe3dViewBundle::default()
    });
    app.world_mut().spawn((
        CelestialBodyPlacement::for_body_relative_to(moon, BodyId::EARTH),
        moon_placement.cell,
        Transform::from_translation(moon_placement.translation),
    ));
    let camera = app
        .world_mut()
        .spawn((
            Globe3dCamera,
            Camera3d::default(),
            CellCoord::default(),
            Transform::default(),
        ))
        .id();

    app.update();

    let world = app.world();
    let Some(actual_cell) = world.get::<CellCoord>(camera).copied() else {
        return;
    };
    let Some(actual_transform) = world.get::<Transform>(camera) else {
        return;
    };

    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&actual_cell, actual_transform),
        shared_pivot + globe_camera_position(state),
    );
}

#[test]
fn sync_globe_camera_ignores_metric_body_surface_pivot_as_body_center() {
    let celestial_space = CelestialSpace::default();
    let earth = CelestialBody::earth();
    let state = Globe3dState::for_body(&earth, RenderScale::PHYSICAL);
    let surface =
        MetricSceneSurfaceTarget::from_body(&earth, LonLat::new(90.0, 0.0), Kilometers::new(0.0));
    let mut app = App::new();
    app.insert_resource(celestial_space)
        .insert_resource(CelestialFocus::earth())
        .insert_resource(MetricSceneFocusPivot::resolved(
            MetricSceneId::PRIMARY,
            MetricSceneFocusTarget::BodySurface(surface),
            DVec3::new(10_000.0, 0.0, 0.0),
            Some(state.radius),
        ))
        .insert_resource(ActiveTileSource::from(TileSource::xyz(
            "https://tiles.example/{z}/{x}/{y}.png",
        )))
        .add_systems(Update, sync_globe_camera);
    let globe_view = app
        .world_mut()
        .spawn(Globe3dViewBundle {
            globe_state: state,
            ..Globe3dViewBundle::default()
        })
        .id();
    app.world_mut().spawn((
        CelestialBodyPlacement::for_body_relative_to(earth, BodyId::EARTH),
        CellCoord::default(),
        Transform::default(),
    ));
    let camera = app
        .world_mut()
        .spawn((
            Globe3dCamera,
            Camera3d::default(),
            CellCoord::default(),
            Transform::default(),
        ))
        .id();

    app.update();

    let world = app.world();
    let Some(actual_cell) = world.get::<CellCoord>(camera).copied() else {
        return;
    };
    let Some(actual_transform) = world.get::<Transform>(camera) else {
        return;
    };
    let Some(actual_state) = world.get::<Globe3dState>(globe_view).copied() else {
        return;
    };

    assert_dvec3_close(
        celestial_space
            .grid()
            .grid_position_double(&actual_cell, actual_transform),
        globe_camera_position(actual_state),
    );
}

#[test]
fn ray_sphere_intersection_hits_the_visible_surface() {
    let ray = Ray3d::new(Vec3::new(0.0, 0.0, 10.0), Dir3::NEG_Z);
    let intersection = ray_sphere_intersection(ray, DVec3::ZERO, 4.0);

    assert!(intersection.is_some());
    let Some(intersection) = intersection else {
        return;
    };

    assert!((intersection.z - 4.0).abs() < 1.0e-9);
}

#[test]
fn ray_sphere_intersection_handles_offset_big_space_centers() {
    let center = DVec3::new(0.0, 0.0, -20_000.0);
    let ray = Ray3d::new(Vec3::new(0.0, 0.0, -4_000.0), Dir3::NEG_Z);
    let intersection = ray_sphere_intersection(ray, center, 6_371.0);

    assert!(intersection.is_some());
    let Some(intersection) = intersection else {
        return;
    };

    assert!((intersection.z - (center.z + 6_371.0)).abs() < 1.0e-9);
}

#[test]
fn ray_sphere_intersection_reconstructs_big_space_camera_origin() {
    let celestial_space = CelestialSpace::default();
    let absolute_origin = DVec3::new(0.0, 0.0, 15_927.5);
    let placement = celestial_space.place_position_units(absolute_origin);
    let ray_origin = ray_origin_units(
        celestial_space,
        Some(&placement.cell),
        placement.translation,
    );
    let intersection =
        ray_sphere_intersection_from_origin(ray_origin, DVec3::NEG_Z, DVec3::ZERO, 6_371.0);

    assert_dvec3_close(ray_origin, absolute_origin);
    assert!(intersection.is_some());
    let Some(intersection) = intersection else {
        return;
    };

    assert!((intersection.z - 6_371.0).abs() < 1.0e-9);
}

#[test]
fn orbit_angles_match_cardinal_surface_directions() {
    let (yaw_deg, pitch_deg) = orbit_angles_for_surface_direction(DVec3::new(0.0, 0.0, 1.0));
    assert!(yaw_deg.abs() < 1.0e-9);
    assert!(pitch_deg.abs() < 1.0e-9);

    let (yaw_deg, pitch_deg) = orbit_angles_for_surface_direction(DVec3::new(1.0, 0.0, 0.0));
    assert!((yaw_deg - 90.0).abs() < 1.0e-9);
    assert!(pitch_deg.abs() < 1.0e-9);
}

#[test]
fn globe_controls_slow_down_near_the_surface() {
    let far = Globe3dState {
        camera_distance: 10.0,
        base_camera_distance: 10.0,
        ..Globe3dState::default()
    };
    let near = Globe3dState {
        camera_distance: 5.0,
        base_camera_distance: 10.0,
        ..Globe3dState::default()
    };
    let very_near = Globe3dState {
        camera_distance: 4.01,
        base_camera_distance: 10.0,
        ..Globe3dState::default()
    };

    assert!((distance_control_scale(far, far.radius) - 1.0).abs() < f64::EPSILON);
    assert!(distance_control_scale(near, near.radius) < distance_control_scale(far, far.radius));
    assert!(
        distance_control_scale(very_near, very_near.radius)
            < distance_control_scale(near, near.radius)
    );
    assert!(distance_control_scale(very_near, very_near.radius) > 0.0);
}

#[test]
fn globe_state_can_be_derived_from_physical_body_metadata() {
    let state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);

    assert!((state.radius - 6_371.008).abs() < 0.001);
    assert!((state.camera_distance - state.radius * 2.5).abs() < 0.001);
    assert!((state.base_camera_distance - state.camera_distance).abs() < f32::EPSILON);
}

#[test]
fn invalid_body_render_radius_falls_back_to_demo_radius() {
    assert!(
        (globe_radius_from_render_units(0.0) - Globe3dState::default().radius).abs() < f32::EPSILON
    );
    assert!(
        (globe_radius_from_render_units(f64::INFINITY) - Globe3dState::default().radius).abs()
            < f32::EPSILON
    );
}

#[test]
fn wheel_input_is_capped_per_frame() {
    assert!((clamp_scroll_delta(99.0) - MAX_SCROLL_DELTA_PER_FRAME).abs() < f64::EPSILON);
    assert!((clamp_scroll_delta(-99.0) + MAX_SCROLL_DELTA_PER_FRAME).abs() < f64::EPSILON);
    assert!((clamp_scroll_delta(0.1) - 0.1).abs() < f64::EPSILON);
}

#[test]
fn wheel_zoom_delta_does_not_slow_down_near_the_surface() {
    assert!((wheel_zoom_delta(0.08) - 0.08).abs() < f64::EPSILON);
    assert!((wheel_zoom_delta(-0.08) + 0.08).abs() < f64::EPSILON);
}

#[test]
fn globe_orbit_angles_clamp_pitch_and_wrap_yaw() {
    let (yaw, pitch) = globe_orbit_angles(179.0, 79.0, Vec2::new(-20.0, 20.0), 0.2);

    assert!((yaw + 177.0).abs() < 1.0e-9);
    assert!((pitch - 80.0).abs() < 1.0e-9);
}

#[test]
fn surface_pan_sensitivity_tracks_camera_surface_height() {
    let far = Globe3dState {
        radius: 4.0,
        camera_distance: 10.0,
        ..Globe3dState::default()
    };
    let near = Globe3dState {
        radius: 4.0,
        camera_distance: 5.0,
        ..Globe3dState::default()
    };

    let far_pan = globe_surface_pan_degrees_per_pixel(far, far.radius, 720.0, 45_f64.to_radians());
    let near_pan =
        globe_surface_pan_degrees_per_pixel(near, near.radius, 720.0, 45_f64.to_radians());

    assert!(far_pan > near_pan);
    assert!(near_pan > 0.0);
}

#[test]
fn surface_grab_pan_rotates_camera_with_surface_drag() {
    let previous_surface = DVec3::Z;
    let current_surface = DVec3::X;
    let camera_direction = DVec3::Z;

    let next_direction =
        camera_direction_for_surface_grab_pan(previous_surface, current_surface, camera_direction);

    assert!(next_direction.is_some());
    let Some(next_direction) = next_direction else {
        return;
    };
    assert!(next_direction.x < -0.999);
    assert!(next_direction.y.abs() < 1.0e-9);
    assert!(next_direction.z.abs() < 1.0e-9);
}

#[test]
fn pinch_zoom_delta_zooms_in_when_touch_distance_grows() {
    assert!(pinch_zoom_delta(140.0, 100.0) > 0.0);
    assert!(pinch_zoom_delta(100.0, 140.0) < 0.0);
    assert!(pinch_zoom_delta(4.0, 100.0).abs() < f64::EPSILON);
}

#[test]
fn source_resolution_camera_floor_tracks_tile_source_zoom() {
    let state = Globe3dState::default();
    let low_detail_source =
        TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 6);
    let high_detail_source =
        TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 19);

    let low_detail_floor =
        source_resolution_surface_distance(state, &low_detail_source, 720.0, 45_f64.to_radians());
    let high_detail_floor =
        source_resolution_surface_distance(state, &high_detail_source, 720.0, 45_f64.to_radians());

    assert!(high_detail_floor < low_detail_floor / 1_000.0);
    assert!(high_detail_floor > 0.0);
}

#[test]
fn source_resolution_camera_floor_tracks_raster_tile_density() {
    let state = Globe3dState::default();
    let standard_density_source =
        TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 12);
    let high_density_source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}@2x.png")
        .with_zoom_range(0, 12)
        .with_raster_tile_size(512);

    let standard_density_floor = source_resolution_surface_distance(
        state,
        &standard_density_source,
        720.0,
        45_f64.to_radians(),
    );
    let high_density_floor =
        source_resolution_surface_distance(state, &high_density_source, 720.0, 45_f64.to_radians());

    assert!(high_density_floor < standard_density_floor);
}

#[test]
fn camera_distance_clamps_to_the_source_resolution_floor() {
    let mut state = Globe3dState {
        camera_distance: 4.01,
        ..Globe3dState::default()
    };
    let low_detail_source =
        TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 6);
    let min_camera_distance = min_camera_distance_for_source(state, &low_detail_source, None, None);

    state.clamp_camera_distance_for_radius(state.radius, min_camera_distance);

    assert!(state.camera_distance >= min_camera_distance);
}

#[test]
fn projection_near_clip_plane_tracks_dynamic_near_distance() {
    let state = Globe3dState {
        camera_distance: 4.01,
        ..Globe3dState::default()
    };
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 19);
    let mut projection = Projection::Perspective(PerspectiveProjection::default());

    sync_globe_projection(
        &mut projection,
        state,
        earth_focus_target(state),
        &source,
        None,
    );

    let Projection::Perspective(perspective) = projection else {
        return;
    };
    assert!(perspective.near < 0.1);
    assert!(perspective.near < state.camera_distance - state.radius);
    assert!((perspective.near_clip_plane.w + perspective.near).abs() < f32::EPSILON);
}

#[test]
fn projection_far_clip_plane_expands_for_physical_globe_scale() {
    let state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 8);
    let mut projection = Projection::Perspective(PerspectiveProjection::default());

    sync_globe_projection(
        &mut projection,
        state,
        earth_focus_target(state),
        &source,
        None,
    );

    let Projection::Perspective(perspective) = projection else {
        return;
    };
    assert!(perspective.far >= min_globe_far_plane(state, state.radius));
    assert!(perspective.near > 0.1);
    assert!(
        perspective.near
            < state.camera_distance - state.radius - max_default_surface_visual_lift(state.radius)
    );
}

#[test]
fn projection_far_clip_plane_tracks_current_focus_radius() {
    let earth_state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);
    let moon_radius = globe_radius_from_render_units(
        CelestialBody::moon().mean_radius_units(RenderScale::PHYSICAL),
    );
    let moon_state = Globe3dState {
        radius: moon_radius,
        camera_distance: moon_radius * 3.0,
        base_camera_distance: moon_radius * 3.0,
        ..earth_state
    };
    let source = TileSource::xyz("https://tiles.example/{z}/{x}/{y}.png").with_zoom_range(0, 8);
    let mut projection = Projection::Perspective(PerspectiveProjection {
        far: 1_000_000.0,
        ..PerspectiveProjection::default()
    });

    sync_globe_projection(
        &mut projection,
        earth_state,
        earth_focus_target(earth_state),
        &source,
        None,
    );
    let Projection::Perspective(perspective) = &projection else {
        return;
    };
    let earth_far = perspective.far;

    sync_globe_projection(
        &mut projection,
        moon_state,
        GlobeFocusTarget {
            body: BodyId::MOON,
            center_units: DVec3::ZERO,
            radius: moon_radius,
            body_rotation: Quat::IDENTITY,
        },
        &source,
        None,
    );
    let Projection::Perspective(perspective) = projection else {
        return;
    };

    assert!(perspective.far < earth_far);
    assert!((perspective.far - min_globe_far_plane(moon_state, moon_radius)).abs() < 0.001);
}

#[test]
fn far_clip_plane_reaches_the_configured_outward_orbit_limit() {
    let state = Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL);

    assert!(
        min_globe_far_plane(state, state.radius) >= max_camera_distance_for_radius(state.radius)
    );
}

#[test]
fn wheel_zoom_scales_surface_height_instead_of_center_distance() {
    let mut state = Globe3dState {
        radius: 4.0,
        camera_distance: 5.0,
        ..Globe3dState::default()
    };
    let min_camera_distance = 4.0001;
    let radius = state.radius;

    zoom_globe_camera(&mut state, 0.25, radius, min_camera_distance);

    let expected_surface_height = (-0.25 * ZOOM_EXP_FACTOR).exp();
    let actual_surface_height = f64::from(state.camera_distance - state.radius);
    assert!((actual_surface_height - expected_surface_height).abs() < 1.0e-6);
}

#[test]
fn wheel_zoom_remains_responsive_near_the_surface() {
    let mut state = Globe3dState {
        radius: 4.0,
        camera_distance: 4.01,
        ..Globe3dState::default()
    };
    let min_camera_distance = 4.0001;
    let radius = state.radius;

    zoom_globe_camera(&mut state, 0.08, radius, min_camera_distance);

    let actual_surface_height = f64::from(state.camera_distance - state.radius);
    assert!(actual_surface_height < 0.01 * 0.95);
    assert!(actual_surface_height > f64::from(min_camera_distance - state.radius));
}
