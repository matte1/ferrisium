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
        .insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.1)))
        .insert_resource(ActiveTileSource::from(TileSource::nasa_gibs_blue_marble()))
        .insert_resource(FerrisiumViewMode::Map2d)
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
        .add_systems(Startup, setup_map)
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), &'static str> {
    Err("minimal_map is browser-only; use Trunk to run the wasm build.")
}

#[cfg(target_arch = "wasm32")]
fn setup_map(mut commands: Commands<'_, '_>) {
    commands.spawn(Map2dViewBundle {
        map_state: Map2dState {
            center: LonLat::new(-98.5, 39.5),
            zoom: 3.0,
        },
        ..default()
    });
}

#[cfg(target_arch = "wasm32")]
fn primary_window() -> Window {
    Window {
        title: "Ferrisium Minimal Map".into(),
        resolution: (1280, 720).into(),
        resizable: true,
        canvas: Some("#bevy".into()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}
