//! The shared character **data model** — marker/config components both binaries compile against.
//!
//! Since the ahoy migration (see `docs/agents/adr/0008`), the movement *simulation* is ahoy's KCC
//! (`bevy_ahoy::CharacterController`, on the player bundle — aliased `AhoyCharacterController`
//! at use sites to disambiguate from the old shared type of the same name, now deleted). What
//! survives here is the replicated vocabulary the rest of the game speaks: `Character`/`Idle`
//! (animation markers) and `GameLayer` (collision layers). Grounded state has no marker of its
//! own — consumers read ahoy's `CharacterControllerState::grounded` directly.

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
