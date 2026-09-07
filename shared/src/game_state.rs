use bevy::input::gamepad::{GamepadAxisChangedEvent, GamepadButtonChangedEvent};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseMotion};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerState {
    #[default]
    Lobby,
    Loading,
    // Server pause is for all players.
    Paused,
    InGame,
}

/// `AssetLoading` is the app's real starting state now, not `MainMenu` — `main.rs`'s
/// `LoadingState::new(GameState::AssetLoading).continue_to_state(GameState::MainMenu)` blocks the
/// transition to `MainMenu` until `assets::CommonAssets` finishes loading, so nothing that reacts
/// to `OnEnter(GameState::MainMenu)` (spawning the main menu scene, etc.) can run before every
/// asset in that collection is ready.
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameState {
    #[default]
    AssetLoading,
    MainMenu,
    Lobby,
    Loading,
    // A player can be "paused" while the server simulation is still running.
    Paused,
    InGame,
}

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VRState {
    #[default]
    Desktop,
    VR,
}

/// Whether the in-game pause/modal menu (`modal_menu.rs`) is up — a second, independent state
/// machine layered on top of `GameState::InGame`, the same pattern `VRState`/`InputDeviceState`
/// use rather than adding another `GameState` variant: the modal doesn't replace `InGame`, it
/// overlays it (gameplay entities stay alive underneath, `GameState` itself never changes while
/// the modal is up). `controls::toggle_modal_menu` (bound to `KeyCode::Tab`/`GamepadButton::Select`)
/// flips this; `modal_menu.rs` reacts to it to spawn/despawn the menu UI.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalMenuState {
    #[default]
    Closed,
    Open,
}

/// Which input device the player last actually used — a `States` type (not a plain `Resource`)
/// since "which device is active" is exactly the kind of thing other systems want to branch on
/// (e.g. `hud.rs`'s `controls_tips` picking a keyboard/mouse vs. gamepad icon set), the same
/// reason `GameState`/`VRState` are states rather than resources. Unlike `VRState` (set once, at
/// startup, and never changed again), this one updates continuously — see
/// `track_last_input_device`.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputDeviceState {
    #[default]
    KeyboardMouse,
    Gamepad,
}

/// Below this analog value, a `GamepadAxisChangedEvent` is ignored — otherwise resting stick
/// drift on a gamepad that's merely plugged in (not actually being used) would keep flipping
/// `InputDeviceState` back to `Gamepad` even while the player is actively using mouse/keyboard.
const GAMEPAD_AXIS_DEADZONE: f32 = 0.2;

/// Whichever category produces an event this frame wins — a real player only ever touches one
/// input device at a time, so there's no need for anything cleverer (timestamps, etc.) than "did
/// this kind of event fire this frame." Left unchanged on a frame with no input from either
/// category, so the state reflects the *last* device used, not "no device."
fn track_last_input_device(
    mut keys: MessageReader<KeyboardInput>,
    mut mouse_buttons: MessageReader<MouseButtonInput>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut gamepad_buttons: MessageReader<GamepadButtonChangedEvent>,
    mut gamepad_axes: MessageReader<GamepadAxisChangedEvent>,
    mut next_state: ResMut<NextState<InputDeviceState>>,
) {
    if keys.read().next().is_some()
        || mouse_buttons.read().next().is_some()
        || mouse_motion.read().next().is_some()
    {
        next_state.set(InputDeviceState::KeyboardMouse);
    }
    if gamepad_buttons.read().next().is_some()
        || gamepad_axes
            .read()
            .any(|event| event.value.abs() > GAMEPAD_AXIS_DEADZONE)
    {
        next_state.set(InputDeviceState::Gamepad);
    }
}

/// `vr_enabled` decides `VRState`'s starting value — set once, at construction, to match whichever
/// plugin group `main.rs`'s `Prototype19::build` actually added (`DefaultPlugins` vs
/// `add_xr_plugins(...)`), read from the same `networking::is_vr_enabled_presync()` call that
/// decision itself uses. `insert_state` (not `init_state`, which would only ever give `VRState`'s
/// `#[default]` value) is what lets this carry an explicit runtime value in.
pub struct GameStatePlugin {
    pub vr_enabled: bool,
}

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
        app.insert_state(if self.vr_enabled {
            VRState::VR
        } else {
            VRState::Desktop
        });
        app.init_state::<InputDeviceState>();
        app.init_state::<ModalMenuState>();
        app.add_systems(Update, track_last_input_device);
    }
}
