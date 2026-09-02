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
use shared::client_events::LoadLevelRequest;

const SERVER_PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingLevelId>();
        app.add_systems(OnEnter(ClientState::Connected), on_connected)
            .add_systems(OnEnter(ClientState::Disconnected), on_disconnected);
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

/// Opens the connection to the server on localhost, unless one is already open or opening —
/// safe to call every time `Play` is pressed, including a second press before the first
/// connection attempt has resolved. Skips certificate verification since the server generates a
/// self-signed cert — fine for this dev scaffold, not for a real deployment.
pub fn connect_to_server(channels: &RepliconChannels, client: &mut QuinnetClient) {
    if client.is_connected() || client.is_connecting() {
        return;
    }
    client
        .open_connection(ClientConnectionConfiguration {
            addr_config: ClientAddrConfiguration::from_ips(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
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
