//! Scaffolding for room-based interest management (`lightyear_replication::visibility::room`).
//!
//! `HierarchySendPlugin::<ChildOf>` itself needs no registration here — it's already active
//! automatically. `lightyear::server::ServerPlugins` (added in `main.rs`) pulls in
//! `LightyearRepliconServerBackend`, which unconditionally adds both `HierarchyPlugin` and
//! `HierarchySendPlugin::<ChildOf>` (confirmed by reading `lightyear_replication`'s own plugin
//! wiring). Any `ChildOf`-parented entity under a `Replicate`d root already gets `ReplicateLike`
//! inserted, and (once a root has `Rooms`) inherits that room membership automatically. Adding
//! either of those plugins again here would double-register the same plugin type and panic.
//!
//! What *isn't* wired up anywhere yet is `RoomPlugin` itself (the thing that makes the `Rooms`
//! component do anything at all) — this module adds that, plus allocates the one room the game
//! world currently uses.
//!
//! Server-only: rooms gate what the server *sends*, so there's nothing for the client to
//! register — the same pattern `NetworkVisibilityPlugin` (the analogous plugin for the
//! *immediate*, per-entity visibility API) already follows: it too is only added inside
//! `LightyearRepliconServerBackend`, never client-side.

use bevy::prelude::*;
use lightyear::prelude::*;
use shared::level::{InGameRoot, LobbyRoot};

/// The single room every in-game entity currently belongs to. Only `InGameRoot` is tagged with
/// this directly (see `networking.rs`'s `load_level_request`) — everything parented under it
/// (level geometry, cubes, NPCs — see `spawn.rs`) inherits membership automatically via
/// `HierarchySendPlugin::<ChildOf>`'s cascade, no per-entity tagging needed. Also inserted onto a
/// client's own connection entity once it's actually spawning into the game (see
/// `networking.rs`'s `spawn_player_for_client`).
///
/// Server-only, deliberately: `Rooms` (the component this gates) is never replicated to a client
/// — room membership is server-side interest-management bookkeeping that decides what the server
/// *sends*, not client-visible state — so this type has no reason to live in `shared`. It briefly
/// did, to make a client-side `On<Add, Rooms>` observer compile; reverted once it became clear
/// that observer could never actually fire client-side (the client's own ECS world never contains
/// a `Rooms` component on anything), so the compiling-but-dead code wasn't worth keeping.
#[derive(Resource, Clone, Copy)]
pub struct GameRoom(pub RoomId);

/// The room every connected client is in from the moment it connects — see `crate::lobby`'s
/// `join_lobby_room_on_connect` (client side) and `spawn_levels_list` (the `Levels` entity that's
/// actually gated by it). This project's own `GameState` flow puts a client straight into
/// `GameState::Lobby` as soon as it connects, with no separate "connected but not in the lobby
/// yet" state, so "just connected" and "in the lobby" are the same moment here.
///
/// Deliberately never removed once a client loads into a level — see `crate::lobby`'s own doc
/// comment for why that's a known, accepted gap for now (the `Levels` list keeps replicating to
/// an in-game client too — harmless wasted bandwidth, not incorrect) rather than something this
/// scaffolding solves.
#[derive(Resource, Clone, Copy)]
pub struct LobbyRoom(pub RoomId);

fn allocate_rooms(mut commands: Commands, mut allocator: ResMut<RoomAllocator>) {
    let game_room = allocator.allocate();
    let lobby_room = allocator.allocate();
    commands.insert_resource(GameRoom(game_room));
    commands.insert_resource(LobbyRoom(lobby_room));
    commands.spawn((LobbyRoot, Rooms::single(lobby_room)));
    commands.spawn((InGameRoot, Rooms::single(game_room)));
}

pub struct GameRoomPlugin;

impl Plugin for GameRoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RoomPlugin);
        app.add_systems(Startup, allocate_rooms);
    }
}
