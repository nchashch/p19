use bevy::{input_focus::AutoFocus, prelude::*};
use shared::game_state::GameState;

use crate::{
    assets::collections::CommonAssets,
    events::{Disconnect, Play},
    ui::widgets::{Activate, Tooltip, button, panel},
};

pub fn lobby_ui(common_assets: &CommonAssets) -> impl Scene {
    bsn![
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [ lobby_buttons() ]
        WorldAssetRoot({common_assets.lobby_background.clone()})
        DespawnOnExit::<GameState>(GameState::Lobby)
    ]
}

fn lobby_buttons() -> impl Scene {
    bsn![
        panel(px(800), px(820))
        Children[
            (
                button(px(200), px(100), "lobby-play")
                Tooltip::new("lobby-play-tooltip")
                AutoFocus
                on(lobby_play_button)
            ),
            (
                button(px(200), px(100), "lobby-main-menu")
                Tooltip::new("lobby-main-menu-tooltip")
                on(lobby_main_menu_button)
            )
        ]
    ]
}

fn lobby_play_button(_: On<Activate>, mut commands: Commands) {
    commands.trigger(Play);
}

fn lobby_main_menu_button(_: On<Activate>, mut commands: Commands) {
    commands.trigger(Disconnect);
    commands.set_state(GameState::MainMenu);
}
