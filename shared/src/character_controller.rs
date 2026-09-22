//! The shared character **data model** — marker/config components both binaries compile against.
//!
//! Since the ahoy migration (see `docs/adr/0008`), the movement *simulation* is ahoy's KCC
//! (`bevy_ahoy::CharacterController`, on the player bundle — aliased `AhoyCharacterController`
//! at use sites to disambiguate from the old shared type of the same name, now deleted). What
//! survives here is the replicated vocabulary the rest of the game speaks: `Character`/`Idle`
//! (animation markers), `GameLayer` (collision layers), and `Grounded` — plus a small bridge
//! that keeps `Grounded` written from ahoy's own ground state (the old controller used to be
//! its writer; see `bridge_grounded`).

use avian3d::prelude::PhysicsLayer;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Marks an entity as a character driven by a character controller — the player and NPCs alike.
#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Character;

/// Physics layers for [`CollisionLayers`], so the player capsule and NPC bodies can be told not
/// to physically collide with each other while each still collides normally with level geometry
/// and props (which stay on the implicit `Default` layer, since nothing else in this codebase
/// uses `CollisionLayers` yet).
#[derive(PhysicsLayer, Default, Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub enum GameLayer {
    #[default]
    Default,
    Player,
    Npc,
}

/// Moving or standing still — not performing any kind of action (attack, hurt, etc). Removed
/// while a one-off animation/action plays and re-inserted once it finishes; systems that drive
/// locomotion (walk/idle/jump) only act while this is present.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
#[component(storage = "SparseSet")]
pub struct Idle;

/// A marker component indicating that an entity is on a surface that is considered ground.
///
/// Written by [`bridge_grounded`] from ahoy's own ground state on both binaries (the old
/// controller used to compute this itself; the old `GroundDetection` config component is gone).
/// Replicated (see `shared::replication::SharedReplicationPlugin`) — the client's local bridge
/// keeps the *predicted* entity's animation reactions latency-free, and the server's keeps the
/// replicated state authoritative.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// Keeps `Grounded` in sync with ahoy's `CharacterControllerState::grounded` — the old
/// controller wrote this itself before the ahoy migration; consumers (`presentation/animation.rs`'s
/// grounded/idle transitions, `hud.rs`'s grounded indicator) still read it.
///
/// Cheap to run every frame: the `Changed` filter only matches when ahoy rewrote the state
/// (every sim tick), and the contains-checks make the insert/remove a no-op when already
/// correct.
pub fn bridge_grounded(
    mut commands: Commands,
    kccs: Query<(Entity, &bevy_ahoy::CharacterControllerState), Changed<bevy_ahoy::CharacterControllerState>>,
    grounded: Query<(), With<Grounded>>,
) {
    for (entity, state) in &kccs {
        let is_grounded = state.grounded.is_some();
        if is_grounded && !grounded.contains(entity) {
            commands.entity(entity).insert(Grounded);
        } else if !is_grounded && grounded.contains(entity) {
            commands.entity(entity).remove::<Grounded>();
        }
    }
}
