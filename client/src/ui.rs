use crate::events::LoadLevel;
use crate::game_state::GameState;
use crate::hud::HudPlugin;
use crate::player_character::PlayerName;
use crate::widgets::{Tooltip, WidgetsPlugin, button, panel};
use bevy::{
    feathers::controls::{FeathersTextInput, FeathersTextInputContainer},
    prelude::*,
    text::{EditableText, TextEdit},
};

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WidgetsPlugin, HudPlugin));
        app.add_observer(seed_player_name_input);
    }
}

pub fn main_menu_scene() -> impl SceneList {
    bsn_list![main_menu()]
}

fn main_menu() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Start,
        }
        Children [
            panel(px(400), px(400))
            Children [
                (
                    @FeathersTextInputContainer
                    Node {
                        width: px(200),
                    }
                    Children [
                        (
                            @FeathersTextInput {
                                @visible_width: 16f32,
                                @max_characters: 24usize,
                            }
                            PlayerNameInput
                        ),
                    ]
                ),
                (
                    button(px(200), px(50), "main-menu-play")
                    Tooltip::new("main-menu-play-tooltip")
                    on(play_button)
                ),
            ]
        ]
        WorldAssetRoot("MenuBackground.glb#Scene0")
        DespawnOnExit::<GameState>(GameState::MainMenu)
    }
}

/// Marks the main menu's name-entry `FeathersTextInput` so `play_button` can read it and
/// `seed_player_name_input` can pre-fill it with the current `PlayerName`.
#[derive(Component, Clone, Default)]
struct PlayerNameInput;

/// Pre-fills the name field with the current `PlayerName` as soon as it's spawned — `EditableText`
/// has no plain "initial text" field to set inline in the `bsn!` scene, so this queues an edit
/// instead (applied by `bevy_text`'s own `apply_text_edits` system).
fn seed_player_name_input(
    added: On<Add, PlayerNameInput>,
    mut inputs: Query<&mut EditableText>,
    player_name: Res<PlayerName>,
) {
    let Ok(mut editable_text) = inputs.get_mut(added.entity) else {
        return;
    };
    editable_text.queue_edit(TextEdit::Insert(player_name.0.clone().into()));
}

fn play_button(
    _event: On<Pointer<Press>>,
    name_input: Query<&EditableText, With<PlayerNameInput>>,
    mut player_name: ResMut<PlayerName>,
    mut commands: Commands,
) {
    if let Ok(editable_text) = name_input.single() {
        let entered = editable_text.value().to_string();
        let trimmed = entered.trim();
        if !trimmed.is_empty() {
            player_name.0 = trimmed.to_string();
        }
    }
    commands.trigger(LoadLevel {
        id: "levels/Level.glb#Scene0".to_string(),
    });
}
