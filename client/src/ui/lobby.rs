use bevy::{
    asset::AssetPath,
    ecs::system::EntityCommands,
    feathers::{
        controls::ButtonVariant,
        theme::{ThemeBackgroundColor, ThemeBorderColor},
        tokens,
    },
    input_focus::AutoFocus,
    prelude::*,
};
use bevy_fluent::prelude::Localization;
use fluent_content::Content;
use lightyear::prelude::MessageSender;
use shared::client_events::{InGameRequest, LoadLevelRequest};
use shared::game_state::GameState;
use shared::level::Levels;
use shared::replication::OrderedReliable;

use crate::{
    assets::collections::CommonAssets,
    events::{Disconnect, Play},
    ui::{
        selector,
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
                selector::LockedWhileSelectorOpen
                Tooltip::new("lobby-play-tooltip")
                AutoFocus
                on(lobby_play_button)
            ),
            level_picker(),
            (
                menu_button("lobby-main-menu", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                Tooltip::new("lobby-main-menu-tooltip")
                on(lobby_main_menu_button)
            )
        ]
    ]
}

fn lobby_play_button(_: On<Activate>, mut sender: Single<&mut MessageSender<InGameRequest>>) {
    sender.send::<OrderedReliable>(InGameRequest);
}

fn lobby_main_menu_button(_: On<Activate>, mut commands: Commands) {
    commands.trigger(Disconnect);
    commands.set_state(GameState::MainMenu);
}

/// Tags the wrapper around `[toggle button, selector popup]` for the level picker, so
/// `sync_level_options` can find *this* selector specifically to (re)seed it — see
/// `client/src/ui/selector.rs`'s own doc comment for the worked example this mirrors.
#[derive(Component, Clone, Default)]
pub(crate) struct LevelPicker;

/// The "Level" button plus its (initially empty/hidden) selector popup — the same
/// `selector::selector_popup()` widget `ui.rs`'s `language_picker()`/`options_picker()` use,
/// populated from the server-replicated `shared::level::Levels` rather than static local data —
/// see `sync_level_options`.
fn level_picker() -> impl Scene {
    bsn! {
        LevelPicker
        Node {
            position_type: PositionType::Relative,
        }
        Children [
            (
                menu_button("lobby-level", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                Tooltip::new("lobby-level-tooltip")
                on(selector::toggle_selector)
            ),
            selector::selector_popup(),
        ]
    }
}

/// Which level a level-picker row loads (see `apply_selected_level`) — inserted onto whichever
/// slot entity currently shows a given level by that option's own
/// `selector::SelectorOption::payload` closure, same as `ui.rs`'s `LocaleOption`/`OptionChoice`.
#[derive(Component, Clone, Default)]
pub(crate) struct LevelChoice(AssetPath<'static>);

/// (Re)seeds the level picker's options from the replicated `Levels` singleton — three separate
/// triggers, since (unlike `ui.rs`'s static `language_options()`/`stub_options()`) this data
/// arrives asynchronously over the network, not synchronously at spawn time:
/// - the picker's own wrapper just spawned (`Added<LevelPicker>`) — `Levels` may already be
///   present (replicated before the lobby UI even spawns) or may still be in flight;
/// - `Levels` itself just changed (`Changed<Levels>`) — covers it arriving after the picker
///   already exists, the common case (see `rooms::LobbyRoom`'s doc comment: a client joins the
///   lobby room, then `Levels` replicates to it);
/// - the active `Localization` changed — `Level::name` is an `.ftl` key (see `Level`'s own doc
///   comment), resolved to display text here rather than via `LocalizedText` (which `selector`'s
///   row slots deliberately skip — see `selector_row`'s doc comment, a slot's content changes at
///   runtime as the window scrolls), so a runtime language switch needs the same re-seed a
///   locale-agnostic label wouldn't.
///
/// Falls back to the raw `.ftl` key itself if it's missing from the current locale, matching
/// `localization::localized`'s own fallback (not reusing that helper directly since it takes a
/// `&'static str` key — `Level::name` is owned, runtime-loaded data).
pub(crate) fn sync_level_options(
    mut commands: Commands,
    changed_levels: Query<(), Changed<Levels>>,
    fresh_pickers: Query<(), Added<LevelPicker>>,
    all_levels: Query<&Levels>,
    wrappers: Query<&Children, With<LevelPicker>>,
    panels: Query<Entity, With<selector::Selector>>,
    localization: Option<Res<Localization>>,
) {
    let localization_changed = localization.as_ref().is_some_and(|l| l.is_changed());
    if changed_levels.is_empty() && fresh_pickers.is_empty() && !localization_changed {
        return;
    }
    let Ok(levels) = all_levels.single() else {
        return;
    };
    let Ok(children) = wrappers.single() else {
        return;
    };
    let Some(panel) = children.iter().find(|&entity| panels.contains(entity)) else {
        return;
    };
    let options = levels
        .iter()
        .map(|(path, level)| {
            let label = localization
                .as_deref()
                .and_then(|localization| localization.content(level.name.as_str()))
                .unwrap_or_else(|| level.name.clone());
            let path = path.clone();
            selector::SelectorOption {
                label,
                payload: Box::new(move |entity: &mut EntityCommands| {
                    entity.insert(LevelChoice(path.clone()));
                }),
            }
        })
        .collect();
    selector::set_selector_options(&mut commands, panel, options);
}

/// Reacts to a level actually being picked — reads `LevelChoice` straight off `selected.entity`
/// and sends `LoadLevelRequest` directly, same "resolve identity from the entity an event fires
/// on" pattern `ui.rs`'s `apply_selected_language`/`apply_selected_option` already use. No-ops
/// for any other selector's `UiSelected` — the payload component itself disambiguates which
/// selector this event came from.
pub(crate) fn apply_selected_level(
    selected: On<selector::UiSelected>,
    choices: Query<&LevelChoice>,
    mut sender: Single<&mut MessageSender<LoadLevelRequest>>,
) {
    if let Ok(choice) = choices.get(selected.entity) {
        sender.send::<OrderedReliable>(LoadLevelRequest {
            asset_path: choice.0.clone(),
        });
    }
}
