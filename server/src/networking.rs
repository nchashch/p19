//! Opens the authoritative QUIC (`bevy_quinnet`) endpoint clients connect to, and drives the
//! scaffold's one demo entity so there's something server-authoritative to observe replicating.

use std::net::Ipv4Addr;

use bevy::prelude::*;
use bevy_quinnet::server::{
    EndpointAddrConfiguration, QuinnetServer, ServerEndpointConfiguration,
    ServerEndpointConfigurationDefaultables, certificate::CertificateRetrievalMode,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;
use shared::replication::DemoPosition;

pub const PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (start_endpoint, spawn_demo_entity))
            .add_systems(Update, bob_demo_entity)
            .add_observer(on_client_connected)
            .add_observer(on_client_disconnected);
    }
}

/// Binds the QUIC endpoint on all interfaces with a self-signed cert — fine for a
/// dev/LAN scaffold, but real deployment would want a properly issued certificate.
fn start_endpoint(channels: Res<RepliconChannels>, mut server: ResMut<QuinnetServer>) {
    server
        .start_endpoint(ServerEndpointConfiguration {
            addr_config: EndpointAddrConfiguration::from_ip(Ipv4Addr::UNSPECIFIED, PORT),
            cert_mode: CertificateRetrievalMode::GenerateSelfSigned {
                server_hostname: "localhost".to_string(),
            },
            defaultables: ServerEndpointConfigurationDefaultables {
                send_channels_cfg: channels.server_configs(),
            },
        })
        .expect("server endpoint should bind");
    info!("listening for clients on 0.0.0.0:{PORT}");
}

fn spawn_demo_entity(mut commands: Commands) {
    commands.spawn((Replicated, DemoPosition::default(), Transform::default()));
}

/// Bobs the demo entity up and down so replicated clients have visible movement to check.
fn bob_demo_entity(time: Res<Time>, mut demo: Single<&mut Transform, With<DemoPosition>>) {
    demo.translation.y = time.elapsed_secs().sin();
}

fn on_client_connected(add: On<Add, ConnectedClient>) {
    info!("client `{}` connected", add.entity);
}

fn on_client_disconnected(remove: On<Remove, ConnectedClient>) {
    info!("client `{}` disconnected", remove.entity);
}
