//! The agent/QA tool API (ADR 0009) — p19's **game-specific** extension of
//! [`bevy_mcp_harness`]. The generic half (the BRP server; `game/screenshot` + `game/ui`;
//! `game/gamepad`/`game/keyboard`/`game/mouse`; `game/client_info`; `game/cameras`; the MCP
//! server and its built-in tools, including the bundled `read_guide` agent guides) lives in the
//! harness crate. This module adds what only p19 knows:
//!
//! - `game/state`'s payload (the harness's `state_snapshot` hook): the app's `GameState`, the
//!   local player's entity/position/velocity/look/grounded/HP/GCD/death, the crosshair target,
//!   the replicated-action bindings introspection (playtest 0012's F7), and p19's
//!   launch-configuration extras under `"launch"` (the harness's own `game/client_info` covers
//!   ports and render flags).
//! - `game/input` — action-level `ActionMock` on the replicated ahoy action entities — plus
//!   its MCP wrapper `inject_input` (`McpHarnessConfig::extra_tools`; the harness's built-in
//!   tools cover only the device-level mocks).
//! - `game/trigger`, `game/select`, `game/levels`, `game/select_level` — client-local events,
//!   crosshair targeting, and the lobby's level list/selection.
//!
//! Design: ADR 0009. **`dev-tools` feature only — never enable in player-facing builds**: the
//! client is untrusted in this architecture, and a tool API in it is a cheat surface. BRP is
//! unauthenticated by design — localhost bind only.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult};
use bevy_enhanced_input::prelude::{
    Action, ActionMock, ActionValue, Actions, MockSpan, TriggerState,
};
use serde_json::json;

use avian3d::prelude::LinearVelocity;
use bevy_ahoy::{CharacterControllerState, CharacterLook};
use p19_shared::client_events::{InGameRequest, ObserveRequest};
use p19_shared::combat::{Dead, Gcd, HitPoints};
use p19_shared::game_state::GameState;
use p19_shared::inputs::PlayerInputContext;
use p19_shared::player::{PlayerCharacter, Selectable};

use crate::controls::actions::RotateCamera;
use crate::controls::targeting::Selected;
use crate::gameplay::player_character::LocalPlayer;

/// The agent/QA tool API: [`bevy_mcp_harness`] for everything generic, plus p19's own BRP
/// methods and MCP tools registered here.
pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        // p19's offscreen target is created by `main.rs`'s headless branch (feature-
        // independent — the headless camera machinery is not dev-only, so it cannot move into
        // the optional harness dependency). Hand its handle to the harness instead of letting
        // it create a second texture: the harness then adds only its cursor overlay, while
        // p19 keeps ownership of camera retargeting/bootstrap/clear-order. Windowed sessions
        // pass `None` and the harness captures the primary window.
        let offscreen_target = app
            .world()
            .get_resource::<crate::controls::camera::OffscreenRenderTarget>()
            .map(|target| target.0.clone());
        let config = bevy_mcp_harness::McpHarnessConfig {
            brp_port: crate::config::brp_port_presync(),
            mcp_port: crate::config::mcp_port_presync(),
            screenshots_dir: p19_screenshots_dir(),
            // p19 owns its headless camera machinery (feature-independent — see the module
            // docs); it hands the harness its offscreen handle and the harness adds only the
            // cursor overlay (no public resource of its own).
            offscreen: match offscreen_target {
                Some(handle) => bevy_mcp_harness::OffscreenMode::HostManaged(handle),
                None => bevy_mcp_harness::OffscreenMode::Windowed,
            },
            no_render: crate::config::is_no_render_presync(),
            state_snapshot: Some(Arc::new(game_state_snapshot)),
            client_info_host: Some(Arc::new(client_info_launch)),
            // bevy_markup UI declares clicks via `data-on-click` element signals, not
            // `bevy_ui::Interaction` — without this hook `game/ui` would never report the
            // menu buttons `clickable`.
            clickable: Some(Arc::new(markup_clickable)),
            extra_tools: vec![inject_input_tool()],
            ..Default::default()
        };
        app.add_plugins(bevy_mcp_harness::BevyMcpHarnessPlugin { config });

        // p19-specific BRP methods attach any time after the harness plugin, with declared
        // preconditions so `plan_check` can pre-flight an intended call sequence.
        bevy_mcp_harness::register_game_method_with_precondition(
            app,
            "game/input",
            Some(Arc::new(input_precondition)),
            input_method,
        );
        bevy_mcp_harness::register_game_method_with_precondition(
            app,
            "game/trigger",
            Some(Arc::new(trigger_precondition)),
            trigger_method,
        );
        bevy_mcp_harness::register_game_method_with_precondition(
            app,
            "game/select",
            Some(Arc::new(select_precondition)),
            select_method,
        );
        bevy_mcp_harness::register_game_method_with_precondition(
            app,
            "game/levels",
            Some(Arc::new(levels_precondition)),
            levels_method,
        );
        bevy_mcp_harness::register_game_method_with_precondition(
            app,
            "game/select_level",
            Some(Arc::new(select_level_precondition)),
            select_level_method,
        );
    }
}

// --- declared preconditions (advisory; the methods' own checks remain authoritative) ---------

fn require_local_player(world: &World) -> Result<(), String> {
    match world
        .get_resource::<LocalPlayer>()
        .and_then(|lp| lp.0)
    {
        Some(player) => {
            if world.get::<Actions<PlayerInputContext>>(player).is_some() {
                Ok(())
            } else {
                Err("local player has no replicated input context yet".to_owned())
            }
        }
        None => Err("no local player connected (connect + play first)".to_owned()),
    }
}

fn input_precondition(world: &World, _params: Option<&serde_json::Value>) -> Result<(), String> {
    require_local_player(world)
}

/// `game/trigger`'s requirements are per-event: `play`/`observe` need the connection's message
/// senders, `attack`/`kill` need a selected target, `spawn_*` needs the in-game player;
/// `connect`/`disconnect` are always available.
fn trigger_precondition(world: &World, params: Option<&serde_json::Value>) -> Result<(), String> {
    let event = params
        .and_then(|params| params.get("event"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match event {
        "play" => require(
            &|entity| {
                entity
                    .get::<lightyear::prelude::MessageSender<p19_shared::client_events::InGameRequest>>()
                    .is_some()
            },
            "InGameRequest",
            world,
        ),
        "observe" => require(
            &|entity| {
                entity
                    .get::<lightyear::prelude::MessageSender<p19_shared::client_events::ObserveRequest>>()
                    .is_some()
            },
            "ObserveRequest",
            world,
        ),
        "attack" | "kill" => {
            if world
                .get_resource::<Selected>()
                .and_then(|selected| selected.0)
                .is_none()
            {
                return Err(
                    "no target selected — game/select first (the crosshair raycast needs a window)"
                        .to_owned(),
                );
            }
            Ok(())
        }
        "spawn_cube" | "spawn_npc" => require_local_player(world),
        "connect" | "disconnect" => Ok(()),
        _ => Ok(()),
    }
}

/// `game/select`: only `{"nearest": true}` depends on the local player (it excludes it and
/// measures distances); entity/name selection targets other entities and is always available.
fn select_precondition(world: &World, params: Option<&serde_json::Value>) -> Result<(), String> {
    let wants_nearest = params
        .and_then(|params| params.get("nearest"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if wants_nearest {
        return require_local_player(world);
    }
    Ok(())
}

fn levels_precondition(world: &World, _params: Option<&serde_json::Value>) -> Result<(), String> {
    let has_levels = world
        .iter_entities()
        .any(|entity| entity.get::<p19_shared::level::Levels>().is_some());
    if has_levels {
        Ok(())
    } else {
        Err("no Levels entity replicated yet (not in the lobby?)".to_owned())
    }
}

fn select_level_precondition(
    world: &World,
    _params: Option<&serde_json::Value>,
) -> Result<(), String> {
    require(
        &|entity| {
            entity
                .get::<lightyear::prelude::MessageSender<
                    p19_shared::client_events::LoadLevelRequest,
                >>()
                .is_some()
        },
        "LoadLevelRequest",
        world,
    )
}

/// `ok` when any entity carries the component the closure checks for — the connected-app
/// shape of a message-sender precondition, kept concrete (lightyear's `MessageSender<M>` is
/// a component whose `M` carries no component bound of its own).
fn require(
    has: &dyn Fn(EntityRef<'_>) -> bool,
    what: &str,
    world: &World,
) -> Result<(), String> {
    if world.iter_entities().any(has) {
        Ok(())
    } else {
        Err(format!("no MessageSender<{what}> (not connected?)"))
    }
}

/// p19's capture convention (the harness default is the crate-independent
/// `<cwd>/mcp_harness/screenshots/`): persistent, human-browsable staging under the workspace's
/// agent-docs tree, isolated per client under fleet testing (a shared directory would make one
/// client's `game/screenshot/get` return another's capture).
fn p19_screenshots_dir() -> PathBuf {
    let base =
        p19_shared::paths::workspace_root().join("docs/agents/playtests/dist/screenshots");
    let port = crate::config::brp_port_presync();
    if port == bevy::remote::http::DEFAULT_PORT {
        base
    } else {
        base.join(format!("client-{port}"))
    }
}

// ---------------------------------------------------------------------------
// `game/state` — the harness's state hook
// ---------------------------------------------------------------------------

/// The `game/state` payload as a plain JSON value. The harness fuses it into every capture
/// response (and writes it to a `.json` sidecar next to the PNG) — a screenshot arrives with
/// its ground-truth state attached, so the agent never has to OCR the HUD or correlate "which
/// call came after which action". Snapshot is taken at *poll* time, i.e. a few hundred ms after
/// the capture started; that is the state the agent wants anyway (the world as it is right
/// after its action), and the capture→poll gap is bounded by the poll loop (~100ms
/// granularity).
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
    if let Some(camera) = world
        .query::<&crate::controls::fps_controller::FpsCamera>()
        .iter(world)
        .next()
    {
        out.insert("camera_yaw".into(), json!(camera.yaw));
        out.insert("camera_pitch".into(), json!(camera.pitch));
    }

    let Ok(player_entity) = world.get_entity(player) else {
        out.insert("player_despawned".into(), json!(true));
        return serde_json::Value::Object(out);
    };

    if let Some(name) = player_entity.get::<Name>() {
        out.insert("name".into(), json!(name.to_string()));
    }
    if let Some(transform) = player_entity.get::<Transform>() {
        out.insert("position".into(), json!(transform.translation));
    }
    if let Some(velocity) = player_entity.get::<LinearVelocity>() {
        out.insert("velocity".into(), json!(velocity.0));
    }
    // `look_*`: the look the KCC steers by (`CharacterLook`, set from this client's `Look`
    // input after the input delay). `camera_*`: the `FpsCamera` direction being sent now.
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
    // Current crosshair target (or `game/select` injection) — what `game/trigger attack|kill`
    // would hit right now. Absent when nothing is selected.
    if let Some(selected) = world.get_resource::<Selected>() {
        if let Some(entity) = selected.0 {
            out.insert("selected".into(), json!(entity));
        }
    }

    // Binding introspection (playtest 0012's F7): whether `bind_replicated_ahoy_actions`
    // actually bound the local player's action entities. `game/input`'s action-mocks bypass
    // bindings, so a broken-bindings desync (e.g. the second client's locked look/movement)
    // is invisible to everything else here — this makes it visible: one row per action,
    // `bound` = Bindings present, `context_is_local` = the action's context is this client's
    // own context (vs another player's replicated action, which must never be bound here).
    let local = Some(player);
    let mut bindings_rows = Vec::new();
    for (entity, action_of, bound) in world
        .query_filtered::<(Entity, &bevy_enhanced_input::prelude::ActionOf<PlayerInputContext>, Has<bevy_enhanced_input::prelude::Bindings>), With<bevy_enhanced_input::prelude::Action<bevy_ahoy::input::Movement>>>()
        .iter(world)
    {
        bindings_rows.push(json!({
            "action": "movement", "entity": entity, "context": action_of.get(),
            "bound": bound, "context_is_local": Some(action_of.get()) == local,
        }));
    }
    for (entity, action_of, bound) in world
        .query_filtered::<(Entity, &bevy_enhanced_input::prelude::ActionOf<PlayerInputContext>, Has<bevy_enhanced_input::prelude::Bindings>), With<bevy_enhanced_input::prelude::Action<bevy_ahoy::input::Jump>>>()
        .iter(world)
    {
        bindings_rows.push(json!({
            "action": "jump", "entity": entity, "context": action_of.get(),
            "bound": bound, "context_is_local": Some(action_of.get()) == local,
        }));
    }
    // `Look` has no bindings: "bound" means the client drives it (lightyear's `InputMarker`).
    for (entity, action_of, mocked) in world
        .query_filtered::<(Entity, &bevy_enhanced_input::prelude::ActionOf<PlayerInputContext>, Has<lightyear_inputs_bei::prelude::InputMarker<PlayerInputContext>>), With<bevy_enhanced_input::prelude::Action<p19_shared::inputs::Look>>>()
        .iter(world)
    {
        bindings_rows.push(json!({
            "action": "look", "entity": entity, "context": action_of.get(),
            "bound": mocked, "context_is_local": Some(action_of.get()) == local,
        }));
    }
    out.insert("bindings".into(), json!(bindings_rows));

    serde_json::Value::Object(out)
}

/// The host half of `game/client_info` (served under the `"host"` key; the harness's own
/// payload covers ports, `no_render`, and the target size). p19's launch-configuration extras,
/// reported with the *effective* flags including implications (`--no-render` implies `--mcp`
/// and `--no-common-assets`).
fn client_info_launch(_world: &mut World) -> serde_json::Value {
    let no_render = crate::config::is_no_render_presync();
    json!({
        "mcp": crate::config::is_mcp_mode_presync() || no_render,
        "no_common_assets": crate::config::is_no_common_assets_presync() || no_render,
        "vr": crate::config::is_vr_enabled_presync(),
        "headless_render": crate::config::is_headless_render_presync() && !no_render,
    })
}

/// The `clickable` convention for `game/ui`: bevy_markup UI declares clicks via `data-on-click`
/// element signals instead of `bevy_ui::Interaction` — without this hook the dump would never
/// report the menu buttons clickable.
fn markup_clickable(world: &World, entity: Entity) -> bool {
    world
        .get::<bevy_markup::prelude::ElementSignals>(entity)
        .is_some_and(|signals| {
            signals
                .0
                .iter()
                .any(|binding| binding.trigger == bevy_markup::prelude::SignalTrigger::Click)
        })
}

// ---------------------------------------------------------------------------
// p19-specific BRP methods — handlers run in the main world with `&mut World` access.
// ---------------------------------------------------------------------------

/// `game/input` — inject player input for `ticks` fixed ticks (1–600) by mocking one of the
/// replicated ahoy action entities. The mocked value rides the exact same BEI → replicated-BEI
/// → server-sim → corrected-prediction path a gamepad does, so agent playtesting exercises the
/// real netcode.
///
/// Params:
/// - `action`: `"movement"` (`x` = strafe right, `y` = forward, both −1..1, like a stick),
///   `"jump"` (`down` = press-hold, releases on tick expiry), or `"rotate"` (`yaw_delta` /
///   `pitch_delta`: total **radians**, positive turns right / looks down, spread over `ticks`)
/// - `ticks`: how many fixed ticks (16.7ms each) the input stays active. `rotate` mocks the
///   client-local camera action instead (the server only ever receives the resulting camera
///   direction, ADR 0017), which BEI evaluates per frame — at `--mcp`'s 60 Hz the same count.
fn input_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
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

    if action == "rotate" {
        let yaw_delta = params.get("yaw_delta").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
        let pitch_delta = params.get("pitch_delta").and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
        let Some(entity) = world
            .query_filtered::<Entity, With<Action<RotateCamera>>>()
            .iter(world)
            .next()
        else {
            return Err(BrpError::internal("no camera RotateCamera action (not in game?)"));
        };
        // The mock bypasses the binding's `Scale`, so the value is radians directly; BEI fires
        // it every update the mock lasts, so the total is divided across them.
        world.entity_mut(entity).insert(ActionMock::new(
            TriggerState::Fired,
            ActionValue::Axis2D(Vec2::new(yaw_delta / ticks as f32, pitch_delta / ticks as f32)),
            MockSpan::Updates(ticks),
        ));
        return Ok(json!({"mocked_action_entity": entity, "ticks": ticks}).into());
    }

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
                if action_entity_mut.get::<Action<bevy_ahoy::input::Movement>>().is_some() {
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
                if action_entity_mut.get::<Action<bevy_ahoy::input::Jump>>().is_some() {
                    let down = params.get("down").and_then(serde_json::Value::as_bool).unwrap_or(true);
                    action_entity_mut.insert(ActionMock::new(
                        TriggerState::Fired,
                        ActionValue::Bool(down),
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

/// `game/select` — injects crosshair targeting headlessly: sets the `Selected` resource.
/// Three mutually exclusive ways to name the target (exactly one params key):
/// - `{"entity": <u64>}` — the raw entity id *this client* reports (`world.query`/`game/state`).
/// - `{"nearest": true}` — the nearest *other* player character (excludes this client's own).
/// - `{"name": "<string>"}` — exact `Name` match among players; ambiguous matches are rejected
///   with a count (note: player names are hardcoded to `"player name"` today, so this is only
///   useful once names are actually unique).
/// All paths validate the target is `Selectable` — the same gate the real raycast applies.
/// The attack/kill hotkey send path (`game/trigger attack|kill`) consumes exactly this, so
/// combat QA works without a window (`raycast_from_center` needs one).
fn select_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let Some(params) = params.0 else {
        return Err(BrpError::internal("missing params"));
    };
    let by_entity = params.get("entity");
    let nearest = params.get("nearest").and_then(serde_json::Value::as_bool);
    let by_name = params.get("name").and_then(serde_json::Value::as_str);
    let provided = [by_entity.is_some(), nearest.unwrap_or(false), by_name.is_some()]
        .into_iter()
        .filter(|provided| *provided)
        .count();
    if provided != 1 {
        return Err(BrpError::internal(
            "provide exactly one of: entity (u64 id) | nearest (true) | name (string)",
        ));
    }

    if let Some(name) = by_name {
        let mut candidates: Vec<Entity> = Vec::new();
        for (entity, name_component, _) in world
            .query_filtered::<(Entity, &Name, Has<Selectable>), With<PlayerCharacter>>()
            .iter(world)
        {
            if name_component.as_str() == name {
                candidates.push(entity);
            }
        }
        return if candidates.len() == 1 {
            let entity = candidates[0];
            world.resource_mut::<Selected>().0 = Some(entity);
            Ok(json!({"selected": entity, "name": name}).into())
        } else {
            Err(BrpError::internal(&format!(
                "name {name:?} matched {} players (names must be unique to select by name)",
                candidates.len()
            )))
        };
    }

    let entity = if let Some(bits) = by_entity
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
    {
        bevy::ecs::entity::Entity::from_bits(bits)
    } else if nearest == Some(true) {
        let Some(local_player) = world
            .get_resource::<LocalPlayer>()
            .and_then(|lp| lp.0)
        else {
            return Err(BrpError::internal("no local player (not in game?)"));
        };
        let Some(my_position) = world.get::<Transform>(local_player).map(|t| t.translation)
        else {
            return Err(BrpError::internal("local player has no Transform"));
        };
        let (mut best, mut best_distance) = (None, f32::MAX);
        // Exclude by entity value, not a query filter: `LocalPlayer` is a Resource (Bevy 0.19
        // accepts resources in query filters, where `Without<LocalPlayer>` excludes nothing —
        // verified live: without this check `nearest` selected the attacker's own player at
        // distance 0).
        for (entity, transform, _) in world
            .query_filtered::<(Entity, &Transform, Has<Selectable>), With<PlayerCharacter>>()
            .iter(world)
        {
            if entity == local_player {
                continue;
            }
            let distance = my_position.distance(transform.translation);
            if distance < best_distance {
                (best, best_distance) = (Some(entity), distance);
            }
        }
        best.ok_or_else(|| BrpError::internal("no other player character to select"))?
    } else {
        return Err(BrpError::internal("missing params.target"));
    };

    let selectable = world
        .query_filtered::<(), bevy::ecs::query::With<Selectable>>()
        .get(world, entity)
        .is_ok();
    if !selectable {
        return Err(BrpError::internal(&format!(
            "entity {entity:?} is not a targetable `Selectable` on this client (ids are client-local; query this same client)"
        )));
    }
    world.resource_mut::<Selected>().0 = Some(entity);
    Ok(json!({"selected": entity}).into())
}

/// `game/trigger` — triggers the app's own client-local events, the same ones the menu buttons
/// fire. `connect` → the main menu's Connect (opens the lightyear connection); `play` → the
/// lobby's Play (sends `InGameRequest` via the client's own `MessageSender`); `disconnect` →
/// the lobby/menu's Main Menu + Disconnect.
fn trigger_method(params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    use crate::events::{Connect, Disconnect};
    use p19_shared::replication::OrderedReliable;

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
            // The lobby Play button's exact behavior: `InGameRequest` (with this client's
            // `ClientPrediction`) over the client's own message sender.
            let predict = world
                .resource::<crate::gameplay::player_character::ClientPrediction>()
                .0;
            let mut sender = world
                .query::<&mut lightyear::prelude::MessageSender<InGameRequest>>()
                .iter_mut(world)
                .next()
                .ok_or_else(|| BrpError::internal("no MessageSender<InGameRequest> (not connected?)"))?;
            sender.send::<OrderedReliable>(InGameRequest { predict });
            Ok(json!({"triggered": "play", "sent": "InGameRequest", "predict": predict}).into())
        }
        "observe" => {
            // The observer's counterpart to `play` (`--headless-render` clients): joins the
            // game room / receives replicated world state WITHOUT spawning a player character.
            let mut sender = world
                .query::<&mut lightyear::prelude::MessageSender<ObserveRequest>>()
                .iter_mut(world)
                .next()
                .ok_or_else(|| BrpError::internal("no MessageSender<ObserveRequest> (not connected?)"))?;
            sender.send::<OrderedReliable>(ObserveRequest);
            Ok(json!({"triggered": "observe", "sent": "ObserveRequest"}).into())
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
        // The attack/kill hotkeys' send path, headlessly: no window means no crosshair
        // targeting, so `game/select` injects `Selected` directly and these trigger the same
        // send observers the hotkeys do (see `controls.rs`'s `send_attack`/`send_kill`).
        "attack" => {
            world.commands().trigger(crate::events::AttackSelected);
            Ok(json!({"triggered": "attack"}).into())
        }
        "kill" => {
            world.commands().trigger(crate::events::KillSelected);
            Ok(json!({"triggered": "kill"}).into())
        }
        other => Err(BrpError::internal(&format!(
            "unknown event {other:?} (expected connect|play|observe|disconnect|spawn_cube|spawn_npc|attack|kill)"
        ))),
    }
}

/// `game/levels` — lists the server-replicated `Levels` singleton (the same list the lobby's
/// level picker reads): asset paths + names, so an agent can pick a level by asset path.
fn levels_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    // `Levels` is a replicated COMPONENT on an entity (not a resource).
    let mut query = world.query::<&p19_shared::level::Levels>();
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
    use p19_shared::client_events::LoadLevelRequest;
    use p19_shared::replication::OrderedReliable;

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
// p19's MCP tools — the harness's built-ins cover the device-level mocks; the action-level
// input mock stays p19-specific and wraps the `game/input` BRP method as an `extra_tool`.
// ---------------------------------------------------------------------------

/// The `inject_input` MCP tool's parameters — the harness generates the tool's JSON schema
/// from these derives (the host keeps a direct `schemars` dependency for the derive; see
/// `docs/agents/api-friction.md` #6), so the agent sees the documented shapes.
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

/// The action-level input mock as an MCP tool. Proxies to the `game/input` BRP method over the
/// provided loopback client — the same shape the harness's built-in tools use.
fn inject_input_tool() -> bevy_mcp_harness::HarnessTool {
    bevy_mcp_harness::HarnessTool::new(
        "inject_input",
        "Inject player input through the real input pipeline (the same replicated path a gamepad uses) for `ticks` fixed ticks (~16.7ms each). Actions: `movement` (x = strafe right −1..1, y = forward −1..1), `jump` (down = hold), `rotate` (yaw_delta / pitch_delta in RADIANS this call, spread across the ticks; positive yaw_delta turns right, positive pitch_delta looks up). Combine calls (e.g. movement + rotate across consecutive calls) to drive the character; call game_state after to observe the effect. Blocked while the dev console or pause modal is open (the whole input context is deactivated there) — close it first.",
        |client: bevy_mcp_harness::BrpClient, args: InjectInputParams| async move {
            let mut params = match args.action.as_str() {
                "movement" => json!({"action": "movement", "x": args.x.unwrap_or(0.0), "y": args.y.unwrap_or(0.0)}),
                "jump" => json!({"action": "jump", "down": args.down.unwrap_or(true)}),
                "rotate" => json!({"action": "rotate", "yaw_delta": args.yaw_delta.unwrap_or(0.0), "pitch_delta": args.pitch_delta.unwrap_or(0.0)}),
                other => return Err(format!("unknown action {other:?} (expected movement|jump|rotate)")),
            };
            if let Some(ticks) = args.ticks {
                params["ticks"] = json!(ticks);
            }
            client.call("game/input", params).await
        },
    )
}
