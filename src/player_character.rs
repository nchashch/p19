use crate::{
    character_controller::{
        CharacterCollision, CharacterCollisions, CharacterController, CharacterControllerPlugin,
        CharacterMovementSettings, GroundDetection,
    },
    game_state::GameState,
};
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CharacterControllerPlugin);
        app.add_observer(respawn_player);
    }
}

#[derive(Event)]
pub struct RespawnPlayer;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacterSpawner;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacter;

pub fn respawn_player(
    _event: On<RespawnPlayer>,
    mut commands: Commands,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    player_character: Query<Entity, With<PlayerCharacter>>,
) {
    if let Ok(player_character) = player_character.single() {
        commands.entity(player_character).despawn();
    } else {
    };
    let Ok(spawner_transform) = player_spawner.single() else {
        return;
    };
    let character_movement_settings = CharacterMovementSettings {
        acceleration: 50.0,
        damping: 10.0,
        jump_impulse: 20.0,
        gravity: -10.0 * Vec3::Y * 2.0,
        terminal_velocity: 300.0,
    };
    commands.spawn((
        PlayerCharacter,
        CharacterController,
        character_movement_settings,
        CharacterCollisions::default(),
        GroundDetection {
            // Use a slightly smaller capsule for shape casts used for ground detection
            cast_shape: Some(Collider::capsule(0.399, 1.0)),
            ..default()
        },
        Collider::capsule(0.4, 1.0),
        RigidBody::Kinematic,
        *spawner_transform,
        DespawnOnEnter(GameState::MainMenu),
    ));
}
