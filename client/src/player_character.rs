use crate::{
    add_observers_run_if,
    character_controller::{
        CharacterCollisions, CharacterController, CharacterControllerPlugin,
        CharacterMovementSettings, DesiredMotion, GroundDetection, Grounded,
    },
    combat::{AttackAction, CombatPlugin, DespawnCube},
    cube_spawner::{CubeSpawner, SpawnCube},
    fps_controller::{Crosshair, DisableFpsCameraControl, FpsCamera, FpsCameraRotation},
    game_state::GameState,
    npc_spawner::{NpcSpawner, SpawnNpc},
    targeting::{Deselect, Hovered, Select, TargetingPlugin},
};
use avian3d::prelude::*;
use shared::combat::HitPoints;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    light::Skybox,
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
    window::{CursorGrabMode, CursorOptions},
};
use bevy_enhanced_input::prelude::{Press, *};
use chill_bevy_console::console_closed;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Cubemap {
            is_loaded: false,
            image_handle: None,
        });
        app.add_plugins((CharacterControllerPlugin, TargetingPlugin, CombatPlugin));
        app.add_systems(OnEnter(GameState::MainMenu), unlock_cursor);
        app.add_systems(OnEnter(GameState::InGame), (unlock_cursor, initial_respawn));

        app.add_observer(respawn_player);
        app.add_observer(unlock_cursor_after_rotation);
        app.add_observer(on_movement_stop);

        add_observers_run_if!(
            app,
            console_closed,
            lock_cursor_for_rotation,
            main_menu,
            shoot,
            spawn_npc,
            on_jump,
            on_movement,
        );

        app.add_plugins(EnhancedInputPlugin)
            .add_input_context::<PlayerCharacter>();
        app.add_systems(Update, asset_loaded);
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

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Character;

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

pub fn respawn_player(
    _event: On<RespawnPlayer>,
    mut commands: Commands,
    player_spawner: Query<&Transform, With<PlayerCharacterSpawner>>,
    player_character: Query<Entity, With<PlayerCharacter>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    let skybox_handle = asset_server.load("Ryfjallet_cubemap.png");
    commands
        .spawn((
            (
                PlayerCharacter,
                Character,
                Idle,
                CharacterController,
                Name::new("Player"),
                HitPoints {
                    hit_points: 100,
                    max_hit_points: 100,
                },
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
            MeshMaterial3d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
            DesiredMotion::default(),
            RigidBody::Kinematic,
            Transform::from_translation(spawner_transform.translation),
            Actions::<PlayerCharacter>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
                context.spawn((
                    Action::<Movement>::new(),
                    Bindings::spawn((Cardinal::wasd_keys(),)),
                ));
                context.spawn((
                    Action::<Movement>::new(),
                    DeadZone {
                        kind: DeadZoneKind::Radial, // circular; correct for a stick
                        lower_threshold: 0.15,      // below this magnitude → zero
                        upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
                    },
                    Bindings::spawn(Axial::left_stick()),
                ));
                context.spawn((
                    Action::<Jump>::new(),
                    Press::new(1.0),
                    bindings![KeyCode::Space, GamepadButton::South],
                ));
                context.spawn((Action::<DespawnCube>::new(), bindings![KeyCode::KeyT]));

                context.spawn((Action::<AttackAction>::new(), bindings![KeyCode::KeyF]));

                // context.spawn((Action::<Shoot>::new(), bindings![MouseButton::Left]));
                context.spawn((
                    Action::<Shoot>::new(),
                    bindings![KeyCode::KeyE, GamepadButton::West],
                ));
                context.spawn((
                    Action::<SpawnNpcAction>::new(),
                    bindings![KeyCode::KeyR, GamepadButton::East],
                ));
                context.spawn((Action::<Deselect>::new(), bindings![KeyCode::Escape]));
                context.spawn((Action::<Select>::new(), bindings![MouseButton::Left]));
                // context.spawn((Action::<Interact>::new(), bindings![MouseButton::Right]));

                context.spawn((Action::<MainMenu>::new(), bindings![KeyCode::F1]));
                let id = context
                    .spawn((
                        Action::<RotateCamera>::new(),
                        bindings![MouseButton::Right],
                        // Toggle::new(1.0),
                        // bindings![KeyCode::Tab],
                    ))
                    .id();
                context.spawn((
                    Action::<FpsCameraRotation>::new(),
                    Chord::single(id),
                    bindings![Binding::mouse_motion()],
                ));
                context.spawn((
                    Action::<FpsCameraRotation>::new(),
                    DeadZone {
                        kind: DeadZoneKind::Radial, // circular; correct for a stick
                        lower_threshold: 0.15,      // below this magnitude → zero
                        upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
                    },
                    Bindings::spawn(Axial::right_stick()),
                ));
            })),
            DespawnOnEnter(GameState::MainMenu),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayerModel,
                WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            ));
            parent
                .spawn((Transform::from_xyz(0., 0.5, 0.),))
                .with_children(|parent| {
                    parent
                        .spawn((FpsCamera::new(), Transform::IDENTITY))
                        .with_children(|parent| {
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), NpcSpawner));
                            parent.spawn((
                                Transform::from_xyz(0.0, 0.0, 3.0),
                                Camera3d::default(),
                                IsDefaultUiCamera,
                                Msaa::Off,
                                TemporalAntiAliasing::default(),
                                ScreenSpaceAmbientOcclusion::default(),
                                /*
                                                                Skybox {
                                                                    image: Some(skybox_handle.clone()),
                                                                    brightness: 1000.0,
                                                                    ..default()
                                                                },
                                */
                            ));
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

    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: Some(skybox_handle),
    });
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct RotateCamera;

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
pub struct SpawnNpcAction;

fn unlock_cursor(
    mut cursor_options: Single<&mut CursorOptions>,
    _disable_fps_camera: ResMut<DisableFpsCameraControl>,
    mut crosshair: Query<&mut Visibility, With<Crosshair>>,
) {
    cursor_options.visible = true;
    cursor_options.grab_mode = CursorGrabMode::None;
    let Ok(mut visibility) = crosshair.single_mut() else {
        return;
    };
    *visibility = Visibility::Hidden;
}

fn shoot(_: On<Start<Shoot>>, mut commands: Commands) {
    commands.trigger(SpawnCube);
}

fn spawn_npc(_: On<Start<SpawnNpcAction>>, mut commands: Commands) {
    commands.trigger(SpawnNpc);
}

fn main_menu(_: On<Start<MainMenu>>, mut commands: Commands) {
    commands.set_state(GameState::MainMenu);
}

fn asset_loaded(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if cubemap.image_handle.is_none() {
        return;
    }
    if !cubemap.is_loaded
        && asset_server
            .load_state(&cubemap.image_handle.clone().unwrap())
            .is_loaded()
    {
        let mut image = images
            .get_mut(&cubemap.image_handle.clone().unwrap())
            .unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            let layers = image.height() / image.width();
            image
                .reinterpret_stacked_2d_as_array(layers)
                .expect("asset should be 2d texture and height will always be evenly divisible with the given layers");
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }

        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }

        cubemap.is_loaded = true;
    }
}

#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Option<Handle<Image>>,
}

fn unlock_cursor_after_rotation(
    _: On<Complete<RotateCamera>>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
) {
    cursor_options.grab_mode = CursorGrabMode::None;
    disable_fps_camera.0 = true;
}

fn lock_cursor_for_rotation(
    _: On<Fire<RotateCamera>>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
    mut hovered: ResMut<Hovered>,
) {
    cursor_options.grab_mode = CursorGrabMode::Locked;
    disable_fps_camera.0 = false;
    hovered.0 = None;
}

fn on_jump(
    _: On<Fire<Jump>>,
    mut controllers: Query<(
        &CharacterMovementSettings,
        &mut LinearVelocity,
        Has<Grounded>,
    )>,
) {
    for (movement, mut linear_velocity, is_grounded) in &mut controllers {
        if is_grounded {
            linear_velocity.y = movement.jump_impulse;
        }
    }
}

// Moving or standing still -- not performing any kind of action.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Idle;

fn on_movement_stop(
    _: On<Complete<Movement>>,
    mut controllers: Query<&mut DesiredMotion>,
    player: Query<Entity, With<PlayerCharacter>>,
    _commands: Commands,
) {
    for mut acceleration in &mut controllers {
        acceleration.0 = Vec3::ZERO;
    }
    let Ok(_player) = player.single() else {
        return;
    };
}

/// Responds to [`MovementAction`] events and moves character controllers accordingly.
fn on_movement(
    movement_event: On<Fire<Movement>>,
    fps_camera: Query<&FpsCamera>,
    mut controllers: Query<(&CharacterMovementSettings, &mut DesiredMotion)>,
    player: Query<Entity, With<PlayerCharacter>>,
    _commands: Commands,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    for (_movement, mut acceleration) in &mut controllers {
        let rotation = Rot2::radians(fps_camera.yaw);
        let acceleration2 = rotation * movement_event.value;
        acceleration.0.x = -acceleration2.x;
        acceleration.0.z = acceleration2.y;
    }
    let Ok(_player) = player.single() else {
        return;
    };
}
