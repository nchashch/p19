//! Opens the authoritative QUIC (`bevy_quinnet`) endpoint clients connect to, and drives the
//! scaffold's one demo entity so there's something server-authoritative to observe replicating.

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy_quinnet::server::{
    EndpointAddrConfiguration, QuinnetServer, ServerEndpointConfiguration,
    ServerEndpointConfigurationDefaultables, certificate::CertificateRetrievalMode,
};
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::ChannelsConfigurationExt;
use shared::assets::asset_exists;
use shared::server_events::ServerInGame;
use shared::{
    character_controller::{JumpInput, MovementInput},
    client_events::{Jump, LoadLevelRequest, Movement},
    game_state::ServerState,
    level::LevelRoot,
    player::{PlayerCharacterSpawner, player},
    server_events::{LoadLevel, PlayerSpawned},
};
use std::net::Ipv4Addr;

use crate::level_state::LevelState;

pub const PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_endpoint)
            .add_observer(on_load_level_request)
            .add_observer(on_authorized_client_connected)
            .add_observer(on_client_connected)
            .add_observer(on_client_disconnected)
            .add_observer(on_movement)
            .add_observer(on_jump);
    }
}

/// Resolves which entity a `Movement`/`Jump` request applies to from the sending connection
/// itself, per `ClientId::entity()` — never from anything the client's payload claims. See
/// `on_level_ready` below: the client's own connection entity *is* its player character.
fn on_movement(request: On<FromClient<Movement>>, mut commands: Commands) {
    if let ClientId::Client(entity) = request.client_id {
        commands.trigger(MovementInput {
            entity,
            direction: request.direction,
        });
    }
}

fn on_jump(request: On<FromClient<Jump>>, mut commands: Commands) {
    if let ClientId::Client(entity) = request.client_id {
        commands.trigger(JumpInput { entity });
    }
}

fn on_load_level_request(
    request: On<FromClient<LoadLevelRequest>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let id = request.id.clone();
    if !asset_exists(&asset_server, &id) {
        info!("level {} doesn't exit", id);
        return;
    }
    commands.set_state(ServerState::Loading);
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset(id.clone()));
    commands
        .spawn((LevelRoot, Replicated, Transform::IDENTITY))
        .with_children(|parent| {
            parent.spawn(WorldAssetRoot(handle)).observe(on_level_ready);
        });
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: LoadLevel { id },
    });
}

fn on_level_ready(
    _ready: On<WorldInstanceReady>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    connected_clients: Query<Entity, With<AuthorizedClient>>,
    server_state: Res<State<ServerState>>,
    mut commands: Commands,
) {
    if !matches!(server_state.get(), ServerState::Loading) {
        return;
    }
    commands.set_state(ServerState::InGame);
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: ServerInGame,
    });
    // The client's own connection entity *is* its player character (see `on_movement`/`on_jump`
    // above, which resolve straight off `ClientId::entity()`) — no separate client->player
    // mapping needed. Only clients already connected when the level finishes loading get a
    // character this way; one connecting later would need its own spawn path, not added here.
    if let Ok(player_spawner_transform) = player_spawner.single() {
        for client_entity in &connected_clients {
            spawn_player_for_client(
                client_entity,
                player_spawner_transform.translation,
                &mut commands,
            );
        }
    }
}

/// Inserts the `player(...)` bundle onto `client_entity` (the client's own connection entity
/// doubles as its player character — see `on_movement`/`on_jump`) and tells that one client which
/// entity is theirs via a targeted `PlayerSpawned`, so client-side code doesn't have to guess
/// which of the (possibly several, once other players are connected) replicated
/// `PlayerCharacter` entities is its own.
fn spawn_player_for_client(client_entity: Entity, at: Vec3, commands: &mut Commands) {
    commands
        .entity(client_entity)
        .insert((player("Player".to_string(), at), Replicated));
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(client_entity)),
        message: PlayerSpawned {
            entity: client_entity,
        },
    });
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

fn on_authorized_client_connected(
    add: On<Add, AuthorizedClient>,
    level_state: Res<LevelState>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    mut commands: Commands,
) {
    if matches!(*level_state, LevelState::LevelLoaded(_))
        && let Ok(player_spawner_transform) = player_spawner.single()
    {
        spawn_player_for_client(
            add.entity,
            player_spawner_transform.translation,
            &mut commands,
        );
    }
    info!("authorized client `{}` connected", add.entity);
}

fn on_client_connected(add: On<Add, ConnectedClient>) {
    info!("client `{}` connected", add.entity);
}

fn on_client_disconnected(remove: On<Remove, ConnectedClient>) {
    info!("client `{}` disconnected", remove.entity);
}
