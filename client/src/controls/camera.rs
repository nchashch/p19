use bevy::light::Skybox;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::RenderTarget,
    pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion},
    prelude::*,
    render::render_resource::{TextureFormat, TextureUsages},
};
use bevy_mod_xr::camera::XrCamera;

use crate::assets::collections::CommonAssets;

/// Matches `main.rs`'s `ClearColor` — a fallback background for whatever the skybox doesn't
/// cover (there's always a moment before `skyboxes/night_sky.ktx2` finishes streaming in where
/// nothing is rendered there yet), so fog blends into that instead of a visibly different flat
/// color in the meantime.
const FOG_COLOR: Color = Color::srgb(0.1, 0.1, 0.15);
/// Chosen relative to existing gameplay distances rather than arbitrarily: `targeting::SELECT_RANGE`
/// and `nameplate.rs`'s `FADE_END_DISTANCE` (30.0) are both well inside `FOG_START`, so fog never
/// visibly interferes with targeting or nameplate readability at any range they actually matter.
const FOG_START: f32 = 40.0;
const FOG_END: f32 = 180.0;

const SKYBOX_BRIGHTNESS: f32 = 100.0;

fn distance_fog() -> DistanceFog {
    DistanceFog {
        color: FOG_COLOR,
        falloff: FogFalloff::Linear {
            start: FOG_START,
            end: FOG_END,
        },
        ..default()
    }
}

fn skybox(common_assets: &CommonAssets) -> Skybox {
    // Unlike the old PNG-vertical-strip cubemap this replaces, a KTX2 file carries its own
    // cubemap metadata (see `scripts/hdri_to_skybox.py`'s doc comment) — Bevy's KTX2 loader
    // detects the 6 faces and sets up `TextureViewDimension::Cube` automatically, so this can
    // just be loaded and inserted directly, with no manual "wait for load, then reinterpret
    // the texture" dance required (that machinery lived here before; see git history). Loaded via
    // `CommonAssets` (see `assets.rs`) rather than `asset_server.load(...)` here, so it's already
    // resident by the time any camera is spawned instead of streaming in after the fact.
    Skybox {
        image: Some(common_assets.skybox.clone()),
        brightness: SKYBOX_BRIGHTNESS,
        ..default()
    }
}

pub fn player_camera(common_assets: &CommonAssets) -> impl Bundle {
    (
        Camera3d::default(),
        IsDefaultUiCamera,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        distance_fog(),
        skybox(common_assets),
    )
}

/// The offscreen texture every camera renders to in `--mcp` (headless) mode — the rendered view
/// the agent's `game/screenshot` tool reads. Created (1280×720) when the app starts in
/// headless mode; see the headless branch in `main.rs` and `retarget_cameras_to_offscreen`.
#[derive(Resource, Clone)]
pub struct OffscreenRenderTarget(pub Handle<Image>);

impl OffscreenRenderTarget {
    pub fn new(width: u32, height: u32, images: &mut Assets<Image>) -> Self {
        let mut image =
            Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
        // The screenshot readback copies from this texture; the default render-attachment
        // usage doesn't include COPY_SRC.
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        Self(images.add(image))
    }
}

/// Rewrites every camera whose target is the (nonexistent, in headless mode) primary window to
/// render into [`OffscreenRenderTarget`] instead — the loaded level/background worlds carry
/// their own cameras that would otherwise render nowhere, and the player camera and menu UI
/// camera are covered by the same sweep. Polling rather than an `On<Add, Camera>` observer,
/// matching the repo's replication-arrival precedent (cameras can appear at any time from
/// loaded worlds, and `RenderTarget` may also *change back* after spawn).
///
/// Only run in headless mode: on desktop, `Window(Primary)` is exactly right.
pub fn retarget_cameras_to_offscreen(
    offscreen: Option<Res<OffscreenRenderTarget>>,
    mut cameras: Query<&mut RenderTarget>,
) {
    let Some(offscreen) = offscreen else {
        return;
    };
    for mut target in &mut cameras {
        if matches!(*target, RenderTarget::Window(_)) {
            *target = RenderTarget::Image(offscreen.0.clone().into());
        }
    }
}

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_xr_camera_added);
    }
}

/// In VR, rendering goes through separate `XrCamera` entities (one per eye — spawned by
/// `bevy_openxr`'s view-setup code with only a bare default `Camera3d`, entirely independent of
/// the `Camera3d` `player_camera()` spawns), so `Skybox`/`DistanceFog`/etc. on our own camera
/// never reach them. An `On<Add, XrCamera>` observer rather than a polling system, per this
/// project's component-lifecycle convention — these entities are created at a time relative to
/// session startup that nothing else here controls (see `camera.rs`'s git history / AGENTS.md
/// for the broader pattern of XR entities appearing later than `Startup`/`OnEnter`).
fn on_xr_camera_added(
    added: On<Add, XrCamera>,
    mut commands: Commands,
    // `Option`, not a bare `Res` — the OpenXR session (and its `XrCamera` entities) starts at app
    // boot in VR mode (`main.rs` inserts `OxrSessionConfig` unconditionally in `Prototype19::build`,
    // not gated on any `GameState`), which can run before `GameState::AssetLoading` finishes and
    // inserts `CommonAssets`.
    common_assets: Option<Res<CommonAssets>>,
) {
    let Some(common_assets) = common_assets else {
        return;
    };
    commands
        .entity(added.entity)
        .insert((distance_fog(), skybox(&common_assets)));
}
