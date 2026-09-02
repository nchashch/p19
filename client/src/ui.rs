use crate::actions::{UiConfirm, UiNavigate};
use crate::add_observers_run_if;
use crate::events::LoadLevel;
use crate::game_state::GameState;
use crate::hud::HudPlugin;
use crate::player_character::PlayerName;
use crate::widgets::{Activate, Tooltip, WidgetsPlugin, button, panel};
use bevy::{
    feathers::controls::{FeathersTextInput, FeathersTextInputContainer},
    input_focus::{AutoFocus, InputFocus, directional_navigation::DirectionalNavigationPlugin},
    prelude::*,
    text::{EditableText, TextEdit},
    ui::auto_directional_navigation::AutoDirectionalNavigator,
};
use bevy_enhanced_input::prelude::{Press, *};
use chill_bevy_console::console_closed;

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WidgetsPlugin, HudPlugin, DirectionalNavigationPlugin));
        app.add_input_context::<MenuControls>();
        app.add_systems(OnEnter(GameState::MainMenu), spawn_menu_controls);
        app.add_observer(seed_player_name_input);
        add_observers_run_if!(app, console_closed, on_ui_navigate, on_ui_confirm);
    }
}

/// The `bevy_enhanced_input` context for gamepad/keyboard main-menu navigation — see
/// `menu_controls()`. Lives on its own entity, spawned/despawned alongside the main menu itself
/// (`DespawnOnExit(GameState::MainMenu)`), separate from `controls::PlayerControls` since that
/// context only exists once a player character has spawned (see `player_character.rs`), which
/// hasn't happened yet while this menu is up.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct MenuControls;

fn spawn_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(GameState::MainMenu)));
}

fn menu_controls() -> impl Bundle {
    (
        MenuControls,
        Actions::<MenuControls>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
            context.spawn((
                Action::<UiNavigate>::new(),
                Bindings::spawn((Cardinal::dpad(),)),
            ));
            context.spawn((
                Action::<UiNavigate>::new(),
                Bindings::spawn((Cardinal::arrows(),)),
            ));
            context.spawn((
                Action::<UiNavigate>::new(),
                DeadZone {
                    kind: DeadZoneKind::Radial, // circular; correct for a stick
                    lower_threshold: 0.15,      // below this magnitude → zero
                    upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
                },
                Bindings::spawn(Axial::left_stick()),
            ));
            context.spawn((
                Action::<UiConfirm>::new(),
                Press::new(1.0),
                bindings![GamepadButton::South, KeyCode::Enter],
            ));
        })),
    )
}

/// Moves `InputFocus` one step in the pushed direction — fires once per push (`Start`, not
/// `Fire`), since a stick held over the dead zone shouldn't keep re-navigating every frame.
fn on_ui_navigate(navigate: On<Start<UiNavigate>>, mut navigator: AutoDirectionalNavigator) {
    if let Ok(direction) = Dir2::new(navigate.value) {
        let _ = navigator.navigate(direction.into());
    }
}

/// "Presses" whichever UI element currently holds `InputFocus` — see `widgets::Activate`.
fn on_ui_confirm(_confirm: On<Start<UiConfirm>>, focus: Res<InputFocus>, mut commands: Commands) {
    if let Some(entity) = focus.get() {
        commands.trigger(Activate { entity });
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
                    AutoFocus
                    on(play_button)
                ),
                (
                    button(px(200), px(50), "main-menu-options")
                    Tooltip::new("main-menu-options-tooltip")
                    on(stub_button)
                ),
                (
                    button(px(200), px(50), "main-menu-credits")
                    Tooltip::new("main-menu-credits-tooltip")
                    on(stub_button)
                ),
                (
                    button(px(200), px(50), "main-menu-quit")
                    Tooltip::new("main-menu-quit-tooltip")
                    on(stub_button)
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
    _event: On<Activate>,
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

/// "Options"/"Credits"/"Quit" — stub buttons that exist to be navigable, not functional yet.
fn stub_button(_event: On<Activate>) {
    info!("not implemented yet");
}
