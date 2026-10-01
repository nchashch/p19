//! Opens the authoritative UDP/netcode endpoint clients connect to, plus the server-side
//! in-game flow (level loading, player spawning).

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfLoaderSettings;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use lightyear::prelude::*;
use shared::assets::level::{ClientReplicate, Level};
use shared::client_events::{ClientDespawn, InGameRequest, LoadLevelRequest, ObserveRequest};
use shared::player::PlayerCharacter;
use shared::replication::ClientInGame;
use shared::{
    game_state::ServerState,
    level::InGameRoot,
    player::{PlayerCharacterSpawner, player},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::level_state::LevelState;
use crate::rooms::{GameRoom, LobbyRoom};
use crate::replay::{RecordedMessage, ReplayRecorder};

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
            (in_game_request, observe_request, client_despawn).run_if(in_state(ServerState::InGame)),
        );
        app.add_systems(Update, (load_level_request, setup_client_replicate));
    }
}

/// The player(s) belonging to a connection, for cleanup: everything the server spawned for
/// that client's `player()` bundle is `ControlledBy { owner: <connection> }` — including the
/// `Lifetime::Persistent` player that would otherwise linger after the client leaves.
/// Also the caster-resolution path for `server::spawn` (the connection has no `Gcd` — the
/// player character it owns does).
pub(crate) fn owned_players(
    connection: Entity,
    controlled: Query<(Entity, &ControlledBy)>,
) -> Vec<Entity> {
    controlled
        .iter()
        .filter(|(_, controlled_by)| controlled_by.owner == connection)
        .map(|(entity, _)| entity)
        .collect()
}

/// Handles `ClientDespawn` — the client saying "my player should go away now" (it is leaving
/// the game / shutting down while the connection is still alive, so the message rides it).
/// Despawns everything the server spawned for that connection. Complements
/// [`cleanup_disconnected_players`]: the message path is immediate for graceful leaves, the
/// `Disconnected`-observer path catches every other kind of disconnect (netcode timeout,
/// hard crash) once lightyear notices.
///
/// The actual work is [`apply_client_despawn`], extracted so `server::replay`'s replay driver
/// can call the exact same code path against a recorded [`ClientDespawn`] instead of a live
/// [`MessageReceiver`]-drained one.
pub(crate) fn apply_client_despawn(
    connection: Entity,
    controlled: &Query<(Entity, &ControlledBy)>,
    commands: &mut Commands,
) {
    let owned = owned_players(connection, *controlled);
    info!("client `{connection}` despawn request: dropping {owned:?}");
    for player in owned {
        commands.entity(player).despawn();
    }
}

fn client_despawn(
    receivers: Query<(Entity, &mut MessageReceiver<ClientDespawn>)>,
    controlled: Query<(Entity, &ControlledBy)>,
    mut commands: Commands,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) {
    for (entity, mut receiver) in receivers {
        for _request in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    entity,
                    RecordedMessage::Despawn(ClientDespawn),
                );
            }
            apply_client_despawn(entity, &controlled, &mut commands);
        }
    }
}

/// Handles `ObserveRequest` — the player-free counterpart to `in_game_request`: joins the
/// sending client to the game room (so it receives all replicated world state) but spawns
/// **no player character** and no `ClientInGame`. Used by observer clients
/// (`--headless-render` agent hosts) that exist to render the shared world on demand.
fn observe_request(
    receivers: Query<(Entity, &mut MessageReceiver<ObserveRequest>)>,
    game_room: Res<GameRoom>,
    mut commands: Commands,
) {
    for (entity, mut receiver) in receivers {
        for _request in receiver.receive() {
            info!("client `{entity}` is now observing (no player spawned)");
            commands.entity(entity).insert(Rooms::single(game_room.0));
        }
    }
}

fn setup_client_replicate(entities: Query<Entity, With<ClientReplicate>>, mut commands: Commands) {
    for entity in entities {
        commands
            .entity(entity)
            .remove::<ClientReplicate>()
            .insert(Replicate::to_clients(
                lightyear::connection::network_target::Target::All,
            ));
    }
}

// TODO: Gate this on some form of authentication and authorization, so only game host can load
// levels at will, or perhaps people the host has given the rights to change level.
/// The per-message resolution logic, extracted out of [`load_level_request`] so
/// `server::replay`'s replay driver can call the exact same code path against a recorded
/// [`LoadLevelRequest`] instead of a live [`MessageReceiver`]-drained one.
pub(crate) fn apply_load_level_request(
    asset_path: bevy::asset::AssetPath<'static>,
    asset_server: &AssetServer,
    commands: &mut Commands,
    levels: &Assets<Level>,
    in_game_root: Entity,
    level_state: &mut LevelState,
) -> Result {
    // Reject a second `LoadLevelRequest` while one is already loading/loaded — without
    // this, a double-fired UI button, a retried packet, or a client re-picking a level
    // after reconnecting spawns a second `InGameRoot`/`WorldAssetRoot` into the same
    // world on top of the first: duplicate colliders, duplicate `PlayerCharacterSpawner`s
    // (confirmed live — 9 accumulated after 9 un-deduplicated requests in one session),
    // and downstream chaos (`in_game_request`'s `player_spawner.single()` starts failing
    // once >1 spawner exists, and overlapping duplicate geometry destabilizes the KCC).
    // Switching to a genuinely different level isn't supported yet either way (the server
    // never despawns a previous level's geometry on reload), so any request beyond the
    // first is rejected regardless of id, matching `LevelState`'s own doc comment.
    if *level_state != LevelState::Idle {
        info!("load level request for {asset_path} rejected: level already {level_state:?}");
        return Ok(());
    }
    info!("load level request received for {asset_path}");
    commands.set_state(ServerState::Loading);
    *level_state = LevelState::Loading(asset_path.clone());
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
    commands.entity(in_game_root).with_children(|parent| {
        parent.spawn(WorldAssetRoot(model)).observe(on_level_ready);
    });
    Ok(())
}

fn load_level_request(
    receivers: Query<(Entity, &mut MessageReceiver<LoadLevelRequest>)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    levels: Res<Assets<Level>>,
    in_game_root: Single<Entity, With<InGameRoot>>,
    mut level_state: ResMut<LevelState>,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) -> Result {
    for (entity, mut receiver) in receivers {
        for request in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    entity,
                    RecordedMessage::LoadLevel(request.clone()),
                );
            }
            apply_load_level_request(
                request.asset_path.clone(),
                &asset_server,
                &mut commands,
                &levels,
                *in_game_root,
                &mut level_state,
            )?;
        }
    }
    Ok(())
}

fn on_level_ready(
    _ready: On<WorldInstanceReady>,
    server_state: Res<State<ServerState>>,
    mut level_state: ResMut<LevelState>,
    mut commands: Commands,
) -> Result {
    if !matches!(server_state.get(), ServerState::Loading) {
        return Ok(());
    }
    info!("server is in game");
    if let LevelState::Loading(path) = &*level_state {
        *level_state = LevelState::LevelLoaded(path.clone());
    }
    commands.set_state(ServerState::InGame);
    Ok(())
}

/// The per-message resolution logic, extracted out of [`in_game_request`] so `server::replay`'s
/// replay driver can call the exact same code path against a recorded [`InGameRequest`] instead
/// of a live [`MessageReceiver`]-drained one.
pub(crate) fn apply_in_game_request(
    entity: Entity,
    name_seed: u32,
    names: &Query<&Name, With<PlayerCharacter>>,
    player_spawner: &Query<&Transform, With<PlayerCharacterSpawner>>,
    in_game_root: Entity,
    game_room: &GameRoom,
    remote_ids: &Query<&RemoteId>,
    commands: &mut Commands,
) {
    if let Ok(player_spawner_transform) = player_spawner.single() {
        // Unique random two-word name (e.g. "Brisk Falcon", "Brisk Falcon #2" on collision) —
        // every joiner used to be hardcoded to `"player name"`, which broke name-based
        // targeting (`game/select {name}` answered "ambiguous") and nameplate/kill-feed
        // semantics. `taken` is every existing player's `Name`.
        let taken: std::collections::HashSet<String> =
            names.iter().map(Name::to_string).collect();
        let name = shared::player::generate_player_name(name_seed, &taken);
        let at = player_spawner_transform.translation;
        let room = game_room.0;
        commands.entity(entity).insert(Rooms::single(room));
        // The owning client predicts this entity (its local ahoy sim becomes the
        // prediction, reconciled by lightyear's rollback); `PredictionTarget`
        // materializes as `Predicted` on that client's received entity. Other
        // clients currently just get the plain replicated entity (their Transform
        // follows the replicated `Position` via lightyear_avian's sync).
        let own_client = remote_ids
            .get(entity)
            .map(|remote| NetworkTarget::Single(remote.0))
            .unwrap_or(NetworkTarget::None);
        commands.spawn((
            player(name, at),
            Replicate::to_clients(NetworkTarget::All),
            PredictionTarget::to_clients(own_client),
            ControlledBy {
                owner: entity,
                lifetime: Lifetime::Persistent,
            },
            ClientInGame,
            ChildOf(in_game_root),
        ));
        info!("player character spawned");
    }
}

fn in_game_request(
    receivers: Query<(Entity, &mut MessageReceiver<InGameRequest>)>,
    names: Query<&Name, With<PlayerCharacter>>,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    in_game_root: Single<Entity, With<InGameRoot>>,
    game_room: Res<GameRoom>,
    remote_ids: Query<&RemoteId>,
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) {
    for (entity, mut receiver) in receivers {
        for _request in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    entity,
                    RecordedMessage::InGame(InGameRequest),
                );
            }
            apply_in_game_request(
                entity,
                // Same replay-determinism seeding convention as `spawn.rs`'s RNG: tick XOR
                // connection-entity bits.
                timeline.tick().0 ^ entity.to_bits() as u32,
                &names,
                &player_spawner,
                *in_game_root,
                &game_room,
                &remote_ids,
                &mut commands,
            );
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
                // The server binds `0.0.0.0` (see `LocalAddr` below), but a real client on the
                // LAN connects to a concrete interface IP (e.g. `192.168.x.x:6000`, from its own
                // `config.toml`'s `server_ip`) — its self-signed connect token embeds that
                // concrete address, which never equals `0.0.0.0` and gets silently dropped by
                // `NetcodeServer`'s default same-address check ("server ignored connection
                // request. server address not in connect token whitelist" in the server log).
                // Hardcoding that IP into `additional_expected_addresses` would just replace one
                // fragile assumption with another (DHCP reassigns it, another client may connect
                // over a different interface) — disabling the check entirely is the correct fix
                // for this dev/LAN scaffold, matching the already-hardcoded zero
                // `PROTOCOL_ID`/`PRIVATE_KEY` above.
                server_addr_check: false,
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

fn on_client_disconnected(
    disconnected: On<Add, Disconnected>,
    controlled: Query<(Entity, &ControlledBy)>,
    mut commands: Commands,
) {
    info!("client `{}` disconnected", disconnected.entity);
    // Zombie-player fix: the player bundle is `Lifetime::Persistent` (lightyear deliberately
    // does not despawn it when the owner disconnects) — so without this, every client that
    // leaves leaves its player behind in the world forever (playtest 0011 F4 measured four
    // static corpses polluting one session). `Disconnected` covers graceful netcode
    // disconnects *and* timeouts after a hard crash; the immediate path for clean leaves is
    // the `ClientDespawn` message.
    let owned = owned_players(disconnected.entity, controlled);
    if !owned.is_empty() {
        info!(
            "client `{}` disconnected: despawning {owned:?}",
            disconnected.entity
        );
        for player in owned {
            commands.entity(player).despawn();
        }
    }
}
