//! Opens the QUIC (`bevy_quinnet`) connection to the authoritative server and logs replication
//! state — a scaffold, not yet wired into any real gameplay presentation.

use std::net::{IpAddr, Ipv4Addr};

use bevy::prelude::*;
use bevy_quinnet::client::{
    ClientConnectionConfiguration, ClientConnectionConfigurationDefaultables, QuinnetClient,
    certificate::CertificateVerificationMode, connection::ClientAddrConfiguration,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;
use shared::game_state::GameState;

use crate::config::load_client_config;
use crate::events::{Connect, Disconnect};

const SERVER_PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerAddress>();
        app.add_systems(Startup, load_client_config);
        app.add_systems(OnEnter(ClientState::Connected), on_connected)
            .add_systems(OnEnter(ClientState::Disconnected), on_disconnected);
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

/// Opens the connection to `addr`, unless one is already open or opening — safe to call every
/// time `Play` is pressed, including a second press before the first connection attempt has
/// resolved. Skips certificate verification since the server generates a self-signed cert — fine
/// for this dev scaffold, not for a real deployment.
pub fn on_connect_request(
    _: On<Connect>,
    channels: Res<RepliconChannels>,
    server_address: Res<ServerAddress>,
    mut client: ResMut<QuinnetClient>,
) {
    info!("on_connect_request");
    if client.is_connected() || client.is_connecting() {
        return;
    }
    let Ok(addr) = server_address.0.trim().parse::<IpAddr>() else {
        warn!(
            "on_level_assets_loaded: {:?} is not a valid IP address",
            server_address.0
        );
        return;
    };
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

pub fn on_disconnect_request(_: On<Disconnect>, mut client: ResMut<QuinnetClient>) {
    client.close_all_connections();
}

fn on_connected(mut commands: Commands) {
    info!("connected to server");
    commands.set_state(GameState::Lobby);
}

/// `ClientState::Disconnected` is `#[default]` (confirmed against `bevy_replicon`'s source), so
/// this fires once on the very first frame too — before the client has ever tried to connect to
/// anything, and before `GameState::AssetLoading`'s `LoadingState` has had any chance to finish.
/// Without this guard, that spurious startup firing force-transitioned `GameState` straight to
/// `MainMenu`, bypassing `LoadingState::continue_to_state(GameState::MainMenu)` entirely and
/// panicking every `OnEnter(GameState::MainMenu)` system that needs `Res<CommonAssets>` (it
/// doesn't exist yet). A real disconnect can only happen after `AssetLoading` has already
/// finished — the client can't attempt a connection before reaching `MainMenu` — so gating on
/// that is precise, not just a startup-only special case.
fn on_disconnected(mut commands: Commands, game_state: Res<State<GameState>>) {
    info!("disconnected from server");
    if !matches!(game_state.get(), GameState::AssetLoading) {
        commands.set_state(GameState::MainMenu);
    }
}
