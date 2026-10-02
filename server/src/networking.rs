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
use std::sync::Arc;
use rustls::pki_types::CertificateDer;
use sha2::Digest;

use crate::level_state::LevelState;
use crate::rooms::{GameRoom, LobbyRoom};
use crate::replay::{RecordedMessage, ReplayRecorder};

/// Netcode protocol version tag (ASCII `"proto19"`) — part of the netcode standard's
/// anti-downgrade/replay mix, not a secret. Must only match the *token issuer's* value, which
/// is this same server (the client never needs it: it receives pre-encrypted connect tokens).
const PROTOCOL_ID: u64 = 0x7072_6F74_6F31_39;

/// Port of the token-issuing HTTP endpoint (`GET /connect_token`). Kept separate from the game
/// port on purpose — in production this is the path that becomes an HTTPS request to a real
/// backend; see [`start_token_http_endpoint`].
const TOKEN_HTTP_PORT: u16 = 6001;

/// Where the netcode private key persists, relative to the asset root (`server/assets/`).
/// Plain-text hex — dev/LAN posture per the user's direction; not world-readable permissions
/// or secret management. Created with a fresh random key on first run.
const PRIVATE_KEY_FILE: &str = "assets/netcode.key";

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode_32(text: &str) -> Option<[u8; 32]> {
    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

/// Reads the netcode private key from [`PRIVATE_KEY_FILE`] (hex text), creating it with a fresh
/// random key (`lightyear::netcode::generate_key()`) on first run. The file is the only place
/// the key lives server-side; clients never see it — they receive pre-encrypted connect tokens
/// over the token HTTP endpoint.
fn load_or_create_private_key() -> Result<lightyear::netcode::Key, String> {
    let path = std::path::Path::new(&std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".into()))
        .join(PRIVATE_KEY_FILE);
    if let Ok(text) = std::fs::read_to_string(&path) {
        let key = hex_decode_32(text.trim())
            .ok_or_else(|| format!("{} is not 32 bytes of hex", path.display()))?;
        return Ok(key);
    }
    let key = lightyear::netcode::generate_key();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", path.display()))?;
    }
    std::fs::write(&path, hex_encode(&key))
        .map_err(|e| format!("writing {}: {e}", path.display()))?;
    info!("generated new netcode private key at {}", path.display());
    Ok(key)
}

/// TLS identity (self-signed) for the token HTTPS endpoint: cert + key persist under
/// `server/assets/` (`token-tls.crt`/`token-tls.key` — PEM, generated on first run via
/// `rcgen`). SANs cover `localhost`/`127.0.0.1` plus the default-route local IP; clients don't
/// verify the hostname anyway — they pin the certificate's SHA-256 fingerprint (TOFU, see the
/// client's `fetch_connect_token`) — but complete SANs keep standard tooling (`curl -k`)
/// warnings meaningful. Returns the server TLS config plus the leaf fingerprint (hex, logged
/// at startup so an operator can pre-pin clients).
fn load_or_create_tls_identity()
-> Result<(std::sync::Arc<rustls::ServerConfig>, String), String> {
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".into());
    let cert_path = std::path::Path::new(&root).join("assets/token-tls.crt");
    let key_path = std::path::Path::new(&root).join("assets/token-tls.key");
    let (cert_pem, key_pem) =
        if let (Ok(cert), Ok(key)) = (std::fs::read_to_string(&cert_path), std::fs::read_to_string(&key_path)) {
            (cert, key)
        } else {
            let mut subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
            if let Some(local_ip) = default_local_ip() {
                subject_alt_names.push(local_ip.to_string());
            }
            let certified_key = rcgen::generate_simple_self_signed(subject_alt_names)
                .map_err(|error| format!("generating self-signed token cert: {error:?}"))?;
            let cert_pem = certified_key.cert.pem();
            let key_pem = certified_key.key_pair.serialize_pem();
            std::fs::write(&cert_path, &cert_pem)
                .map_err(|error| format!("writing {}: {error}", cert_path.display()))?;
            std::fs::write(&key_path, &key_pem)
                .map_err(|error| format!("writing {}: {error}", key_path.display()))?;
            info!("generated new self-signed token TLS identity (cert fingerprint below)");
            (cert_pem, key_pem)
        };

    let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<_, _>>()
        .map_err(|error| format!("parsing {certificate}: {error}", certificate = cert_path.display()))?;
    let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
        .map_err(|error| format!("parsing {}: {error}", key_path.display()))?
        .ok_or_else(|| format!("{} contains no private key", key_path.display()))?;
    let fingerprint = hex_encode(&sha2::Sha256::digest(certs[0].as_ref()));
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| format!("tls protocol versions: {error}"))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|error| format!("tls identity: {error}"))?;
    Ok((std::sync::Arc::new(config), fingerprint))
}

/// Best-effort default-route local IP (UDP `connect` doesn't send packets — it just picks the
/// interface the route table would use), for the token cert's SANs.
fn default_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind(std::net::SocketAddr::from(([0, 0, 0, 0], 0))).ok()?;
    socket
        .connect(std::net::SocketAddr::from(([8, 8, 8, 8], 80)))
        .ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

/// The IP a client should use to reach the *game* server, derived from its token request.
/// The request's `Host` header is authoritative: it names the exact address the client used
/// to reach this endpoint, and the game server is reachable on that same IP (only the port
/// differs). This matters on LAN/multi-homed setups — using the *peer's* IP instead (a
/// plausible-looking but wrong choice, correct only on loopback) told the client the "server"
/// was at its own address, and the netcode handshake knocked on the client's own door
/// forever (bug_0005). Fallback chain: `Host` header IP → this host's default-route IP → the
/// peer's IP (loopback-only last resort).
fn token_server_ip(request: &str, peer: std::net::SocketAddr) -> std::net::IpAddr {
    for line in request.lines() {
        let Some(host) = line
            .strip_prefix("Host: ")
            .or_else(|| line.strip_prefix("host: "))
        else {
            continue;
        };
        // `Host` is `ip:port` for IPv4 configs (the only supported client addressing); strip
        // the port and parse. A hostname falls through to the default-route IP.
        if let Some(ip_text) = host.rsplit_once(':').map(|(ip, _)| ip) {
            if let Ok(ip) = ip_text.parse::<std::net::IpAddr>() {
                return ip;
            }
        }
        break;
    }
    default_local_ip().unwrap_or(peer.ip())
}

/// Minimal HTTPS token issuer (`GET /connect_token` on [`TOKEN_HTTP_PORT`], TLS via the
/// self-signed identity from [`load_or_create_tls_identity`]) so headless clients can fetch a
/// fresh connect token without holding the private key — the "basic flow"; in production this
/// endpoint is what an HTTPS request to a real backend (or an asymmetric LAN key-exchange)
/// replaces. Per request it mints a fresh netcode connect token whose *public server address*
/// is the HTTP requester's own IP + the game port, so the token's server-address whitelist
/// matches exactly the address the client will play on. A monotonic counter provides the
/// per-client netcode `client_id`.
fn start_token_http_endpoint(
    identity: std::sync::Arc<rustls::ServerConfig>,
    private_key: lightyear::netcode::Key,
) {
    const CONNECT_TOKEN_BYTES: usize = 2048;
    static NEXT_CLIENT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, TOKEN_HTTP_PORT))
        .expect("token HTTPS endpoint should bind");
    info!("token https endpoint listening on 0.0.0.0:{TOKEN_HTTP_PORT}");
    std::thread::Builder::new()
        .name("netcode-token-https".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(tcp) = stream else { continue };
                let Ok(peer) = tcp.peer_addr() else { continue };
                // Blocking TLS over the raw TCP stream (no async runtime — the endpoint is a
                // plain accept thread; `StreamOwned` gives Read/Write directly).
                let Ok(server_connection) = rustls::ServerConnection::new(identity.clone()) else {
                    continue;
                };
                let mut stream = rustls::StreamOwned::new(server_connection, tcp);
                let mut request = [0u8; 1024];
                // Best-effort read; a single segment always contains the request line.
                let _ = std::io::Read::read(&mut stream, &mut request);
                let request = String::from_utf8_lossy(&request);
                // Build the whole HTTP response first, then write it once and send TLS
                // `close_notify` before dropping the stream — rustls clients error with
                // `UnexpectedEof` on read-to-end if the connection just closes.
                let response = if !request.starts_with("GET /connect_token") {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                } else {
                    // The token's public server address = the address the client used to
                    // reach this endpoint (its `Host` header) + the game port — see
                    // `token_server_ip` for why the peer's own IP would be wrong everywhere
                    // except loopback.
                    let server_addr =
                        std::net::SocketAddr::new(token_server_ip(&request, peer), PORT);
                    let client_id =
                        NEXT_CLIENT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    match lightyear::netcode::ConnectToken::build(
                        server_addr,
                        PROTOCOL_ID,
                        client_id,
                        private_key,
                    )
                    .expire_seconds(30)
                    .generate()
                    {
                        Ok(token) => match token.try_into_bytes() {
                            Ok(bytes) => {
                                let mut response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    bytes.len()
                                )
                                .into_bytes();
                                response.extend_from_slice(&bytes);
                                info!("issued connect token #{client_id} to {peer}");
                                response
                            }
                            Err(error) => {
                                warn!("connect token serialization failed: {error:?}");
                                b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                                    .to_vec()
                            }
                        },
                        Err(error) => {
                            warn!("connect token generation failed: {error:?}");
                            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                                .to_vec()
                        }
                    }
                };
                let ok = response.starts_with(b"HTTP/1.1 200");
                let _ = std::io::Write::write_all(&mut stream, &response);
                let _ = std::io::Write::flush(&mut stream);
                if ok {
                    // Signals a clean end-of-stream so the client's read_to_end succeeds.
                    stream.conn.send_close_notify();
                    let _ = std::io::Write::flush(&mut stream);
                }
            }
        })
        .expect("token HTTPS thread should spawn");
}

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
    let private_key = match load_or_create_private_key() {
        Ok(key) => key,
        Err(error) => {
            panic!("netcode private key unavailable: {error}");
        }
    };
    let (tls_identity, tls_fingerprint) = match load_or_create_tls_identity() {
        Ok(identity) => identity,
        Err(error) => {
            panic!("token TLS identity unavailable: {error}");
        }
    };
    info!("token TLS cert fingerprint (sha256): {tls_fingerprint}");
    start_token_http_endpoint(tls_identity, private_key);
    // The netcode server validates connect tokens against the address it is bound on —
    // `LocalAddr` below is `0.0.0.0:6000`, a wildcard that no concrete token address can
    // match (`server_addr_matches` compares exact IPs; its loopback special-cases don't help
    // LAN clients). Tokens legitimately contain the concrete address the client used to
    // reach the token endpoint (its `Host` header = this host's LAN IP for LAN clients,
    // 127.0.0.1 for local ones), so advertise exactly those as expected.
    let mut additional_expected_addresses =
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT)];
    if let Some(local_ip) = default_local_ip() {
        additional_expected_addresses.push(SocketAddr::new(local_ip, PORT));
    }
    let server_entity = commands
        .spawn((
            server::NetcodeServer::new(server::NetcodeConfig {
                protocol_id: PROTOCOL_ID,
                private_key,
                // The token-issuing endpoint (see `start_token_http_endpoint`) embeds the
                // address the client used to reach it, and `additional_expected_addresses`
                // above whitelists this host's concrete addresses for the validation — the
                // standard, secure posture, satisfiable despite the wildcard bind. This is
                // why the check used to be disabled: the old flow had every client mint its
                // own token with `0.0.0.0`, which never matched a real interface address.
                server_addr_check: true,
                additional_expected_addresses,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_header_ip_wins() {
        let request = "GET /connect_token HTTP/1.1\r\nHost: 192.168.101.5:6001\r\n\r\n";
        let peer = "192.168.101.6:36272".parse().unwrap();
        assert_eq!(
            token_server_ip(request, peer),
            "192.168.101.5".parse::<std::net::IpAddr>().unwrap()
        );
    }

    #[test]
    fn loopback_host_header() {
        let request = "GET /connect_token HTTP/1.1\r\nHost: 127.0.0.1:6001\r\n\r\n";
        let peer = "127.0.0.1:40000".parse().unwrap();
        assert_eq!(
            token_server_ip(request, peer),
            "127.0.0.1".parse::<std::net::IpAddr>().unwrap()
        );
    }

    #[test]
    fn missing_host_falls_back_to_default_route_then_peer() {
        let request = "GET /connect_token HTTP/1.1\r\n\r\n";
        let peer: std::net::SocketAddr = "192.168.101.6:36272".parse().unwrap();
        // Chain: Host header (absent here) -> this host's default-route IP -> peer IP.
        let expected = default_local_ip().unwrap_or(peer.ip());
        assert_eq!(token_server_ip(request, peer), expected);
    }
}
