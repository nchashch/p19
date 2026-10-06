//! The lobby menu (`lobby.html`): Play, the Level selector (options from the replicated
//! `p19_shared::level::Levels`) and Main Menu.

use bevy::{asset::AssetPath, asset::embedded_asset, prelude::*};
use bevy_markup::prelude::*;
use lightyear::prelude::MessageSender;
use p19_shared::client_events::{InGameRequest, LoadLevelRequest};
use p19_shared::game_state::GameState;
use p19_shared::level::Levels;
use p19_shared::replication::OrderedReliable;

use crate::{
    assets::collections::CommonAssets,
    events::Disconnect,
    ui::markup,
    ui::selector::{Selector, SelectorOption, SelectorPicked},
};

/// The level selector's key; `lobby.html`'s toggle names it in its `data-with`.
const LEVEL_SELECTOR: &str = "lobby.level";

/// Lobby UI assets and handlers; `lifecycle::lobby::LobbyPlugin` adds it and spawns
/// [`spawn_lobby_ui`] on entering the lobby.
pub struct LobbyUiPlugin;

impl Plugin for LobbyUiPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/lobby.html");
        app.on_html_click("lobby.play", play)
            .on_html_click(
                "lobby.main-menu",
                |_: In<ElementSignal>, mut commands: Commands| {
                    commands.trigger(Disconnect);
                    commands.set_state(GameState::MainMenu);
                },
            )
            .add_systems(Update, (sync_level_options, load_picked_level));
    }
}

/// The lobby panel (bottom-left, over the lobby background scene) and its level selector. The
/// background is its own entity: an `HtmlUi` root's children belong to the pipeline.
pub fn spawn_lobby_ui(
    mut commands: Commands,
    common_assets: Res<CommonAssets>,
    asset_server: Res<AssetServer>,
    levels: Query<&Levels>,
) {
    commands.spawn((
        WorldAssetRoot(common_assets.lobby_background.clone()),
        DespawnOnExit(GameState::Lobby),
    ));
    commands.spawn((
        markup::template(&asset_server, "lobby.html"),
        DespawnOnExit(GameState::Lobby),
    ));
    // `Levels` may already be replicated, or arrive later (`sync_level_options`).
    let options = levels.single().map(level_options).unwrap_or_default();
    commands.spawn((
        Selector::new(LEVEL_SELECTOR, options),
        DespawnOnExit(GameState::Lobby),
    ));
}

/// One option per level: value = its asset path, label = its `.ftl` name key.
fn level_options(levels: &Levels) -> Vec<SelectorOption> {
    levels
        .iter()
        .map(|(path, level)| SelectorOption::fluent(path.to_string(), level.name.clone()))
        .collect()
}

/// Re-seeds the level selector whenever `Levels` (re)replicates — usually after the lobby UI
/// exists (a client joins the lobby room, then `Levels` arrives).
fn sync_level_options(
    levels: Query<&Levels, Changed<Levels>>,
    mut selectors: Query<&mut Selector>,
) {
    let Ok(levels) = levels.single() else {
        return;
    };
    if let Some(mut selector) = selectors
        .iter_mut()
        .find(|selector| selector.key() == LEVEL_SELECTOR)
    {
        selector.set_options(level_options(levels));
    }
}

/// `lobby.play`: ask the server to put this client in the game.
fn play(_: In<ElementSignal>, mut in_game: Query<&mut MessageSender<InGameRequest>>) {
    if let Ok(mut sender) = in_game.single_mut() {
        info!("InGameRequest sent");
        sender.send::<OrderedReliable>(InGameRequest);
    }
}

/// A level pick asks the server to load it.
fn load_picked_level(
    mut picked: MessageReader<SelectorPicked>,
    mut load_level: Query<&mut MessageSender<LoadLevelRequest>>,
) {
    for pick in picked.read().filter(|pick| pick.selector == LEVEL_SELECTOR) {
        if let Ok(mut sender) = load_level.single_mut() {
            sender.send::<OrderedReliable>(LoadLevelRequest {
                asset_path: AssetPath::from(pick.value.clone()),
            });
        }
    }
}
