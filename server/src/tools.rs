//! Server-side debugging surface: BRP + MCP, mirroring the client's `dev::tool_api` shape
//! (ADR 0009) but exposing **authoritative server state** — the ground truth that
//! client-side views (predicted, room-filtered) can only approximate. Built for debugging
//! desync/multi-client bugs: "what does the server think position of player X is" versus
//! what each client reports.
//!
//! Always compiled in (unlike the client's `dev-tools` feature gate): the server is trusted,
//! everything binds to localhost only, and having the authoritative view permanently available
//! is the point. Ports: BRP **15701**, MCP **15711** (`--brp-port`/`--mcp-port` to override —
//! distinct from the client's 15702/15710 so a client and its server can be probed side by
//! side).

use bevy::prelude::*;
use bevy::remote::http::{RemoteHttpPlugin, DEFAULT_PORT as CLIENT_BRP_PORT};
use bevy::remote::{BrpError, BrpResult, RemotePlugin};
use serde_json::json;

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
        // here and panic — confirmed live) — this is the "user is responsible" case Skein's own
        // warning describes, so the tool API adds `RemotePlugin` unconditionally here and the
        // custom methods attach via `RemoteMethods` right after.
        app.add_plugins((
            RemotePlugin::default(),
            RemoteHttpPlugin::default().with_port(brp_port),
        ));

        let state_method = app.register_system(server_state_method);
        let mut methods = app
            .world_mut()
            .resource_mut::<bevy::remote::RemoteMethods>();
        methods.insert("server/state", bevy::remote::RemoteMethodSystemId::Instant(state_method));

        start_mcp_server(brp_port, mcp_port);
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

/// `server/state` — the authoritative view: app state, every connected client, every live
/// player with its server-side transform, HP and owning connection, plus the replicated-world
/// entity count (a proxy for join-burst size). Params are ignored.
fn server_state_method(_params: In<Option<serde_json::Value>>, world: &mut World) -> BrpResult {
    let mut out = serde_json::Map::new();

    if let Some(state) = world.get_resource::<State<crate::ServerState>>() {
        out.insert("server_state".into(), json!(format!("{:?}", state.get())));
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
        player_rows.push(json!({
            "entity": entity,
            "position": [transform.translation.x, transform.translation.y, transform.translation.z],
            "owner_connection": controlled_by.owner,
            "hit_points": hit_points.hit_points,
            "look": {"yaw": look.yaw, "pitch": look.pitch},
        }));
    }
    out.insert("player_count".into(), json!(player_rows.len()));
    out.insert("players".into(), json!(player_rows));

    // Connected clients: lightyear's per-connection entities, tagged by the netcode id.
    let mut connections = world.query::<(Entity, &lightyear::prelude::RemoteId)>();
    let connection_rows: Vec<serde_json::Value> = connections
        .iter(world)
        .map(|(entity, id)| json!({ "entity": entity, "net_id": format!("{:?}", id.0) }))
        .collect();
    out.insert("connection_count".into(), json!(connection_rows.len()));
    out.insert("connections".into(), json!(connection_rows));

    out.insert(
        "entity_count".into(),
        json!(world.entities().len()),
    );

    Ok(out.into())
}

// ---------------------------------------------------------------------------
// MCP surface — same shape as the client's: rmcp Streamable HTTP on its own thread,
// tools proxy to this server's BRP over loopback.
// ---------------------------------------------------------------------------

fn start_mcp_server(brp_port: u16, mcp_port: u16) {
    std::thread::Builder::new()
        .name("server-mcp".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("server mcp: tokio runtime should build");
            if let Err(err) = runtime.block_on(serve_mcp(brp_port, mcp_port)) {
                error!("server mcp stopped: {err:?}");
            }
        })
        .expect("server mcp: thread should spawn");
}

async fn serve_mcp(brp_port: u16, mcp_port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    };

    let session_manager: std::sync::Arc<rmcp::transport::streamable_http_server::session::local::LocalSessionManager> = Default::default();
    let service = StreamableHttpService::new(
        move || Ok(ServerTools::new(brp_port)),
        session_manager,
        Default::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", mcp_port)).await?;
    info!("server mcp listening on http://127.0.0.1:{mcp_port}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}

#[derive(Clone)]
struct ServerTools {
    brp_url: String,
}

impl ServerTools {
    fn new(brp_port: u16) -> Self {
        Self {
            brp_url: format!("http://127.0.0.1:{brp_port}"),
        }
    }

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
        Ok(envelope.get("result").cloned().unwrap_or(json!(null)))
    }
}

/// The one MCP tool: the authoritative server snapshot. Debugging desyncs means comparing
/// this against each client's view.
#[rmcp::tool_router]
impl ServerTools {
    /// Proxy to the server's `server/state` BRP method.
    #[rmcp::tool(description = "Authoritative SERVER state: app/server state, every connected client, every live player with its SERVER-side position, HP and owning connection, plus total entity count. The ground truth for desync debugging — compare against each client's game/state to see who diverged from whom.")]
    async fn server_state(&self) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let result = self.brp("server/state", json!({})).await?;
        Ok(rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&result).map_err(|err| rmcp::ErrorData::internal_error(format!("{err}"), None))?,
        )]))
    }
}

#[rmcp::tool_handler]
impl rmcp::ServerHandler for ServerTools {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let mut info = rmcp::model::ServerInfo::default();
        info.instructions = Some(
            "Authoritative server state for prototype_19 desync debugging.".into(),
        );
        info
    }
}
