use bevy::prelude::*;
use p19_shared::game_state::GameState;

use crate::ui::lobby::{LobbyUiPlugin, spawn_lobby_ui};
use crate::ui::markup::menu_controls;

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LobbyUiPlugin);
        app.add_systems(
            OnEnter(GameState::Lobby),
            (spawn_lobby_ui, spawn_lobby_menu_controls),
        );
    }
}

/// Gamepad/keyboard navigation for the lobby menu (`MenuControls` is spawned per UI state;
/// without this only mouse clicks worked in the lobby).
fn spawn_lobby_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(GameState::Lobby)));
}
