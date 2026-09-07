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
pub struct Play;

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

#[derive(Event)]
pub struct RespawnPlayer;
