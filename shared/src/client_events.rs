use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct Join {
    pub name: String,
}

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
    pub id: String,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct Movement {
    pub direction: Vec3,
}

#[derive(Event, Serialize, Deserialize, Clone)]
pub struct Jump;
