use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use ferrisium_bevy::prelude::{FerrisiumInputCapture, FerrisiumSet};

use crate::browser_params::{browser_query_param, parse_bool_flag};

#[derive(Resource, Debug)]
struct DemoEguiOverlay {
    capture_ferrisium_input: bool,
    brightness: f32,
    click_count: u32,
}

impl Default for DemoEguiOverlay {
    fn default() -> Self {
        Self {
            capture_ferrisium_input: true,
            brightness: 0.42,
            click_count: 0,
        }
    }
}

pub(crate) fn demo_egui_overlay_enabled() -> bool {
    browser_query_param("egui").is_some_and(|value| parse_bool_flag(&value))
}

pub(crate) fn configure_demo_egui_overlay(app: &mut App) {
    app.add_plugins(EguiPlugin::default())
        .init_resource::<DemoEguiOverlay>()
        .add_systems(
            Update,
            sync_demo_egui_pointer_capture.in_set(FerrisiumSet::CaptureInput),
        )
        .add_systems(EguiPrimaryContextPass, draw_demo_egui_overlay);
}

fn sync_demo_egui_pointer_capture(
    overlay: Res<'_, DemoEguiOverlay>,
    egui_wants_input: Res<'_, EguiWantsInput>,
    mut capture: ResMut<'_, FerrisiumInputCapture>,
) {
    if overlay.capture_ferrisium_input && egui_wants_input.wants_any_pointer_input() {
        capture.capture_pointer();
    }
}

fn draw_demo_egui_overlay(
    mut contexts: EguiContexts<'_, '_>,
    mut overlay: ResMut<'_, DemoEguiOverlay>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Ferrisium egui")
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .default_width(260.0)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.checkbox(&mut overlay.capture_ferrisium_input, "Capture input");
            ui.add(egui::Slider::new(&mut overlay.brightness, 0.0..=1.0).text("Level"));
            if ui.button("Button").clicked() {
                overlay.click_count = overlay.click_count.saturating_add(1);
            }
            ui.label(format!("Clicks: {}", overlay.click_count));
        });
}
