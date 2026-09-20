use bevy::{
    feathers::{
        controls::ButtonVariant,
        theme::{ThemeBackgroundColor, ThemeBorderColor},
        tokens,
    },
    input_focus::AutoFocus,
    prelude::*,
};
use shared::game_state::GameState;

use crate::{
    assets::collections::CommonAssets,
    events::{Disconnect, Play},
    ui::{
        ui::menu_button,
        widgets::{Activate, Tooltip},
    },
};

pub fn lobby_ui(common_assets: &CommonAssets) -> impl Scene {
    bsn![
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Start,
        }
        Children [ lobby_buttons() ]
        WorldAssetRoot({common_assets.lobby_background.clone()})
        DespawnOnExit::<GameState>(GameState::Lobby)
    ]
}

fn lobby_buttons() -> impl Scene {
    bsn![
        Node {
            width: px(400),
            height: px(400),
            border: px(1),
            border_radius: px(3),
            margin: UiRect::axes(px(50), px(50)),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: px(10),
        }
        ThemeBorderColor(tokens::GROUP_BODY_BORDER)
        ThemeBackgroundColor(tokens::WINDOW_BG)
        Children[
            (
                menu_button("lobby-play", ButtonVariant::Primary)
                Tooltip::new("lobby-play-tooltip")
                AutoFocus
                on(lobby_play_button)
            ),
            (
                menu_button("lobby-main-menu", ButtonVariant::default())
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
