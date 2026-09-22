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
//!    and `game/input` (inject input through the *real* pipeline: `ActionMock` on the
//!    replicated ahoy action entities, so the agent's input flows BEI → replicated-BEI →
//!    server-authoritative sim → corrected prediction, exactly like a gamepad's).
//! 3. **An in-process MCP server** (`rmcp`, Streamable HTTP on `127.0.0.1:15703`, stateless
//!    mode) whose tools proxy to the BRP methods over loopback HTTP — the MCP layer owns only
//!    the protocol surface (tool listing + schemas), never the `World` (the handlers are async
//!    and run outside Bevy's world; all `World` access stays in BRP's systems).
//!
//! All of this only compiles/enables under the `dev-tools` cargo feature — **never enable it
//! in player-facing builds**: the client is untrusted in this architecture, and a tool API in
//! it is a cheat surface. BRP is also unauthenticated by design — localhost bind only.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::remote::http::{RemoteHttpPlugin, DEFAULT_PORT as BRP_PORT};
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use bevy_enhanced_input::prelude::{Action, ActionMock, ActionValue, Actions, MockSpan, TriggerState};
use serde_json::json;

use bevy_ahoy::{CharacterControllerState, CharacterLook};
use shared::combat::{Dead, Gcd, HitPoints};
use shared::game_state::GameState;
use shared::inputs::PlayerInputContext;

use avian3d::prelude::LinearVelocity;
use bevy_ahoy::input::{Jump as AhoyJump, Movement as AhoyMovement, RotateCamera as AhoyRotate};

use crate::gameplay::player_character::LocalPlayer;

/// The MCP surface's TCP port. NOT 15703: that's `bevy_remote`'s **render-subapp BRP port**
/// (`DEFAULT_RENDER_PORT`, active whenever `bevy_render` runs) — binding our MCP listener
/// there made the render app's BRP bind fail and the main BRP pipeline hang in release builds.
const MCP_PORT: u16 = 15710;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        // BRP: `bevy_skein` (via its default-on `brp` feature + `handle_brp`, which defaults
        // to enabled under `debug_assertions`) may have already added `RemotePlugin` +
        // `RemoteHttpPlugin` with the default method set — reuse that server when present and
        // only add the plugins when Skein didn't (e.g. release builds, where `handle_brp`
        // defaults off). Double-adding panics.
        if !app.is_plugin_added::<bevy::remote::RemotePlugin>() {
            app.add_plugins((RemotePlugin::default(), RemoteHttpPlugin::default()));
        }

        // Custom methods attach post-build via the `RemoteMethods` resource (the plugins'
        // `with_method_main` only works at construction, and Skein may have built the plugin
        // before us).
        let state_method = app.register_system(game_state_method);
        let screenshot_start = app.register_system(screenshot_start_method);
        let screenshot_get = app.register_system(screenshot_get_method);
        let input_method_id = app.register_system(input_method);
        let mut methods = app
            .world_mut()
            .resource_mut::<bevy::remote::RemoteMethods>();
        methods.insert("game/state", bevy::remote::RemoteMethodSystemId::Instant(state_method));
        methods.insert("game/screenshot", bevy::remote::RemoteMethodSystemId::Instant(screenshot_start));
        methods.insert("game/screenshot/get", bevy::remote::RemoteMethodSystemId::Instant(screenshot_get));
        methods.insert("game/input", bevy::remote::RemoteMethodSystemId::Instant(input_method_id));

        start_mcp_server();
    }
}

// ---------------------------------------------------------------------------
// Custom BRP methods — handlers run in the main world with `&mut World` access.
// ---------------------------------------------------------------------------

/// `game/state` — a curated snapshot of everything an agent needs to reason about the game:
/// the app's `GameState`, the local player's entity id, position, velocity, look yaw/pitch,
/// grounded, HP, GCD and death timers. Params are ignored.
fn game_state_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let mut out = serde_json::Map::new();
    if let Some(state) = world.get_resource::<State<GameState>>() {
        out.insert("game_state".into(), json!(format!("{:?}", state.get())));
    }

    let Some(player) = world.get_resource::<LocalPlayer>().and_then(|lp| lp.0) else {
        out.insert("connected".into(), json!(false));
        return Ok(serde_json::Value::Object(out).into());
    };
    out.insert("connected".into(), json!(true));
    out.insert("player_entity".into(), json!(player));

    let Ok(player_entity) = world.get_entity(player) else {
        out.insert("player_despawned".into(), json!(true));
        return Ok(serde_json::Value::Object(out).into());
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

    Ok(serde_json::Value::Object(out).into())
}

/// The temp file the screenshot pipeline round-trips through: `game/screenshot` writes it (via
/// bevy's own `save_to_disk`, which encodes the PNG), `game/screenshot/get` drains it. A fixed
/// path + one-shot consumption keeps the flow trivial for a single-agent QA loop.
fn screenshot_path() -> PathBuf {
    std::env::temp_dir().join("prototype19_devtools_screenshot.png")
}

/// `game/screenshot` — starts an async capture of the primary window. The PNG is written to a
/// temp file by bevy's own `save_to_disk` observer when the render completes (async!); poll
/// `game/screenshot/get` until it reports `ready`.
fn screenshot_start_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let path = screenshot_path();
    let _ = std::fs::remove_file(&path);
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()))
        .observe(|_trigger: On<ScreenshotCaptured>| {
            // The entity is despawned after capture; nothing extra to do — the file is the
            // delivery mechanism.
        });
    Ok(json!({"status": "capturing", "poll": "game/screenshot/get"}).into())
}

/// `game/screenshot/get` — one-shot drain of the last capture: `{"ready": true, "png_base64":
/// …}` once the PNG is on disk, `{"ready": false}` while still rendering. The file is consumed
/// (deleted) on the ready read.
fn screenshot_get_method(_params: In<Option<serde_json::Value>>, _world: &mut World) -> BrpResult {
    let path = screenshot_path();
    match std::fs::read(&path) {
        Ok(bytes) => {
            let _ = std::fs::remove_file(&path);
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            Ok(json!({"ready": true, "png_base64": encoded}).into())
        }
        Err(_) => Ok(json!({"ready": false}).into()),
    }
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

    // The mouse-look binding scales pixels→radians (MOUSE_LOOK_SENSITIVITY); an agent speaks
    // radians, so pre-divide to hand the binding the pixel-equivalent it expects.
    const MOUSE_LOOK_SENSITIVITY: f32 = 0.001;

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
                    action_entity_mut.insert(ActionMock::new(
                        TriggerState::Fired,
                        ActionValue::Axis2D(Vec2::new(-yaw_delta / MOUSE_LOOK_SENSITIVITY, pitch_delta)),
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

// ---------------------------------------------------------------------------
// The in-process MCP server (rmcp, Streamable HTTP, stateless) — the protocol surface whose
// tools proxy to the BRP methods above over loopback HTTP.
// ---------------------------------------------------------------------------

fn start_mcp_server() {
    std::thread::Builder::new()
        .name("mcp-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("mcp server: tokio runtime should build");
            if let Err(err) = runtime.block_on(serve_mcp()) {
                error!("mcp server stopped: {err:?}");
            }
        })
        .expect("mcp server: thread should spawn");
}

async fn serve_mcp() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", MCP_PORT)).await?;
    info!("mcp tool server listening on http://127.0.0.1:{MCP_PORT}/mcp");
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

/// The MCP tool surface — every tool is a thin proxy to a BRP method over loopback HTTP; all
/// `World` access lives in the BRP handlers above.
#[derive(Clone)]
struct GameTools {
    brp_url: String,
}

impl Default for GameTools {
    fn default() -> Self {
        Self {
            brp_url: format!("http://127.0.0.1:{BRP_PORT}"),
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

    /// Captures a screenshot of the game window (PNG) and returns it as image content. The
    /// capture is async (one render frame), so this polls `game/screenshot/get` briefly.
    #[rmcp::tool(description = "Capture a screenshot of the game window. Returns the PNG as image content. Use this to SEE the game (the camera view IS the player's view).")]
    async fn screenshot(&self) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        self.brp("game/screenshot", json!({})).await?;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let result = self.brp("game/screenshot/get", json!({})).await?;
            if result.get("ready").and_then(serde_json::Value::as_bool) == Some(true) {
                let png_base64 = result
                    .get("png_base64")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| rmcp::ErrorData::internal_error("screenshot missing data", None))?;
                return Ok(rmcp::model::CallToolResult::success(vec![
                    rmcp::model::ContentBlock::image(png_base64.to_string(), "image/png"),
                ]));
            }
        }
        Err(rmcp::ErrorData::internal_error(
            "screenshot timed out (is the game rendering?)",
            None,
        ))
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
