#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use bevy::asset::{AssetMetaCheck, AssetPlugin};
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::*;

#[cfg(target_arch = "wasm32")]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.06)))
        .insert_resource(ActiveTileSource::from(TileSource::nasa_gibs_blue_marble()))
        .insert_resource(FerrisiumViewMode::Globe3d)
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.68, 0.76, 0.95),
            brightness: 320.0,
            affects_lightmapped_meshes: true,
        })
        .add_plugins(
            DefaultPlugins
                .build()
                // Ferrisium installs big_space's transform propagation.
                .disable::<TransformPlugin>()
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(primary_window()),
                    ..default()
                }),
        )
        .add_plugins(FerrisiumPlugin)
        .add_systems(Startup, setup_globe)
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), &'static str> {
    Err("minimal_globe is browser-only; use Trunk to run the wasm build.")
}

#[cfg(target_arch = "wasm32")]
fn setup_globe(mut commands: Commands<'_, '_>) {
    commands.spawn(Globe3dViewBundle {
        globe_state: Globe3dState {
            yaw_deg: -20.0,
            pitch_deg: 24.0,
            tile_zoom: 4,
            max_selected_tiles: 768,
            ..Globe3dState::for_body(&CelestialBody::earth(), RenderScale::PHYSICAL)
        },
        ..default()
    });

    commands.spawn((
        Name::new("Sun Light"),
        DirectionalLight {
            illuminance: 32_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_x(-0.65) * Quat::from_rotation_y(-0.45)),
    ));
}

#[cfg(target_arch = "wasm32")]
fn primary_window() -> Window {
    Window {
        title: "Ferrisium Minimal Globe".into(),
        resolution: (1280, 720).into(),
        resizable: true,
        canvas: Some("#bevy".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}
