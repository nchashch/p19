use crate::add_observers_run_if;
use crate::controls::actions::*;
use crate::controls::fps_controller::FpsCamera;
use crate::controls::targeting::{Hovered, SELECT_RANGE, Selected, TargetingPlugin};
use crate::events::{Disconnect, SpawnCube, SpawnNpc};
use crate::ui::hud::DataFrameVisible;
use crate::ui::menu_screen::{MenuScreen, close_topmost_screen};
use crate::ui::selector::SelectorPopup;
use bevy::ecs::relationship::Relationship;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
// Ahoy's KCC consumes its OWN `InputAction` types (see `AhoyInputPlugin`'s observers), which
// would name-collide with this repo's legacy `Movement`/`Jump` actions (`actions.rs`) —
// aliased until the M4 cleanup removes the legacy pair.
use bevy_ahoy::input::{Jump as AhoyJump, Movement as AhoyMovement};
use bevy_enhanced_input::EnhancedInputSystems;
use bevy_enhanced_input::prelude::{Press, *};
use bevy_markup::prelude::HtmlElement;
use chill_bevy_console::{ConsoleState, console_closed};
use p19_shared::client_events::{AttackAttempt, KillAttempt};
use p19_shared::game_state::{GameState, ModalMenuState};
use p19_shared::inputs::{Look, PlayerInputContext};
use p19_shared::player::Selectable;
use p19_shared::replication::OrderedReliable;
use lightyear::prelude::*;
use lightyear_inputs_bei::prelude::InputMarker;

pub struct PlayerControlsPlugin;

/// Right-stick look speed, in radians/second — tuned independently of the mouse's
/// [`MOUSE_LOOK_SENSITIVITY`], since the stick reports a held position rather than a per-frame
/// delta. See `player_controls()`'s stick `RotateCamera` binding.
const GAMEPAD_LOOK_SPEED: f32 = 3.0;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerControls;

impl Plugin for PlayerControlsPlugin {
    fn build(&self, app: &mut App) {
        // `EnhancedInputPlugin` is also added by `p19_shared::inputs::SharedInputsPlugin` (via
        // `lightyear_inputs_bei`'s `InputPlugin`, which sits earlier in `main.rs`'s plugin
        // tuple since the M0 prediction wiring) — adding it unconditionally here would
        // double-register it and panic. The guard keeps this plugin self-sufficient if
        // plugin order ever changes.
        app.add_plugins(TargetingPlugin);
        if !app.is_plugin_added::<EnhancedInputPlugin>() {
            app.add_plugins(EnhancedInputPlugin);
        }
        app.add_input_context::<PlayerControls>();
        app.init_resource::<MouseSensitivity>()
            .register_type::<MouseSensitivity>()
            .add_systems(Update, apply_mouse_sensitivity);
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
            send_attack,
            send_kill,
            rotate_camera,
            select,
            deselect,
            shoot,
            spawn_npc,
            toggle_data_frame,
        );

        // The camera's absolute direction is the look input the server receives (ADR 0017):
        // written into the `Look` action's mock right before BEI evaluates the replicated
        // context each tick, so the tick's input carries this frame's camera.
        app.add_systems(
            FixedPreUpdate,
            write_look_input.before(EnhancedInputSystems::Update),
        );

        // Bind the server-authored, replicated ahoy action entities for OUR controlled context
        // (see `bind_replicated_ahoy_actions`'s doc comment — the M2 input flow). Polling
        // `Update`, not observers — same replication-arrival reasoning as
        // `on_player_spawned`.
        app.add_systems(Update, bind_replicated_ahoy_actions);

        // Freeze the replicated gameplay input while a UI surface owns the keyboard (dev
        // console or pause modal) — see `gate_replicated_input_context`.
        app.add_systems(Update, gate_replicated_input_context);
    }
}

/// Sets the local player's `Look` action (`p19_shared::inputs`) to the camera's absolute
/// direction; BEI reports it as `Fired` with that value, and lightyear buffers and sends it with
/// the rest of the tick's input. The mock is enabled once by `bind_replicated_ahoy_actions`;
/// `InputMarker` selects this client's own action (remote players' actions replicate too).
fn write_look_input(
    camera: Query<&FpsCamera>,
    mut looks: Query<
        &mut ActionMock,
        (With<Action<Look>>, With<InputMarker<PlayerInputContext>>),
    >,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let value = ActionValue::Axis2D(Vec2::new(camera.yaw, camera.pitch));
    for mut mock in &mut looks {
        if mock.value != value {
            mock.value = value;
        }
    }
}

// `Query`, not `Single<&mut …>`: in `--mcp` (headless) mode there is no window, so there are
// no `CursorOptions` at all — a `Single` would panic on param validation every state
// transition. Windowless mode simply has no cursor to manage.
fn unlock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    for mut options in &mut cursor_options {
        options.visible = true;
        options.grab_mode = CursorGrabMode::None;
    }
}

fn lock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    for mut options in &mut cursor_options {
        options.visible = false;
        options.grab_mode = CursorGrabMode::Locked;
    }
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
///
/// Mirrors `ui/lobby.rs`'s `lobby_main_menu_button` exactly (the one place this pattern already
/// worked before this function existed): `Disconnect` is *this crate's* local event
/// (`crate::events::Disconnect`, not `lightyear::prelude::Disconnect`), routing through
/// `lifecycle::networking::on_disconnect_request`, which is the only correct way to disconnect —
/// see that function's own doc comment for why triggering lightyear's `Disconnect` alone (skipping
/// `Unlink`) leaves a stale `Linked` marker that panics `lightyear_udp` on the *next* connect
/// attempt. `set_state(GameState::MainMenu)` here is technically redundant with
/// `on_disconnected`'s own `On<Add, Disconnected>` transition once the disconnect actually
/// completes, but harmless, and kept for consistency with the lobby button's working reference
/// implementation rather than relying solely on the observer from here specifically.
pub(crate) fn return_to_main_menu(mut commands: Commands) {
    commands.trigger(Disconnect);
    commands.set_state(GameState::MainMenu);
}

/// Opens/closes the pause modal (`modal_menu.rs`) — toggling rather than only-opening lets Tab
/// double as its own "close" as well, alongside the modal's explicit Resume button. Also drives
/// the cursor directly here (rather than via `ModalMenuState`'s `OnEnter`/`OnExit`, which would
/// race `GameState`'s own cursor lock/unlock on the frame the "Main Menu" button changes both
/// states at once) — see `modal_menu.rs`'s Resume button for the matching close-side logic.
/// While a screen is open over the modal (the options submenu), Escape closes its topmost layer
/// instead of the modal — `menu_screen.rs`'s pause-scoped screens don't bind Escape, so this is
/// the only handler the physical key reaches.
fn toggle_modal_menu(
    _: On<Start<ToggleModalMenu>>,
    state: Res<State<ModalMenuState>>,
    mut next_state: ResMut<NextState<ModalMenuState>>,
    mut cursor_options: Query<&mut CursorOptions>,
    screens: Query<(Entity, &MenuScreen)>,
    popups: Query<(Entity, &SelectorPopup)>,
    elements: Query<(Entity, &HtmlElement)>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    // A screen over the modal (the pause menu's Options submenu) closes first: pause-scoped
    // screens deliberately don't bind Escape — `ToggleModalMenu` already reads it, and BEI
    // leaves one physical input to the first action that reads it per tick — so the pause
    // toggle closes the screen (or its open selector popup) itself.
    if !screens.is_empty()
        && close_topmost_screen(&mut commands, &screens, &popups, &elements, &mut focus)
    {
        return;
    }
    match state.get() {
        ModalMenuState::Closed => {
            next_state.set(ModalMenuState::Open);
            for mut options in &mut cursor_options {
                options.visible = true;
                options.grab_mode = CursorGrabMode::None;
            }
        }
        ModalMenuState::Open => {
            next_state.set(ModalMenuState::Closed);
            for mut options in &mut cursor_options {
                options.visible = false;
                options.grab_mode = CursorGrabMode::Locked;
            }
        }
    }
}

fn toggle_data_frame(
    _: On<Start<ToggleDataFrame>>,
    mut data_frame_visible: ResMut<DataFrameVisible>,
) {
    data_frame_visible.0 = !data_frame_visible.0;
}

/// Turns the FPS camera by the local `RotateCamera` deltas (mouse or right stick; the per-device
/// scaling lives in `player_controls()`'s bindings, so the value is radians for both). Purely
/// local: the server learns the result through `write_look_input`, never the deltas.
fn rotate_camera(rotate: On<Fire<RotateCamera>>, mut fps_camera: Query<&mut FpsCamera>) {
    if let Ok(mut fps_camera) = fps_camera.single_mut() {
        fps_camera.turn(-rotate.value.x, -rotate.value.y);
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

fn attack(_: On<Start<AttackAction>>, mut commands: Commands) {
    commands.trigger(crate::events::AttackSelected);
}

fn kill(_: On<Start<KillAction>>, mut commands: Commands) {
    commands.trigger(crate::events::KillSelected);
}

/// The single `AttackAttempt` send path, shared by the `AttackAction` hotkey and
/// `game/trigger attack`. Split from the hotkey observer so headless QA can attack without a
/// window (crosshair targeting needs one — `game/select` injects `Selected` directly instead).
fn send_attack(
    _: On<crate::events::AttackSelected>,
    selected: Res<Selected>,
    mut sender: Single<&mut MessageSender<AttackAttempt>>,
) {
    let Some(entity) = selected.0 else {
        return;
    };
    sender.send::<OrderedReliable>(AttackAttempt { entity });
}

/// The single `KillAttempt` send path — see [`send_attack`].
fn send_kill(
    _: On<crate::events::KillSelected>,
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
            // Look: two action entities because each device needs its own action-level
            // scaling (radians/pixel vs radians/second × dt). Local only — the server receives
            // the camera's absolute direction (`write_look_input`), not these deltas.
            context.spawn((
                Action::<RotateCamera>::new(),
                MouseLookAction,
                // Kept at `MOUSE_LOOK_SENSITIVITY × MouseSensitivity` by `apply_mouse_sensitivity`.
                Scale::splat(MOUSE_LOOK_SENSITIVITY),
                Bindings::spawn(Spawn(Binding::mouse_motion())),
            ));
            // `DeadZone` matches the left stick's: without it, resting-position drift (real on
            // Steam Deck's sticks) slowly turns the camera with no input.
            context.spawn((
                Action::<RotateCamera>::new(),
                DeadZone {
                    kind: DeadZoneKind::Radial,
                    lower_threshold: 0.15,
                    upper_threshold: 1.0,
                },
                Scale::splat(GAMEPAD_LOOK_SPEED),
                DeltaScale::AUTO,
                Negate::y(), // stick up looks up
                Bindings::spawn(Axial::right_stick()),
            ));
        })),
    )
}

/// Mouse-look sensitivity at 1× in radians/pixel — the mouse `RotateCamera` binding's `Scale`,
/// times [`MouseSensitivity`].
const MOUSE_LOOK_SENSITIVITY: f32 = 0.005;

/// Options → Mouse sensitivity: a multiplier on [`MOUSE_LOOK_SENSITIVITY`] (default 1×). Applies
/// immediately. Reflected, so BRP's `world.insert_resources` can set it too. Not persisted.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct MouseSensitivity(pub f32);

impl Default for MouseSensitivity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Marks the mouse `RotateCamera` action (the stick has its own, unaffected by
/// [`MouseSensitivity`]).
#[derive(Component)]
struct MouseLookAction;

/// Keeps the mouse look action's `Scale` at the current [`MouseSensitivity`], including on a
/// freshly spawned `PlayerControls` context.
fn apply_mouse_sensitivity(
    sensitivity: Res<MouseSensitivity>,
    mut scales: Query<&mut Scale, With<MouseLookAction>>,
) {
    let factor = Vec3::splat(MOUSE_LOOK_SENSITIVITY * sensitivity.0);
    for mut scale in &mut scales {
        if scale.factor != factor {
            scale.factor = factor;
        }
    }
}

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
    // Every BEI action carries a disabled `ActionMock` by default (BEI requires it), so the
    // self-terminating filter is lightyear's `InputMarker`, which only this insert adds.
    look: Query<Entity, (With<Action<Look>>, Without<InputMarker<PlayerInputContext>>)>,
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
    for entity in &look {
        // No bindings: `write_look_input` sets this mock's value from the camera every tick.
        // `InputMarker` is what makes lightyear buffer and send an action; bindings would add
        // it automatically, a bare mock doesn't.
        let Ok(action_of) = action_of.get(entity) else {
            continue;
        };
        if !controlled.contains(action_of.get()) {
            continue;
        }
        commands.entity(entity).insert((
            ActionMock::new(TriggerState::Fired, Vec2::ZERO, MockSpan::Manual),
            InputMarker::<PlayerInputContext>::default(),
        ));
    }
}

/// Freezes the **replicated** gameplay input (`PlayerInputContext` — the ahoy
/// `Movement`/`Jump`/`RotateCamera` actions streamed to the server) while a UI surface owns
/// the keyboard: the dev console open or the pause modal open. Without this, BEI's binding
/// readers keep consuming the real WASD/Space/mouse state and the server's KCC keeps moving
/// the character while the player is typing in the console.
///
/// Uses BEI's own `ContextActivity` mechanism: deactivating the context transitions all of
/// its action states to zero/release (the streamed state releases any held keys server-side)
/// while the bindings survive untouched for reactivation on close. Escape/Tab live in the
/// separate local `PlayerControls` context, so modal/data-frame toggles keep working while
/// this one is frozen — and the console's own toggle reads raw `KeyboardInput`, unaffected.
///
/// Polling `Update`, same reasoning as `bind_replicated_ahoy_actions`: the open state is a
/// plain resource field with no component transition to observe, and a player spawned while
/// a surface is open defaults to `ACTIVE` and is gated on the next frame. `ContextActivity`
/// is an immutable component, so a change is remove+insert — done only when the desired
/// state differs from the current one, i.e. exactly twice per open/close cycle.
fn gate_replicated_input_context(
    console: Option<Res<ConsoleState>>,
    modal: Option<Res<State<ModalMenuState>>>,
    contexts: Query<(Entity, &ContextActivity<PlayerInputContext>)>,
    mut commands: Commands,
) {
    // The same condition the attack/kill observers are gated on in `build` — one gate
    // semantics for both kinds of gameplay input.
    let input_allowed = console.is_none_or(|c| !c.open)
        && modal.as_ref().is_none_or(|m| *m.get() == ModalMenuState::Closed);
    for (entity, activity) in &contexts {
        if **activity != input_allowed {
            commands
                .entity(entity)
                .remove::<ContextActivity<PlayerInputContext>>()
                .insert(ContextActivity::<PlayerInputContext>::new(input_allowed));
        }
    }
}
