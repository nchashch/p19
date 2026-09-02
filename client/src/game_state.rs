use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VRState {
    #[default]
    Desktop,
    VR,
}

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Loading,
    InGame,
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
    }
}
