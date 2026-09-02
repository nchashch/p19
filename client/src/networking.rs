//! Opens the QUIC (`bevy_quinnet`) connection to the authoritative server and logs replication
//! state — a scaffold, not yet wired into any real gameplay presentation.

use std::net::{IpAddr, Ipv4Addr};

use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
    certificate::CertificateVerificationMode, connection::ClientAddrConfiguration,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;
use futures_lite::io::AsyncReadExt;
use serde::Deserialize;
use shared::client_events::LoadLevelRequest;

const SERVER_PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingLevelId>();
        app.init_resource::<ServerAddress>();
        app.init_resource::<DefaultLevel>();
        app.add_systems(Startup, load_client_config);
        app.add_systems(OnEnter(ClientState::Connected), on_connected)
            .add_systems(OnEnter(ClientState::Disconnected), on_disconnected);
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

/// The level `ui.rs`'s `play_button` requests — just the `.glb` filename (e.g. `"Level.glb"`),
/// not the full asset id; `play_button` still builds the actual `levels/{}#Scene0` id itself,
/// since the `levels/` directory and the `#Scene0` GLTF scene label aren't things `config.toml`
/// should need to know about. Config-driven for the same reason `ServerAddress` is — there's no
/// UI to edit it either.
#[derive(Resource)]
pub struct DefaultLevel(pub String);

impl Default for DefaultLevel {
    fn default() -> Self {
        Self("Level.glb".to_string())
    }
}

/// The subset of `assets/config.toml` this binary cares about.
#[derive(Deserialize)]
struct ClientConfig {
    server_ip: String,
    level: String,
}

/// Overwrites `ServerAddress`/`DefaultLevel`'s hardcoded fallbacks with `assets/config.toml`'s
/// `server_ip`/`level`, if the file exists and parses — read the same way `loading.rs`'s
/// `load_level` checks a level id exists (`AssetServer`'s default source reader, blocked on),
/// rather than `std::fs` — a `CARGO_MANIFEST_DIR`-relative path would break once this ships as a
/// packaged build with no source tree next to it, whereas the asset source resolves relative to
/// whatever `AssetPlugin.file_path` the running binary was actually configured with. A missing
/// file is fine (this repo's own `assets/config.toml` aside, nothing requires one to exist) and
/// just keeps the hardcoded fallbacks; a file that exists but fails to parse is `warn!`ed instead,
/// since that's more likely a real mistake worth noticing — and since both fields are read from
/// one `ClientConfig` in one parse, a mistake in either one currently means neither value gets
/// applied, not a partial update.
fn load_client_config(
    asset_server: Res<AssetServer>,
    mut server_address: ResMut<ServerAddress>,
    mut default_level: ResMut<DefaultLevel>,
) {
    let asset_path = AssetPath::parse("config.toml");
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        return;
    };
    let Ok(mut reader) = futures_lite::future::block_on(source.reader().read(asset_path.path()))
    else {
        return;
    };
    let mut contents = String::new();
    if let Err(err) = futures_lite::future::block_on(reader.read_to_string(&mut contents)) {
        warn!("load_client_config: failed to read config.toml ({err})");
        return;
    }
    match toml::from_str::<ClientConfig>(&contents) {
        Ok(config) => {
            server_address.0 = config.server_ip;
            default_level.0 = config.level;
        }
        Err(err) => warn!("load_client_config: failed to parse config.toml ({err})"),
    }
}

/// Set by `loading.rs`'s `load_level` once it's validated the requested level id exists locally
/// and opened the connection (via `connect_to_server`, below) — consumed by `on_connected` once
/// `ClientState::Connected` actually fires, so `LoadLevelRequest` is only ever sent once the
/// connection is real, not queued into a connection that may still be establishing.
///
/// The connection is deliberately *not* opened at `Startup` any more: a client that connects
/// eagerly to a server that already has a level loaded gets that `LevelRoot` replicated to it
/// immediately, and `loading.rs`'s `spawn_level`/`on_level_ready` react to a replicated
/// `LevelRoot` unconditionally (not gated on `GameState`) — so an eager connection used to skip
/// the main menu entirely and drop the client straight into `InGame` before `Play` was ever
/// pressed. Connecting only once `Play` is pressed (see `load_level`) makes that impossible.
#[derive(Resource, Default)]
pub struct PendingLevelId(pub Option<AssetPath<'static>>);

/// Opens the connection to `addr`, unless one is already open or opening — safe to call every
/// time `Play` is pressed, including a second press before the first connection attempt has
/// resolved. Skips certificate verification since the server generates a self-signed cert — fine
/// for this dev scaffold, not for a real deployment.
pub fn connect_to_server(channels: &RepliconChannels, client: &mut QuinnetClient, addr: IpAddr) {
    if client.is_connected() || client.is_connecting() {
        return;
    }
    client
        .open_connection(ClientConnectionConfiguration {
            addr_config: ClientAddrConfiguration::from_ips(
                addr,
                SERVER_PORT,
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                0,
            ),
            cert_mode: CertificateVerificationMode::SkipVerification,
            defaultables: ClientConnectionConfigurationDefaultables {
                send_channels_cfg: channels.client_configs(),
            },
        })
        .expect("client connection should open");
}

fn on_connected(mut pending: ResMut<PendingLevelId>, mut commands: Commands) {
    info!("connected to server");
    if let Some(id) = pending.0.take() {
        commands.client_trigger(LoadLevelRequest { id });
    }
}

fn on_disconnected() {
    info!("disconnected from server");
}
