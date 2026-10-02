use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerState {
    #[default]
    Startup,
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
        app.init_state::<ModalMenuState>();
    }
}
