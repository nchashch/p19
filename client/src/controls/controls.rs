use crate::add_observers_run_if;
use crate::controls::actions::*;
use crate::controls::fps_controller::FpsCamera;
use crate::controls::targeting::{Hovered, SELECT_RANGE, Selected, TargetingPlugin};
use crate::events::{SpawnCube, SpawnNpc};
use crate::gameplay::player_character::LocalPlayer;
use crate::ui::hud::DataFrameVisible;
use bevy::ecs::relationship::Relationship;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
// Ahoy's KCC consumes its OWN `InputAction` types (see `AhoyInputPlugin`'s observers), which
// would name-collide with this repo's legacy `Movement`/`Jump` actions (`actions.rs`) —
// aliased until the M4 cleanup removes the legacy pair.
use bevy_ahoy::input::{Jump as AhoyJump, Movement as AhoyMovement, RotateCamera as AhoyRotate};
use bevy_ahoy::CharacterLook;
use bevy_enhanced_input::prelude::{Press, *};
use chill_bevy_console::console_closed;
use shared::client_events::{AttackAttempt, KillAttempt};
use shared::game_state::{GameState, ModalMenuState};
use shared::inputs::{MouseLook, PlayerInputContext, StickLook};
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
        // `EnhancedInputPlugin` is also added by `shared::inputs::SharedInputsPlugin` (via
        // `lightyear_inputs_bei`'s `InputPlugin`, which sits earlier in `main.rs`'s plugin
        // tuple since the M0 prediction wiring) — adding it unconditionally here would
        // double-register it and panic. The guard keeps this plugin self-sufficient if
        // plugin order ever changes.
        app.add_plugins(TargetingPlugin);
        if !app.is_plugin_added::<EnhancedInputPlugin>() {
            app.add_plugins(EnhancedInputPlugin);
        }
        app.add_input_context::<PlayerControls>();
        app.add_systems(OnEnter(GameState::MainMenu), unlock_cursor);
        app.add_systems(OnEnter(GameState::InGame), lock_cursor);

        add_observers_run_if!(app, console_closed, main_menu, toggle_modal_menu);

        // Gameplay actions also pause while the modal menu (`modal_menu.rs`) is open — same idea
        // as the `console_closed` gate, just for a second UI surface that shouldn't let the player
        // keep moving/fighting underneath it.
        add_observers_run_if!(
            app,
            console_closed.and_then(in_state(ModalMenuState::Closed)),
            attack,
            kill,
            rotate_camera,
            select,
            deselect,
            shoot,
            spawn_npc,
            toggle_data_frame,
        );

        // Not an observer — a continuous feed (ahoy's KCC reads `CharacterLook` every fixed
        // tick, so it must track the camera even when no look input fires this frame).
        app.add_systems(Update, update_character_look);

        // Bind the server-authored, replicated ahoy action entities for OUR controlled context
        // (see `bind_replicated_ahoy_actions`'s doc comment — the M2 input flow). Polling
        // `Update`, not observers — same replication-arrival reasoning as
        // `on_player_spawned`.
        app.add_systems(Update, bind_replicated_ahoy_actions);
    }
}

/// Feeds `bevy_ahoy`'s `CharacterLook` on the local player character from the FPS camera's
/// world rotation — ahoy derives movement direction (and swimming pitch, etc.) from it.
///
/// Ahoy's own `CharacterControllerCameraOf` would maintain this out of the box, but this repo
/// keeps its own `FpsCamera` rig (mouse-pixel + gamepad-stick handling, modal/console gating),
/// so the look is bridged instead. `from_quat` on the camera's *global* rotation sidesteps
/// every yaw/pitch sign-convention question — it's the exact conversion ahoy's own camera
/// uses.
fn update_character_look(
    local_player: Res<LocalPlayer>,
    mut looks: Query<&mut CharacterLook>,
    cameras: Query<&GlobalTransform, With<FpsCamera>>,
) {
    let (Some(player), Ok(camera)) = (local_player.0, cameras.single()) else {
        return;
    };
    let Ok(mut look) = looks.get_mut(player) else {
        return;
    };
    let (yaw, pitch, _) = camera.rotation().to_euler(EulerRot::YXZ);
    look.yaw = yaw;
    look.pitch = pitch;
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

/// Rotates the FPS camera from the replicated ahoy `RotateCamera` actions — **the same events
/// the server's look accumulator consumes (`server::input`)**, which is the whole point of this
/// observer: ONE consumer per input. BEI gives a binding's input to the first action that reads
/// it each tick (other actions read zero — "already consumed"), so the old dual path (legacy
/// `FpsCameraRotation` + replicated `RotateCamera` both bound to mouse motion) split the mouse
/// deltas nondeterministically between them, starving the server's look to ~20-30% of the
/// client's turn rate — the server's wish direction then diverged from the camera and its
/// corrections dragged the player sideways ("movement locked to one axis").
///
/// The per-device scaling lives in the bindings (`bind_replicated_ahoy_actions`: mouse scaled
/// by radians/pixel, stick by rate×dt), so the value here is radians-per-tick for both devices —
/// no further sensitivity math (unlike the old observer, which applied `FpsCamera::sensitivity`
/// itself to raw pixel deltas).
fn rotate_camera(
    rotate: On<Fire<AhoyRotate>>,
    mut fps_camera: Query<(&mut FpsCamera, &mut Transform)>,
) {
    if let Ok((mut fps_camera, mut camera_transform)) = fps_camera.single_mut() {
        let delta_pitch = rotate.value.y;
        let delta_yaw = -rotate.value.x;
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
            // (The legacy `Movement`/`Jump`/`FpsCameraRotation` action entities used to be
            // spawned here — the pre-ahoy message-sending path. Deleted in M4: the WASD/Space/
            // mouse inputs are consumed by the ahoy-typed actions on the replicated
            // `PlayerInputContext` context now (BEI's one-consumer-per-input rule made the
            // duplicate bindings a nondeterministic input split), and the `Movement`/`Jump`
            // network messages they fed are gone (`client_events.rs`)).
            // Ahoy's action entities live under `ahoy_controls()`'s context — NOT here. Ahoy's
            // input observers write the *context entity's* `AccumulatedInput`, and its KCC runs
            // on the entity carrying `CharacterController` — both must be the same entity, and
            // this legacy context entity is a child of the player (see `player_controls()`'s
            // doc comment), so its AccumulatedInput would never reach the character.
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
            // (The look actions used to be here — legacy `FpsCameraRotation` mouse + stick
            // entities. Removed: BEI gives a binding's input to the *first* action that reads
            // it each tick, so binding the same mouse-motion to both the legacy action and the
            // replicated ahoy `RotateCamera` split the deltas nondeterministically and starved
            // the server's look accumulation — see `rotate_camera`'s doc comment. The camera
            // now rotates from the replicated actions alone.)
        })),
    )
}

/// Mouse-look sensitivity in radians/pixel — baked into the mouse `RotateCamera` binding's
/// `Scale` modifier (the value that reaches both the client camera and the server's look
/// accumulator is already in radians-per-tick). Historically `FpsCamera` carried this as its
/// `sensitivity` field; the field is gone (the observer no longer applies it), so this constant
/// is the single source.
const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;

/// Adds local-only bindings to the **server-authored, replicated** ahoy action entities (M2
/// input flow: `player()` spawns the context + bare actions server-side; they replicate via
/// `ActionOf<C>`'s hierarchy sender; the owning client is the only side that binds real
/// inputs — everyone else just sees the entities).
///
/// A **polling `Update` system, not `On<Add, …>` observers** — same reasoning as
/// `on_player_spawned`'s (see its doc comment): replication-inserted components don't reliably
/// fire per-component `Add` observers, and the arrival *order* is racy anyway (`ActionOf` and
/// `Controlled` may lag the `Action` component by a tick); a poll just retries until the
/// world is consistent. The `Without<Bindings>` filter makes it self-terminating: once bound,
/// the entity stops matching.
///
/// Inserting `Bindings` is also what makes lightyear's `add_input_marker_from_binding`
/// observer add `InputMarker<C>` — the marker that starts buffering this action's state each
/// tick and streaming it to the server.
///
/// The bindings mirror the legacy pair's (`player_controls()`) so gameplay feel is unchanged;
/// M4 deletes the legacy pair once the message path is gone.
fn bind_replicated_ahoy_actions(
    movement: Query<Entity, (With<Action<AhoyMovement>>, Without<Bindings>)>,
    jump: Query<Entity, (With<Action<AhoyJump>>, Without<Bindings>)>,
    rotate: Query<Entity, (With<Action<AhoyRotate>>, Without<Bindings>)>,
    mouse_look: Query<(), With<MouseLook>>,
    stick_look: Query<(), With<StickLook>>,
    action_of: Query<&ActionOf<PlayerInputContext>>,
    controlled: Query<(), With<Controlled>>,
    mut commands: Commands,
) {
    for entity in &movement {
        let Ok(action_of) = action_of.get(entity) else {
            continue; // relationship not replicated yet — retried next frame
        };
        if !controlled.contains(action_of.get()) {
            continue; // someone else's actions — never bound here
        }
        commands.entity(entity).insert((
            DeadZone {
                kind: DeadZoneKind::Radial, // circular; correct for a stick
                lower_threshold: 0.15,      // below this magnitude → zero
                upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
            },
            Bindings::spawn((Cardinal::wasd_keys(), Axial::left_stick())),
        ));
    }
    for entity in &jump {
        let Ok(action_of) = action_of.get(entity) else {
            continue;
        };
        if !controlled.contains(action_of.get()) {
            continue;
        }
        commands
            .entity(entity)
            .insert((Press::new(1.0), bindings![KeyCode::Space, GamepadButton::South]));
    }
    for entity in &rotate {
        // Look input for the server's `CharacterLook` accumulator (`server::input`): the two
        // device-marked `RotateCamera` action entities get their respective bindings — mouse
        // scaled by radians/pixel, stick by the same rate×dt scaling the legacy
        // `FpsCameraRotation` stick binding uses — so each action's per-tick value is radians,
        // and the accumulator's signs match `apply_fps_camera_rotation` exactly.
        let Ok(action_of) = action_of.get(entity) else {
            continue;
        };
        if !controlled.contains(action_of.get()) {
            continue;
        }
        if mouse_look.contains(entity) {
            commands.entity(entity).insert((
                Scale::splat(MOUSE_LOOK_SENSITIVITY),
                Bindings::spawn(Spawn(Binding::mouse_motion())),
            ));
        } else if stick_look.contains(entity) {
            // Action-level modifiers (they hit all of this action's bindings — there's only
            // one).
            commands.entity(entity).insert((
                Scale::splat(GAMEPAD_LOOK_SPEED),
                DeltaScale::AUTO,
                Negate::y(), // invert vertical (pitch) axis for the stick — matches the legacy binding
                Bindings::spawn(Axial::right_stick()),
            ));
        }
    }
}
