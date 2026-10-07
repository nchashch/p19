use bevy::prelude::*;
use bevy::{asset::AssetPath, ecs::entity::MapEntities};
use serde::{Deserialize, Serialize};

/// Spawn the sender's player character and put it in the game.
#[derive(Event, Serialize, Deserialize, Clone)]
pub struct InGameRequest {
    /// Whether the sender predicts its own character (the client's Options → Client-side
    /// prediction). The server scopes the character's `PredictionTarget` to the owner only if
    /// set; otherwise the owner interpolates the server's state like any remote body.
    pub predict: bool,
}

/// Join the game room (receive replicated world state) **without** spawning a player
/// character. Sent by observer clients (e.g. a `--headless-render` agent host) that want
/// vision over the shared world but no in-game avatar.
#[derive(Event, Serialize, Deserialize, Clone)]
pub struct ObserveRequest;

/// The sender's player character should be despawned. Fired by the client when it leaves the
/// game (its `Controlled` is going away — main-menu return, disconnect, app shutdown), so the
/// server can drop its `Lifetime::Persistent` player immediately instead of letting it linger
/// as a zombie until the netcode timeout.
#[derive(Event, Serialize, Deserialize, Clone)]
pub struct ClientDespawn;

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct LobbyRequest;

#[derive(EntityEvent, Serialize, Deserialize, Clone, MapEntities)]
pub struct AttackAttempt {
    #[entities]
    pub entity: Entity,
}

#[derive(EntityEvent, Serialize, Deserialize, Clone, MapEntities)]
pub struct KillAttempt {
    #[entities]
    pub entity: Entity,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct SpawnCubeRequest {
    pub transform: Transform,
    pub aim_direction: Vec3,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct SpawnNpcRequest {
    pub transform: Transform,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct LoadLevelRequest {
    // pub id: String,
    pub asset_path: AssetPath<'static>,
}
