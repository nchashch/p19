use bevy::prelude::*;

#[derive(EntityEvent)]
pub struct AnimationFinished {
    pub entity: Entity,
}

#[derive(EntityEvent)]
pub struct PlayAnimationLooping {
    pub entity: Entity,
    pub name: String,
}

#[derive(EntityEvent)]
pub struct PlayAnimationOnce {
    pub entity: Entity,
    pub name: String,
}

#[derive(Event)]
pub struct Connect;

#[derive(Event)]
pub struct Disconnect;

/// Client-local trigger (bound to input) — translated into a `SpawnCubeRequest` carrying the
/// player's current aim direction, since `shared` has no `FpsCamera` of its own.
#[derive(Event)]
pub struct SpawnCube;

/// Client-local trigger (bound to input) — translated into a `SpawnNpcRequest`.
#[derive(Event)]
pub struct SpawnNpc;

/// Client-local trigger — attacks whatever crosshair targeting (or, headlessly,
/// `game/select`) has put in [`crate::controls::targeting::Selected`], translating into an
/// `AttackAttempt`. Split from the `AttackAction` hotkey observer so `game/trigger attack`
/// can drive the exact same send path without a window (crosshair targeting needs one).
#[derive(Event)]
pub struct AttackSelected;

/// Client-local trigger — the [`KillSelected`] counterpart to [`AttackSelected`]: instant-kill
/// variant (`KillAttempt`), same `Selected`-driven send path.
#[derive(Event)]
pub struct KillSelected;

#[derive(Event)]
pub struct RespawnPlayer;
