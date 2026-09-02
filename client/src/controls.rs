use crate::actions::*;
use crate::add_observers_run_if;
use crate::events::{SpawnCube, SpawnNpc};
use crate::fps_controller::FpsCamera;
use crate::game_state::GameState;
use crate::targeting::{Hovered, SELECT_RANGE, Selected, TargetingPlugin};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_replicon::prelude::ClientTriggerExt;
use chill_bevy_console::console_closed;
use shared::client_events::{AttackAttempt, KillAttempt};
use shared::player::Selectable;
use std::f32::consts::PI;

pub struct PlayerControlsPlugin;

/// Right-stick look speed, in radians/second — tuned independently of `FpsCamera::sensitivity`
/// (mouse-pixel units), since the stick reports a held position rather than a per-frame delta.
/// See `player_controls()`'s `FpsCameraRotation` stick binding.
const GAMEPAD_LOOK_SPEED: f32 = 3.0;

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
            attack,
            kill,
            apply_fps_camera_rotation,
            main_menu,
            select,
            deselect,
            shoot,
            spawn_npc,
            on_jump,
            on_movement,
        );
    }
}

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

fn on_movement(
    movement_event: On<Fire<Movement>>,
    fps_camera: Query<&FpsCamera>,
    mut commands: Commands,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let rotation = Rot2::radians(fps_camera.yaw);
    let rotated = rotation * movement_event.value;

    commands.client_trigger(shared::client_events::Movement {
        direction: Vec3::new(-rotated.x, 0.0, rotated.y),
    });
    commands.trigger(shared::client_events::Movement {
        direction: Vec3::new(-rotated.x, 0.0, rotated.y),
    });
}

fn on_movement_stop(_: On<Complete<Movement>>, mut commands: Commands) {
    commands.client_trigger(shared::client_events::Movement {
        direction: Vec3::ZERO,
    });
    commands.trigger(shared::client_events::Movement {
        direction: Vec3::ZERO,
    });
}

/// Translates the `bevy_enhanced_input` jump action into the input-library-agnostic
/// `shared::character_controller::JumpInput` the controller actually runs on.
fn on_jump(_: On<Fire<Jump>>, mut commands: Commands) {
    commands.client_trigger(shared::client_events::Jump);
    commands.trigger(shared::client_events::Jump);
}

/// Marks the right-stick's `FpsCameraRotation` action entity (as opposed to the mouse-motion
/// one) so `apply_fps_camera_rotation` can tell which fired it — the stick's value already
/// carries its own rate/sensitivity scaling (`GAMEPAD_LOOK_SPEED` + `DeltaScale`, see
/// `player_controls()`), unlike mouse's raw pixel delta, which still needs `FpsCamera::sensitivity`.
#[derive(Component)]
struct GamepadLook;

fn apply_fps_camera_rotation(
    rotation: On<Fire<FpsCameraRotation>>,
    mut fps_camera: Query<(&mut FpsCamera, &mut Transform)>,
    gamepad_look: Query<(), With<GamepadLook>>,
) {
    if let Ok((mut fps_camera, mut camera_transform)) = fps_camera.single_mut() {
        // The stick binding pre-scales its value (see `GAMEPAD_LOOK_SPEED`/`DeltaScale`), so
        // only the mouse's raw pixel delta still needs `FpsCamera::sensitivity` applied here.
        let scale = if gamepad_look.contains(rotation.action) {
            1.0
        } else {
            fps_camera.sensitivity
        };
        let delta_pitch = rotation.value.y * scale;
        let delta_yaw = -rotation.value.x * scale;
        fps_camera.pitch =
            (fps_camera.pitch + delta_pitch).clamp(-PI / 2. + 0.0001, PI / 2. - 0.0001);
        fps_camera.yaw = fps_camera.yaw + delta_yaw;
        let d = Vec3::Z.rotate_x(fps_camera.pitch);
        let d = d.rotate_y(fps_camera.yaw);
        let d = d.normalize_or_zero();
        fps_camera.direction = d;
        camera_transform.look_at(fps_camera.direction, Vec3::Y);
    }
}

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

fn attack(_: On<Start<AttackAction>>, selected: Res<Selected>, mut commands: Commands) {
    let Some(entity) = selected.0 else {
        return;
    };
    commands.client_trigger(AttackAttempt { entity });
}

fn kill(_: On<Start<KillAction>>, selected: Res<Selected>, mut commands: Commands) {
    let Some(entity) = selected.0 else {
        return;
    };
    commands.client_trigger(KillAttempt { entity });
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
            context.spawn((
                Action::<KillAction>::new(),
                bindings![KeyCode::KeyT, GamepadButton::RightTrigger],
            ));
            context.spawn((
                Action::<AttackAction>::new(),
                bindings![KeyCode::KeyF, GamepadButton::RightTrigger2],
            ));
            context.spawn((
                Action::<Shoot>::new(),
                bindings![KeyCode::KeyE, GamepadButton::West],
            ));
            context.spawn((
                Action::<SpawnNpcAction>::new(),
                bindings![KeyCode::KeyR, GamepadButton::East],
            ));
            context.spawn((
                Action::<Deselect>::new(),
                bindings![KeyCode::Escape, GamepadButton::LeftThumb],
            ));
            context.spawn((
                Action::<Select>::new(),
                bindings![MouseButton::Left, GamepadButton::RightThumb],
            ));
            context.spawn((Action::<MainMenu>::new(), bindings![KeyCode::F1]));
            context.spawn((
                Action::<FpsCameraRotation>::new(),
                bindings![Binding::mouse_motion()],
            ));
            context.spawn((
                Action::<FpsCameraRotation>::new(),
                GamepadLook,
                DeadZone {
                    kind: DeadZoneKind::Radial, // circular; correct for a stick
                    lower_threshold: 0.15,      // below this magnitude → zero
                    upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
                },
                // The stick reports a held position (-1..1), not a per-frame delta like mouse
                // motion does, so it needs its own sensitivity plus a delta-time scale to turn
                // it into a proper rate (rad/s) — otherwise it's ~50x weaker than a mouse flick
                // and its turn speed scales with frame rate. Mouse's binding is untouched.
                Scale::splat(GAMEPAD_LOOK_SPEED),
                DeltaScale::AUTO,
                Negate::y(), // invert vertical (pitch) axis for the stick only
                Bindings::spawn(Axial::right_stick()),
            ));
        })),
    )
}
