use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::character_controller::{
    Character, CharacterCollisions, CharacterController, CharacterMovementSettings, DesiredMotion,
    GroundDetection, Idle,
};
use crate::combat::{Gcd, HitPoints};

const PLAYER_ACCELERATION: f32 = 100.0;
const PLAYER_DAMPING: f32 = 10.0;
const PLAYER_JUMP_IMPULSE: f32 = 10.0;
const PLAYER_GRAVITY: Vec3 = Vec3::new(0.0, -20.0, 0.0);
const PLAYER_TERMINAL_VELOCITY: f32 = 300.0;

#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PlayerCharacterSpawner;

#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PlayerCharacter;

#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Selectable;

pub fn player(player_name: String, position: Vec3) -> impl Bundle {
    let character_movement_settings = CharacterMovementSettings {
        acceleration: PLAYER_ACCELERATION,
        damping: PLAYER_DAMPING,
        jump_impulse: PLAYER_JUMP_IMPULSE,
        gravity: PLAYER_GRAVITY,
        terminal_velocity: PLAYER_TERMINAL_VELOCITY,
    };
    (
        (
            PlayerCharacter,
            Selectable,
            Character,
            Idle,
            CharacterController,
            Name::new(player_name),
            HitPoints {
                hit_points: 100,
                max_hit_points: 100,
            },
            Gcd::default(),
        ),
        Visibility::default(),
        character_movement_settings,
        CharacterCollisions::default(),
        GroundDetection {
            // Use a slightly smaller capsule for shape casts used for ground detection
            cast_shape: Some(Collider::capsule(0.399, 1.0)),
            ..default()
        },
        Collider::capsule(0.4, 1.0),
        DesiredMotion::default(),
        RigidBody::Kinematic,
        Transform::from_translation(position),
    )
}
