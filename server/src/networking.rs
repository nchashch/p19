//! Opens the authoritative QUIC (`bevy_quinnet`) endpoint clients connect to, and drives the
//! scaffold's one demo entity so there's something server-authoritative to observe replicating.

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfLoaderSettings;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use lightyear::prelude::*;
use shared::assets::level::Level;
use shared::client_events::{InGameRequest, LobbyRequest};
use shared::level::LobbyRoot;
use shared::replication::ClientInGame;
use shared::{
    character_controller::{JumpInput, MovementInput},
    client_events::{Jump, LoadLevelRequest, Movement},
    game_state::ServerState,
    level::InGameRoot,
    player::{PlayerCharacterSpawner, player},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::level_state::LevelState;
use crate::rooms::{GameRoom, LobbyRoom};

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
        app.add_systems(
            Update,
            (movement, jump, in_game_request).run_if(in_state(ServerState::InGame)),
        );
        app.add_systems(Update, load_level_request);
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

// TODO: Gate this on some form of authentication and authorization, so only game host can load
// levels at will, or perhaps people the host has given the rights to change level.
fn load_level_request(
    receivers: Query<(Entity, &mut MessageReceiver<LoadLevelRequest>)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    levels: Res<Assets<Level>>,
    in_game_root: Single<Entity, With<InGameRoot>>,
) -> Result {
    for (_entity, mut receiver) in receivers {
        for request in receiver.receive() {
            let asset_path = request.asset_path.clone();
            info!("load level request received for {asset_path}");
            commands.set_state(ServerState::Loading);
            let Some(handle) = asset_server.get_handle::<Level>(&asset_path) else {
                info!("level {asset_path} doesn't exit");
                return Ok(());
            };
            let Some(level) = levels.get(&handle) else {
                info!("failed to load metadata for level {asset_path}");
                return Ok(());
            };
            dbg!(&level);
            // TODO: Add a script or some other kind of step/stage to the assets pipeline that would
            // strip .glb files of all meshes, textures, materials -- anything visual and not
            // strictly necessary for server side logic -- for the .glb files in the server assets.
            // This would make it cheaper to provision servers in terms of storage for large levels.
            let model: Handle<WorldAsset> = asset_server
                .load_builder()
                .with_settings(|settings: &mut GltfLoaderSettings| {
                    settings.load_meshes = RenderAssetUsages::empty();
                    settings.load_materials = RenderAssetUsages::empty();
                })
                .load(GltfAssetLabel::Scene(0).from_asset(&level.model));
            commands.entity(*in_game_root).with_children(|parent| {
                parent.spawn(WorldAssetRoot(model)).observe(on_level_ready);
            });
        }
    }
    Ok(())
}

fn on_level_ready(
    _ready: On<WorldInstanceReady>,
    server_state: Res<State<ServerState>>,
    mut commands: Commands,
) -> Result {
    if !matches!(server_state.get(), ServerState::Loading) {
        return Ok(());
    }
    info!("server is in game");
    commands.set_state(ServerState::InGame);
    Ok(())
}

fn in_game_request(
    receivers: Query<(Entity, &mut MessageReceiver<InGameRequest>)>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    in_game_root: Single<Entity, With<InGameRoot>>,
    game_room: Res<GameRoom>,
    mut commands: Commands,
) {
    for (entity, mut receiver) in receivers {
        for _request in receiver.receive() {
            if let Ok(player_spawner_transform) = player_spawner.single() {
                let name = "player name".to_string();
                let at = player_spawner_transform.translation;
                let room = game_room.0;
                commands.entity(entity).insert(Rooms::single(room));
                commands.spawn((
                    player(name, at),
                    Replicate::to_clients(NetworkTarget::All),
                    ControlledBy {
                        owner: entity,
                        lifetime: Lifetime::Persistent,
                    },
                    ClientInGame,
                    ChildOf(*in_game_root),
                ));
                info!("player character spawned");
            }
        }
    }
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

fn on_authorized_client_connected(add: On<Add, server::ClientOf>, mut commands: Commands) {
    commands.entity(add.entity).insert(ReplicationSender);
    info!("authorized client `{}` connected", add.entity);
}

fn on_client_connected(add: On<Add, LinkOf>) {
    info!("client `{}` connected", add.entity);
}

fn on_client_disconnected(disconnected: On<Add, Disconnected>) {
    info!("client `{}` disconnected", disconnected.entity);
}
