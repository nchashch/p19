//! Opens the authoritative QUIC (`bevy_quinnet`) endpoint clients connect to, and drives the
//! scaffold's one demo entity so there's something server-authoritative to observe replicating.

use std::net::Ipv4Addr;

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy_quinnet::server::{
    EndpointAddrConfiguration, QuinnetServer, ServerEndpointConfiguration,
    ServerEndpointConfigurationDefaultables, certificate::CertificateRetrievalMode,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;
use shared::{
    character_controller::{CharacterController, JumpInput, MovementInput},
    client_events::{Jump, LoadLevelRequest, Movement},
    combat::SharedCombatPlugin,
    level::LevelRoot,
    player::{PlayerCharacter, PlayerCharacterSpawner, player},
    server_events::LoadLevel,
};

pub const PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_endpoint)
            .add_observer(on_load_level_request)
            .add_observer(on_client_connected)
            .add_observer(on_client_disconnected)
            .add_observer(on_movement)
            .add_observer(on_jump);
    }
}

fn on_movement(
    request: On<FromClient<Movement>>,
    player: Single<Entity, (With<PlayerCharacter>, With<CharacterController>)>,
    mut commands: Commands,
) {
    commands.trigger(MovementInput {
        entity: *player,
        direction: request.direction,
    });
}

fn on_jump(
    _request: On<FromClient<Jump>>,
    player: Single<Entity, (With<PlayerCharacter>, With<CharacterController>)>,
    mut commands: Commands,
) {
    commands.trigger(JumpInput { entity: *player });
}

fn on_load_level_request(
    request: On<FromClient<LoadLevelRequest>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands, /* ... */
) {
    let id = request.id.clone();
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset(&id));
    // `Transform`/`Visibility` come from `LevelRoot`'s `#[require(...)]` now, but `Transform`
    // is spelled out explicitly anyway to enforce the "must stay at IDENTITY" invariant at the
    // spawn site rather than relying on the require's default matching it by coincidence.
    commands
        .spawn((LevelRoot { id }, Replicated, Transform::IDENTITY))
        .with_children(|parent| {
            parent.spawn(WorldAssetRoot(handle)).observe(on_level_ready);
        });
}

/// Fires once the level's local `WorldAssetRoot` (spawned above, never itself replicated — each
/// side loads its own copy independently) has actually finished instantiating, including any
/// Skein-reflected `ColliderConstructor`s Avian still needs to turn into real `Collider`s from
/// the scene's `Mesh` data. Broadcasts `LoadLevel` so clients know the `LevelRoot` they've
/// already received over replication is now backed by real, collidable geometry server-side —
/// not just once its asset bytes are loaded, which can be true before the entities themselves
/// exist (same reasoning as the client's own `on_level_ready` in `loading.rs`).
fn on_level_ready(
    ready: On<WorldInstanceReady>,
    parents: Query<&ChildOf>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    mut commands: Commands,
) {
    let Ok(level_root) = parents.get(ready.entity) else {
        return;
    };
    info!(
        "level ready, broadcasting LoadLevel for `{}`",
        level_root.parent()
    );
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: LoadLevel {
            entity: level_root.parent(),
        },
    });
    if let Ok(player_spawner_transform) = player_spawner.single() {
        commands.spawn((
            player("Player".to_string(), player_spawner_transform.translation),
            Replicated,
        ));
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

fn on_client_connected(add: On<Add, ConnectedClient>) {
    info!("client `{}` connected", add.entity);
}

fn on_client_disconnected(remove: On<Remove, ConnectedClient>) {
    info!("client `{}` disconnected", remove.entity);
}
