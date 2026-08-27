use crate::add_observers_run_if;
use crate::combat::{AttackAction, KillAction};
use crate::cube_spawner::SpawnCube;
use crate::fps_controller::{FpsCamera, FpsCameraRotation};
use crate::game_state::GameState;
use crate::npc_spawner::SpawnNpc;
use crate::player_character::PlayerCharacter;
use crate::targeting::{Deselect, Select, TargetingPlugin};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_enhanced_input::prelude::{Press, *};
use chill_bevy_console::console_closed;
use shared::character_controller::{JumpInput, MovementInput};

pub struct PlayerControlsPlugin;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerControls;

impl Plugin for PlayerControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EnhancedInputPlugin, TargetingPlugin))
            .add_input_context::<PlayerControls>();

        app.add_systems(OnEnter(GameState::MainMenu), unlock_cursor);
        app.add_systems(OnEnter(GameState::InGame), lock_cursor);

        app.add_observer(on_movement_stop);

        add_observers_run_if!(
            app,
            console_closed,
            main_menu,
            shoot,
            spawn_npc,
            on_jump,
            on_movement,
        );
    }
}

pub fn player_controls() -> impl Bundle {
    (
        PlayerControls,
        Actions::<PlayerControls>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
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
            context.spawn((Action::<KillAction>::new(), bindings![KeyCode::KeyT]));
            context.spawn((Action::<AttackAction>::new(), bindings![KeyCode::KeyF]));
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
            context.spawn((Action::<MainMenu>::new(), bindings![KeyCode::F1]));
            context.spawn((
                Action::<FpsCameraRotation>::new(),
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
    )
}

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

fn unlock_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = true;
    cursor_options.grab_mode = CursorGrabMode::None;
}

fn lock_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
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

/// Translates the `bevy_enhanced_input` movement action (plus the camera's yaw — a client-only
/// concept the controller itself knows nothing about) into a world-space
/// `shared::character_controller::MovementInput`.
fn on_movement(
    movement_event: On<Fire<Movement>>,
    fps_camera: Query<&FpsCamera>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let Ok(player) = player.single() else {
        return;
    };
    let rotation = Rot2::radians(fps_camera.yaw);
    let rotated = rotation * movement_event.value;
    commands.trigger(MovementInput {
        entity: player,
        direction: Vec3::new(-rotated.x, 0.0, rotated.y),
    });
}

fn on_movement_stop(
    _: On<Complete<Movement>>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(player) = player.single() else {
        return;
    };
    commands.trigger(MovementInput {
        entity: player,
        direction: Vec3::ZERO,
    });
}

/// Translates the `bevy_enhanced_input` jump action into the input-library-agnostic
/// `shared::character_controller::JumpInput` the controller actually runs on.
fn on_jump(
    _: On<Fire<Jump>>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(player) = player.single() else {
        return;
    };
    commands.trigger(JumpInput { entity: player });
}
