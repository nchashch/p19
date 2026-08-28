use bevy::prelude::*;

/// A request to attack `entity` (the target), from `attacker` — not yet confirmed to land.
/// Fired by client input today; a future server would fire it from a received network message
/// instead.
#[derive(EntityEvent)]
pub struct AttackAttempt {
    pub entity: Entity,
    pub attacker: Entity,
}

/// Fired once an `AttackAttempt` is confirmed in range and damage has been applied — this is the
/// fact client-side presentation (animation, sound) reacts to, not `AttackAttempt` itself.
#[derive(EntityEvent)]
pub struct Attack {
    pub entity: Entity,
    pub attacker: Entity,
}

/// A request to instantly kill `entity` (the target), from `killer` — not yet confirmed to land.
/// Mirrors `AttackAttempt` exactly (same range check, same target-exists checks) except it sets
/// `HitPoints` straight to zero instead of subtracting `DAMAGE`.
#[derive(EntityEvent)]
pub struct KillAttempt {
    pub entity: Entity,
    pub killer: Entity,
}

/// Fired once a `KillAttempt` is confirmed in range — the fact client-side presentation reacts to,
/// not `KillAttempt` itself.
#[derive(EntityEvent)]
pub struct Kill {
    pub entity: Entity,
    pub killer: Entity,
}

/// Fired when an entity's `HitPoints` drop to zero or below, right before it's despawned —
/// carries its last `Transform` since client-side presentation (particles) needs a spawn
/// position after the entity itself is already gone.
#[derive(EntityEvent)]
pub struct EntityDied {
    pub entity: Entity,
    pub transform: Transform,
}

/// A request to spawn a cube at each `CubeSpawner`, launched along `aim_direction` — not yet
/// confirmed (a spawner currently overlapping geometry may reject it, or `caster` may still be on
/// its global cooldown). Fired by client input today; a future server would fire it from a
/// received network message instead. `aim_direction` is supplied by the caller since `shared` has
/// no camera/look-direction concept of its own.
#[derive(Event)]
pub struct SpawnCubeRequest {
    pub caster: Entity,
    pub aim_direction: Vec3,
}

/// Fired once a cube has actually been spawned — the fact client-side presentation (model,
/// selectability, despawn-on-menu) reacts to.
#[derive(EntityEvent)]
pub struct CubeSpawned {
    pub entity: Entity,
}

/// A request to spawn an NPC at each `NpcSpawner` — not yet confirmed (a spawner currently
/// overlapping geometry rejects it, or `caster` may still be on its global cooldown). Fired by
/// client input today; a future server would fire it from a received network message instead.
#[derive(Event)]
pub struct SpawnNpcRequest {
    pub caster: Entity,
}

/// Fired once an NPC has actually been spawned — the fact client-side presentation (model,
/// selectability, despawn-on-menu) reacts to. `facing_yaw` is the spawn-time random rotation,
/// purely cosmetic (applied to the visual model only, not the authoritative `Transform`).
#[derive(EntityEvent)]
pub struct NpcSpawned {
    pub entity: Entity,
    pub facing_yaw: f32,
}
