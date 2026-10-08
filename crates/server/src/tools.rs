//! Server-side debugging surface: [`bevy_mcp_harness`] serving **authoritative server state**
//! — the ground truth that client-side views (predicted, room-filtered) can only approximate.
//! Built for debugging desync/multi-client bugs: "what does the server think position of player
//! X is" versus what each client reports. Exposed as the harness's `game/state` payload (and
//! the `game_state` MCP tool), plus every other harness surface.
//!
//! Always compiled in (unlike the client's `dev-tools` feature gate): the server is trusted,
//! everything binds to localhost only, and having the authoritative view permanently available
//! is the point. Ports: BRP **15701**, MCP **15711** (`--brp-port`/`--mcp-port` to override —
//! distinct from the client's 15702/15710 so a client and its server can be probed side by
//! side).

use std::sync::Arc;

use bevy::prelude::*;

/// The server's BRP port — one above the client's default (15702), memorable as "the server
/// side of the same debugging surface".
pub const BRP_PORT_DEFAULT: u16 = 15701;
/// The server's MCP port — mirrors the client's 15710 → 15711.
pub const MCP_PORT_DEFAULT: u16 = 15711;

pub struct ServerToolsPlugin;

impl Plugin for ServerToolsPlugin {
    fn build(&self, app: &mut App) {
        let brp_port = port_flag("--brp-port").unwrap_or(BRP_PORT_DEFAULT);
        let mcp_port = port_flag("--mcp-port").unwrap_or(MCP_PORT_DEFAULT);

        // Skein's `SkeinPlugin` is explicitly given `handle_brp: false` in `main.rs` (it
        // defaults to `cfg!(debug_assertions)`, which would otherwise double-add `RemotePlugin`
        // here and panic — confirmed live). The harness re-checks `is_plugin_added` itself, so
        // ordering is safe either way.
        app.add_plugins(bevy_mcp_harness::BevyMcpHarnessPlugin {
            config: bevy_mcp_harness::McpHarnessConfig {
                brp_port,
                mcp_port,
                state_snapshot: Some(Arc::new(server_state_snapshot)),
                ..Default::default()
            },
        });
    }
}

/// Reads `<flag> N` out of the process args. `None` when absent or malformed.
fn port_flag(flag: &str) -> Option<u16> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|value| value.parse().ok())
}

/// The `game/state` payload — the authoritative view: app state, every connected client, every
/// live player with its server-side transform, HP and owning connection, plus the
/// replicated-world entity count (a proxy for join-burst size).
fn server_state_snapshot(world: &mut World) -> serde_json::Value {
    let mut out = serde_json::Map::new();

    if let Some(state) = world.get_resource::<State<crate::ServerState>>() {
        out.insert("server_state".into(), serde_json::json!(format!("{:?}", state.get())));
    }

    // Players: everything the server spawned for a connection (`player()` bundle) —
    // PlayerCharacter + ControlledBy (owner = connection entity) + Transform + HitPoints.
    // CharacterLook is included per playtest 0012's F7: it is the desync-diagnosis field
    // (server-authoritative yaw/pitch to compare against each client's own view).
    let mut players = world.query_filtered::<(
        Entity,
        &Transform,
        &p19_shared::player::PlayerCharacter,
        &p19_shared::combat::HitPoints,
        &lightyear::prelude::ControlledBy,
        &bevy_ahoy::CharacterLook,
    ), ()>();
    let mut player_rows = Vec::new();
    for (entity, transform, _, hit_points, controlled_by, look) in players.iter(world) {
        player_rows.push(serde_json::json!({
            "entity": entity,
            "position": [transform.translation.x, transform.translation.y, transform.translation.z],
            "owner_connection": controlled_by.owner,
            "hit_points": hit_points.hit_points,
            "look": {"yaw": look.yaw, "pitch": look.pitch},
        }));
    }
    out.insert("player_count".into(), serde_json::json!(player_rows.len()));
    out.insert("players".into(), serde_json::json!(player_rows));

    // Connected clients: lightyear's per-connection entities, tagged by the netcode id.
    let mut connections = world.query::<(Entity, &lightyear::prelude::RemoteId)>();
    let connection_rows: Vec<serde_json::Value> = connections
        .iter(world)
        .map(|(entity, id)| serde_json::json!({ "entity": entity, "net_id": format!("{:?}", id.0) }))
        .collect();
    out.insert("connection_count".into(), serde_json::json!(connection_rows.len()));
    out.insert("connections".into(), serde_json::json!(connection_rows));

    out.insert(
        "entity_count".into(),
        serde_json::json!(world.entities().len()),
    );

    serde_json::Value::Object(out)
}
