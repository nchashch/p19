use bevy::prelude::*;
use shared::game_state::GameState;

use crate::{lifecycle::assets::CommonAssets, ui::lobby::lobby_ui};

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Lobby),
            (spawn_lobby_menu, spawn_lobby_menu_controls),
        );
    }
}

/// A real system (not the usual `some_scene.spawn()` adapter, which only works for a zero-arg
/// `Fn() -> impl SceneList`) since `main_menu()`'s `WorldAssetRoot` needs `Res<CommonAssets>` —
/// see `modal_menu.rs`'s `spawn_modal_menu` for the same pattern.
pub fn spawn_lobby_menu(mut commands: Commands, common_assets: Res<CommonAssets>) {
    commands.spawn_scene_list(bsn_list![lobby_ui(&common_assets)]);
}

/// Without this, gamepad/keyboard directional navigation did nothing in the lobby — `ui.rs`'s
/// `MenuControls` context (the thing that actually turns d-pad/stick/arrow presses into
/// `InputFocus` movement, and South/Enter into `Activate`) was only ever spawned for
/// `GameState::MainMenu` and `modal_menu.rs`'s pause menu, never for `GameState::Lobby`. Mouse
/// clicks still worked (`activate_on_press` is a plain pointer observer, independent of input
/// contexts), which is why this was easy to miss.
fn spawn_lobby_menu_controls(mut commands: Commands) {
    commands.spawn((
        crate::ui::ui::menu_controls(),
        DespawnOnExit(GameState::Lobby),
    ));
}
