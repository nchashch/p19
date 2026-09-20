use bevy::prelude::*;
use bevy::{asset::AssetPath, ecs::entity::MapEntities};
use serde::{Deserialize, Serialize};

use crate::game_state::GameState;

/// Fired once an `AttackAttempt` is confirmed in range and damage has been applied — this is the
/// fact client-side presentation (animation, sound) reacts to, not `AttackAttempt` itself.
///
/// `entity`/`attacker` are `#[entities]`-mapped — the server's ids get remapped to each client's
/// local copy on receive, via `SharedReplicationPlugin`'s `add_mapped_server_event`.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities, Clone)]
pub struct Attack {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub attacker: Entity,
}

#[derive(Event, Serialize, Deserialize, MapEntities, Clone)]
pub struct Kill {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub killer: Entity,
}

#[derive(Event, Serialize, Deserialize)]
pub struct UnloadLevel {
    pub next_state: GameState,
}

#[derive(Event, Serialize, Deserialize)]
pub struct LoadLevel {
    pub asset_path: AssetPath<'static>,
}

#[derive(Event, Serialize, Deserialize)]
pub struct LoadRig {
    pub transform: Transform,
    pub asset_path: AssetPath<'static>,
}

#[derive(Event, Serialize, Deserialize)]
pub struct LoadSkybox {
    pub asset_path: AssetPath<'static>,
}

#[derive(Event, Serialize, Deserialize)]
pub struct ServerInGame;

/// Fired once a character's `HitPoints` actually reach zero in combat, right before the server
/// despawns it — the fact client-side presentation (death sound, particle effect) reacts to,
/// distinct from that entity's `HitPoints`/whole self simply being removed for some *other*
/// reason (returning to the main menu despawns every `InGame`-scoped entity via
/// `DespawnOnExit`, a respawn replacing one character entity with another, etc.). `position` is
/// captured server-side and sent directly rather than looked up from the entity client-side,
/// since by the time this arrives the entity may already be gone via its own despawn replicating
/// through — `entity` itself is still `#[entities]`-mapped, but only used for the best-effort
/// "clear `Selected` if it pointed at whatever just died" check, not for placing the effect.
#[derive(EntityEvent, Serialize, Deserialize, MapEntities, Clone)]
pub struct EntityDied {
    #[entities]
    pub entity: Entity,
    pub position: Vec3,
}
