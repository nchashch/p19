//! Replicates the server's available level list (`shared::level::Levels`) to clients while
//! they're in the lobby — see `rooms::LobbyRoom`'s doc comment for the room this rides on.

use bevy::prelude::*;
use lightyear::prelude::*;
use shared::assets::level::{Level, LevelMetadataAssets};
use shared::game_state::ServerState;
use shared::level::Levels;

use crate::rooms::LobbyRoom;

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Lobby), spawn_levels_list);
        app.add_observer(join_lobby_room_on_connect);
    }
}

/// Builds and replicates the `Levels` list once, from `LevelMetadataAssets` — the folder
/// collection of every `.level.ron` under `assets/levels/`, already loaded by the time
/// `ServerState::Lobby` is entered (see `main.rs`'s `LoadingState` chain). Each entry pairs a
/// level's own asset path (recovered via `AssetServer::get_path`, the standard way to go from a
/// handle back to the source path it was loaded from) with a clone of its `Level` metadata — the
/// exact `AssetPath` a client should echo straight back in a `LoadLevelRequest`.
///
/// Tagged with `Rooms::single(lobby_room)` so only clients currently in the lobby room ever
/// receive it — see `rooms::LobbyRoom`'s own doc comment.
fn spawn_levels_list(
    mut commands: Commands,
    metadata: Res<LevelMetadataAssets>,
    levels: Res<Assets<Level>>,
    asset_server: Res<AssetServer>,
    lobby_room: Res<LobbyRoom>,
) {
    let levels: Vec<_> = metadata
        .levels
        .iter()
        .filter_map(|handle| {
            let path = asset_server.get_path(handle.id())?.into_owned();
            let level = levels.get(handle)?.clone();
            Some((path, level))
        })
        .collect();
    commands.spawn((
        Levels::new(levels),
        Replicate::to_clients(NetworkTarget::All),
        Rooms::single(lobby_room.0),
    ));
}

/// Every newly connected client joins the lobby room immediately — see `rooms::LobbyRoom`'s doc
/// comment for why "just connected" and "in the lobby" are the same moment in this project's
/// current flow. Mirrors `networking.rs`'s `on_authorized_client_connected` (same trigger,
/// `server::ClientOf` being added is what marks a connection as authorized) — kept as its own
/// observer here rather than folded into that one, so lobby-room membership stays owned by this
/// module rather than spreading room bookkeeping across `networking.rs` too.
fn join_lobby_room_on_connect(
    add: On<Add, server::ClientOf>,
    lobby_room: Res<LobbyRoom>,
    mut commands: Commands,
) {
    commands.entity(add.entity).insert(Rooms::single(lobby_room.0));
}
