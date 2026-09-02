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

#[derive(Event, Serialize, Deserialize, MapEntities)]
pub struct Kill {
    #[entities]
    pub killer: Entity,
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

/// Fired once a character's `HitPoints` actually reach zero in combat, right before the server
/// despawns it — the fact client-side presentation (death sound, particle effect) reacts to,
/// distinct from that entity's `HitPoints`/whole self simply being removed for some *other*
/// reason (returning to the main menu despawns every `InGame`-scoped entity via
/// `DespawnOnExit`, a respawn replacing one character entity with another, etc.). `position` is
/// captured server-side and sent directly rather than looked up from the entity client-side,
/// since by the time this arrives the entity may already be gone via its own despawn replicating
/// through — `entity` itself is still `#[entities]`-mapped, but only used for the best-effort
/// "clear `Selected` if it pointed at whatever just died" check, not for placing the effect.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities)]
pub struct EntityDied {
    #[entities]
    pub entity: Entity,
    pub position: Vec3,
}
