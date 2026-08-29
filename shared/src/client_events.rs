use bevy::prelude::*;
use bevy::{asset::AssetPath, ecs::entity::MapEntities};
use serde::{Deserialize, Serialize};

/// A request to attack `entity` (the target), from `attacker` — not yet confirmed to land.
/// Fired by client input today; a future server would fire it from a received network message
/// instead.
///
/// Both fields are `#[entities]`-mapped: `entity`/`attacker` are server-authoritative once
/// replicated, so the raw `Entity` ids sent over the wire need remapping to each side's local
/// copy — see `SharedReplicationPlugin`'s `add_mapped_client_event` registration.
#[derive(EntityEvent, Serialize, Deserialize, Clone, MapEntities)]
pub struct AttackAttempt {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub attacker: Entity,
}

/// A request to instantly kill `entity` (the target), from `killer` — not yet confirmed to land.
/// Mirrors `AttackAttempt` exactly (same range check, same target-exists checks, same
/// `#[entities]` mapping) except it sets `HitPoints` straight to zero instead of subtracting
/// `DAMAGE`.
#[derive(EntityEvent, Serialize, Deserialize, Clone, MapEntities)]
pub struct KillAttempt {
    #[entities]
    pub entity: Entity,
    #[entities]
    pub killer: Entity,
}

/// A request to spawn a cube at each `CubeSpawner`, launched along `aim_direction` — not yet
/// confirmed (a spawner currently overlapping geometry may reject it, or `caster` may still be on
/// its global cooldown). Fired by client input today; a future server would fire it from a
/// received network message instead. `aim_direction` is supplied by the caller since `shared` has
/// no camera/look-direction concept of its own — it's a plain vector, not `#[entities]`, since
/// only `caster` needs remapping.
#[derive(Event, Serialize, Deserialize, Clone, MapEntities)]
pub struct SpawnCubeRequest {
    #[entities]
    pub caster: Entity,
    pub aim_direction: Vec3,
}

/// A request to spawn an NPC at each `NpcSpawner` — not yet confirmed (a spawner currently
/// overlapping geometry rejects it, or `caster` may still be on its global cooldown). Fired by
/// client input today; a future server would fire it from a received network message instead.
#[derive(Event, Serialize, Deserialize, Clone, MapEntities)]
pub struct SpawnNpcRequest {
    #[entities]
    pub caster: Entity,
}

/// A request to respawn the player.
#[derive(Event, Serialize, Deserialize, Clone, MapEntities)]
pub struct LoadLevelRequest {
    pub id: AssetPath<'static>,
}

#[derive(Event, Serialize, Deserialize, Clone, MapEntities)]
pub struct Movement {
    pub direction: Vec3,
}

#[derive(Event, Serialize, Deserialize, Clone, MapEntities)]
pub struct Jump;
