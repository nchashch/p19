use bevy::prelude::*;
use lightyear::prelude::*;
use p19_shared::replication::OrderedReliable;
use p19_shared::{client_events::LoadLevelRequest, game_state::GameState};

use crate::{
    assets::collections::CommonAssets,
    events::Play,
    ui::lobby::{apply_selected_level, lobby_ui, sync_level_options},
};

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Lobby),
            (spawn_lobby_menu, spawn_lobby_menu_controls),
        );
        app.add_systems(Update, sync_level_options);
        app.add_observer(on_play);
        app.add_observer(apply_selected_level);
    }
}

pub fn on_play(_: On<Play>, mut sender: Single<&mut MessageSender<LoadLevelRequest>>) {
    info!("play event received");
    let asset_path = "levels/spawn.level.ron".into();
    sender.send::<OrderedReliable>(LoadLevelRequest { asset_path });
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
