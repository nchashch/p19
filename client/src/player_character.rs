use crate::{
    camera::{Cubemap, PlayerCameraPlugin, player_camera},
    combat::CombatPlugin,
    controls::{self, PlayerControlsPlugin},
    fps_controller::FpsCamera,
    game_state::GameState,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use shared::character_controller::{
    Character, CharacterCollisions, CharacterController, CharacterControllerPlugin,
    CharacterMovementSettings, DesiredMotion, GroundDetection, Idle,
};
use shared::combat::{Gcd, HitPoints};
use shared::cube_spawner::CubeSpawner;
use shared::npc_spawner::NpcSpawner;

use crate::events::RespawnPlayer;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerName>();

        app.add_plugins((
            CharacterControllerPlugin,
            CombatPlugin,
            PlayerControlsPlugin,
            PlayerCameraPlugin,
        ));
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);

        app.add_observer(respawn_player);
    }
}

/// The name entered in the main menu's text field, used for the player's `Name` component.
/// Defaults to "Player" so a fresh app (or skipping the field) behaves as before.
#[derive(Resource)]
pub struct PlayerName(pub String);

impl Default for PlayerName {
    fn default() -> Self {
        Self("Player".to_string())
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacterSpawner;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerModel;

pub fn initial_respawn(mut commands: Commands) {
    commands.trigger(RespawnPlayer);
}

const PLAYER_ACCELERATION: f32 = 100.0;
const PLAYER_DAMPING: f32 = 10.0;
const PLAYER_JUMP_IMPULSE: f32 = 10.0;
const PLAYER_GRAVITY: Vec3 = Vec3::new(0.0, -20.0, 0.0);
const PLAYER_TERMINAL_VELOCITY: f32 = 300.0;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacter;

pub fn respawn_player(
    _event: On<RespawnPlayer>,
    mut commands: Commands,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    player_character: Query<Entity, With<PlayerCharacter>>,
    asset_server: Res<AssetServer>,
    player_name: Res<PlayerName>,
    mut cubemap: ResMut<Cubemap>,
) {
    if let Ok(player_character) = player_character.single() {
        commands.entity(player_character).despawn();
    }
    let Ok(spawner_transform) = player_spawner.single() else {
        return;
    };
    let character_movement_settings = CharacterMovementSettings {
        acceleration: PLAYER_ACCELERATION,
        damping: PLAYER_DAMPING,
        jump_impulse: PLAYER_JUMP_IMPULSE,
        gravity: PLAYER_GRAVITY,
        terminal_velocity: PLAYER_TERMINAL_VELOCITY,
    };
    commands
        .spawn((
            (
                PlayerCharacter,
                Character,
                Idle,
                CharacterController,
                Name::new(player_name.0.clone()),
                HitPoints {
                    hit_points: 100,
                    max_hit_points: 100,
                },
                Gcd::default(),
            ),
            InheritedVisibility::default(),
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
            Transform::from_translation(spawner_transform.translation),
            DespawnOnExit(GameState::InGame),
        ))
        .with_children(|parent| {
            parent.spawn(controls::player_controls());
            parent
                .spawn((Transform::from_xyz(0., 0.5, 0.),))
                .with_children(|parent| {
                    parent
                        .spawn((FpsCamera::new(), Transform::IDENTITY))
                        .with_children(|parent| {
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), NpcSpawner));
                            parent.spawn(player_camera(&asset_server, &mut cubemap));
                        });
                });
        });

    // ambient light
    // NOTE: The ambient light is used to scale how bright the environment map is so with a bright
    // environment map, use an appropriate color and brightness to match
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb_u8(210, 220, 240),
        brightness: 400.0,
        ..default()
    });
}
