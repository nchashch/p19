//! Opens the UDP/netcode connection to the authoritative server (replacing the old
//! QUIC/`bevy_quinnet` endpoint) and drives the post-connect state transition.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use lightyear::prelude::*;
use shared::game_state::GameState;

use crate::config::load_client_config;
use crate::events::{Connect, Disconnect};

const SERVER_PORT: u16 = 6000;

/// Must match the server's `PROTOCOL_ID`/`PRIVATE_KEY` (`server::networking`) exactly — both
/// sides need to agree on these bytes for a connect token to validate. See that module's doc
/// comment for why this is hardcoded rather than generated per-run.
const PROTOCOL_ID: u64 = 0;
const PRIVATE_KEY: [u8; 32] = [0; 32];

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerAddress>();
        app.add_systems(Startup, (load_client_config, spawn_client_link));
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(on_connect_request);
        app.add_observer(on_disconnect_request);
    }
}

/// The server address the client connects to — entirely config-driven now, not user-editable in
/// the main menu (there used to be a text field for this; it was removed in favor of just always
/// using `assets/config.toml`'s `server_ip`). Still a raw `String`, not a parsed `IpAddr`, since
/// `loading.rs`'s `load_level` is what actually parses it, at the point it's about to connect —
/// same "validate right before committing" shape as the level id check there, and keeps the parse
/// failure mode (a malformed address in `config.toml`) reported at the moment it'd actually matter
/// rather than at startup. `"127.0.0.1"` here is only the *fallback* default, used if
/// `load_default_server_address` can't find/parse `assets/config.toml` — that resource init runs
/// well before any `Startup` system, so `load_default_server_address` overwriting it afterward is
/// always safe ordering, not a race.
#[derive(Resource)]
pub struct ServerAddress(pub String);

impl Default for ServerAddress {
    fn default() -> Self {
        Self("127.0.0.1".to_string())
    }
}

/// The client's own connection/link entity, spawned once at `Startup` and reused across every
/// connect attempt (including a reconnect after returning to the main menu) — every other system
/// that needs "my own connection" (e.g. `controls.rs`'s `Single<&mut MessageSender<T>>` queries)
/// relies on there being exactly one such entity for the app's whole lifetime, so this must not be
/// a fresh entity per connect. `on_connect_request` is what actually inserts `NetcodeClient`/
/// `UdpIo`/etc. onto it and triggers `Connect`.
#[derive(Resource)]
struct ClientLink(Entity);

fn spawn_client_link(mut commands: Commands) {
    let entity = commands.spawn_empty().id();
    commands.insert_resource(ClientLink(entity));
}

/// Opens the connection to `ServerAddress`, unless one is already open or opening — safe to call
/// every time `Play` is pressed, including a second press before the first connection attempt has
/// resolved. Uses a dummy all-zero netcode key, same as the server's `PRIVATE_KEY` — fine for this
/// dev/LAN scaffold, not for a real deployment (see `server::networking`'s doc comment).
fn on_connect_request(
    _: On<Connect>,
    link: Res<ClientLink>,
    server_address: Res<ServerAddress>,
    status: Query<(
        Has<lightyear::prelude::Connected>,
        Has<lightyear::prelude::Connecting>,
    )>,
    mut commands: Commands,
) -> Result {
    info!("on_connect_request");
    if let Ok((connected, connecting)) = status.get(link.0)
        && (connected || connecting)
    {
        return Ok(());
    }
    let Ok(addr) = server_address.0.trim().parse::<IpAddr>() else {
        warn!(
            "on_connect_request: {:?} is not a valid IP address",
            server_address.0
        );
        return Ok(());
    };
    let server_addr = SocketAddr::new(addr, SERVER_PORT);
    // Only needs to be unique per running client instance, not cryptographically random — this is
    // a dev/LAN scaffold (see `PROTOCOL_ID`/`PRIVATE_KEY` above), not a real deployment.
    let client_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    let netcode_client = client::NetcodeClient::new(
        Authentication::Manual {
            server_addr,
            client_id,
            private_key: PRIVATE_KEY,
            protocol_id: PROTOCOL_ID,
        },
        client::NetcodeConfig {
            client_timeout_secs: -1,
            token_expire_secs: -1,
            ..default()
        },
    )?;
    commands.entity(link.0).insert((
        lightyear::prelude::Client,
        ReplicationReceiver,
        LocalAddr(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)),
        PeerAddr(server_addr),
        netcode_client,
        UdpIo::default(),
    ));
    commands.trigger(lightyear::prelude::Connect { entity: link.0 });
    Ok(())
}

/// Triggers **both** `Disconnect` (the connection/netcode layer — sends a disconnect packet so
/// the server also cleans up its `ClientOf` entity promptly) and `Unlink` (the IO/`Link` layer —
/// closes the actual UDP socket and removes `Linked`). These are two separate layers in lightyear
/// and neither implies the other: confirmed by tracing `NetcodeClientPlugin`'s `Disconnect`
/// handler (`lightyear_netcode`'s `client_plugin.rs`), which only calls `client.inner.disconnect()`
/// and inserts `Disconnected` — nothing on the client side observes that to trigger `Unlink` in
/// turn. Triggering `Disconnect` alone left `Linked` permanently stuck on `ClientLink`'s entity
/// (it's never despawned, see that resource's doc comment) — the next `on_connect_request` then
/// inserted a fresh `UdpIo::default()` (`socket: None`) onto an entity `LinkStart`'s bind handler
/// refuses to touch because it requires `Without<Linked>`, so the stale `Linked` marker never got
/// a real socket rebound under it. `lightyear_udp::UdpPlugin::receive` matches on `(Linked,
/// UdpIo)` alone and unconditionally unwraps `UdpIo::socket` — the observed crash
/// ("`Option::unwrap()` on a `None` value" in `lightyear_udp`) on a second `Connect` after
/// returning to the main menu.
fn on_disconnect_request(_: On<Disconnect>, link: Res<ClientLink>, mut commands: Commands) {
    commands.trigger(lightyear::prelude::Disconnect { entity: link.0 });
    commands.trigger(Unlink {
        entity: link.0,
        reason: UnlinkReason::UserRequested(None),
    });
}

fn on_connected(_: On<Add, lightyear::prelude::Connected>, mut commands: Commands) {
    info!("connected to server");
    commands.set_state(GameState::Lobby);
}

/// `Disconnected` is a required component of `NetcodeClient`, so this also fires once the very
/// first time `on_connect_request` inserts `NetcodeClient` (before the connection has actually
/// succeeded or failed) — mirrors the old `bevy_replicon`-era `ClientState::Disconnected` being
/// `#[default]` and firing on the very first frame. Harmless: at that point `GameState` is already
/// `MainMenu` (that's the only place `Connect` is ever triggered from), so `set_state` below is a
/// redundant no-op, not a wrong transition.
fn on_disconnected(
    _: On<Add, lightyear::prelude::Disconnected>,
    mut commands: Commands,
    game_state: Res<State<GameState>>,
) {
    info!("disconnected from server");
    if !matches!(game_state.get(), GameState::AssetLoading) {
        commands.set_state(GameState::MainMenu);
    }
}
