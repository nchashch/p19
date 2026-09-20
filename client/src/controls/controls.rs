use crate::add_observers_run_if;
use crate::controls::actions::*;
use crate::controls::fps_controller::FpsCamera;
use crate::controls::targeting::{Hovered, SELECT_RANGE, Selected, TargetingPlugin};
use crate::events::{SpawnCube, SpawnNpc};
use crate::ui::hud::DataFrameVisible;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_enhanced_input::prelude::{Press, *};
use chill_bevy_console::console_closed;
use shared::client_events::{AttackAttempt, KillAttempt};
use shared::game_state::{GameState, ModalMenuState};
use shared::player::Selectable;
use shared::replication::OrderedReliable;
use std::f32::consts::PI;

use lightyear::prelude::*;

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

        add_observers_run_if!(app, console_closed, main_menu, toggle_modal_menu);

        // Gameplay actions also pause while the modal menu (`modal_menu.rs`) is open — same idea
        // as the `console_closed` gate, just for a second UI surface that shouldn't let the player
        // keep moving/fighting underneath it.
        add_observers_run_if!(
            app,
            console_closed.and_then(in_state(ModalMenuState::Closed)),
            attack,
            kill,
            apply_fps_camera_rotation,
            select,
            deselect,
            shoot,
            spawn_npc,
            on_jump,
            on_movement,
            toggle_data_frame,
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

fn main_menu(_: On<Start<MainMenu>>, commands: Commands) {
    return_to_main_menu(commands);
}

/// Shared by the `MainMenu` action (Escape/Start, above), `modal_menu.rs`'s pause-modal "Main
/// Menu" button, and its in-game VR wrist-panel equivalent — all three close the connection and
/// drop the player back to `GameState::MainMenu` the same way, so this is factored out rather than
/// duplicated across input surfaces.
pub(crate) fn return_to_main_menu(mut commands: Commands) {
    todo!();
}

/// Opens/closes the pause modal (`modal_menu.rs`) — toggling rather than only-opening lets Tab
/// double as its own "close" as well, alongside the modal's explicit Resume button. Also drives
/// the cursor directly here (rather than via `ModalMenuState`'s `OnEnter`/`OnExit`, which would
/// race `GameState`'s own cursor lock/unlock on the frame the "Main Menu" button changes both
/// states at once) — see `modal_menu.rs`'s Resume button for the matching close-side logic.
fn toggle_modal_menu(
    _: On<Start<ToggleModalMenu>>,
    state: Res<State<ModalMenuState>>,
    mut next_state: ResMut<NextState<ModalMenuState>>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    match state.get() {
        ModalMenuState::Closed => {
            next_state.set(ModalMenuState::Open);
            cursor_options.visible = true;
            cursor_options.grab_mode = CursorGrabMode::None;
        }
        ModalMenuState::Open => {
            next_state.set(ModalMenuState::Closed);
            cursor_options.visible = false;
            cursor_options.grab_mode = CursorGrabMode::Locked;
        }
    }
}

fn toggle_data_frame(
    _: On<Start<ToggleDataFrame>>,
    mut data_frame_visible: ResMut<DataFrameVisible>,
) {
    data_frame_visible.0 = !data_frame_visible.0;
}

fn on_movement(
    movement_event: On<Fire<Movement>>,
    fps_camera: Query<&FpsCamera>,
    mut sender: Single<&mut MessageSender<shared::client_events::Movement>>,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let rotation = Rot2::radians(fps_camera.yaw);
    let rotated = rotation * movement_event.value;

    sender.send::<OrderedReliable>(shared::client_events::Movement {
        direction: Vec3::new(-rotated.x, 0.0, rotated.y),
    });
}

fn on_movement_stop(
    _: On<Complete<Movement>>,
    mut sender: Single<&mut MessageSender<shared::client_events::Movement>>,
) {
    sender.send::<OrderedReliable>(shared::client_events::Movement {
        direction: Vec3::ZERO,
    });
}

/// Translates the `bevy_enhanced_input` jump action into the input-library-agnostic
/// `shared::character_controller::JumpInput` the controller actually runs on.
fn on_jump(_: On<Fire<Jump>>, mut sender: Single<&mut MessageSender<shared::client_events::Jump>>) {
    sender.send::<OrderedReliable>(shared::client_events::Jump);
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

fn attack(
    _: On<Start<AttackAction>>,
    selected: Res<Selected>,
    mut sender: Single<&mut MessageSender<AttackAttempt>>,
) {
    let Some(entity) = selected.0 else {
        return;
    };
    sender.send::<OrderedReliable>(AttackAttempt { entity });
}

fn kill(
    _: On<Start<KillAction>>,
    selected: Res<Selected>,
    mut sender: Single<&mut MessageSender<KillAttempt>>,
) {
    let Some(entity) = selected.0 else {
        return;
    };
    sender.send::<OrderedReliable>(KillAttempt { entity });
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
                bindings![KeyCode::KeyE, GamepadButton::LeftTrigger],
            ));
            context.spawn((
                Action::<SpawnNpcAction>::new(),
                bindings![KeyCode::KeyR, GamepadButton::LeftTrigger2],
            ));
            context.spawn((
                Action::<Deselect>::new(),
                bindings![MouseButton::Right, GamepadButton::LeftThumb],
            ));
            context.spawn((
                Action::<Select>::new(),
                bindings![MouseButton::Left, GamepadButton::RightThumb],
            ));
            /*
                        context.spawn((
                            Action::<MainMenu>::new(),
                            bindings![KeyCode::Escape, GamepadButton::Start],
                        ));
            */
            context.spawn((
                Action::<ToggleModalMenu>::new(),
                bindings![KeyCode::Escape, GamepadButton::Start],
            ));
            context.spawn((
                Action::<ToggleDataFrame>::new(),
                bindings![KeyCode::Tab, GamepadButton::Select],
            ));
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
