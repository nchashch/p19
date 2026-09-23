use bevy::light::Skybox;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{ClearColorConfig, RenderTarget},
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

/// Rewrites every camera that would render nowhere to render into
/// [`OffscreenRenderTarget`] instead — the loaded level/background worlds carry their own
/// cameras that would otherwise render nowhere, and the player camera and menu UI camera are
/// covered by the same sweep. Polling rather than an `On<Add, Camera>` observer, matching the
/// repo's replication-arrival precedent (cameras can appear at any time from loaded worlds,
/// and `RenderTarget` may also *change back* after spawn).
///
/// Two value shapes render nowhere in headless mode: `Window(_)` (the default — there is no
/// window) and `RenderTarget::None { .. }` (authored/glTF cameras that opt out of a target).
/// Deliberately untouched: `Image(_)` (cameras already rendering into a texture — e.g. the
/// UI-quad cameras whose texture is displayed on a quad; stealing it would blank the quad)
/// and `TextureView(_)` (XR eye targets).
///
/// All the claimed cameras share ONE target, so each must also stop clearing it
/// (`ClearColorConfig::None`) — a camera that clears erases every camera that rendered before
/// it, which is exactly why the menu background used to vanish under the UI camera's clear.
/// The startup UI camera keeps the only `Default` clear and order 0, so it runs first every
/// frame; each newly claimed camera gets the next order (1, 2, 3, …), i.e. the most recently
/// arrived camera's 3D view wins — the player camera arrives after the level's authored one,
/// so the agent sees through the player's eyes in game.
///
/// **Known, deliberately unfixed follow-on issue**: with this scheme, the menu/lobby UI panel
/// (Connect/Play/etc.) ends up drawn *under* the menu/lobby background once that background
/// actually renders (see the `target_info` fix below) — the background arrives on a later
/// frame than the UI camera and, correctly per this scheme, draws on top of it with no clear.
/// Three different attempts at making the UI camera draw last *without* breaking in-game
/// rendering were tried and reverted in the same session this fix landed (bumping the UI
/// camera's order dynamically; re-deriving clear ownership every tick; giving the UI camera a
/// fixed high sentinel order) — each fixed the menu/lobby ordering but regressed in-game
/// rendering back to a blank frame, for reasons not fully root-caused (suspected: something
/// order-dependent downstream of `camera.order`, e.g. frustum/visible-entities computation,
/// which isn't BRP-inspectable to confirm directly since `VisibleEntities` is
/// `#[reflect(ignore)]`). Reverted to this known-good version rather than ship an unverified
/// fix for a cosmetic issue at the cost of the actual rendering-works-at-all fix. Worth
/// revisiting with more room to instrument the render world directly (e.g. a temporary custom
/// extract-schedule system logging `VisibleEntities` counts) rather than only BRP-probing the
/// main world's `Camera` component, which reports "correct" configuration even on the runs
/// where nothing actually rendered.
///
/// Only run in headless mode: on desktop, `Window(Primary)` is exactly right.
///
/// **The camera-target-never-resolves bug and its fix** (found via a direct read of
/// `bevy_render::camera::camera_system`, `bevy_render-0.19.0/src/camera.rs`): every camera here
/// spawns pointed at the default `RenderTarget::Window(Primary)`, but headless mode never has a
/// primary window (`WindowPlugin { primary_window: None, .. }`) — so `RenderTarget::normalize`
/// returns `None` for it, and `camera_system` silently skips its *entire* per-camera body
/// (including the `Camera.computed.target_info` recompute) for any camera still in that state,
/// with no error. Critically, merely running that `Query` item still consumes the camera's
/// one-tick `is_added()` window even though the skipped body never reads it. By the time this
/// system gets around to retargeting a given camera (spawned mid-session — a loaded world's
/// background camera, the player's camera, anything not present at `Startup`), `is_added()` has
/// already gone false, and the shared [`OffscreenRenderTarget`] image's own one-time
/// `AssetEvent::Added` was already consumed by whichever camera claimed it first. Nothing in
/// `camera_system`'s recompute gate (window/image asset events, `is_added()`, projection change,
/// viewport-size change) ever fires again for that camera, so `target_info` — and thus its
/// render output — stays permanently unresolved. Confirmed by testing: only the camera present
/// at `Startup` (retargeted before its first `camera_system` pass) ever got a populated
/// `target_info`; every camera retargeted on a later frame (the player's, and every loaded
/// world's background camera) never did, independent of how long the app kept running.
///
/// The fix forces the one recompute condition this system *can* trigger deliberately: touching
/// `Projection`'s own change-detection flag right when we fix the target, so `camera_system`'s
/// `camera_projection.is_changed()` check is true on the very next pass — regardless of the
/// `is_added()`/`AssetEvent` race above. `set_changed()` alone (no value mutation) is enough.
pub fn retarget_cameras_to_offscreen(
    offscreen: Option<Res<OffscreenRenderTarget>>,
    mut cameras: Query<(Entity, &mut RenderTarget, &mut Camera, &mut Projection)>,
    mut next_order: Local<u32>,
) {
    let Some(offscreen) = offscreen else {
        return;
    };
    for (entity, mut target, mut camera, mut projection) in &mut cameras {
        if matches!(
            *target,
            RenderTarget::Window(_) | RenderTarget::None { .. }
        ) {
            *target = RenderTarget::Image(offscreen.0.clone().into());
            *next_order += 1;
            camera.order = *next_order as isize;
            if *next_order == 1 {
                // The first-claimed camera renders first (lowest order): it is the shared
                // target's base layer and keeps its clear. Every later claim draws on top of
                // it instead of erasing it — a camera left on `Default` clear would erase every
                // camera that rendered before it, leaving only the last camera's frame.
            } else {
                camera.clear_color = ClearColorConfig::None;
            }
            // See the doc comment above: this is what actually makes `camera_system` compute
            // `target_info` for this camera at all, since the `RenderTarget`/`Camera` writes
            // above don't by themselves satisfy its recompute gate.
            projection.set_changed();
            info!("retargeted camera {entity} to the offscreen target (order {})", *next_order);
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
