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
use shared::replication::DemoPosition;

const SERVER_PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, connect_to_server)
            .add_systems(OnEnter(ClientState::Connected), on_connected)
            .add_systems(OnEnter(ClientState::Disconnected), on_disconnected)
            .add_systems(Update, log_demo_position_changes)
            .add_observer(on_demo_entity_received);
    }
}

/// Connects to a server on localhost. Skips certificate verification since the server
/// generates a self-signed cert — fine for this dev scaffold, not for a real deployment.
fn connect_to_server(channels: Res<RepliconChannels>, mut client: ResMut<QuinnetClient>) {
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

fn on_connected() {
    info!("connected to server");
}

fn on_disconnected() {
    info!("disconnected from server");
}

fn on_demo_entity_received(add: On<Add, DemoPosition>) {
    info!("received demo entity `{}` from server", add.entity);
}

fn log_demo_position_changes(demo: Query<&Transform, (Changed<Transform>, With<DemoPosition>)>) {
    for transform in &demo {
        let pos = transform.translation;
        debug!("demo position updated: ({}, {}, {})", pos.x, pos.y, pos.z);
        // info!("demo position updated: ({}, {}, {})", pos.x, pos.y, pos.z);
    }
}
