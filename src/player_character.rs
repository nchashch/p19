use crate::{
    character_controller::{
        CharacterCollisions, CharacterController, CharacterControllerPlugin,
        CharacterMovementSettings, DesiredMotion, GroundDetection,
    },
    cube_spawner::{Cube, CubeSpawner, Selectable, SpawnCube},
    fps_controller::{Crosshair, DisableFpsCameraControl, FpsCamera, FpsCameraRotation},
    game_state::GameState,
    particles::CubeParticleEffect,
};
use avian3d::prelude::*;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    light::Skybox,
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
    window::{CursorGrabMode, CursorOptions},
};
use bevy_enhanced_input::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Cubemap {
            is_loaded: false,
            index: 0,
            image_handle: None,
        });
        app.add_plugins(CharacterControllerPlugin);
        app.add_systems(OnEnter(GameState::MainMenu), unlock_cursor);
        app.add_systems(OnEnter(GameState::InGame), (unlock_cursor, initial_respawn));
        app.add_observer(lock_cursor_menu);
        app.add_observer(unlock_cursor_menu);
        app.add_observer(respawn);
        app.add_observer(respawn_player);
        app.add_observer(main_menu);
        app.add_observer(shoot);
        app.add_observer(despawn_cube);
        app.add_observer(despawn);
        app.add_observer(select);
        app.add_observer(deselect);
        app.add_plugins(EnhancedInputPlugin)
            .add_input_context::<PlayerCharacter>();
        app.add_systems(Update, (raycast_from_center, tick_lifetimes, asset_loaded));
        app.insert_resource(Hovered(None));
        app.insert_resource(Selected(None));
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
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Ok(player_character) = player_character.single() {
        commands.entity(player_character).despawn();
    } else {
    };
    let Ok(spawner_transform) = player_spawner.single() else {
        return;
    };
    let character_movement_settings = CharacterMovementSettings {
        acceleration: 100.0,
        damping: 10.0,
        jump_impulse: 10.0,
        gravity: -10.0 * Vec3::Y * 2.0,
        terminal_velocity: 300.0,
    };
    let skybox_handle = asset_server.load("Ryfjallet_cubemap.png");
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
            Mesh3d(meshes.add(Capsule3d {
                radius: 0.4,
                half_length: (1.0) / 2.0,
            })),
            MeshMaterial3d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
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
                context.spawn((Action::<DespawnCube>::new(), bindings![KeyCode::KeyT]));

                // context.spawn((Action::<Shoot>::new(), bindings![MouseButton::Left]));
                context.spawn((Action::<Shoot>::new(), bindings![KeyCode::KeyE]));
                context.spawn((Action::<Deselect>::new(), bindings![KeyCode::Escape]));
                context.spawn((Action::<Select>::new(), bindings![MouseButton::Left]));
                // context.spawn((Action::<Interact>::new(), bindings![MouseButton::Right]));

                context.spawn((Action::<Respawn>::new(), bindings![KeyCode::KeyR]));
                context.spawn((Action::<Despawn>::new(), bindings![KeyCode::KeyQ]));
                context.spawn((Action::<MainMenu>::new(), bindings![KeyCode::F1]));
                context.spawn((
                    Action::<FpsCameraRotation>::new(),
                    bindings![Binding::mouse_motion()],
                ));
                context.spawn((
                    Action::<RotateCamera>::new(),
                    bindings![MouseButton::Right],
                    // Toggle::new(1.0),
                    // bindings![KeyCode::Tab],
                ));
            })),
            DespawnOnEnter(GameState::MainMenu),
        ))
        .with_children(|parent| {
            parent
                .spawn((Transform::from_xyz(0., 0.5, 0.),))
                .with_children(|parent| {
                    parent
                        .spawn((FpsCamera::new(), Transform::IDENTITY))
                        .with_children(|parent| {
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                            parent.spawn((
                                Transform::from_xyz(0.0, 0.0, 10.0),
                                Camera3d::default(),
                                IsDefaultUiCamera,
                                Msaa::Off,
                                TemporalAntiAliasing::default(),
                                ScreenSpaceAmbientOcclusion::default(),
                                Skybox {
                                    image: Some(skybox_handle.clone()),
                                    brightness: 1000.0,
                                    ..default()
                                },
                            ));
                        });
                });
        });

    // ambient light
    // NOTE: The ambient light is used to scale how bright the environment map is so with a bright
    // environment map, use an appropriate color and brightness to match
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb_u8(210, 220, 240),
        brightness: 1.0,
        ..default()
    });

    commands.insert_resource(Cubemap {
        is_loaded: false,
        index: 0,
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
struct Deselect;

#[derive(InputAction)]
#[action_output(bool)]
struct Select;

#[derive(InputAction)]
#[action_output(bool)]
struct Interact;

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

#[derive(InputAction)]
#[action_output(bool)]
struct DespawnCube;

#[derive(Component)]
struct Lifetime(Timer);

fn play_menu_music(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        SamplePlayer::new(asset_server.load("menu_music.mp3")).looping(),
        DespawnOnExit(GameState::MainMenu),
    ));
}

fn play_music(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        SamplePlayer::new(asset_server.load("music.mp3")).looping(),
        DespawnOnExit(GameState::InGame),
    ));
}

pub const DESPAWN_RANGE: f32 = f32::INFINITY;

fn despawn_cube(
    _: On<Start<DespawnCube>>,
    mut selected: ResMut<Selected>,
    cube: Query<(Entity, &Transform), With<Cube>>,
    effect: Res<CubeParticleEffect>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok((entity, transform)) = cube.get(entity) {
            commands.spawn(SamplePlayer::new(asset_server.load("crunch.wav")));
            commands.entity(entity).despawn();
            commands.spawn((
                ParticleEffect::new(effect.0.clone()),
                *transform,
                Lifetime(Timer::from_seconds(2.0, TimerMode::Once)),
            ));
            selected.0 = None;
        }
    }
}

fn tick_lifetimes(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut lifetime) in &mut query {
        if lifetime.0.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn raycast_from_center(
    player_collider_entity: Query<Entity, (With<PlayerCharacter>, With<Collider>)>,
    spatial_query: SpatialQuery,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    window_query: Query<&Window>,
    mut hovered: ResMut<Hovered>,
    disable_fps_camera_control: Res<DisableFpsCameraControl>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok(player_collider_entity) = player_collider_entity.single() else {
        return;
    };

    // Center of the screen in logical (not physical) pixels.
    let screen_origin = if disable_fps_camera_control.0 {
        window.cursor_position().unwrap_or(Vec2::ZERO)
    } else {
        return;
        // window.size() / 2.0
    };

    // Screen space -> world ray. Returns Err if the camera has no usable
    // viewport/projection this frame.
    let Ok(ray) = camera.viewport_to_world(camera_transform, screen_origin) else {
        return;
    };

    if let Some(hit) = spatial_query.cast_ray(
        ray.origin,
        ray.direction, // already a Dir3
        f32::MAX,      // max distance
        true,          // treat shapes as solid (hit registers if origin is inside)
        &SpatialQueryFilter::from_excluded_entities([player_collider_entity]),
    ) {
        hovered.0 = Some((hit.entity, hit.distance));
    } else {
        hovered.0 = None;
    }
}

#[derive(Resource)]
pub struct Hovered(pub Option<(Entity, f32)>);

#[derive(Resource)]
pub struct Selected(pub Option<Entity>);

pub const SELECT_RANGE: f32 = 50.0;

fn select(
    _event: On<Fire<Select>>,
    hovered: Res<Hovered>,
    mut selected: ResMut<Selected>,
    query: Query<Entity, With<Selectable>>,
) {
    if let Some((entity, distance)) = hovered.0 {
        if query.get(entity).is_ok() {
            if distance < SELECT_RANGE {
                selected.0 = Some(entity);
            }
        }
    }
}

fn deselect(_event: On<Fire<Deselect>>, mut selected: ResMut<Selected>) {
    selected.0 = None;
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
    index: usize,
    image_handle: Option<Handle<Image>>,
}

fn unlock_cursor_menu(
    _: On<Complete<RotateCamera>>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
) {
    cursor_options.grab_mode = CursorGrabMode::None;
    disable_fps_camera.0 = true;
}

fn lock_cursor_menu(
    _: On<Fire<RotateCamera>>,
    mut cursor_options: Single<&mut CursorOptions>,
    mut disable_fps_camera: ResMut<DisableFpsCameraControl>,
    mut hovered: ResMut<Hovered>,
) {
    cursor_options.grab_mode = CursorGrabMode::Locked;
    disable_fps_camera.0 = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
    hovered.0 = None;
}
