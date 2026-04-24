#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use bevy::prelude::{default, App, Commands, IntoScheduleConfigs, Res, Startup, Update};
#[cfg(target_arch = "wasm32")]
use ferrisium_bevy::prelude::{ActiveTileSource, FerrisiumSet, Map2dState, Map2dViewBundle};
#[cfg(target_arch = "wasm32")]
use ferrisium_core::prelude::{LonLat, PlanetaryBody};

#[cfg(target_arch = "wasm32")]
use crate::custom_geo_draw;
#[cfg(target_arch = "wasm32")]
use crate::h3_demo;

#[cfg(target_arch = "wasm32")]
pub(crate) fn configure_map_mode(app: &mut App) {
    app.init_resource::<custom_geo_draw::DemoCustomMapLayer>()
        .add_systems(Startup, (setup_map, h3_demo::spawn_usa_h3_overlay).chain())
        .add_systems(
            Update,
            (
                h3_demo::log_h3_cell_clicks,
                custom_geo_draw::sync_demo_custom_map_layer
                    .in_set(FerrisiumSet::ProjectGeoOverlays),
            ),
        );
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn setup_map(mut commands: Commands<'_, '_>, active_source: Res<'_, ActiveTileSource>) {
    let map_state = if active_source.0.body == PlanetaryBody::earth() {
        Map2dState {
            center: LonLat::new(-98.5, 39.5),
            zoom: 3.25,
        }
    } else {
        Map2dState {
            center: LonLat::default(),
            zoom: 2.0,
        }
    };

    commands.spawn(Map2dViewBundle {
        map_state,
        ..default()
    });
}
