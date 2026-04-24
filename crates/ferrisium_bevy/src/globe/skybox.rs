//! Optional skybox support for Ferrisium 3D scene views.
//!
//! The public [`GlobeSkybox`] resource keeps skybox configuration small and
//! Bevy-native while the systems here handle the loaded-image conversion that
//! stacked PNG cubemaps need before Bevy's [`Skybox`] component can sample them.
//! Apps that want first-frame interaction before large skybox assets start
//! loading can insert [`DeferredGlobeSkybox`] instead.
//! Ferrisium's demo asset is generated so [`Quat::IDENTITY`] skybox rotation
//! corresponds to the inertial J2000 sky frame.

use std::fmt;

use bevy::asset::LoadState;
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::Skybox;
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};

use crate::globe::Globe3dCamera;
use crate::metric_scene::MetricSceneCamera;

// Bevy skyboxes are not lit by scene lights, so the default is intentionally bright.
const DEFAULT_GLOBE_SKYBOX_BRIGHTNESS: f32 = 1_000.0;
/// Default number of idle update frames before starting a deferred skybox load.
pub const DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES: u32 = 30;
/// Default number of idle frames after decode before the skybox is uploaded.
pub const DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES: u32 = 90;
/// Default app asset-folder path for Ferrisium-provided skybox files.
pub const FERRISIUM_SKYBOX_ASSET_ROOT: &str = "ferrisium/skyboxes";
/// Default Milky Way cubemap resolution for apps that want a balanced bundled asset.
pub const DEFAULT_MILKY_WAY_SKYBOX_RESOLUTION: MilkyWaySkyboxResolution =
    MilkyWaySkyboxResolution::Face1024;
// A cubemap has exactly six square faces in Bevy's stacked PNG layout.
const STACKED_CUBEMAP_FACE_COUNT: u32 = 6;

/// Face resolution for Ferrisium's bundled Milky Way stacked-cubemap skybox.
///
/// The asset path assumes the repo's `assets/ferrisium` directory has been
/// copied into the consuming app's Bevy asset folder. Each PNG is stacked in
/// Bevy cubemap order with dimensions `face_size x (face_size * 6)`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MilkyWaySkyboxResolution {
    /// 512px faces. Useful for quick loads and low-memory browser targets.
    Face512,
    /// 1024px faces. Balanced default for most browser demos.
    #[default]
    Face1024,
    /// 2048px faces. Sharper background with a larger decode/upload cost.
    Face2048,
    /// 4096px faces. Full-resolution bundled source.
    Face4096,
}

impl MilkyWaySkyboxResolution {
    /// Returns the square face size in pixels.
    #[must_use]
    pub const fn face_size(self) -> u32 {
        match self {
            Self::Face512 => 512,
            Self::Face1024 => 1_024,
            Self::Face2048 => 2_048,
            Self::Face4096 => 4_096,
        }
    }

    /// Returns the vertically stacked PNG dimensions in pixels.
    #[must_use]
    pub const fn stacked_dimensions(self) -> (u32, u32) {
        let face_size = self.face_size();
        (face_size, face_size * STACKED_CUBEMAP_FACE_COUNT)
    }

    /// Returns the file name for this bundled Milky Way skybox variant.
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Face512 => "milkyway_512.png",
            Self::Face1024 => "milkyway_1024.png",
            Self::Face2048 => "milkyway_2048.png",
            Self::Face4096 => "milkyway_4096.png",
        }
    }

    /// Returns the Bevy [`AssetServer`] path for the default asset-folder layout.
    #[must_use]
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::Face512 => "ferrisium/skyboxes/milkyway_512.png",
            Self::Face1024 => "ferrisium/skyboxes/milkyway_1024.png",
            Self::Face2048 => "ferrisium/skyboxes/milkyway_2048.png",
            Self::Face4096 => "ferrisium/skyboxes/milkyway_4096.png",
        }
    }

    /// Returns approximate decoded RGBA texture memory in bytes.
    #[must_use]
    pub const fn decoded_rgba_bytes(self) -> u64 {
        let face_size = self.face_size() as u64;
        face_size * face_size * (STACKED_CUBEMAP_FACE_COUNT as u64) * 4
    }
}

/// Opt-in skybox configuration for Ferrisium 3D scene cameras.
///
/// Insert this resource before or after [`FerrisiumPlugin`](crate::FerrisiumPlugin)
/// to load a stacked cubemap image and attach it to globe and metric scene cameras.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct GlobeSkybox {
    /// Asset path loaded through Bevy's [`AssetServer`].
    pub image_path: String,
    /// Skybox sample brightness.
    pub brightness: f32,
    /// View-space cubemap rotation.
    pub rotation: Quat,
}

impl GlobeSkybox {
    /// Creates a skybox from Ferrisium's bundled Milky Way cubemap.
    ///
    /// The consuming app must make the repo's `assets/ferrisium` directory
    /// available under its Bevy asset folder. For Trunk apps, this can be done
    /// with a `copy-dir` entry in `index.html`.
    #[must_use]
    pub fn milky_way(resolution: MilkyWaySkyboxResolution) -> Self {
        Self::stacked_png(resolution.asset_path())
    }

    /// Creates a skybox from a vertically stacked PNG cubemap.
    ///
    /// The image must be one square face wide and six square faces tall, in
    /// Bevy's cubemap layer order.
    #[must_use]
    pub fn stacked_png(image_path: impl Into<String>) -> Self {
        Self {
            image_path: image_path.into(),
            brightness: DEFAULT_GLOBE_SKYBOX_BRIGHTNESS,
            rotation: Quat::IDENTITY,
        }
    }

    /// Sets the skybox brightness.
    #[must_use]
    pub const fn with_brightness(mut self, brightness: f32) -> Self {
        self.brightness = brightness;
        self
    }

    /// Sets the skybox rotation.
    #[must_use]
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// Defers loading this skybox for [`DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES`] idle frames.
    ///
    /// Insert the returned resource instead of inserting [`GlobeSkybox`]
    /// directly when a large skybox asset should begin loading after the first
    /// interactive frames have rendered.
    #[must_use]
    pub fn deferred(self) -> DeferredGlobeSkybox {
        DeferredGlobeSkybox::new(self)
    }

    /// Defers loading this skybox for a caller-selected number of idle update frames.
    #[must_use]
    pub fn deferred_after_frames(self, frames: u32) -> DeferredGlobeSkybox {
        DeferredGlobeSkybox::after_frames(self, frames)
    }
}

/// Skybox resource that starts loading after a short idle-frame delay.
///
/// This is a convenience wrapper for apps that want the globe or metric scene
/// to become interactive before a large background cubemap fetch/decode begins.
/// [`FerrisiumPlugin`](crate::FerrisiumPlugin) converts this resource into a
/// normal [`GlobeSkybox`] once the countdown completes. Mouse, wheel, or touch
/// activity resets the countdown so the expensive fetch/decode path starts
/// after a contiguous idle window.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DeferredGlobeSkybox {
    skybox: GlobeSkybox,
    idle_frame_delay: u32,
    idle_frames_remaining: u32,
}

impl DeferredGlobeSkybox {
    /// Creates a deferred skybox using [`DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES`].
    #[must_use]
    pub fn new(skybox: GlobeSkybox) -> Self {
        Self::after_frames(skybox, DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES)
    }

    /// Creates a deferred skybox with a caller-selected idle update-frame delay.
    #[must_use]
    pub fn after_frames(skybox: GlobeSkybox, frames: u32) -> Self {
        Self {
            skybox,
            idle_frame_delay: frames,
            idle_frames_remaining: frames,
        }
    }

    /// Skybox configuration that will be inserted once the delay completes.
    #[must_use]
    pub const fn skybox(&self) -> &GlobeSkybox {
        &self.skybox
    }

    /// Remaining idle update frames before this deferred skybox begins loading.
    #[must_use]
    pub const fn frames_remaining(&self) -> u32 {
        self.idle_frames_remaining
    }

    fn tick_until_ready(&mut self, interaction_active: bool) -> bool {
        if self.idle_frame_delay == 0 {
            return true;
        }
        if interaction_active {
            self.idle_frames_remaining = self.idle_frame_delay;
            return false;
        }
        if self.idle_frames_remaining == 0 {
            return true;
        }

        self.idle_frames_remaining -= 1;

        self.idle_frames_remaining == 0
    }
}

/// Upload policy for decoded globe skybox images.
///
/// Loading a large full-resolution skybox has two expensive phases: CPU decode
/// and GPU upload. Ferrisium loads the image into the main world first, then
/// waits for this many idle update frames before making the image
/// render-eligible. Mouse, wheel, or touch activity resets the countdown so the
/// unavoidable upload stall is less likely to interrupt active navigation.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobeSkyboxUploadSettings {
    /// Required idle frames after image decode before GPU upload starts.
    pub idle_frames_after_decode: u32,
}

impl Default for GlobeSkyboxUploadSettings {
    fn default() -> Self {
        Self {
            idle_frames_after_decode: DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct GlobeSkyboxState {
    config: Option<GlobeSkybox>,
    image_handle: Option<Handle<Image>>,
    upload_idle_frames_remaining: u32,
    status: GlobeSkyboxStatus,
}

impl GlobeSkyboxState {
    fn mark_decoded(&mut self, settings: GlobeSkyboxUploadSettings) {
        self.upload_idle_frames_remaining = settings.idle_frames_after_decode;
        self.status = GlobeSkyboxStatus::Decoded;
    }

    fn decoded_upload_ready(
        &mut self,
        settings: GlobeSkyboxUploadSettings,
        interaction_active: bool,
    ) -> bool {
        if self.status != GlobeSkyboxStatus::Decoded {
            return false;
        }
        if interaction_active {
            self.upload_idle_frames_remaining = settings.idle_frames_after_decode;
            return false;
        }
        if self.upload_idle_frames_remaining > 0 {
            self.upload_idle_frames_remaining -= 1;
            return false;
        }

        true
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum GlobeSkyboxStatus {
    #[default]
    Inactive,
    Loading,
    Decoded,
    Ready,
    Failed,
}

type GlobeSkyboxCameraQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, Option<&'static Skybox>),
    (
        With<Camera3d>,
        Or<(With<Globe3dCamera>, With<MetricSceneCamera>)>,
    ),
>;

/// Converts a deferred skybox request into the normal skybox resource once ready.
pub(crate) fn queue_deferred_globe_skybox(
    mut commands: Commands<'_, '_>,
    deferred: Option<ResMut<'_, DeferredGlobeSkybox>>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    touches: Res<'_, Touches>,
    mut mouse_motion_events: MessageReader<'_, '_, MouseMotion>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
) {
    let Some(mut deferred) = deferred else {
        return;
    };
    let interaction_active = skybox_interaction_active(
        &buttons,
        &touches,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
    );
    if !deferred.tick_until_ready(interaction_active) {
        return;
    }

    commands.insert_resource(deferred.skybox.clone());
    commands.remove_resource::<DeferredGlobeSkybox>();
}

/// Synchronizes the optional globe skybox resource with active Bevy 3D scene cameras.
#[allow(
    clippy::too_many_arguments,
    reason = "Skybox synchronization coordinates asset loading, upload gating, input idleness, and active camera attachment."
)]
pub(crate) fn sync_globe_skybox(
    mut commands: Commands<'_, '_>,
    config: Option<Res<'_, GlobeSkybox>>,
    upload_settings: Res<'_, GlobeSkyboxUploadSettings>,
    asset_server: Res<'_, AssetServer>,
    mut images: ResMut<'_, Assets<Image>>,
    mut state: ResMut<'_, GlobeSkyboxState>,
    buttons: Res<'_, ButtonInput<MouseButton>>,
    touches: Res<'_, Touches>,
    mut mouse_motion_events: MessageReader<'_, '_, MouseMotion>,
    mut mouse_wheel_events: MessageReader<'_, '_, MouseWheel>,
    cameras: GlobeSkyboxCameraQuery<'_, '_>,
) {
    let interaction_active = skybox_interaction_active(
        &buttons,
        &touches,
        &mut mouse_motion_events,
        &mut mouse_wheel_events,
    );
    let Some(config) = config else {
        if state.status != GlobeSkyboxStatus::Inactive {
            for (entity, _) in &cameras {
                commands.entity(entity).remove::<Skybox>();
            }
            *state = GlobeSkyboxState::default();
        }
        return;
    };

    if state.config.as_ref() != Some(config.as_ref()) {
        state.config = Some(config.clone());
        state.image_handle = Some(load_stacked_cubemap(&asset_server, &config.image_path));
        state.upload_idle_frames_remaining = 0;
        state.status = GlobeSkyboxStatus::Loading;
    }

    if state.status == GlobeSkyboxStatus::Loading {
        let Some(image_handle) = state.image_handle.as_ref() else {
            return;
        };
        match asset_server.load_state(image_handle) {
            LoadState::Loaded => {}
            LoadState::Failed(error) => {
                warn!("failed to load globe skybox {}: {error}", config.image_path);
                state.status = GlobeSkyboxStatus::Failed;
                return;
            }
            LoadState::NotLoaded | LoadState::Loading => return,
        }

        let Some(image) = images.get_mut(image_handle) else {
            return;
        };
        if let Err(error) = prepare_stacked_cubemap_image(image) {
            warn!(
                "failed to prepare globe skybox {}: {error}",
                config.image_path
            );
            state.status = GlobeSkyboxStatus::Failed;
            return;
        }

        state.mark_decoded(*upload_settings);
        info!(
            "decoded globe skybox {}; waiting for {} idle frames before GPU upload",
            config.image_path, state.upload_idle_frames_remaining
        );
    }

    if state.decoded_upload_ready(*upload_settings, interaction_active) {
        let Some(image_handle) = state.image_handle.as_ref() else {
            return;
        };
        let Some(image) = images.get_mut(image_handle) else {
            return;
        };
        image.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
        state.status = GlobeSkyboxStatus::Ready;
        info!("queued globe skybox {} for GPU upload", config.image_path);
    }

    if state.status != GlobeSkyboxStatus::Ready {
        return;
    }

    let Some(image_handle) = state.image_handle.as_ref() else {
        return;
    };

    let desired_skybox = Skybox {
        image: image_handle.clone(),
        brightness: config.brightness,
        rotation: config.rotation,
    };
    for (entity, existing_skybox) in &cameras {
        if existing_skybox.is_some_and(|skybox| skybox_matches(skybox, &desired_skybox)) {
            continue;
        }
        commands.entity(entity).insert(desired_skybox.clone());
        info!(
            "attached globe skybox {} to 3D scene camera",
            config.image_path
        );
    }
}

fn load_stacked_cubemap(asset_server: &AssetServer, image_path: &str) -> Handle<Image> {
    asset_server.load_with_settings(
        image_path.to_owned(),
        |settings: &mut ImageLoaderSettings| {
            // Decode first without immediately extracting a render asset. The
            // full-resolution demo skybox expands to hundreds of megabytes, so
            // uploading it in the same moment it finishes decoding can produce
            // a visible interaction hitch.
            settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
            settings.array_layout = Some(ImageArrayLayout::RowCount {
                rows: STACKED_CUBEMAP_FACE_COUNT,
            });
        },
    )
}

fn skybox_matches(left: &Skybox, right: &Skybox) -> bool {
    left.image == right.image
        && (left.brightness - right.brightness).abs() <= f32::EPSILON
        && left.rotation == right.rotation
}

fn skybox_interaction_active(
    buttons: &ButtonInput<MouseButton>,
    touches: &Touches,
    mouse_motion_events: &mut MessageReader<'_, '_, MouseMotion>,
    mouse_wheel_events: &mut MessageReader<'_, '_, MouseWheel>,
) -> bool {
    let mouse_pressed = buttons.pressed(MouseButton::Left)
        || buttons.pressed(MouseButton::Right)
        || buttons.pressed(MouseButton::Middle);
    let mouse_motion = mouse_motion_events.read().next().is_some();
    let mouse_wheel = mouse_wheel_events.read().next().is_some();
    let touch_active = touches.iter().next().is_some()
        || touches.any_just_pressed()
        || touches.any_just_released()
        || touches.any_just_canceled();

    mouse_pressed || mouse_motion || mouse_wheel || touch_active
}

fn prepare_stacked_cubemap_image(image: &mut Image) -> Result<(), String> {
    if image.texture_descriptor.array_layer_count() == 1 {
        let layers = stacked_cubemap_layers(image.width(), image.height()).map_err(|error| {
            format!(
                "{error}; dimensions are {}x{}",
                image.width(),
                image.height()
            )
        })?;

        image
            .reinterpret_stacked_2d_as_array(layers)
            .map_err(|error| error.to_string())?;
    } else {
        cubemap_array_layers(
            image.width(),
            image.height(),
            image.texture_descriptor.array_layer_count(),
        )
        .map_err(|error| {
            format!(
                "{error}; dimensions are {}x{} with {} layers",
                image.width(),
                image.height(),
                image.texture_descriptor.array_layer_count()
            )
        })?;
    }

    // Match Bevy's built-in skybox example: a cube view infers the exact six
    // layers from the texture, which avoids backend-specific layer-count quirks.
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackedCubemapError {
    EmptyWidth,
    NonSquareFaces { width: u32, height: u32 },
    ExpectedSixLayers { layers: u32 },
}

impl fmt::Display for StackedCubemapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyWidth => formatter.write_str("stacked cubemap width must be non-zero"),
            Self::NonSquareFaces { width, height } => write!(
                formatter,
                "stacked cubemap must be one square face wide, but dimensions are {width}x{height}"
            ),
            Self::ExpectedSixLayers { layers } => {
                write!(
                    formatter,
                    "stacked cubemap must contain 6 faces, found {layers}"
                )
            }
        }
    }
}

fn stacked_cubemap_layers(width: u32, height: u32) -> Result<u32, StackedCubemapError> {
    if width == 0 {
        return Err(StackedCubemapError::EmptyWidth);
    }
    if !height.is_multiple_of(width) {
        return Err(StackedCubemapError::NonSquareFaces { width, height });
    }

    let layers = height / width;
    if layers != 6 {
        return Err(StackedCubemapError::ExpectedSixLayers { layers });
    }

    Ok(layers)
}

fn cubemap_array_layers(width: u32, height: u32, layers: u32) -> Result<u32, StackedCubemapError> {
    if width == 0 {
        return Err(StackedCubemapError::EmptyWidth);
    }
    if width != height {
        return Err(StackedCubemapError::NonSquareFaces { width, height });
    }
    if layers != STACKED_CUBEMAP_FACE_COUNT {
        return Err(StackedCubemapError::ExpectedSixLayers { layers });
    }

    Ok(layers)
}

#[cfg(test)]
mod tests {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::Image;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    use super::{
        cubemap_array_layers, prepare_stacked_cubemap_image, stacked_cubemap_layers,
        DeferredGlobeSkybox, GlobeSkybox, GlobeSkyboxState, GlobeSkyboxStatus,
        GlobeSkyboxUploadSettings, MilkyWaySkyboxResolution, StackedCubemapError,
        TextureViewDimension, DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES,
        DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES, DEFAULT_MILKY_WAY_SKYBOX_RESOLUTION,
    };

    #[test]
    fn globe_skybox_can_be_wrapped_for_deferred_loading() {
        let skybox = GlobeSkybox::stacked_png("textures/stars.png").with_brightness(420.0);
        let deferred = skybox.clone().deferred();

        assert_eq!(deferred.skybox(), &skybox);
        assert_eq!(
            deferred.frames_remaining(),
            DEFAULT_GLOBE_SKYBOX_DEFER_FRAMES
        );
    }

    #[test]
    fn deferred_globe_skybox_waits_the_requested_frame_count() {
        let mut deferred =
            DeferredGlobeSkybox::after_frames(GlobeSkybox::stacked_png("textures/stars.png"), 3);

        assert_eq!(deferred.frames_remaining(), 3);
        assert!(!deferred.tick_until_ready(false));
        assert_eq!(deferred.frames_remaining(), 2);
        assert!(!deferred.tick_until_ready(false));
        assert!(deferred.tick_until_ready(false));
        assert!(deferred.tick_until_ready(false));
    }

    #[test]
    fn deferred_globe_skybox_resets_countdown_during_interaction() {
        let mut deferred =
            DeferredGlobeSkybox::after_frames(GlobeSkybox::stacked_png("textures/stars.png"), 3);

        assert!(!deferred.tick_until_ready(false));
        assert_eq!(deferred.frames_remaining(), 2);
        assert!(!deferred.tick_until_ready(true));
        assert_eq!(deferred.frames_remaining(), 3);
        assert!(!deferred.tick_until_ready(false));
        assert_eq!(deferred.frames_remaining(), 2);
    }

    #[test]
    fn skybox_upload_settings_wait_for_idle_frames_after_decode() {
        let settings = GlobeSkyboxUploadSettings {
            idle_frames_after_decode: 2,
        };
        let mut state = GlobeSkyboxState::default();

        state.mark_decoded(settings);

        assert_eq!(state.status, GlobeSkyboxStatus::Decoded);
        assert!(!state.decoded_upload_ready(settings, false));
        assert_eq!(state.upload_idle_frames_remaining, 1);
        assert!(!state.decoded_upload_ready(settings, true));
        assert_eq!(state.upload_idle_frames_remaining, 2);
        assert!(!state.decoded_upload_ready(settings, false));
        assert_eq!(state.upload_idle_frames_remaining, 1);
        assert!(!state.decoded_upload_ready(settings, false));
        assert_eq!(state.upload_idle_frames_remaining, 0);
        assert!(state.decoded_upload_ready(settings, false));
    }

    #[test]
    fn skybox_upload_settings_default_to_nonzero_idle_window() {
        assert_eq!(
            GlobeSkyboxUploadSettings::default().idle_frames_after_decode,
            DEFAULT_GLOBE_SKYBOX_UPLOAD_IDLE_FRAMES
        );
    }

    #[test]
    fn milky_way_resolution_describes_asset_variants() {
        assert_eq!(
            DEFAULT_MILKY_WAY_SKYBOX_RESOLUTION,
            MilkyWaySkyboxResolution::Face1024
        );
        assert_eq!(MilkyWaySkyboxResolution::Face512.face_size(), 512);
        assert_eq!(
            MilkyWaySkyboxResolution::Face2048.stacked_dimensions(),
            (2_048, 12_288)
        );
        assert_eq!(
            MilkyWaySkyboxResolution::Face4096.decoded_rgba_bytes(),
            4_096 * 4_096 * 6 * 4
        );
        assert_eq!(
            MilkyWaySkyboxResolution::Face1024.asset_path(),
            "ferrisium/skyboxes/milkyway_1024.png"
        );
    }

    #[test]
    fn globe_skybox_can_use_bundled_milky_way_path() {
        let skybox = GlobeSkybox::milky_way(MilkyWaySkyboxResolution::Face4096);

        assert_eq!(skybox.image_path, "ferrisium/skyboxes/milkyway_4096.png");
    }

    #[test]
    fn stacked_cubemap_validation_accepts_six_square_faces() {
        assert_eq!(stacked_cubemap_layers(4096, 24576), Ok(6));
    }

    #[test]
    fn cubemap_array_validation_accepts_six_square_faces() {
        assert_eq!(cubemap_array_layers(4096, 4096, 6), Ok(6));
    }

    #[test]
    fn stacked_cubemap_validation_rejects_non_six_layer_images() {
        assert_eq!(
            stacked_cubemap_layers(256, 1280),
            Err(StackedCubemapError::ExpectedSixLayers { layers: 5 })
        );
    }

    #[test]
    fn stacked_cubemap_validation_rejects_non_square_faces() {
        assert_eq!(
            stacked_cubemap_layers(256, 1537),
            Err(StackedCubemapError::NonSquareFaces {
                width: 256,
                height: 1537,
            })
        );
    }

    #[test]
    fn prepare_stacked_cubemap_sets_cube_view() {
        let mut image = Image::new_fill(
            Extent3d {
                width: 4,
                height: 24,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        assert_eq!(prepare_stacked_cubemap_image(&mut image), Ok(()));

        assert_eq!(image.width(), 4);
        assert_eq!(image.height(), 4);
        assert_eq!(image.texture_descriptor.array_layer_count(), 6);
        assert_eq!(
            image
                .texture_view_descriptor
                .as_ref()
                .map(|descriptor| descriptor.dimension),
            Some(Some(TextureViewDimension::Cube))
        );
        assert_eq!(
            image
                .texture_view_descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.array_layer_count),
            None
        );
    }
}
