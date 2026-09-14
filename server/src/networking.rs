//! Opens the authoritative QUIC (`bevy_quinnet`) endpoint clients connect to, and drives the
//! scaffold's one demo entity so there's something server-authoritative to observe replicating.

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use lightyear::prelude::*;
use shared::assets::asset_exists;
use shared::client_events::Join;
use shared::replication::OrderedReliable;
use shared::server_events::ServerInGame;
use shared::{
    character_controller::{JumpInput, MovementInput},
    client_events::{Jump, LoadLevelRequest, Movement},
    game_state::ServerState,
    level::LevelRoot,
    player::{PlayerCharacterSpawner, player},
    server_events::LoadLevel,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::level_state::LevelState;

/// Netcode's shared secret + protocol tag, replacing the old self-signed QUIC cert — both sides
/// must agree on the exact same bytes for a connect token to validate, so the client's own
/// connect code (once rewritten) needs to use these same two values. Hardcoded here rather than
/// generated per-run (`lightyear_netcode::generate_key()`): a fresh random key every server start
/// would mean tokens issued before a restart never validate — the same "fine for a dev/LAN
/// scaffold, not for real deployment" caveat the old self-signed cert already carried.
const PROTOCOL_ID: u64 = 0;
const PRIVATE_KEY: [u8; 32] = [0; 32];

pub const PORT: u16 = 6000;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_endpoint)
            .add_observer(on_authorized_client_connected)
            .add_observer(on_client_connected)
            .add_observer(on_level_ready)
            .add_observer(on_client_disconnected);

        app.add_systems(Update, (movement, jump, load_level_request, join));
    }
}

/// Resolves which entity a `Movement`/`Jump` request applies to from the sending connection
/// itself, per `ClientId::entity()` — never from anything the client's payload claims. See
/// `on_level_ready` below: the client's own connection entity *is* its player character.
fn movement(receivers: Query<(Entity, &mut MessageReceiver<Movement>)>, mut commands: Commands) {
    for (entity, mut receiver) in receivers {
        for request in receiver.receive() {
            commands.trigger(MovementInput {
                entity,
                direction: request.direction,
            });
        }
    }
}

fn jump(receivers: Query<(Entity, &mut MessageReceiver<Jump>)>, mut commands: Commands) {
    for (entity, mut receiver) in receivers {
        for _request in receiver.receive() {
            commands.trigger(JumpInput { entity });
        }
    }
}

fn load_level_request(
    receivers: Query<(Entity, &mut MessageReceiver<LoadLevelRequest>)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for (entity, mut receiver) in receivers {
        for request in receiver.receive() {
            info!("load level request received for {}", &request.id);
            let id = request.id.clone();
            if !asset_exists(&asset_server, &id) {
                info!("level {} doesn't exit", id);
                return Ok(());
            }
            commands.set_state(ServerState::Loading);
            let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset(id.clone()));
            commands
                .spawn((
                    LevelRoot,
                    Replicate::to_clients(NetworkTarget::All),
                    Transform::IDENTITY,
                ))
                .with_children(|parent| {
                    parent.spawn(WorldAssetRoot(handle)).observe(on_level_ready);
                });
            sender.send::<LoadLevel, OrderedReliable>(
                &LoadLevel { id },
                &server,
                &NetworkTarget::All,
            )?;
        }
    }
    Ok(())
}

fn on_level_ready(
    _ready: On<WorldInstanceReady>,
    server_state: Res<State<ServerState>>,
    mut commands: Commands,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    if !matches!(server_state.get(), ServerState::Loading) {
        return Ok(());
    }
    commands.set_state(ServerState::InGame);
    sender.send::<ServerInGame, OrderedReliable>(&ServerInGame, &server, &NetworkTarget::All)?;
    // The client's own connection entity *is* its player character (see `on_movement`/`on_jump`
    // above, which resolve straight off `ClientId::entity()`) — no separate client->player
    // mapping needed. Only clients already connected when the level finishes loading get a
    // character this way; one connecting later would need its own spawn path, not added here.
    Ok(())
}

fn join(
    receivers: Query<(Entity, &mut MessageReceiver<Join>)>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    mut commands: Commands,
) {
    for (entity, mut receiver) in receivers {
        for request in receiver.receive() {
            if let Ok(player_spawner_transform) = player_spawner.single() {
                spawn_player_for_client(
                    entity,
                    request.name,
                    player_spawner_transform.translation,
                    &mut commands,
                );
            }
        }
    }
}

/// Inserts the `player(...)` bundle onto `client_entity` (the client's own connection entity
/// doubles as its player character — see `on_movement`/`on_jump`) and tells that one client which
/// entity is theirs via a targeted `PlayerSpawned`, so client-side code doesn't have to guess
/// which of the (possibly several, once other players are connected) replicated
/// `PlayerCharacter` entities is its own.
fn spawn_player_for_client(client_entity: Entity, name: String, at: Vec3, commands: &mut Commands) {
    commands.entity(client_entity).insert((
        player(name, at),
        Replicate::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: client_entity,
            lifetime: Lifetime::Persistent,
        },
    ));
}

/// Spawns the server's own connection entity — `NetcodeServer` (the connect-token/handshake
/// layer, replacing the old self-signed cert; see `PROTOCOL_ID`/`PRIVATE_KEY` above) plus
/// `LocalAddr`/`ServerUdpIo` (the actual UDP socket, replacing `bevy_quinnet`'s QUIC endpoint) —
/// then triggers `server::Start` to bind it. Binding itself happens later, asynchronously, in
/// `ServerUdpPlugin`'s own `LinkStart` observer — unlike the old `.expect("server endpoint should
/// bind")`, a bind failure (e.g. the port already in use) is not currently surfaced as a panic
/// here; `on_server_started` below only fires once binding actually succeeds.
fn start_endpoint(mut commands: Commands) {
    let server_entity = commands
        .spawn((
            server::NetcodeServer::new(server::NetcodeConfig {
                protocol_id: PROTOCOL_ID,
                private_key: PRIVATE_KEY,
                ..default()
            }),
            LocalAddr(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), PORT)),
            server::ServerUdpIo::default(),
        ))
        .observe(on_server_started)
        .id();
    commands.trigger(server::Start {
        entity: server_entity,
    });
}

/// Fires once `NetcodeServer`'s underlying UDP socket has actually bound — see `start_endpoint`'s
/// doc comment for why this can't just log unconditionally right after spawning.
fn on_server_started(_started: On<Add, server::Started>) {
    info!("listening for clients on 0.0.0.0:{PORT}");
}

fn on_authorized_client_connected(add: On<Add, server::ClientOf>) {
    info!("authorized client `{}` connected", add.entity);
}

fn on_client_connected(add: On<Add, LinkOf>) {
    info!("client `{}` connected", add.entity);
}

fn on_client_disconnected(disconnected: On<Add, Disconnected>) {
    info!("client `{}` disconnected", disconnected.entity);
}
