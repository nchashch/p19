//! Opens the UDP/netcode connection to the authoritative server (replacing the old
//! QUIC/`bevy_quinnet` endpoint) and drives the post-connect state transition.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bevy::prelude::*;
use lightyear::prelude::*;
use rustls::pki_types::CertificateDer;
use p19_shared::client_events::ClientDespawn;
use p19_shared::game_state::GameState;
use p19_shared::replication::ClientInGame;

use crate::config::load_client_config;
use crate::events::{Connect, Disconnect};

const SERVER_PORT: u16 = 6000;

/// Port of the server's token-issuing HTTP endpoint (`p19_server::networking`'s
/// `start_token_http_endpoint`). In production this becomes an HTTPS request to a real
/// backend — or an asymmetric LAN key-exchange — so the token can't be stolen in flight on an
/// unsecured LAN; the plain-HTTP request here is the explicitly-acknowledged dev/LAN posture.
const TOKEN_HTTP_PORT: u16 = 6001;

/// Shared state for the in-flight connect-token fetch (see [`poll_token_fetch`]).
#[derive(Resource, Default, Clone)]
struct TokenFetch(std::sync::Arc<std::sync::Mutex<Option<Result<Vec<u8>, String>>>>);

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerAddress>();
        app.init_resource::<TokenFetch>();
        app.add_systems(Startup, (load_client_config, spawn_client_link));
        app.add_systems(Update, poll_token_fetch);
        app.add_observer(on_connected);
        app.add_observer(on_disconnected);
        app.add_observer(on_connect_request);
        app.add_observer(on_disconnect_request);
        app.add_observer(on_in_game);
        app.add_observer(on_out_of_game);
        // `AppExit` is a Message (not an observer event) in bevy 0.19 — a `MessageReader`
        // system in `Update` sees it during the *final* frame, and this runs before
        // lightyear's post-update packet flush, so the `ClientDespawn` enqueued here still
        // rides the socket before the process ends.
        app.add_systems(Update, send_client_despawn_on_exit);
    }
}

fn on_in_game(
    add: On<Add, ClientInGame>,
    is_controlled: Query<Has<Controlled>>,
    mut commands: Commands,
) {
    info!("client in game add received");
    if is_controlled.get(add.entity) == Ok(true) {
        info!("client is now in game");
        commands.set_state(GameState::InGame);
    }
}

fn on_out_of_game(
    remove: On<Remove, Controlled>,
    is_controlled: Query<Has<Controlled>>,
    mut senders: Query<&mut MessageSender<ClientDespawn>>,
    mut commands: Commands,
) {
    if is_controlled.get(remove.entity) == Ok(true) {
        info!("client is now out of game");
        // Normally redundant — this transition *means* the server already despawned our
        // player (that's what removed `Controlled`) — but kept as the user-requested safety
        // net for any future leave path where the client goes to Lobby while its
        // `Lifetime::Persistent` player somehow survives server-side.
        if let Ok(mut sender) = senders.single_mut() {
            sender.send::<p19_shared::replication::OrderedReliable>(ClientDespawn);
        }
        commands.set_state(GameState::Lobby);
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

/// The client's own connection/link entity, spawned once at `Startup` and reused across every
/// connect attempt (including a reconnect after returning to the main menu) — every other system
/// that needs "my own connection" (e.g. `controls.rs`'s `Single<&mut MessageSender<T>>` queries)
/// relies on there being exactly one such entity for the app's whole lifetime, so this must not be
/// a fresh entity per connect. `on_connect_request` is what actually inserts `NetcodeClient`/
/// `UdpIo`/etc. onto it and triggers `Connect`.
#[derive(Resource)]
struct ClientLink(Entity);

fn spawn_client_link(mut commands: Commands) {
    let entity = commands.spawn_empty().id();
    commands.insert_resource(ClientLink(entity));
}

/// Kicks off the connect-token fetch (see [`poll_token_fetch`]) — the actual connection opens
/// once the token arrives. Safe to call every time Connect is pressed: an in-flight fetch or an
/// already-open connection returns early.
fn on_connect_request(
    _: On<Connect>,
    link: Res<ClientLink>,
    server_address: Res<ServerAddress>,
    token_fetch: Res<TokenFetch>,
    status: Query<(
        Has<lightyear::prelude::Connected>,
        Has<lightyear::prelude::Connecting>,
    )>,
    mut commands: Commands,
) -> Result {
    info!("on_connect_request");
    if let Ok((connected, connecting)) = status.get(link.0)
        && (connected || connecting)
    {
        return Ok(());
    }
    if token_fetch.0.lock().expect("token fetch lock").is_some() {
        info!("connect token fetch already in flight");
        return Ok(());
    }
    let host = server_address.0.trim().to_string();
    let host_for_task = host.clone();
    let token_fetch = token_fetch.0.clone();
    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = fetch_connect_token(&host_for_task, TOKEN_HTTP_PORT);
            *token_fetch.lock().expect("token fetch lock") = Some(result);
        })
        .detach();
    info!("fetching connect token from {host}:{TOKEN_HTTP_PORT}…");
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The client asset root ([`p19_shared::paths::asset_dir`] — the same resolution
/// `AssetPlugin` is configured from), so network state (the TLS pin below) follows the game's
/// assets in every launch mode.
fn network_state_dir() -> std::path::PathBuf {
    p19_shared::paths::asset_dir(p19_shared::paths::AssetSide::Client)
}

/// Trust-on-first-use fingerprint store for the token endpoint's self-signed TLS certificate
/// (hex SHA-256 of the leaf DER). First successful connection records the fingerprint;
/// later connections must match, or the fetch fails (MITM / cert-rotation alarm — delete the
/// file to re-trust a legitimately rotated cert).
fn tls_fingerprint_store_path() -> std::path::PathBuf {
    network_state_dir().join("network/token-tls-fingerprint.txt")
}

/// TLS config for the token fetch: self-signed server certs are accepted at the TLS layer and
/// validated *after* the handshake by fingerprint (TOFU, [`tls_fingerprint_store_path`]) — the
/// verifier deliberately accepts everything so the handshake completes and the presented
/// certificate becomes available for the fingerprint check.
fn token_tls_config() -> rustls::ClientConfig {
    #[derive(Debug)]
    struct AcceptAllServerCert;
    impl rustls::client::danger::ServerCertVerifier for AcceptAllServerCert {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer,
            _intermediates: &[CertificateDer],
            _server_name: &rustls::pki_types::ServerName,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("tls protocol versions")
    .dangerous()
    .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAllServerCert))
    .with_no_client_auth()
}

/// Blocking fetch of a netcode connect token from the server's token HTTPS endpoint
/// (`p19_server::networking`'s `start_token_http_endpoint`): `GET /connect_token` over TLS with the
/// server's self-signed certificate, body = the raw 2048-byte encrypted connect token. The
/// certificate is fingerprint-pinned trust-on-first-use ([`tls_fingerprint_store_path`]) — this
/// is what keeps the token un-stealable on the wire after the first connection. In production
/// this is the request that becomes HTTPS to a real backend (proper CA + hostname validation).
fn fetch_connect_token(host: &str, port: u16) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use sha2::Digest;

    let stored_fingerprint: Option<[u8; 32]> = std::fs::read_to_string(tls_fingerprint_store_path())
        .ok()
        .and_then(|text| {
            let mut fingerprint = [0u8; 32];
            for (index, byte) in fingerprint.iter_mut().enumerate() {
                *byte = u8::from_str_radix(text.trim().get(index * 2..index * 2 + 2)?, 16).ok()?;
            }
            Some(fingerprint)
        });

    let stream = std::net::TcpStream::connect((host, port))
        .map_err(|error| format!("tcp connect: {error}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|error| format!("read timeout: {error}"))?;
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|error| format!("server name: {error:?}"))?;
    let mut connection = rustls::ClientConnection::new(
        std::sync::Arc::new(token_tls_config()),
        server_name,
    )
    .map_err(|error| format!("tls client: {error}"))?;
    let mut tls = rustls::StreamOwned::new(connection, stream);

    tls.write_all(
        format!("GET /connect_token HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )
    .map_err(|error| format!("http write: {error}"))?;
    tls.flush().map_err(|error| format!("tls flush: {error}"))?;
    let mut response = Vec::new();
    tls.read_to_end(&mut response)
        .map_err(|error| format!("http read: {error}"))?;

    // Fingerprint enforcement happens after the handshake: the presented leaf certificate's
    // SHA-256 must match the stored TOFU fingerprint (if one exists).
    let presented: Option<[u8; 32]> = tls
        .conn
        .peer_certificates()
        .and_then(|certs| certs.first())
        .map(|certificate| sha2::Sha256::digest(certificate.as_ref()).into());
    match (stored_fingerprint, presented) {
        (Some(stored), Some(presented)) if stored != presented => {
            return Err(format!(
                "token endpoint TLS certificate fingerprint CHANGED since first use (stored {}, now {}) — refusing to send the token request; if the server legitimately rotated its cert, delete {} and retry",
                hex_encode(&stored),
                hex_encode(&presented),
                tls_fingerprint_store_path().display(),
            ));
        }
        (None, Some(presented)) => {
            // First use: pin what we saw.
            let store_path = tls_fingerprint_store_path();
            if let Some(parent) = store_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("creating {}: {error}", parent.display()))?;
            }
            std::fs::write(&store_path, hex_encode(&presented))
                .map_err(|error| format!("writing tls fingerprint: {error}"))?;
            info!(
                "pinned token endpoint TLS fingerprint (first use): {}",
                hex_encode(&presented)
            );
        }
        _ => {}
    }

    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "malformed https response (no header terminator)".to_string())?;
    let body = &response[header_end + 4..];
    if !response.starts_with(b"HTTP/1.1 200 OK") && !response.starts_with(b"HTTP/1.0 200 OK") {
        return Err(format!(
            "token endpoint returned an error: {}",
            String::from_utf8_lossy(&response[..header_end])
        ));
    }
    Ok(body.to_vec())
}

/// Applies a fetched connect token: this is where the connection actually opens. The client
/// holds only the encrypted token — the netcode private key never leaves the server, closing
/// the old `Authentication::Manual` hole (the client used to hold the shared key).
fn apply_connect_token(
    token_bytes: &[u8],
    link: Entity,
    server_address: &ServerAddress,
    mut commands: Commands,
) -> Result {
    let token = lightyear::netcode::ConnectToken::try_from_bytes(token_bytes)
        .map_err(|error| format!("invalid connect token: {error:?}"))?;
    let Ok(addr) = server_address.0.trim().parse::<IpAddr>() else {
        warn!(
            "server address {:?} is not a valid IP address",
            server_address.0
        );
        return Ok(());
    };
    let server_addr = SocketAddr::new(addr, SERVER_PORT);
    let netcode_client = client::NetcodeClient::new(
        Authentication::Token(token),
        client::NetcodeConfig {
            client_timeout_secs: -1,
            token_expire_secs: -1,
            ..default()
        },
    )?;
    commands.entity(link).insert((
        lightyear::prelude::Client,
        ReplicationReceiver,
        LocalAddr(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)),
        PeerAddr(server_addr),
        netcode_client,
        UdpIo::default(),
    ));
    commands.trigger(lightyear::prelude::Connect { entity: link });
    Ok(())
}

/// Drains the in-flight token fetch each frame; on success opens the connection
/// ([`apply_connect_token`]), on failure logs and clears the slot so `Connect` can be
/// re-pressed.
fn poll_token_fetch(
    token_fetch: Res<TokenFetch>,
    link: Res<ClientLink>,
    server_address: Res<ServerAddress>,
    mut commands: Commands,
) {
    let result = token_fetch
        .0
        .lock()
        .expect("token fetch lock")
        .take();
    let Some(result) = result else {
        return;
    };
    match result {
        Ok(bytes) => {
            if let Err(error) = apply_connect_token(&bytes, link.0, &server_address, commands) {
                error!("failed to open connection with fetched token: {error:?}");
            }
        }
        Err(error) => {
            error!("connect token fetch failed: {error} (press connect to retry)");
        }
    }
}

/// Triggers **both** `Disconnect` (the connection/netcode layer — sends a disconnect packet so
/// the server also cleans up its `ClientOf` entity promptly) and `Unlink` (the IO/`Link` layer —
/// closes the actual UDP socket and removes `Linked`). These are two separate layers in lightyear
/// and neither implies the other: confirmed by tracing `NetcodeClientPlugin`'s `Disconnect`
/// handler (`lightyear_netcode`'s `client_plugin.rs`), which only calls `client.inner.disconnect()`
/// and inserts `Disconnected` — nothing on the client side observes that to trigger `Unlink` in
/// turn. Triggering `Disconnect` alone left `Linked` permanently stuck on `ClientLink`'s entity
/// (it's never despawned, see that resource's doc comment) — the next `on_connect_request` then
/// inserted a fresh `UdpIo::default()` (`socket: None`) onto an entity `LinkStart`'s bind handler
/// refuses to touch because it requires `Without<Linked>`, so the stale `Linked` marker never got
/// a real socket rebound under it. `lightyear_udp::UdpPlugin::receive` matches on `(Linked,
/// UdpIo)` alone and unconditionally unwraps `UdpIo::socket` — the observed crash
/// ("`Option::unwrap()` on a `None` value" in `lightyear_udp`) on a second `Connect` after
/// returning to the main menu.
fn on_disconnect_request(
    _: On<Disconnect>,
    link: Res<ClientLink>,
    mut senders: Query<&mut MessageSender<ClientDespawn>>,
    mut commands: Commands,
) {
    // Tell the server to drop our player *before* tearing the connection down, so the
    // message rides the still-live link and its `Lifetime::Persistent` player despawns
    // immediately instead of waiting for the netcode timeout (see `on_app_exit` for the
    // shutdown variant of the same concern).
    if let Ok(mut sender) = senders.single_mut() {
        sender.send::<p19_shared::replication::OrderedReliable>(ClientDespawn);
    }
    commands.trigger(lightyear::prelude::Disconnect { entity: link.0 });
    commands.trigger(Unlink {
        entity: link.0,
        reason: UnlinkReason::UserRequested(None),
    });
}

/// Sends `ClientDespawn` on a clean app shutdown while a connection is still up — the server
/// drops the player immediately rather than at netcode-timeout. Best-effort: the message is
/// enqueued during the exit frame and flushed by lightyear's post-update send systems; a hard
/// kill (`SIGKILL`/`process::exit`) never gets here, and in those cases the server-side
/// `Disconnected`-observer cleanup is the fallback (it fires when the netcode times the dead
/// client out).
fn send_client_despawn_on_exit(
    mut exits: MessageReader<AppExit>,
    link: Res<ClientLink>,
    connected: Query<(), With<lightyear::prelude::Connected>>,
    mut senders: Query<&mut MessageSender<ClientDespawn>>,
) {
    for exit in exits.read() {
        if exit.is_error() {
            continue; // a crashing exit has no graceful-connection guarantees anyway
        }
        if connected.contains(link.0) && let Ok(mut sender) = senders.single_mut() {
            info!("app exit: sending ClientDespawn before shutdown");
            sender.send::<p19_shared::replication::OrderedReliable>(ClientDespawn);
        }
    }
}

fn on_connected(_: On<Add, lightyear::prelude::Connected>, mut commands: Commands) {
    info!("connected to server");
    commands.set_state(GameState::Lobby);
}

/// `Disconnected` is a required component of `NetcodeClient`, so this also fires once the very
/// first time `on_connect_request` inserts `NetcodeClient` (before the connection has actually
/// succeeded or failed) — mirrors the old `bevy_replicon`-era `ClientState::Disconnected` being
/// `#[default]` and firing on the very first frame. Harmless: at that point `GameState` is already
/// `MainMenu` (that's the only place `Connect` is ever triggered from), so `set_state` below is a
/// redundant no-op, not a wrong transition.
fn on_disconnected(
    _: On<Add, lightyear::prelude::Disconnected>,
    mut commands: Commands,
    game_state: Res<State<GameState>>,
) {
    info!("disconnected from server");
    if !matches!(game_state.get(), GameState::AssetLoading) {
        commands.set_state(GameState::MainMenu);
    }
}
