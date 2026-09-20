use bevy::prelude::*;
use bevy::{asset::AssetPath, ecs::entity::MapEntities};
use serde::{Deserialize, Serialize};

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct InGameRequest;

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

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct Movement {
    pub direction: Vec3,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct Jump;
