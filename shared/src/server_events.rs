use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Fired once an `AttackAttempt` is confirmed in range and damage has been applied — this is the
/// fact client-side presentation (animation, sound) reacts to, not `AttackAttempt` itself.
///
/// `entity`/`attacker` are `#[entities]`-mapped — the server's ids get remapped to each client's
/// local copy on receive, via `SharedReplicationPlugin`'s `add_mapped_server_event`.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct Attack {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub attacker: Entity,
}

/// Fired once a `KillAttempt` is confirmed in range — the fact client-side presentation reacts to,
/// not `KillAttempt` itself.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct Kill {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub killer: Entity,
}

/// Fired when an entity's `HitPoints` drop to zero or below, right before it's despawned —
/// carries its last `Transform` since client-side presentation (particles) needs a spawn
/// position after the entity itself is already gone. Only `entity` is `#[entities]`-mapped;
/// `transform` is plain data, not an id.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct EntityDied {
    #[entities]
    pub entity: Entity,
    pub transform: Transform,
}

/// Fires once a player entity is spawned.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct PlayerSpawned {
    #[entities]
    pub entity: Entity,
}

/// Fired once a cube has actually been spawned — the fact client-side presentation (model,
/// selectability, despawn-on-menu) reacts to.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct CubeSpawned {
    #[entities]
    pub entity: Entity,
}

/// Fired once an NPC has actually been spawned — the fact client-side presentation (model,
/// selectability, despawn-on-menu) reacts to. `facing_yaw` is the spawn-time random rotation,
/// purely cosmetic (applied to the visual model only, not the authoritative `Transform`) — not
/// `#[entities]`, since it isn't an id.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct NpcSpawned {
    #[entities]
    pub entity: Entity,
    pub facing_yaw: f32,
}

#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct LoadLevel {
    #[entities]
    pub entity: Entity,
}
