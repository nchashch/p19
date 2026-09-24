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
        // `None` (playtest-assets/`--no-common-assets` mode omitted the key) passes straight
        // through — `Skybox.image` is itself an `Option`, and the skybox pass simply doesn't
        // run for a camera without one (the clear color shows instead).
        image: common_assets.skybox.clone(),
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
/// the agent's `game/screenshot` tool reads. Created (1280×800 — the Steam Deck's native 800p)
/// when the app starts in
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

/// Marker for `--no-render` mode (headless agent host with the render plugins disabled — no
/// wgpu/Vulkan at all). Consumers: `dev::tool_api`'s screenshot methods return a clean error
/// instead of waiting on a capture that can never complete, and `lifecycle::loading` skips
/// client world visuals (their image/mesh loading needs the render-side asset machinery).
/// Note the mode still lays out UI and computes UI rects — see [`shim_camera_computed`].
#[derive(Resource)]
pub struct NoRenderMode;

/// `--no-render` shim: feeds each camera's `Camera.computed.target_info` by hand, because the
/// system that normally computes it (`camera_system` in `bevy_render`) doesn't run without the
/// render app. `bevy_ui`'s camera propagation, layout, and `bevy_picking`'s UI backend all
/// read exactly these accessors (`target_scaling_factor()` / `physical_target_size()` →
/// `computed.target_info`), so with the shim in place UI *layout*, the `game/ui` dump, hover,
/// and clicks all work with zero rendering — the same 1280×800 logical/physical space as the
/// rendered headless mode (scale factor 1: image targets have no DPI scaling). `clip_from_view`
/// stays identity, which nothing in a render-less app consumes.
pub fn shim_camera_computed(mut cameras: Query<&mut Camera>) {
    const SIZE: UVec2 = UVec2::new(1280, 800);
    for mut camera in &mut cameras {
        let needs_shim = !matches!(
            camera.computed.target_info,
            Some(bevy::camera::RenderTargetInfo {
                physical_size: SIZE,
                scale_factor: 1.0,
            })
        );
        if needs_shim {
            camera.computed.target_info = Some(bevy::camera::RenderTargetInfo {
                physical_size: SIZE,
                scale_factor: 1.0,
            });
        }
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
/// **The menu/lobby-UI-under-background bug, and what actually caused three earlier fix
/// attempts to regress in-game rendering instead** (root-caused via a direct read of
/// `bevy_ui::ui_node::DefaultUiCamera::get()`, `bevy_ui-0.19.0/src/ui_node.rs`): with this
/// scheme alone, the UI panel ends up drawn *under* the menu/lobby background once that
/// background actually renders — the background arrives on a later frame than the UI camera
/// and, correctly per this scheme, draws on top of it with no clear. That part *looks* like a
/// pure ordering problem, and three earlier attempts (bumping the UI camera's order
/// dynamically; re-deriving clear ownership every tick; a fixed high sentinel order) all
/// treated it as one — each produced a `Camera` configuration BRP confirmed was correct, and
/// each still regressed in-game rendering to a blank frame anyway, for reasons that resisted
/// explanation at the time.
///
/// The actual mechanism: `DefaultUiCamera::get()` picks the sole `IsDefaultUiCamera`-bearing
/// entity via `.single()` if exactly one exists; **its fallback path — used when zero or more
/// than one exist — only ever considers cameras whose `RenderTarget` is `Window(Primary)`**,
/// structurally excluding `RenderTarget::Image` entirely. In headless mode every camera targets
/// `Image`, so that fallback can never succeed here, full stop — unlike on desktop, where it's
/// *why* UI compositing needs zero custom code at all (no persistent UI camera exists there;
/// whichever real content camera happens to be the sole `Window(Primary)` candidate is picked
/// by elimination). Headless mode's `Startup`-spawned bootstrap camera below exists specifically
/// because that fallback can't do the same job here — but it never despawned, and
/// `player_camera()` *also* tags itself `IsDefaultUiCamera` once it spawns, so the instant a
/// player exists there were **two** simultaneous holders, `.single()` failed, the
/// (headless-dead) fallback couldn't rescue it, and `DefaultUiCamera::get()` returned `None`.
/// All three earlier ordering-only fixes were tried against this same standing ambiguity —
/// consistent with it being the actual cause of their unexplained in-game regressions, not
/// anything about `order` itself, though this wasn't independently re-tested against each of
/// those three specific reverted attempts individually once the ambiguity fix below was in
/// place (only against the current, simple order/clear scheme).
///
/// The fix, confirmed working end-to-end (`docs/playtests/playtest_0004/`: main menu, lobby,
/// in-game — including the HUD, which never rendered at all before this either — and a full
/// disconnect-back-to-main-menu round-trip), has two independent parts, in
/// `maintain_default_ui_camera`/`keep_ui_camera_drawn_last` below: (1) keep the invariant
/// "exactly one live entity carries `IsDefaultUiCamera`" true at all times, handing it off
/// between the bootstrap camera and `player_camera()` as they come and go, instead of letting
/// them collide; (2) *given* that
/// invariant, re-derive every tick which entity currently holds the marker and keep it drawn
/// last (highest order, no clear) among the cameras sharing this offscreen target — this is the
/// same "re-derive fresh every tick" shape one of the earlier reverted attempts used, now
/// resting on a real invariant instead of an assumption that never held once a player existed.
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

/// Marks headless mode's `Startup`-spawned UI camera specifically, distinguishing it from
/// `player_camera()`'s own `Camera3d` (which also carries `IsDefaultUiCamera` — see the doc
/// comment on [`retarget_cameras_to_offscreen`]). Only ever inserted in `main.rs`'s headless
/// branch.
#[derive(Component)]
pub struct HeadlessUiCameraBootstrap;

/// Keeps "exactly one live entity carries `IsDefaultUiCamera`" true at all times, headless-only.
/// `bevy_ui`'s own fallback for "no unique holder" only considers `RenderTarget::Window(_)`
/// cameras (see the doc comment above) — structurally dead in headless mode, where every camera
/// targets `Image` — so this project has to maintain that invariant itself rather than relying
/// on upstream to recover from a momentary zero- or two-holder state the way desktop implicitly
/// can. Hands the marker to the bootstrap camera whenever nothing else holds it (covers boot,
/// and every return trip from `InGame` back to `Lobby`/`MainMenu` once the player camera
/// despawns), and strips it the instant a real content camera claims it on its own
/// (`player_camera()` self-tags at spawn) — a one-tick window where both exist is possible but
/// harmless, since the next run of this system resolves it before `camera_system`/`bevy_ui` do
/// anything observably wrong with it.
pub fn maintain_default_ui_camera(
    bootstrap: Query<(Entity, Has<IsDefaultUiCamera>), With<HeadlessUiCameraBootstrap>>,
    other_holders: Query<Entity, (With<IsDefaultUiCamera>, Without<HeadlessUiCameraBootstrap>)>,
    mut commands: Commands,
) {
    let Ok((bootstrap_entity, bootstrap_has_marker)) = bootstrap.single() else {
        return;
    };
    let other_holder_exists = !other_holders.is_empty();
    if other_holder_exists && bootstrap_has_marker {
        commands.entity(bootstrap_entity).remove::<IsDefaultUiCamera>();
    } else if !other_holder_exists && !bootstrap_has_marker {
        commands.entity(bootstrap_entity).insert(IsDefaultUiCamera);
    }
}

/// Given `maintain_default_ui_camera`'s invariant (exactly one live `IsDefaultUiCamera` holder),
/// keeps whichever entity currently holds it drawn *last* — highest order, no clear — among the
/// cameras sharing this offscreen target, so it actually composites on top of a later-arriving
/// opaque 3D world camera instead of being painted over by one. Re-derived fresh every tick
/// (not decided once at claim time) because *which* entity holds the marker changes over a
/// session (bootstrap camera → `player_camera()` → back to the bootstrap camera on disconnect).
/// Write-if-different throughout: `Mut<Camera>` flags `Changed<Camera>` on any dereference for
/// write even when the assigned value doesn't change, and this runs every tick.
pub fn keep_ui_camera_drawn_last(
    offscreen: Option<Res<OffscreenRenderTarget>>,
    mut cameras: Query<(Entity, &RenderTarget, &mut Camera, Has<IsDefaultUiCamera>)>,
) {
    let Some(offscreen) = offscreen else {
        return;
    };
    let on_our_target = |target: &RenderTarget| {
        matches!(target, RenderTarget::Image(image) if image.handle == offscreen.0)
    };

    // Phase 1: bump the UI camera above whatever else currently exists, if anything does.
    // Highest order *excluding* the UI camera itself — the bump target has to be a fixed point
    // that doesn't move just because the UI camera's own order changed, or bumping it to
    // "highest + 1" every tick would increment it forever (its own new value becomes next
    // tick's "highest", so "+1" keeps climbing). `None` when no other camera exists yet (the
    // sole-bootstrap-camera moment) — nothing to bump above, so the UI camera just keeps
    // whatever order it already has from `retarget_cameras_to_offscreen`'s initial claim.
    let highest_non_ui_order = cameras
        .iter()
        .filter(|(_, target, _, is_ui_camera)| on_our_target(target) && !is_ui_camera)
        .map(|(_, _, camera, _)| camera.order)
        .max();
    if let Some(highest_non_ui_order) = highest_non_ui_order {
        let wanted = highest_non_ui_order + 1;
        for (_, target, mut camera, is_ui_camera) in &mut cameras {
            if is_ui_camera && on_our_target(target) && camera.order != wanted {
                camera.order = wanted;
            }
        }
    }

    // Phase 2: re-derive lowest order fresh, *after* any Phase 1 bump — computing it before
    // would use the UI camera's stale pre-bump order, meaning on the exact tick it first moves
    // away from being lowest, nothing would end up matching (its old order no longer belongs to
    // any camera, and nothing else has moved down to claim it) and the target would go
    // unrendered-to (not cleared) for that one tick. Cheap to just not have that gap at all.
    let Some(lowest_order) = cameras
        .iter()
        .filter(|(_, target, ..)| on_our_target(target))
        .map(|(_, _, camera, _)| camera.order)
        .min()
    else {
        return;
    };

    // Phase 3: whichever camera now has that lowest order clears; everyone else on this target
    // doesn't. Deliberately not excluding the UI camera here — when it's the only camera that
    // exists (nothing to bump above yet, Phase 1 was a no-op), it's trivially both lowest and
    // highest, and correctly keeps `Default` (the bootstrap-alone case, same as before this
    // system existed). Once a non-UI camera also exists and Phase 1 has bumped the UI camera
    // above it, the UI camera naturally stops being lowest on its own.
    for (entity, target, mut camera, _) in &mut cameras {
        if !on_our_target(target) {
            continue;
        }
        let wants_default = camera.order == lowest_order;
        let is_default = matches!(camera.clear_color, ClearColorConfig::Default);
        if wants_default != is_default {
            camera.clear_color = if wants_default {
                ClearColorConfig::Default
            } else {
                ClearColorConfig::None
            };
            info!(
                "camera {entity} clear -> {}",
                if wants_default { "Default" } else { "None" }
            );
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
