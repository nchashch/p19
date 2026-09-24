//! The agent/QA tool API (ADR 0009) — a localhost tool surface on the client so LLM agents can
//! inspect game state, capture screenshots, and drive input for automated QA/playtesting.
//!
//! Three layers, per the ADR:
//!
//! 1. **BRP** (the Bevy Remote Protocol, `bevy_remote`) as the data layer: JSON-RPC 2.0 over
//!    HTTP on `127.0.0.1:15702`. The built-in methods (`bevy/query`, `bevy/get_components`,
//!    `bevy/list+watch`, `bevy/spawn`, …) expose the whole reflected ECS for free — every
//!    replicated component in this project is `Reflect + Serialize` already.
//! 2. **Custom BRP methods** as the game tools: `game/state` (a curated snapshot — agents work
//!    better with a small structured view than raw ECS dumps), `game/screenshot` +
//!    `game/screenshot/get` (capture via bevy's `Screenshot` → PNG on disk → base64 on poll),
//!    `game/input` (inject input through the *real* pipeline: `ActionMock` on the replicated
//!    ahoy action entities, so the agent's input flows BEI → replicated-BEI →
//!    server-authoritative sim → corrected prediction, exactly like a gamepad's — but only for
//!    the three ahoy gameplay actions, since it mocks at the action level), and `game/gamepad`
//!    (a level below that: mocks `bevy_input::gamepad::Gamepad`'s own button/axis state on a
//!    synthetic gamepad entity, so it also flows through BEI's real binding resolution),
//!    `game/keyboard` (mocks `ButtonInput<KeyCode>` directly — see `keyboard_method`), and
//!    `game/mouse` (mocks `ButtonInput<MouseButton>` plus real `MouseMotion`/`MouseWheel`
//!    events, and drives `bevy_picking`'s own `PointerInput` pipeline for cursor position and
//!    UI clicks — see `mouse_method`'s doc comment, including a real gotcha found by testing:
//!    `AccumulatedMouseMotion`/`AccumulatedMouseScroll` can't be set directly, only injected as
//!    events). `game/gamepad`/`game/keyboard`/`game/mouse` are all the same idea at the device
//!    level, one level below `game/input`'s action-level mocking — together they're the only
//!    way to reach UI navigation (menus, the pause screen, clicking an actual button) through
//!    this API at all, `game/input` only ever drives the three ahoy gameplay actions.
//! 3. **An in-process MCP server** (`rmcp`, Streamable HTTP on `127.0.0.1:15710`, stateless
//!    mode) whose tools proxy to the BRP methods over loopback HTTP — the MCP layer owns only
//!    the protocol surface (tool listing + schemas), never the `World` (the handlers are async
//!    and run outside Bevy's world; all `World` access stays in BRP's systems).
//!
//! All of this only compiles/enables under the `dev-tools` cargo feature — **never enable it
//! in player-facing builds**: the client is untrusted in this architecture, and a tool API in
//! it is a cheat surface. BRP is also unauthenticated by design — localhost bind only.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::ecs::query::QueryState;
use bevy::feathers::controls::FeathersButton;
use bevy::picking::Pickable;
use bevy::picking::hover::Hovered as PickHovered;
use bevy::text::TextSpan;
use bevy::ui::{ComputedUiTargetCamera, Pressed as UiPressed, UiGlobalTransform, UiStack};
use bevy::ui_widgets::Button as UiWidgetsButton;
use std::collections::HashSet;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::path::Path;
use bevy_enhanced_input::prelude::{Action, ActionMock, ActionValue, Actions, MockSpan, TriggerState};
use serde_json::json;

use bevy_ahoy::{CharacterControllerState, CharacterLook};
use shared::combat::{Dead, Gcd, HitPoints};
use shared::game_state::GameState;
use shared::inputs::PlayerInputContext;

use avian3d::prelude::LinearVelocity;

use crate::controls::camera::OffscreenRenderTarget;
use crate::gameplay::player_character::LocalPlayer;

/// The offscreen target's center in screenshot pixel space — the pointer's parking position
/// before anything has moved it. Derived from the actual target (Steam Deck 800p, so 640×400)
/// rather than a baked constant, so the size and the fallback can't drift apart.
fn offscreen_center(world: &World) -> Vec2 {
    world
        .get_resource::<OffscreenRenderTarget>()
        .and_then(|target| {
            world
                .get_resource::<Assets<Image>>()
                .and_then(|images| images.get(&target.0))
                .map(|image| image.size().as_vec2() / 2.0)
        })
        .unwrap_or(Vec2::new(640.0, 400.0))
}

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        let brp_port = crate::config::brp_port_presync();
        // BRP: DevToolsPlugin owns the BRP server (`main.rs` constructs `SkeinPlugin` with
        // `handle_brp: false` for exactly this), so this is always the adder — and the port
        // flag always applies, in dev and release alike. The `is_plugin_added` guard stays as
        // belt-and-braces against a future plugin-ordering change (double-add panics).
        if !app.is_plugin_added::<bevy::remote::RemotePlugin>() {
            app.add_plugins((
                RemotePlugin::default(),
                RemoteHttpPlugin::default().with_port(brp_port),
            ));
        }

        // Custom methods attach post-build via the `RemoteMethods` resource (the plugins'
        // `with_method_main` only works at construction, and Skein may have built the plugin
        // before us).
        let state_method = app.register_system(game_state_method);
        let screenshot_start = app.register_system(screenshot_start_method);
        let screenshot_get = app.register_system(screenshot_get_method);
        let input_method_id = app.register_system(input_method);
        let gamepad_method_id = app.register_system(gamepad_method);
        let keyboard_method_id = app.register_system(keyboard_method);
        let mouse_method_id = app.register_system(mouse_method);
        let trigger_method = app.register_system(trigger_method);
        let levels_method = app.register_system(levels_method);
        let select_level_method = app.register_system(select_level_method);
        let ui_method = app.register_system(ui_dump_method);
        let mut methods = app
            .world_mut()
            .resource_mut::<bevy::remote::RemoteMethods>();
        methods.insert("game/state", bevy::remote::RemoteMethodSystemId::Instant(state_method));
        methods.insert("game/screenshot", bevy::remote::RemoteMethodSystemId::Instant(screenshot_start));
        methods.insert("game/screenshot/get", bevy::remote::RemoteMethodSystemId::Instant(screenshot_get));
        methods.insert("game/input", bevy::remote::RemoteMethodSystemId::Instant(input_method_id));
        methods.insert("game/gamepad", bevy::remote::RemoteMethodSystemId::Instant(gamepad_method_id));
        methods.insert("game/keyboard", bevy::remote::RemoteMethodSystemId::Instant(keyboard_method_id));
        methods.insert("game/mouse", bevy::remote::RemoteMethodSystemId::Instant(mouse_method_id));
        methods.insert("game/trigger", bevy::remote::RemoteMethodSystemId::Instant(trigger_method));
        methods.insert("game/levels", bevy::remote::RemoteMethodSystemId::Instant(levels_method));
        methods.insert("game/select_level", bevy::remote::RemoteMethodSystemId::Instant(select_level_method));
        methods.insert("game/ui", bevy::remote::RemoteMethodSystemId::Instant(ui_method));

        // The agent cursor overlay (headless-only; both systems no-op otherwise).
        app.add_systems(
            Update,
            (spawn_agent_cursor_if_headless, update_agent_cursor),
        );

        app.init_resource::<LastServedCapture>();

        start_mcp_server(crate::config::mcp_port_presync());
    }
}

// ---------------------------------------------------------------------------
// Custom BRP methods — handlers run in the main world with `&mut World` access.
// ---------------------------------------------------------------------------

/// `game/state` — a curated snapshot of everything an agent needs to reason about the game:
/// the app's `GameState`, the local player's entity id, position, velocity, look yaw/pitch,
/// grounded, HP, GCD and death timers. Params are ignored.
fn game_state_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    Ok(game_state_snapshot(world).into())
}

/// The `game/state` payload as a plain JSON value. Shared by the `game/state` method and
/// [`screenshot_get_method`], which fuses it into every capture response (and writes it to a
/// `.json` sidecar next to the PNG) — a screenshot arrives with its ground-truth state
/// attached, so the agent never has to OCR the HUD or correlate "which call came after which
/// action". Snapshot is taken at *poll* time, i.e. a few hundred ms after the capture started;
/// that is the state the agent wants anyway (the world as it is right after its action), and
/// the capture→poll gap is bounded by the poll loop (~100ms granularity).
fn game_state_snapshot(world: &mut World) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(state) = world.get_resource::<State<GameState>>() {
        out.insert("game_state".into(), json!(format!("{:?}", state.get())));
    }

    let Some(player) = world.get_resource::<LocalPlayer>().and_then(|lp| lp.0) else {
        out.insert("connected".into(), json!(false));
        return serde_json::Value::Object(out);
    };
    out.insert("connected".into(), json!(true));
    out.insert("player_entity".into(), json!(player));

    let Ok(player_entity) = world.get_entity(player) else {
        out.insert("player_despawned".into(), json!(true));
        return serde_json::Value::Object(out);
    };

    if let Some(transform) = player_entity.get::<Transform>() {
        out.insert("position".into(), json!(transform.translation));
    }
    if let Some(velocity) = player_entity.get::<LinearVelocity>() {
        out.insert("velocity".into(), json!(velocity.0));
    }
    if let Some(look) = player_entity.get::<CharacterLook>() {
        out.insert("look_yaw".into(), json!(look.yaw));
        out.insert("look_pitch".into(), json!(look.pitch));
    }
    if let Some(state) = player_entity.get::<CharacterControllerState>() {
        out.insert("grounded".into(), json!(state.grounded.is_some()));
        out.insert("crouching".into(), json!(state.crouching));
    }
    if let Some(hp) = player_entity.get::<HitPoints>() {
        out.insert("hit_points".into(), json!(hp.hit_points));
        out.insert("max_hit_points".into(), json!(hp.max_hit_points));
    }
    if let Some(gcd) = player_entity.get::<Gcd>() {
        out.insert("gcd_remaining_secs".into(), json!(gcd.0.remaining_secs()));
    }
    if let Some(dead) = player_entity.get::<Dead>() {
        out.insert("dead".into(), json!(true));
        out.insert("dead_remaining_secs".into(), json!(dead.0.remaining_secs()));
    } else {
        out.insert("dead".into(), json!(false));
    }

    serde_json::Value::Object(out)
}

/// Where captures land: `<workspace>/docs/playtests/dist/screenshots/<utc>-<label>.png` —
/// persistent, NOT consumed on read, so a human can browse everything the agent saw. This is
/// raw, uncurated staging output (gitignored — `docs/playtests/dist/` holds nothing meant to be
/// committed), not the same thing as `docs/playtests/screenshots/playtest_NNNN/`, which is the
/// curated, Git LFS-tracked subset an agent copies in when actually filing a playtest report
/// (see `docs/skills/playtest.md` §10) — this function has no notion of "which playtest number"
/// a capture belongs to, since that's only decided after the fact, when a report gets written.
/// Anchored on `CARGO_MANIFEST_DIR` (set under `cargo run`/`cargo build`, and by the QA harness
/// that launches the client) falling back to the CWD, matching how bevy itself resolves asset
/// roots.
///
/// **Fleet isolation**: a client launched with a non-default `--brp-port` (multi-client
/// testing, several clients on one machine against one server) captures into a per-client
/// `screenshots/client-<port>/` directory instead. Both the capture target and
/// `game/screenshot/get`'s "newest PNG in dir" answer derive from this directory, so a shared
/// dir would cross-contaminate clients — client A's poll would return client B's capture.
fn screenshots_dir() -> PathBuf {
    let base = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(|manifest| PathBuf::from(manifest).join("../docs/playtests/dist/screenshots"))
        .unwrap_or_else(|| PathBuf::from("docs/playtests/dist/screenshots"));
    let port = crate::config::brp_port_presync();
    if port == bevy::remote::http::DEFAULT_PORT {
        base
    } else {
        base.join(format!("client-{port}"))
    }
}

/// A new unique capture path: `<utc-zulu>-<label>.png`, millisecond-resolution so names sort
/// chronologically. A same-millisecond collision (two captures in one instant) appends a
/// counter suffix.
fn next_screenshot_path(label: Option<&str>) -> PathBuf {
    let dir = screenshots_dir();
    let _ = std::fs::create_dir_all(&dir);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let label = label.unwrap_or("capture");
    let stem = format!("{millis}-{label}");
    let mut path = dir.join(format!("{stem}.png"));
    let mut disambiguator = 1u32;
    while path.exists() {
        path = dir.join(format!("{stem}-{disambiguator}.png"));
        disambiguator += 1;
    }
    path
}

/// The newest capture currently on disk (what `game/screenshot/get` reports). The filenames are
/// millisecond timestamps, so lexicographic max = chronological max.
fn newest_screenshot() -> Option<PathBuf> {
    let dir = screenshots_dir();
    let mut newest: Option<(std::ffi::OsString, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "png") {
            let name = entry.file_name();
            if newest.as_ref().is_none_or(|(best, _)| name > *best) {
                newest = Some((name, path));
            }
        }
    }
    newest.map(|(_, path)| path)
}

/// `game/screenshot` — starts an async capture of the primary window. The PNG is written under
/// `docs/playtests/dist/screenshots/` (encoded by [`save_cropped_to_disk`], async); poll
/// `game/screenshot/get` until it reports `ready`. Optional params: `{"label": "..."}` for the
/// filename, `{"crop": [x, y, w, h]}` to save only that sub-rect — in the same screenshot pixel
/// space `game/ui` dumps, so "crop to a button's rect from the dump" just works. A crop costs
/// the model fewer vision tokens (cost is dimension-driven) and, unlike a full frame, a small
/// crop survives the provider's downscale unscaled — full effective resolution on the region
/// of interest. The file PERSISTS (it is the human-browsable record of what the agent saw), so
/// this also returns the path immediately.
fn screenshot_start_method(
    params: In<Option<serde_json::Value>>,
    world: &mut World,
) -> BrpResult {
    let label = params
        .0
        .as_ref()
        .and_then(|p| p.get("label"))
        .and_then(serde_json::Value::as_str);
    if world.get_resource::<crate::controls::camera::NoRenderMode>().is_some() {
        return Err(BrpError::internal(
            "no rendering enabled (--no-render): screenshots are unavailable; use game/ui for \
             on-screen content and game/state for ground truth",
        ));
    }
    let crop = match params.0.as_ref().and_then(|p| p.get("crop")) {
        Some(value) => Some(parse_crop(value).ok_or_else(|| {
            BrpError::internal("crop must be [x, y, w, h] — four pixel numbers")
        })?),
        None => None,
    };
    let path = next_screenshot_path(label);
    // Headless (`--mcp`) mode: the cameras render into the offscreen texture — capture THAT.
    // Windowed: capture the primary window.
    let capture_target = world
        .get_resource::<OffscreenRenderTarget>()
        .map(|target| Screenshot(bevy::camera::RenderTarget::Image(target.0.clone().into())))
        .unwrap_or_else(Screenshot::primary_window);
    world
        .spawn(capture_target)
        .observe(save_cropped_to_disk(path.clone(), crop));
    Ok(json!({
        "status": "capturing",
        "poll": "game/screenshot/get",
        "path": path.display().to_string(),
        "crop": crop,
    })
    .into())
}

/// Parses `game/screenshot`'s `crop` param: `[x, y, w, h]`, four finite pixel numbers
/// (floats rounded), all non-negative. `None` on anything else.
fn parse_crop(value: &serde_json::Value) -> Option<[u32; 4]> {
    let array = value.as_array()?;
    if array.len() != 4 {
        return None;
    }
    array
        .iter()
        .map(|v| {
            let n = v.as_f64()?;
            if !n.is_finite() || n < 0.0 {
                return None;
            }
            Some(n.round() as u32)
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| [parts[0], parts[1], parts[2], parts[3]])
}

/// [`save_to_disk`]'s crop-capable twin: encodes the captured frame's sub-rect `crop` (or the
/// whole frame when `None`) to `<path>` as PNG, clamping the rect to the frame's bounds so an
/// out-of-range crop degrades to the intersection instead of panicking. Mirrors
/// `save_to_disk`'s HDR-safety (`to_rgb8` drops the alpha channel). The capture itself is
/// always of the full render target — the crop is applied at encode time.
fn save_cropped_to_disk(
    path: impl AsRef<Path>,
    crop: Option<[u32; 4]>,
) -> impl FnMut(On<ScreenshotCaptured>) {
    let path = path.as_ref().to_owned();
    move |captured: On<ScreenshotCaptured>| {
        let Ok(dyn_img) = captured.image.clone().try_into_dynamic() else {
            error!("screenshot crop: unsupported capture format at {}", path.display());
            return;
        };
        let rgb = dyn_img.to_rgb8();
        let cropped = match crop {
            None => image::DynamicImage::ImageRgb8(rgb),
            Some([x, y, w, h]) => {
                let x = x.min(rgb.width().saturating_sub(1));
                let y = y.min(rgb.height().saturating_sub(1));
                let w = w.min(rgb.width() - x).max(1);
                let h = h.min(rgb.height() - y).max(1);
                image::DynamicImage::ImageRgb8(image::imageops::crop_imm(&rgb, x, y, w, h).to_image())
            }
        };
        match cropped.save_with_format(&path, image::ImageFormat::Png) {
            Ok(_) => info!("Screenshot saved to {} (crop: {crop:?})", path.display()),
            Err(e) => error!("Cannot save screenshot, IO error: {e}"),
        }
    }
}

/// The pixels of the last capture `game/screenshot/get` served in full (hash of the PNG bytes +
/// the file). If the newest capture's pixels hash identically, the poll answers
/// `unchanged: true` without re-sending the image — agents poll while waiting on state
/// transitions and routinely re-read pixel-identical frames (e.g. a static view while
/// `game/state` changes underneath). Resource rather than `Local` because the handlers are
/// registered as plain systems.
#[derive(Resource, Default)]
struct LastServedCapture {
    hash: Option<u64>,
    path: Option<std::path::PathBuf>,
}

/// `game/screenshot/get` — polls the newest capture: `{"ready": true, "png_base64": …, "path":
/// …, "state": …}` once a PNG is on disk, `{"ready": false}` while still rendering. If the
/// newest capture's pixels are identical to the last capture served in full, responds
/// `{"ready": true, "unchanged": true, "path", "state"}` WITHOUT `png_base64` — the agent
/// already has this exact image; the fresh `state` is still included since the world can
/// change under a static view. The response otherwise embeds the [`game_state_snapshot`]
/// ground truth, and the same snapshot is written
/// once to a `.json` sidecar beside the PNG (`<capture>.json`) so the human-browsable record
/// carries state too. The file is NOT consumed — captures persist in
/// `docs/playtests/dist/screenshots/` for human review.
fn screenshot_get_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let Some(path) = newest_screenshot() else {
        return Ok(json!({"ready": false}).into());
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(json!({"ready": false}).into()),
    };
    let state = game_state_snapshot(world);

    // Unchanged-frame suppression: PNG bytes are a deterministic function of the frame
    // (same encoder, same pixels → same bytes), so hashing the file hashes the frame.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&bytes, &mut hasher);
    let hash = std::hash::Hasher::finish(&hasher);
    let unchanged = world
        .get_resource::<LastServedCapture>()
        .is_some_and(|last| last.hash == Some(hash));
    if unchanged {
        return Ok(json!({
            "ready": true,
            "unchanged": true,
            "path": path.display().to_string(),
            "state": state,
        })
        .into());
    }
    if let Some(mut last) = world.get_resource_mut::<LastServedCapture>() {
        last.hash = Some(hash);
        last.path = Some(path.clone());
    }

    {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let sidecar = path.with_extension("json");
        if !sidecar.exists() {
            let record = json!({
                "screenshot": path.display().to_string(),
                "state": state,
            });
            if let Ok(text) = serde_json::to_string_pretty(&record) {
                let _ = std::fs::write(&sidecar, text);
            }
        }
        Ok(json!({
            "ready": true,
            "png_base64": encoded,
            "path": path.display().to_string(),
            "state": state,
        })
        .into())
    }
}

// ---------------------------------------------------------------------------
// Agent vision aids — everything here exists so a vision model reads LESS
// off the pixels: `game/ui` gives labeled rects + text instead of OCR, and
// the agent cursor makes the mocked pointer's position/hover observable.
// ---------------------------------------------------------------------------

/// `game/ui` — an accessibility-tree-style dump of the current UI: every laid-out node in
/// back-to-front render order with its rect **in screenshot pixel space** (the same space
/// `game/mouse`'s `move_to`/clicks consume), plus per-node text (own `Text` + descendant
/// `TextSpan`s), `Interaction` state, bevy_picking's real hovered-entity set, and the mocked
/// pointer's position. Turns "find the button in the image and guess its pixel center" into
/// "read the row, click its rect" — and doubles as ground truth for verifying text actually
/// rendered (a screenshot shows tofu/blank for missing fonts; this shows the string either
/// way, so disagreement between the two localizes the failure).
///
/// Node filtering: zero-size (`Display::None`/collapsed), invisible, and rotated nodes are
/// skipped (rotated = the billboard quads in `npc_ui_quad.rs`, whose axis-aligned rect would
/// be a lie). Text rows whose text is already included in a dumped interactive ancestor's
/// subtree (a button's label) are folded into that ancestor and not emitted twice. The agent
/// cursor's own overlay nodes are excluded. In headless mode only nodes targeting the
/// offscreen capture are dumped; windowed (feature on, real window) dumps everything.
fn ui_dump_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    Ok(ui_dump_snapshot(world).into())
}

/// The cursor overlay's root node — zero-size, absolutely positioned, follows the mocked
/// pointer. Bars are its children. Excluded from [`ui_dump_snapshot`].
#[derive(Component)]
struct AgentCursorRoot;

/// One crosshair arm of the [`AgentCursorRoot`] overlay.
#[derive(Component)]
struct AgentCursorBar;

/// Red: idle. Yellow: the pointer is over something (bevy_picking's `HoverMap` non-empty).
/// White: left button held.
const CURSOR_IDLE: Color = Color::srgb(1.0, 0.25, 0.25);
const CURSOR_HOVER: Color = Color::srgb(1.0, 0.85, 0.1);
const CURSOR_PRESSED: Color = Color::srgb(1.0, 1.0, 1.0);
/// Crosshair arm length / thickness, logical px (× the UI scale factor on screen).
const CURSOR_ARM: f32 = 14.0;
const CURSOR_THICK: f32 = 3.0;

/// Lazily spawns the agent cursor overlay iff headless mode is active (an
/// `OffscreenRenderTarget` exists — on a real desktop the OS cursor is already visible and an
/// extra crosshair would be noise), and despawns it if the target goes away. Polling rather
/// than an observer because the resource is inserted after plugin build (`main.rs`'s headless
/// branch), matching the repo's replication-arrival precedent for "can appear at any time".
fn spawn_agent_cursor_if_headless(
    offscreen: Option<Res<OffscreenRenderTarget>>,
    existing: Query<Entity, With<AgentCursorRoot>>,
    mut commands: Commands,
) {
    if offscreen.is_some() {
        if !existing.is_empty() {
            return;
        }
        let bar = |width: f32, height: f32| {
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-width / 2.0),
                    top: Val::Px(-height / 2.0),
                    width: Val::Px(width),
                    height: Val::Px(height),
                    ..default()
                },
                BackgroundColor(CURSOR_IDLE),
                Outline {
                    width: Val::Px(1.0),
                    offset: Val::ZERO,
                    color: Color::BLACK,
                },
                // The overlay must never be hit by bevy_picking — it would occlude the UI
                // under the pointer and break exactly the hover/click state it exists to show.
                Pickable::IGNORE,
                AgentCursorBar,
            )
        };
        commands
            .spawn((
                AgentCursorRoot,
                GlobalZIndex(i32::MAX),
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    // Offscreen until the first follow update knows the pointer position.
                    left: Val::Px(-1000.0),
                    top: Val::Px(-1000.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                },
            ))
            .with_children(|parent| {
                parent.spawn(bar(CURSOR_ARM, CURSOR_THICK));
                parent.spawn(bar(CURSOR_THICK, CURSOR_ARM));
            });
    } else {
        for entity in &existing {
            commands.entity(entity).despawn();
        }
    }
}

/// Repositions the cursor overlay onto the mocked pointer and recolors it by hover/press
/// state. `PointerLocation.position` and `ComputedNode`'s rect math are both in physical
/// render-target pixels, but `Val::Px` resolves through the layout scale factor (target scale
/// × `UiScale`) — hence the round-trip through the node's own `inverse_scale_factor`, the
/// same factor `ui_layout_system` derived, so the overlay lands exactly on the pointer
/// whatever the scale.
fn update_agent_cursor(
    offscreen: Option<Res<OffscreenRenderTarget>>,
    images: Option<Res<Assets<Image>>>,
    ui_scale: Res<UiScale>,
    pointers: Query<(&bevy::picking::pointer::PointerId, &bevy::picking::pointer::PointerLocation)>,
    hover: Option<Res<bevy::picking::hover::HoverMap>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut roots: Query<
        (&mut Node, Option<&ComputedNode>),
        (With<AgentCursorRoot>, Without<AgentCursorBar>),
    >,
    mut bars: Query<&mut BackgroundColor, With<AgentCursorBar>>,
) {
    if offscreen.is_none() {
        return;
    }
    let Ok((mut root_node, computed)) = roots.single_mut() else {
        return;
    };

    let physical = pointers
        .iter()
        .find(|(id, _)| **id == AGENT_POINTER)
        .and_then(|(_, loc)| loc.location.as_ref())
        .map(|location| location.position)
        // Same fallback `current_pointer_location` uses: the offscreen target's center.
        .unwrap_or_else(|| {
            offscreen
                .as_ref()
                .zip(images.as_ref())
                .and_then(|(target, images)| images.get(&target.0))
                .map(|image| image.size().as_vec2() / 2.0)
                .unwrap_or(Vec2::new(640.0, 400.0))
        });

    let inverse = computed
        .and_then(|node| (node.inverse_scale_factor > 0.0).then_some(node.inverse_scale_factor))
        .unwrap_or_else(|| ui_scale.0.recip());
    let logical = physical * inverse;
    if root_node.left != Val::Px(logical.x) || root_node.top != Val::Px(logical.y) {
        root_node.left = Val::Px(logical.x);
        root_node.top = Val::Px(logical.y);
    }

    let hovered = hover
        .as_deref()
        .and_then(|map| map.0.get(&AGENT_POINTER))
        .is_some_and(|hits| !hits.is_empty());
    let color = if mouse.pressed(MouseButton::Left) {
        CURSOR_PRESSED
    } else if hovered {
        CURSOR_HOVER
    } else {
        CURSOR_IDLE
    };
    for mut background in &mut bars {
        if background.0 != color {
            background.0 = color;
        }
    }
}

/// The [`ui_dump_method`] payload. See that function's doc comment for the semantics.
fn ui_dump_snapshot(world: &mut World) -> serde_json::Value {
    let offscreen = world
        .get_resource::<OffscreenRenderTarget>()
        .map(|target| target.0.clone());

    // Headless: only nodes whose UI camera renders into the capture target. The vec is empty
    // (== unfiltered) in windowed mode, where every camera's node is fair game.
    let target_cameras: Vec<Entity> = match &offscreen {
        Some(handle) => {
            let mut cameras = world.query::<(Entity, &RenderTarget)>();
            cameras
                .iter(world)
                .filter(|(_, target)| {
                    target
                        .as_image()
                        .is_some_and(|image| image.id() == handle.id())
                })
                .map(|(entity, _)| entity)
                .collect()
        }
        None => Vec::new(),
    };

    let target_size = offscreen.as_ref().and_then(|handle| {
        world
            .get_resource::<Assets<Image>>()
            .and_then(|images| images.get(handle))
            .map(|image| image.size())
    });

    let pointer = offscreen
        .as_ref()
        .map(|handle| current_pointer_location(world, handle));
    let hovered_entities: Vec<Entity> = world
        .get_resource::<bevy::picking::hover::HoverMap>()
        .and_then(|map| map.0.get(&AGENT_POINTER))
        .map(|hits| hits.keys().copied().collect())
        .unwrap_or_default();

    // UiStack order IS render order (back-to-front) — the dump inherits it, so the model can
    // reason about occlusion ("a later row draws on top of an earlier one").
    let stack = world.resource::<UiStack>().uinodes.clone();

    let mut nodes = world.query_filtered::<(
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiTargetCamera,
        Option<&InheritedVisibility>,
        Has<UiWidgetsButton>,
        Has<FeathersButton>,
        Has<UiPressed>,
        Option<&PickHovered>,
    ), (
        Without<AgentCursorRoot>,
        Without<AgentCursorBar>,
    )>();
    let mut texts = world.query::<&Text>();
    let mut spans = world.query::<&TextSpan>();
    let mut children = world.query::<&Children>();
    let mut parents = world.query::<&ChildOf>();

    // First pass: raw rows in stack order. Interactive detection uses the widget markers this
    // project's UI actually carries — `bevy_ui_widgets::Button`/`FeathersButton` (every menu,
    // lobby, and selector button) — NOT bevy_ui's `Interaction`, which feathers buttons don't
    // carry (confirmed via `world.list_components` on a live `FeathersButton`). The hand-rolled
    // `widgets::button()` carries no marker at all (picking observers only) — those rows still
    // appear with their labels, just without a clickable flag.
    struct Row {
        entity: Entity,
        rect: [i32; 4],
        clickable: bool,
        interaction: Option<String>,
        text: Option<String>,
    }
    let mut rows = Vec::new();
    for &entity in &stack {
        let Ok((node, transform, target, visibility, ui_button, feathers, pressed, hovered)) =
            nodes.get(world, entity)
        else {
            continue;
        };
        let size = node.size();
        if size == Vec2::ZERO {
            continue;
        }
        if visibility.is_some_and(|visibility| !visibility.get()) {
            continue;
        }
        if !target_cameras.is_empty()
            && !target
                .get()
                .is_some_and(|camera| target_cameras.contains(&camera))
        {
            continue;
        }
        // An axis-aligned rect of a rotated node would be a lie; the only rotated UI in this
        // project is the billboard NPC quads on their own texture cameras anyway.
        let (_, angle, translation) = transform.to_scale_angle_translation();
        if angle.abs() > 0.01 {
            continue;
        }
        let clickable = ui_button || feathers;
        let interaction = if pressed {
            Some("Pressed".to_owned())
        } else if hovered.is_some_and(|hovered| hovered.0) {
            Some("Hovered".to_owned())
        } else if clickable {
            Some("Idle".to_owned())
        } else {
            None
        };
        // Clickable rows aggregate their subtree text (that's the button's label); plain
        // containers show only their own text — otherwise the root panel row would repeat
        // every label on screen.
        let text = if clickable {
            subtree_text(entity, &*world, &mut texts, &mut spans, &mut children)
        } else {
            direct_text(entity, &*world, &mut texts, &mut spans)
        };
        rows.push(Row {
            entity,
            rect: [
                (translation.x - size.x / 2.0).round() as i32,
                (translation.y - size.y / 2.0).round() as i32,
                size.x.round() as i32,
                size.y.round() as i32,
            ],
            clickable,
            interaction,
            text,
        });
    }

    // Second pass: fold a non-clickable row away if it sits under a dumped clickable ancestor
    // (the button-label case — its text is already in the button's row).
    let clickable: HashSet<Entity> = rows
        .iter()
        .filter(|row| row.clickable)
        .map(|row| row.entity)
        .collect();
    let mut covered_by_ancestor = |entity: Entity| -> bool {
        let mut current = entity;
        for _ in 0..16 {
            let Ok(parent) = parents.get(world, current) else {
                return false;
            };
            current = parent.0;
            if clickable.contains(&current) {
                return true;
            }
        }
        false
    };
    rows.retain(|row| row.clickable || !covered_by_ancestor(row.entity));

    let nodes_json: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut entry = serde_json::Map::new();
            entry.insert("entity".into(), json!(row.entity));
            entry.insert("rect".into(), json!(row.rect));
            if row.clickable {
                entry.insert("clickable".into(), json!(true));
            }
            if let Some(text) = &row.text {
                entry.insert("text".into(), json!(text));
            }
            if let Some(interaction) = &row.interaction {
                entry.insert("interaction".into(), json!(interaction));
            }
            if hovered_entities.contains(&row.entity) {
                entry.insert("pointer_hovered".into(), json!(true));
            }
            serde_json::Value::Object(entry)
        })
        .collect();

    json!({
        "note": "Nodes in back-to-front render order. rect = [x, y, w, h] in screenshot pixel space — the same coordinates game/mouse move_to consumes. `clickable: true` = a real button (safe to click). `text` is the node's own text (buttons: their whole label). `interaction`: Pressed | Hovered | Idle. `pointer_hovered`: the mocked pointer is over this node right now.",
        "target_size": target_size.map(|size| json!([size.x, size.y])),
        "pointer": pointer.map(|location| {
            json!({
                "x": location.position.x.round() as i32,
                "y": location.position.y.round() as i32,
            })
        }),
        "hovered_entities": hovered_entities,
        "nodes": nodes_json,
    })
}

/// The text carried by `entity`'s whole subtree: its own `Text` plus every descendant's
/// `TextSpan`/`Text`, in child order, joined with spaces. `None` when the subtree carries no
/// non-empty text.
fn subtree_text(
    entity: Entity,
    world: &World,
    texts: &mut QueryState<&Text>,
    spans: &mut QueryState<&TextSpan>,
    children: &mut QueryState<&Children>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Ok(text) = texts.get(world, entity) {
        let text = text.trim();
        if !text.is_empty() {
            parts.push(text.to_owned());
        }
    }
    if let Ok(spans) = spans.get(world, entity) {
        let text = spans.trim();
        if !text.is_empty() {
            parts.push(text.to_owned());
        }
    }
    if let Ok(kids) = children.get(world, entity) {
        for kid in kids.iter() {
            if let Some(more) = subtree_text(kid, world, texts, spans, children) {
                parts.push(more);
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Only the text components on `entity` itself — no descent. Used for non-clickable rows so a
/// container node doesn't repeat every descendant label (buttons aggregate via
/// [`subtree_text`] instead).
fn direct_text(
    entity: Entity,
    world: &World,
    texts: &mut QueryState<&Text>,
    spans: &mut QueryState<&TextSpan>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Ok(text) = texts.get(world, entity) {
        let text = text.trim();
        if !text.is_empty() {
            parts.push(text.to_owned());
        }
    }
    if let Ok(span) = spans.get(world, entity) {
        let text = span.trim();
        if !text.is_empty() {
            parts.push(text.to_owned());
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// `game/input` — inject player input for `ticks` fixed ticks (1–600) by mocking one of the
/// replicated ahoy action entities. The mocked value rides the exact same BEI → replicated-BEI
/// → server-sim → corrected-prediction path a gamepad does, so agent playtesting exercises the
/// real netcode.
///
/// Params:
/// - `action`: `"movement"` (`x` = strafe right, `y` = forward, both −1..1, like a stick),
///   `"jump"` (`down` = press-hold, releases on tick expiry), or `"rotate"` (`yaw_delta` /
///   `pitch_delta` in **radians** per tick — pre-scales into the mouse binding's pixels)
/// - `ticks`: how many fixed ticks (16.7ms each) the input stays active
fn input_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    use bevy_ahoy::input::{Jump as AhoyJump, Movement as AhoyMovement, RotateCamera as AhoyRotate};

    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let action = params
        .get("action")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.action"))?;
    let ticks = params
        .get("ticks")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 600) as u32;

    let Some(player) = world.get_resource::<LocalPlayer>().and_then(|lp| lp.0) else {
        return Err(BrpError::internal("no local player connected"));
    };
    let Ok(player_entity) = world.get_entity(player) else {
        return Err(BrpError::internal("local player despawned"));
    };
    let Some(actions) = player_entity.get::<Actions<PlayerInputContext>>() else {
        return Err(BrpError::internal(
            "player has no replicated input context yet",
        ));
    };

    // Collect the target action entities first: `player_entity` borrows `world` immutably, and
    // the mock insert below needs `&mut World` (via `get_entity_mut`).
    let action_entities: Vec<Entity> = actions.iter().collect();
    drop(player_entity);

    let mut mocked = None;
    for action_entity in action_entities {
        let Ok(mut action_entity_mut) = world.get_entity_mut(action_entity) else {
            continue;
        };
        match action {
            "movement" => {
                if action_entity_mut.get::<Action<AhoyMovement>>().is_some() {
                    let x = params.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                    let y = params.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                    action_entity_mut.insert(ActionMock::new(
                        TriggerState::Fired,
                        ActionValue::Axis2D(Vec2::new(x, y)),
                        MockSpan::Updates(ticks),
                    ));
                    mocked = Some(action_entity);
                }
            }
            "jump" => {
                if action_entity_mut.get::<Action<AhoyJump>>().is_some() {
                    let down = params.get("down").and_then(serde_json::Value::as_bool).unwrap_or(true);
                    action_entity_mut.insert(ActionMock::new(
                        TriggerState::Fired,
                        ActionValue::Bool(down),
                        MockSpan::Updates(ticks),
                    ));
                    mocked = Some(action_entity);
                }
            }
            "rotate" => {
                if action_entity_mut.get::<Action<AhoyRotate>>().is_some() {
                    let yaw_delta = params.get("yaw_delta").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                    let pitch_delta = params.get("pitch_delta").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                    // The mock BYPASSES the binding's `Scale` modifier (mocks replace the
                    // whole binding-evaluation step), so the value is radians DIRECTLY — no
                    // pixel-equivalent pre-scaling. And BEI fires the action on EVERY tick the
                    // mock is active, so the value is a PER-TICK RATE: `ticks` total is
                    // `ticks * value`. Divide the requested total turn by the tick count.
                    // `rotate_camera`'s `delta_yaw = -value.x` makes positive yaw_delta turn
                    // right (Bevy yaw decreases clockwise); positive pitch_delta looks DOWN
                    // (look_pitch reads positive when looking up — verified visually).
                    action_entity_mut.insert(ActionMock::new(
                        TriggerState::Fired,
                        ActionValue::Axis2D(Vec2::new(
                            yaw_delta / ticks as f32,
                            pitch_delta / ticks as f32,
                        )),
                        MockSpan::Updates(ticks),
                    ));
                    mocked = Some(action_entity);
                }
            }
            other => {
                return Err(BrpError::internal(&format!(
                    "unknown action {other:?} (expected movement|jump|rotate)"
                )));
            }
        }
        if mocked.is_some() {
            break;
        }
    }

    match mocked {
        Some(entity) => Ok(json!({"mocked_action_entity": entity, "ticks": ticks}).into()),
        None => Err(BrpError::internal("no matching action entity found")),
    }
}

/// Marks the one synthetic gamepad entity `gamepad_method` mocks input on — spawned lazily on
/// first use, not at `Startup`, so a session that never touches `game/gamepad` never pays for
/// it. Distinct from [`Gamepad`] itself only so this entity is unambiguously identifiable
/// (`world.query`-able) as agent-injected rather than a real, `bevy_gilrs`-detected controller —
/// nothing reads this marker at runtime.
#[derive(Component)]
struct AgentVirtualGamepad;

/// `game/gamepad` — mocks a real gamepad's button/axis state directly (`bevy_input::gamepad::
/// Gamepad`'s `analog` field — see the note in the `"button"` match arm below for why buttons
/// go through this too, not the seemingly-obvious `digital`/`ButtonInput` field — on a
/// synthetic, agent-owned gamepad entity), rather than `game/input`'s action-level `ActionMock`.
/// This is the load-bearing difference: the injected
/// state flows through `bevy_enhanced_input`'s *real* binding resolution (dead zones, `Scale`
/// modifiers, `require_reset`, which action currently owns a shared physical input, …) exactly
/// like a human's controller does, rather than skipping straight to "this action fired with this
/// value." Confirmed by testing (`bevy_enhanced_input-0.26.0/src/context.rs`'s `GamepadDevice`):
/// contexts that don't explicitly set a `GamepadDevice` component default to `GamepadDevice::Any`
/// ("input will be read from all connected gamepads") — none of this project's contexts set one,
/// so a bare `Gamepad` component on any entity, real controller or not, is read identically. This
/// is what makes UI navigation (`ui/ui.rs`'s `MenuControls` — the pause menu, main menu, level
/// picker, everything `game/input` categorically can't reach since it only knows the three ahoy
/// gameplay actions) actually testable: the exact same button/stick state a human's controller
/// would report drives the exact same `Press`/`Axial` bindings, `UiNavigate`/`UiConfirm` actions,
/// and `on_ui_navigate`/`on_ui_confirm` observers.
///
/// Gamepad button/stick state is level-triggered on a real controller (held until physically
/// released), not duration-based like `game/input`'s `ticks` — so this mirrors that instead of
/// introducing a separate auto-expiry mechanism: every `press`/`release`/`set_axis` call is a
/// direct, persistent state change the caller is responsible for undoing (release what you
/// press). `{"input": "reset"}` clears all button/axis state in one call — cheap insurance
/// against a forgotten release leaving an input stuck for the rest of the session; reach for it
/// between unrelated test scenarios rather than trying to track exactly what's still held.
///
/// Params:
/// - `{"input": "button", "button": "South", "pressed": true}` — press or release one of the 19
///   standard `GamepadButton` variants (see `parse_gamepad_button`'s match arms for the exact
///   names; `Other(u8)` isn't exposed, this project doesn't bind it anywhere).
/// - `{"input": "axis", "axis": "LeftStickX", "value": 0.8}` — set one of the 6 standard
///   `GamepadAxis` variants to a value in roughly −1.0..1.0 (see `parse_gamepad_axis`).
/// - `{"input": "reset"}` — release every button and zero every axis on the virtual gamepad.
fn gamepad_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let input = params
        .get("input")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.input"))?;

    // Lazily find-or-spawn the one virtual gamepad entity. Not `Startup`-spawned: see
    // `AgentVirtualGamepad`'s doc comment.
    let gamepad_entity = {
        let mut query = world.query_filtered::<Entity, With<AgentVirtualGamepad>>();
        match query.single(world) {
            Ok(entity) => entity,
            Err(_) => world.spawn((Gamepad::default(), AgentVirtualGamepad)).id(),
        }
    };
    let mut gamepad = world
        .get_mut::<Gamepad>(gamepad_entity)
        .ok_or_else(|| BrpError::internal("virtual gamepad entity has no Gamepad component"))?;

    match input {
        "button" => {
            let button_name = params
                .get("button")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| BrpError::internal("missing params.button"))?;
            let button = parse_gamepad_button(button_name)
                .ok_or_else(|| BrpError::internal(&format!("unknown button {button_name:?}")))?;
            let pressed = params
                .get("pressed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            // `bevy_enhanced_input`'s `Binding::GamepadButton` reader reads `Gamepad::get`,
            // i.e. the `analog` map — NOT `digital`/`ButtonInput` — confirmed by testing (a
            // first attempt using `digital_mut().press()` compiled fine, but the modal-menu
            // gamepad-Start test below never opened the modal at all) and by reading
            // `bevy_enhanced_input-0.26.0/src/context/input_reader.rs`'s `Binding::GamepadButton`
            // arm directly. `1.0`/`0.0` here is what a real button reports through this same
            // path — Bevy's own button-axis convention, not something specific to this project.
            gamepad.analog_mut().set(button, if pressed { 1.0 } else { 0.0 });
            Ok(json!({"gamepad_entity": gamepad_entity, "button": button_name, "pressed": pressed}).into())
        }
        "axis" => {
            let axis_name = params
                .get("axis")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| BrpError::internal("missing params.axis"))?;
            let axis = parse_gamepad_axis(axis_name)
                .ok_or_else(|| BrpError::internal(&format!("unknown axis {axis_name:?}")))?;
            let value = params
                .get("value")
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| BrpError::internal("missing params.value"))? as f32;
            gamepad.analog_mut().set(axis, value);
            Ok(json!({"gamepad_entity": gamepad_entity, "axis": axis_name, "value": value}).into())
        }
        "reset" => {
            *gamepad = Gamepad::default();
            Ok(json!({"gamepad_entity": gamepad_entity, "reset": true}).into())
        }
        other => Err(BrpError::internal(&format!(
            "unknown input {other:?} (expected button|axis|reset)"
        ))),
    }
}

fn parse_gamepad_button(name: &str) -> Option<GamepadButton> {
    Some(match name {
        "South" => GamepadButton::South,
        "East" => GamepadButton::East,
        "North" => GamepadButton::North,
        "West" => GamepadButton::West,
        "C" => GamepadButton::C,
        "Z" => GamepadButton::Z,
        "LeftTrigger" => GamepadButton::LeftTrigger,
        "LeftTrigger2" => GamepadButton::LeftTrigger2,
        "RightTrigger" => GamepadButton::RightTrigger,
        "RightTrigger2" => GamepadButton::RightTrigger2,
        "Select" => GamepadButton::Select,
        "Start" => GamepadButton::Start,
        "Mode" => GamepadButton::Mode,
        "LeftThumb" => GamepadButton::LeftThumb,
        "RightThumb" => GamepadButton::RightThumb,
        "DPadUp" => GamepadButton::DPadUp,
        "DPadDown" => GamepadButton::DPadDown,
        "DPadLeft" => GamepadButton::DPadLeft,
        "DPadRight" => GamepadButton::DPadRight,
        _ => return None,
    })
}

fn parse_gamepad_axis(name: &str) -> Option<GamepadAxis> {
    Some(match name {
        "LeftStickX" => GamepadAxis::LeftStickX,
        "LeftStickY" => GamepadAxis::LeftStickY,
        "LeftZ" => GamepadAxis::LeftZ,
        "RightStickX" => GamepadAxis::RightStickX,
        "RightStickY" => GamepadAxis::RightStickY,
        "RightZ" => GamepadAxis::RightZ,
        _ => return None,
    })
}

/// `game/keyboard` — mocks real keyboard key state directly on `ButtonInput<KeyCode>`, the
/// same resource `bevy_enhanced_input`'s `Binding::Keyboard` reads
/// (`bevy_enhanced_input-0.26.0/src/context/input_reader.rs:79-86`, `keys.pressed(key)` — a
/// plain resource, no per-device entity needed the way `Gamepad` is, so unlike `game/gamepad`
/// there's nothing to lazily spawn here). Level-triggered like a real key: held until released.
///
/// Params:
/// - `{"key": "KeyW", "pressed": true}` — press or release a `KeyCode` by its exact Rust variant
///   name, deserialized directly via `KeyCode`'s own `serde` impl (this project's `bevy` already
///   enables the `serialize` feature) rather than a hand-maintained name list — every one of
///   Bevy's 160+ variants works, not just a hand-picked subset (e.g. `KeyA`..`KeyZ`,
///   `Digit0`..`Digit9`, `Escape`, `Space`, `Enter`, `Tab`, `ArrowUp`/`Down`/`Left`/`Right`,
///   `ShiftLeft`/`Right`, `ControlLeft`/`Right`, `AltLeft`/`Right`).
/// - `{"reset": true}` — release every currently-pressed key.
fn keyboard_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    if params.get("reset").and_then(serde_json::Value::as_bool) == Some(true) {
        keys.release_all();
        return Ok(json!({"reset": true}).into());
    }
    let key_name = params
        .get("key")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.key"))?;
    let key: KeyCode = serde_json::from_value(json!(key_name))
        .map_err(|err| BrpError::internal(&format!("unknown key {key_name:?}: {err}")))?;
    let pressed = params
        .get("pressed")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if pressed {
        keys.press(key);
    } else {
        keys.release(key);
    }
    Ok(json!({"key": key_name, "pressed": pressed}).into())
}

/// The pointer this whole method drives. **Not a synthetic entity we spawn** — unlike
/// `game/gamepad`'s virtual `Gamepad`, `bevy_picking`'s own `spawn_mouse_pointer` (part of its
/// default plugin set) unconditionally spawns a `PointerId::Mouse` entity at `Startup`
/// regardless of whether a window exists (confirmed by reading
/// `bevy_picking-0.19.0/src/input.rs:115-118`) — headless mode already has a real mouse pointer
/// entity, it just never receives real `PointerInput` events (those come from `WindowEvent`s
/// this mode never gets, since there's no window). This project's own screenshot mechanism
/// aside, `PointerId::Custom(Uuid)` exists in `bevy_picking` specifically "for mocking inputs",
/// but reusing the real `PointerId::Mouse` is simpler (no new `uuid` dependency, no lazy-spawn
/// bookkeeping) and arguably more faithful — it *is* the mouse, not a stand-in for it.
const AGENT_POINTER: bevy::picking::pointer::PointerId = bevy::picking::pointer::PointerId::Mouse;

/// The pointer's current position, read back from its own `PointerLocation` component so a
/// relative `"motion"` move (below) computes the right new absolute position — falls back to
/// the offscreen target's center (640, 400 for the 1280×800 Steam-Deck target) if nothing has
/// moved it yet this session.
fn current_pointer_location(
    world: &mut World,
    offscreen: &Handle<Image>,
) -> bevy::picking::pointer::Location {
    use bevy::picking::pointer::{Location, PointerLocation};
    let target = bevy::camera::NormalizedRenderTarget::Image(offscreen.clone().into());
    let mut query = world.query::<(&bevy::picking::pointer::PointerId, &PointerLocation)>();
    query
        .iter(world)
        .find(|(id, _)| **id == AGENT_POINTER)
        .and_then(|(_, loc)| loc.location.clone())
        .unwrap_or(Location {
            target,
            position: offscreen_center(world),
        })
}

/// `game/mouse` — mocks real mouse button/motion/wheel/cursor-position state. Buttons and
/// motion/wheel deltas go through the same plain resources `game/keyboard`'s doc comment
/// describes for keyboard (`ButtonInput<MouseButton>`, `AccumulatedMouseMotion`,
/// `AccumulatedMouseScroll` — all three read directly by `bevy_enhanced_input`'s
/// `Binding::MouseButton`/`MouseMotion`/`MouseWheel`, confirmed via the same
/// `input_reader.rs` read that found `game/gamepad`'s analog-not-digital gotcha). Cursor
/// position and clicks go through `bevy_picking`'s real event pipeline instead
/// (`PointerInput`/`PointerAction` — see `AGENT_POINTER`'s doc comment) since UI hover/click
/// hit-testing needs a *position*, which the button/motion resources above don't carry — this
/// is the mechanism that makes clicking an actual UI button (as opposed to just a raw mouse-
/// bound gameplay action) possible at all through this tool.
///
/// A real mouse click drives both mechanisms simultaneously in real life (`ButtonInput` for
/// direct mouse-bound gameplay bindings, `PointerInput` for UI hit-testing), so `"button"`
/// below updates both at once for `Left`/`Right`/`Middle` (mapped to `PointerButton`'s
/// `Primary`/`Secondary`/`Middle` — `Back`/`Forward`/`Other` update `ButtonInput` only, since
/// `PointerButton` has no equivalent).
///
/// Params:
/// - `{"input":"button","button":"Left","pressed":true}` — `Left`/`Right`/`Middle`/`Back`/
///   `Forward` (matches `MouseButton`). Level-triggered: held until an explicit
///   `pressed:false` call.
/// - `{"input":"motion","dx":10,"dy":-5}` — a *relative* delta, matching real mouse-motion
///   events: sets this tick's `AccumulatedMouseMotion` (for camera-look-style bindings) and
///   also moves the tracked cursor position by the same delta (for hover), mirroring how a
///   single physical mouse movement feeds both systems in reality regardless of which one a
///   given game state is actually listening to.
/// - `{"input":"move_to","x":640,"y":400}` — sets the cursor to an *absolute* position in the
///   same 1280×800 pixel space `game/screenshot` captures, for UI hover/click testing when you
///   already know where something is from a screenshot. Doesn't touch
///   `AccumulatedMouseMotion` — this is a convenience teleport, not a simulated drag.
/// - `{"input":"wheel","x":0,"y":1,"unit":"Line"}` — `unit` is `Line` (default) or `Pixel`,
///   matching `MouseScrollUnit`.
/// - `{"input":"reset"}` — releases every mouse button (both mechanisms) and zeros the motion/
///   wheel accumulators. Doesn't move the cursor back to center.
fn mouse_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
    use bevy::input::touch::TouchPhase;
    use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerInput};

    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let input = params
        .get("input")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.input"))?;
    let offscreen = world
        .get_resource::<OffscreenRenderTarget>()
        .map(|target| target.0.clone());

    match input {
        "button" => {
            let button_name = params
                .get("button")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| BrpError::internal("missing params.button"))?;
            let button = parse_mouse_button(button_name)
                .ok_or_else(|| BrpError::internal(&format!("unknown button {button_name:?}")))?;
            let pressed = params
                .get("pressed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            {
                let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
                if pressed {
                    buttons.press(button);
                } else {
                    buttons.release(button);
                }
            }
            if let (Some(pointer_button), Some(offscreen)) =
                (mouse_button_to_pointer_button(button), offscreen)
            {
                let location = current_pointer_location(world, &offscreen);
                let action = if pressed {
                    PointerAction::Press(pointer_button)
                } else {
                    PointerAction::Release(pointer_button)
                };
                world.write_message(PointerInput::new(AGENT_POINTER, location, action));
            }
            Ok(json!({"button": button_name, "pressed": pressed}).into())
        }
        "motion" => {
            let dx = params.get("dx").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let dy = params.get("dy").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let delta = Vec2::new(dx, dy);
            // A direct `insert_resource(AccumulatedMouseMotion{..})` doesn't stick: Bevy's own
            // `accumulate_mouse_motion_system` (bevy_input-0.19.0/src/mouse.rs:259-268)
            // unconditionally overwrites this resource from `MouseMotion` events every frame
            // ("reset to zero every frame", per its own doc comment) — it ran before our BRP
            // write reached the world in one live test, wiping the value before
            // `bevy_enhanced_input`'s reader ever saw it (confirmed: `look_yaw` stayed exactly
            // 0.0 after a `dx: 200` call that should have turned the camera). Writing a real
            // `MouseMotion` event instead lets that system pick it up in its own scheduled slot,
            // whichever frame that lands on — the same mechanism a real winit mouse-delta uses.
            world.write_message(bevy::input::mouse::MouseMotion { delta });
            if let Some(offscreen) = offscreen {
                let mut location = current_pointer_location(world, &offscreen);
                location.position += delta;
                world.write_message(PointerInput::new(
                    AGENT_POINTER,
                    location,
                    PointerAction::Move { delta },
                ));
            }
            Ok(json!({"dx": dx, "dy": dy}).into())
        }
        "move_to" => {
            let Some(offscreen) = offscreen else {
                return Err(BrpError::internal("no OffscreenRenderTarget (desktop client?)"));
            };
            let x = params.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let y = params.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let previous = current_pointer_location(world, &offscreen);
            let position = Vec2::new(x, y);
            let location = Location {
                target: previous.target,
                position,
            };
            world.write_message(PointerInput::new(
                AGENT_POINTER,
                location,
                PointerAction::Move {
                    delta: position - previous.position,
                },
            ));
            Ok(json!({"x": x, "y": y}).into())
        }
        "wheel" => {
            let x = params.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let y = params.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            let unit_name = params
                .get("unit")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Line");
            let unit = match unit_name {
                "Line" => MouseScrollUnit::Line,
                "Pixel" => MouseScrollUnit::Pixel,
                other => {
                    return Err(BrpError::internal(&format!(
                        "unknown scroll unit {other:?} (expected Line|Pixel)"
                    )));
                }
            };
            // Same reasoning as `"motion"` above: a real `MouseWheel` event, not a direct
            // resource write, so `accumulate_mouse_scroll_system` computes
            // `AccumulatedMouseScroll` on its own terms. `window: Entity::PLACEHOLDER` is safe
            // here — that system never reads the field, it only exists for consumers that care
            // which window received the scroll, which headless mode has none of.
            world.write_message(bevy::input::mouse::MouseWheel {
                unit,
                x,
                y,
                window: Entity::PLACEHOLDER,
                phase: TouchPhase::Moved,
            });
            if let Some(offscreen) = offscreen {
                let location = current_pointer_location(world, &offscreen);
                world.write_message(PointerInput::new(
                    AGENT_POINTER,
                    location,
                    PointerAction::Scroll {
                        unit,
                        x,
                        y,
                        phase: TouchPhase::Moved,
                    },
                ));
            }
            Ok(json!({"x": x, "y": y, "unit": unit_name}).into())
        }
        "reset" => {
            world.resource_mut::<ButtonInput<MouseButton>>().release_all();
            if let Some(offscreen) = offscreen {
                let location = current_pointer_location(world, &offscreen);
                for button in [
                    PointerButton::Primary,
                    PointerButton::Secondary,
                    PointerButton::Middle,
                ] {
                    world.write_message(PointerInput::new(
                        AGENT_POINTER,
                        location.clone(),
                        PointerAction::Release(button),
                    ));
                }
            }
            world.insert_resource(AccumulatedMouseMotion { delta: Vec2::ZERO });
            world.insert_resource(AccumulatedMouseScroll {
                unit: MouseScrollUnit::Line,
                delta: Vec2::ZERO,
            });
            Ok(json!({"reset": true}).into())
        }
        other => Err(BrpError::internal(&format!(
            "unknown input {other:?} (expected button|motion|move_to|wheel|reset)"
        ))),
    }
}

fn parse_mouse_button(name: &str) -> Option<MouseButton> {
    Some(match name {
        "Left" => MouseButton::Left,
        "Right" => MouseButton::Right,
        "Middle" => MouseButton::Middle,
        "Back" => MouseButton::Back,
        "Forward" => MouseButton::Forward,
        _ => return None,
    })
}

fn mouse_button_to_pointer_button(
    button: MouseButton,
) -> Option<bevy::picking::pointer::PointerButton> {
    match button {
        MouseButton::Left => Some(bevy::picking::pointer::PointerButton::Primary),
        MouseButton::Right => Some(bevy::picking::pointer::PointerButton::Secondary),
        MouseButton::Middle => Some(bevy::picking::pointer::PointerButton::Middle),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

/// `game/trigger` — triggers the app's own client-local events, the same ones the menu buttons
/// fire. `connect` → the main menu's Connect (opens the lightyear connection); `play` → the
/// lobby's Play (sends `InGameRequest` via the client's own `MessageSender`); `disconnect` →
/// the lobby/menu's Main Menu + Disconnect.
fn trigger_method(params: In<Option<serde_json::Value>>, mut world: &mut World) -> BrpResult {
    use crate::events::{Connect, Disconnect};
    use shared::client_events::InGameRequest;
    use shared::replication::OrderedReliable;

    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let event = params
        .get("event")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.event"))?;
    match event {
        "connect" => {
            world.commands().trigger(Connect);
            Ok(json!({"triggered": "connect"}).into())
        }
        "disconnect" => {
            world.commands().trigger(Disconnect);
            Ok(json!({"triggered": "disconnect"}).into())
        }
        "play" => {
            // The lobby Play button's exact behavior: `InGameRequest` over the client's own
            // message sender.
            let mut sender = world
                .query::<&mut lightyear::prelude::MessageSender<InGameRequest>>()
                .iter_mut(world)
                .next()
                .ok_or_else(|| BrpError::internal("no MessageSender<InGameRequest> (not connected?)"))?;
            sender.send::<shared::replication::OrderedReliable>(InGameRequest);
            Ok(json!({"triggered": "play", "sent": "InGameRequest"}).into())
        }
        // The spawn hotkeys: client-local triggers whose observers wrap the player's current
        // aim/camera into the server request, so the cube/NPC appears where the player is
        // looking.
        "spawn_cube" => {
            world.commands().trigger(crate::events::SpawnCube);
            Ok(json!({"triggered": "spawn_cube"}).into())
        }
        "spawn_npc" => {
            world.commands().trigger(crate::events::SpawnNpc);
            Ok(json!({"triggered": "spawn_npc"}).into())
        }
        other => Err(BrpError::internal(&format!(
            "unknown event {other:?} (expected connect|play|disconnect)"
        ))),
    }
}

/// `game/levels` — lists the server-replicated `Levels` singleton (the same list the lobby's
/// level picker reads): asset paths + names, so an agent can pick a level by asset path.
fn levels_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    // `Levels` is a replicated COMPONENT on an entity (not a resource).
    let mut query = world.query::<&shared::level::Levels>();
    let Some(levels) = query.iter(world).next() else {
        return Err(BrpError::internal(
            "no Levels entity replicated yet (not in the lobby?)",
        ));
    };
    let list: Vec<serde_json::Value> = levels
        .iter()
        .map(|(asset_path, level)| {
            json!({
                "asset_path": asset_path.to_string(),
                "name": level.name,
            })
        })
        .collect();
    Ok(json!({"levels": list}).into())
}

/// `game/select_level` — sends `LoadLevelRequest { asset_path }` via the client's own message
/// sender, the same message the lobby's level picker sends after a selection. `asset_path`
/// comes from `game/levels` (e.g. `levels/minimal.level.ron`).
fn select_level_method(params: In<Option<serde_json::Value>>, mut world: &mut World) -> BrpResult {
    use shared::client_events::LoadLevelRequest;
    use shared::replication::OrderedReliable;

    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let asset_path = params
        .get("asset_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BrpError::internal("missing params.asset_path"))?;
    let mut sender = world
        .query::<&mut lightyear::prelude::MessageSender<LoadLevelRequest>>()
        .iter_mut(world)
        .next()
        .ok_or_else(|| BrpError::internal("no MessageSender<LoadLevelRequest> (not connected?)"))?;
    sender.send::<OrderedReliable>(LoadLevelRequest {
        asset_path: bevy::asset::AssetPath::parse(asset_path).into_owned(),
    });
    Ok(
        json!({"sent": "LoadLevelRequest", "asset_path": asset_path}).into(),
    )
}

// ---------------------------------------------------------------------------
// The in-process MCP server// ---------------------------------------------------------------------------
// The in-process MCP server (rmcp, Streamable HTTP, stateless) — the protocol surface whose
// tools proxy to the BRP methods above over loopback HTTP.
// ---------------------------------------------------------------------------

/// Binds the MCP surface. The port is `--mcp-port N` (default 15710) — NOT 15703, which is
/// `bevy_remote`'s **render-subapp BRP port** (`DEFAULT_RENDER_PORT`, active whenever
/// `bevy_render` runs): binding our MCP listener there made the render app's BRP bind fail and
/// the main BRP pipeline hang in release builds.
fn start_mcp_server(mcp_port: u16) {
    std::thread::Builder::new()
        .name("mcp-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("mcp server: tokio runtime should build");
            if let Err(err) = runtime.block_on(serve_mcp(mcp_port)) {
                error!("mcp server stopped: {err:?}");
            }
        })
        .expect("mcp server: thread should spawn");
}

async fn serve_mcp(mcp_port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };

    let session_manager: std::sync::Arc<rmcp::transport::streamable_http_server::session::local::LocalSessionManager> = Default::default();
    let service = StreamableHttpService::new(
        || Ok(GameTools::default()),
        session_manager,
        Default::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", mcp_port)).await?;
    info!("mcp tool server listening on http://127.0.0.1:{mcp_port}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}

/// The `inject_input` tool's parameters — rmcp generates the tool's JSON schema from this
/// (schemars), so the agent sees the documented shapes.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct InjectInputParams {
    /// Which action to mock: `movement`, `jump`, or `rotate`.
    pub action: String,
    /// `movement`: strafe right, −1..1 (like a stick).
    pub x: Option<f64>,
    /// `movement`: forward, −1..1 (like a stick).
    pub y: Option<f64>,
    /// `jump`: press-hold while the mock is active.
    pub down: Option<bool>,
    /// `rotate`: look-yaw delta in radians, spread across the ticks (positive = turn right).
    pub yaw_delta: Option<f64>,
    /// `rotate`: look-pitch delta in radians, spread across the ticks (positive = look up).
    pub pitch_delta: Option<f64>,
    /// How many fixed ticks (~16.7ms each) the input stays active. Default 1.
    pub ticks: Option<u32>,
}

/// The `gamepad_input` tool's parameters — see `gamepad_method`'s doc comment for the full
/// button/axis name lists and why this is a genuinely different mechanism from `inject_input`.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct GamepadInputParams {
    /// Which kind of input this call sets: `button`, `axis`, or `reset` (releases everything).
    pub input: String,
    /// `button`: one of the 19 standard `GamepadButton` names (`South`, `East`, `North`,
    /// `West`, `C`, `Z`, `LeftTrigger`, `LeftTrigger2`, `RightTrigger`, `RightTrigger2`,
    /// `Select`, `Start`, `Mode`, `LeftThumb`, `RightThumb`, `DPadUp`, `DPadDown`, `DPadLeft`,
    /// `DPadRight`).
    pub button: Option<String>,
    /// `button`: `true` to press (default), `false` to release. Held until you explicitly
    /// release it — this is level-triggered like a real controller, not duration-based.
    pub pressed: Option<bool>,
    /// `axis`: one of the 6 standard `GamepadAxis` names (`LeftStickX`, `LeftStickY`, `LeftZ`,
    /// `RightStickX`, `RightStickY`, `RightZ`).
    pub axis: Option<String>,
    /// `axis`: the value to set, roughly −1.0..1.0 for sticks.
    pub value: Option<f64>,
}

/// The `keyboard_input` tool's parameters — see `keyboard_method`'s doc comment.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct KeyboardInputParams {
    /// `true` to release every currently-pressed key, ignoring `key`/`pressed`.
    pub reset: Option<bool>,
    /// The exact Rust `KeyCode` variant name, e.g. `KeyW`, `Digit1`, `Escape`, `Space`, `Enter`,
    /// `Tab`, `ArrowUp`, `ShiftLeft`, `ControlLeft`, `AltLeft`.
    pub key: Option<String>,
    /// `true` to press (default), `false` to release. Held until you explicitly release it —
    /// level-triggered like a real key, not duration-based.
    pub pressed: Option<bool>,
}

/// The `mouse_input` tool's parameters — see `mouse_method`'s doc comment for the full
/// design (why cursor motion/clicks go through `bevy_picking`'s real event pipeline).
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct MouseInputParams {
    /// Which kind of input this call sets: `button`, `motion`, `move_to`, `wheel`, or `reset`.
    pub input: String,
    /// `button`: `Left`, `Right`, `Middle`, `Back`, or `Forward` (matches `MouseButton`).
    pub button: Option<String>,
    /// `button`: `true` to press (default), `false` to release. Held until released.
    pub pressed: Option<bool>,
    /// `motion`: relative X delta this call. `move_to`: absolute X in the 1280×800 screenshot
    /// pixel space. `wheel`: horizontal scroll amount.
    pub x: Option<f64>,
    /// `motion`: relative Y delta this call. `move_to`: absolute Y. `wheel`: vertical scroll
    /// amount (most wheels only use this one).
    pub y: Option<f64>,
    /// `motion`: alias for `x` (relative dx) — either name works, `dx`/`dy` mirror the BRP
    /// method's own param names exactly.
    pub dx: Option<f64>,
    /// `motion`: alias for `y` (relative dy).
    pub dy: Option<f64>,
    /// `wheel`: `Line` (default, one detent per unit) or `Pixel` (raw pixel scroll).
    pub unit: Option<String>,
}

/// The MCP tool surface — every tool is a thin proxy to a BRP method over loopback HTTP; all
/// `World` access lives in the BRP handlers above.
#[derive(Clone)]
struct GameTools {
    brp_url: String,
}

/// The `screenshot` tool's parameters — both optional; omit them for a full-frame capture.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ScreenshotParams {
    /// Optional filename label; the PNG lands as `<millistamp>-<label>.png` under
    /// docs/playtests/dist/screenshots/.
    pub label: Option<String>,
    /// Optional `[x, y, w, h]` sub-rect to capture, in the same screenshot pixel space game/ui
    /// dumps (e.g. a button's rect). A crop costs fewer vision tokens and keeps full effective
    /// resolution on the region of interest. Clamped to frame bounds.
    pub crop: Option<Vec<f64>>,
}

impl Default for GameTools {
    fn default() -> Self {
        Self {
            // Same port the BRP server binds (this client's own `--brp-port`): the MCP tools
            // proxy to *this* client's BRP over loopback.
            brp_url: format!("http://127.0.0.1:{}", crate::config::brp_port_presync()),
        }
    }
}

/// The tool definitions live in this block; the handlers proxy to BRP.
#[rmcp::tool_router]
impl GameTools {
    pub fn new() -> Self {
        Self::default()
    }

    /// One BRP JSON-RPC round-trip; unwraps the BRP envelope into the `result` (or maps the
    /// error into an MCP tool error).
    async fn brp(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, rmcp::ErrorData> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let response = reqwest::Client::new()
            .post(&self.brp_url)
            .json(&body)
            .send()
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(format!("brp unreachable: {err}"), None))?;
        let envelope: serde_json::Value = response
            .json()
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(format!("brp bad response: {err}"), None))?;
        if let Some(error) = envelope.get("error") {
            return Err(rmcp::ErrorData::internal_error(error.to_string(), None));
        }
        Ok(envelope
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    /// A snapshot of the game's state: app state, and the local player's entity, position,
    /// velocity, look angles, grounded/crouching flags, HP, GCD and death timers.
    #[rmcp::tool(description = "Snapshot of the current game state (app state + the local player's entity, position, velocity, look, grounded flag, HP, GCD/death timers). Call this before/after other tools to see what changed.")]
    async fn game_state(&self) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let result = self.brp("game/state", json!({})).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }

    /// Captures a screenshot of the game (PNG) and returns it as image content PLUS the game's
    /// ground-truth state as JSON. The capture is async (one render frame), so this polls
    /// `game/screenshot/get` briefly. Optional `crop` targets a region of interest.
    #[rmcp::tool(description = "Capture a screenshot of the game. Returns the PNG as image content PLUS the game's ground-truth state as JSON text (same payload as game_state), so you never need to read numbers off the HUD. Optional `crop` [x,y,w,h] captures just a region (read the rect off game/ui first) — cheaper and sharper than a full frame. A visible crosshair marks your mocked mouse cursor (red = idle, yellow = hovering, white = left held). If the response says unchanged:true, the pixels are IDENTICAL to the last image you were served — do not ask for it again; read the included state instead.")]
    async fn screenshot(
        &self,
        rmcp::handler::server::wrapper::Parameters(ScreenshotParams { label, crop }): rmcp::handler::server::wrapper::Parameters<ScreenshotParams>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let mut params = json!({});
        if let Some(label) = label {
            params["label"] = json!(label);
        }
        if let Some(crop) = crop {
            if crop.len() != 4 || crop.iter().any(|v| !v.is_finite() || *v < 0.0) {
                return Err(rmcp::ErrorData::invalid_params(
                    "crop must be [x, y, w, h] — four non-negative pixel numbers".to_owned(),
                    None,
                ));
            }
            params["crop"] = json!(crop);
        }
        self.brp("game/screenshot", params).await?;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let result = self.brp("game/screenshot/get", json!({})).await?;
            if result.get("ready").and_then(serde_json::Value::as_bool) != Some(true) {
                continue;
            }
            let state = result.get("state").cloned().unwrap_or(json!(null));
            let path = result.get("path").and_then(serde_json::Value::as_str).unwrap_or("");
            let state_json = serde_json::to_string_pretty(&json!({
                "path": path,
                "state": state,
            }))
            .map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?;
            if result.get("unchanged").and_then(serde_json::Value::as_bool) == Some(true) {
                return Ok(rmcp::model::CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(format!(
                        "unchanged: the newest capture has PIXEL-IDENTICAL content to the last image you were served — it is not attached again.\n{state_json}"
                    )),
                ]));
            }
            let png_base64 = result
                .get("png_base64")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| rmcp::ErrorData::internal_error("screenshot missing data", None))?;
            let mut blocks = vec![rmcp::model::ContentBlock::image(png_base64.to_string(), "image/png")];
            blocks.push(rmcp::model::ContentBlock::text(state_json));
            return Ok(rmcp::model::CallToolResult::success(blocks));
        }
        Err(rmcp::ErrorData::internal_error(
            "screenshot timed out (is the game rendering?)",
            None,
        ))
    }

    /// Dumps the UI tree: labeled rects + text for every visible UI node, in screenshot pixel
    /// space, so clicks can target coordinates read off this dump instead of guessed from
    /// pixels.
    #[rmcp::tool(description = "Dump the UI tree as an accessibility-tree-style list: every visible UI node's rect [x,y,w,h] in the SAME screenshot pixel space game/mouse move_to consumes, its text (button labels), and interaction/hover state. Read THIS to find what to click and where, then use game/mouse move_to + button Left to click it. Much more reliable than estimating coordinates from the screenshot image. Also useful to verify text rendered (the dump shows the string regardless of font issues).")]
    async fn ui_tree(&self) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let result = self.brp("game/ui", json!({})).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }

    /// Injects player input for `ticks` fixed ticks (1 tick ≈ 16.7ms at 60Hz).
    #[rmcp::tool(description = "Inject player input through the real input pipeline (the same replicated path a gamepad uses) for `ticks` fixed ticks (~16.7ms each). Actions: `movement` (x = strafe right −1..1, y = forward −1..1), `jump` (down = hold), `rotate` (yaw_delta / pitch_delta in RADIANS this call, spread across the ticks). Combine calls (e.g. movement + rotate across consecutive calls) to drive the character; call game_state after to observe the effect.")]
    async fn inject_input(
        &self,
        rmcp::handler::server::wrapper::Parameters(InjectInputParams { action, x, y, down, yaw_delta, pitch_delta, ticks }):
            rmcp::handler::server::wrapper::Parameters<InjectInputParams>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let params = match action.as_str() {
            "movement" => json!({"action": "movement", "x": x.unwrap_or(0.0), "y": y.unwrap_or(0.0)}),
            "jump" => json!({"action": "jump", "down": down.unwrap_or(true)}),
            "rotate" => json!({"action": "rotate", "yaw_delta": yaw_delta.unwrap_or(0.0), "pitch_delta": pitch_delta.unwrap_or(0.0)}),
            other => {
                return Err(rmcp::ErrorData::invalid_params(
                    format!("unknown action {other:?} (expected movement|jump|rotate)"),
                    None,
                ))
            }
        };
        let params = match ticks {
            Some(ticks) => {
                let mut params = params;
                params["ticks"] = json!(ticks);
                params
            }
            None => params,
        };
        let result = self.brp("game/input", params).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }

    /// Mocks real gamepad button/stick state (not an action-level mock — see the tool
    /// description). Held until explicitly released/reset.
    #[rmcp::tool(description = "Mock a real gamepad's button/stick state directly, so it flows through the actual binding/dead-zone/context resolution bevy_enhanced_input does for a human's controller — unlike inject_input, this reaches UI navigation (menus, the pause screen) too, not just the three gameplay actions. `input`: `button` (name one of the 19 standard GamepadButton names, `pressed` true/false — held until you release it, like a real controller, not duration-based), `axis` (name one of the 6 standard GamepadAxis names, `value` roughly -1..1), or `reset` (releases/zeros everything — use this between unrelated test scenarios so a forgotten release doesn't linger). Example: press Start to open the pause menu, then South to activate whatever's focused, then release both.")]
    async fn gamepad_input(
        &self,
        rmcp::handler::server::wrapper::Parameters(GamepadInputParams { input, button, pressed, axis, value }):
            rmcp::handler::server::wrapper::Parameters<GamepadInputParams>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let params = match input.as_str() {
            "button" => {
                let button = button.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing button for input=\"button\"", None)
                })?;
                json!({"input": "button", "button": button, "pressed": pressed.unwrap_or(true)})
            }
            "axis" => {
                let axis = axis.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing axis for input=\"axis\"", None)
                })?;
                let value = value.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing value for input=\"axis\"", None)
                })?;
                json!({"input": "axis", "axis": axis, "value": value})
            }
            "reset" => json!({"input": "reset"}),
            other => {
                return Err(rmcp::ErrorData::invalid_params(
                    format!("unknown input {other:?} (expected button|axis|reset)"),
                    None,
                ))
            }
        };
        let result = self.brp("game/gamepad", params).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }

    /// Mocks real keyboard key state (`ButtonInput<KeyCode>`). Held until explicitly
    /// released/reset.
    #[rmcp::tool(description = "Mock a real keyboard key, held until released — the same ButtonInput<KeyCode> resource bevy_enhanced_input's Binding::Keyboard reads for a physical key. `key` is the exact Rust KeyCode variant name (KeyW, KeyA..KeyZ, Digit0..Digit9, Escape, Space, Enter, Tab, ArrowUp/Down/Left/Right, ShiftLeft/Right, ControlLeft/Right, AltLeft/Right, etc — every KeyCode variant works). `pressed` true (default) or false. Or pass `reset: true` to release every held key.")]
    async fn keyboard_input(
        &self,
        rmcp::handler::server::wrapper::Parameters(KeyboardInputParams { reset, key, pressed }):
            rmcp::handler::server::wrapper::Parameters<KeyboardInputParams>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let params = if reset == Some(true) {
            json!({"reset": true})
        } else {
            let key = key.ok_or_else(|| {
                rmcp::ErrorData::invalid_params("missing key (or pass reset: true)", None)
            })?;
            json!({"key": key, "pressed": pressed.unwrap_or(true)})
        };
        let result = self.brp("game/keyboard", params).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }

    /// Mocks real mouse button/motion/wheel state plus cursor position, driving both raw input
    /// resources and `bevy_picking`'s real event pipeline so UI clicks/hover work too.
    #[rmcp::tool(description = "Mock real mouse input — buttons, motion, absolute cursor position, and wheel — through both the raw ButtonInput<MouseButton>/AccumulatedMouseMotion/AccumulatedMouseScroll resources bevy_enhanced_input reads AND bevy_picking's real PointerInput event pipeline, so this can click actual UI buttons (not just drive mouse-bound gameplay actions). `input`: `button` (`button`: Left|Right|Middle|Back|Forward, `pressed` true/default or false — held until released), `motion` (`dx`/`dy` relative delta, like a mouse-look turn), `move_to` (`x`/`y` absolute position in the 1280x800 screenshot pixel space — use this to click something you can see at a known pixel from a screenshot), `wheel` (`x`/`y` scroll amount, `unit`: Line|Pixel), `reset` (releases all buttons, zeros motion/scroll — does not recenter the cursor). To click a UI button: move_to its position, then button press, then button release.")]
    async fn mouse_input(
        &self,
        rmcp::handler::server::wrapper::Parameters(MouseInputParams { input, button, pressed, x, y, dx, dy, unit }):
            rmcp::handler::server::wrapper::Parameters<MouseInputParams>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let params = match input.as_str() {
            "button" => {
                let button = button.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing button for input=\"button\"", None)
                })?;
                json!({"input": "button", "button": button, "pressed": pressed.unwrap_or(true)})
            }
            "motion" => {
                json!({
                    "input": "motion",
                    "dx": dx.or(x).unwrap_or(0.0),
                    "dy": dy.or(y).unwrap_or(0.0),
                })
            }
            "move_to" => {
                let x = x.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing x for input=\"move_to\"", None)
                })?;
                let y = y.ok_or_else(|| {
                    rmcp::ErrorData::invalid_params("missing y for input=\"move_to\"", None)
                })?;
                json!({"input": "move_to", "x": x, "y": y})
            }
            "wheel" => {
                json!({
                    "input": "wheel",
                    "x": x.unwrap_or(0.0),
                    "y": y.unwrap_or(0.0),
                    "unit": unit.unwrap_or_else(|| "Line".to_string()),
                })
            }
            "reset" => json!({"input": "reset"}),
            other => {
                return Err(rmcp::ErrorData::invalid_params(
                    format!("unknown input {other:?} (expected button|motion|move_to|wheel|reset)"),
                    None,
                ))
            }
        };
        let result = self.brp("game/mouse", params).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }
}

#[rmcp::tool_handler]
impl rmcp::ServerHandler for GameTools {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let mut info = rmcp::model::ServerInfo::default();
        info.instructions = Some(
            "Tool API for the prototype_19 game client (dev/QA only). Drive the character with \
             inject_input, observe with game_state/screenshot."
                .into(),
        );
        info
    }
}
