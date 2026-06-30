use crate::{
    character_controller::{
        CharacterCollisions, CharacterController, CharacterControllerPlugin,
        CharacterMovementSettings, DesiredMotion, GroundDetection,
    },
    cube_spawner::{Cube, CubeSpawner, SpawnCube},
    fps_controller::{Crosshair, DisableFpsCameraControl, FpsCamera, FpsCameraRotation},
    game_state::GameState,
};
use avian3d::prelude::*;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use bevy_enhanced_input::prelude::*;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CharacterControllerPlugin);
        app.add_systems(OnEnter(GameState::MainMenu), unlock_cursor);
        app.add_systems(OnEnter(GameState::InGame), (lock_cursor, initial_respawn));
        app.add_observer(respawn);
        app.add_observer(respawn_player);
        app.add_observer(main_menu);
        app.add_observer(shoot);
        app.add_observer(despawn);
        app.add_plugins(EnhancedInputPlugin)
            .add_input_context::<PlayerCharacter>();
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct Respawn;

#[derive(Event)]
pub struct RespawnPlayer;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacterSpawner;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerCharacter;

pub fn initial_respawn(mut commands: Commands) {
    commands.trigger(RespawnPlayer);
}

pub fn respawn(_event: On<Complete<Respawn>>, mut commands: Commands) {
    commands.trigger(RespawnPlayer);
}

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
    dbg!("respawn_player");
    let character_movement_settings = CharacterMovementSettings {
        acceleration: 100.0,
        damping: 10.0,
        jump_impulse: 10.0,
        gravity: -10.0 * Vec3::Y * 2.0,
        terminal_velocity: 300.0,
    };
    commands
        .spawn((
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
            DesiredMotion::default(),
            PointLight { ..default() },
            RigidBody::Kinematic,
            Transform::from_translation(spawner_transform.translation),
            Actions::<PlayerCharacter>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
                context.spawn((
                    Action::<Movement>::new(),
                    Bindings::spawn((Cardinal::wasd_keys(),)),
                ));
                context.spawn((Action::<Jump>::new(), bindings![KeyCode::Space]));
                context.spawn((Action::<Shoot>::new(), bindings![MouseButton::Left]));
                context.spawn((Action::<Respawn>::new(), bindings![KeyCode::KeyR]));
                context.spawn((Action::<Despawn>::new(), bindings![KeyCode::KeyQ]));
                context.spawn((Action::<MainMenu>::new(), bindings![KeyCode::Escape]));
                context.spawn((
                    Action::<FpsCameraRotation>::new(),
                    bindings![Binding::mouse_motion()],
                ));
                context.spawn((
                    Action::<MenuAction>::new(),
                    Toggle::new(1.0),
                    bindings![KeyCode::Tab],
                ));
            })),
            DespawnOnEnter(GameState::MainMenu),
        ))
        .with_children(|parent| {
            parent
                .spawn((Transform::from_xyz(0., 0.5, 0.),))
                .with_children(|parent| {
                    parent
                        .spawn((
                            Camera3d::default(),
                            FpsCamera::new(),
                            Transform::IDENTITY,
                            IsDefaultUiCamera,
                        ))
                        .with_children(|parent| {
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                        });
                });
        });
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct MenuAction;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Movement;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

#[derive(InputAction)]
#[action_output(bool)]
pub struct MainMenu;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Shoot;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Despawn;

fn unlock_cursor(
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
    mut crosshair: Query<&mut Visibility, With<Crosshair>>,
) {
    cursor_options.visible = true;
    cursor_options.grab_mode = CursorGrabMode::None;
    disable_fps_camera.0 = true;
    let Ok(mut visibility) = crosshair.single_mut() else {
        return;
    };
    *visibility = Visibility::Hidden;
}

fn shoot(_: On<Start<Shoot>>, mut commands: Commands) {
    commands.trigger(SpawnCube);
}

fn despawn(_: On<Start<Despawn>>, mut commands: Commands, cubes: Query<Entity, With<Cube>>) {
    for cube in cubes {
        commands.entity(cube).despawn();
    }
}

fn main_menu(_: On<Start<MainMenu>>, mut commands: Commands) {
    commands.set_state(GameState::MainMenu);
}

fn lock_cursor(
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
    mut crosshair: Query<&mut Visibility, With<Crosshair>>,
) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
    disable_fps_camera.0 = false;
    let Ok(mut visibility) = crosshair.single_mut() else {
        return;
    };
    *visibility = Visibility::Visible;
}
