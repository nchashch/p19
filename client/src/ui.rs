use crate::actions::{UiConfirm, UiNavigate};
use crate::add_observers_run_if;
use crate::events::LoadLevel;
use crate::game_state::GameState;
use crate::hud::HudPlugin;
use crate::networking::DefaultLevel;
use crate::widgets::{Activate, Tooltip, WidgetsPlugin, button, panel};
use bevy::{
    input_focus::{AutoFocus, InputFocus, directional_navigation::DirectionalNavigationPlugin},
    prelude::*,
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
        add_observers_run_if!(app, console_closed, on_ui_navigate, on_ui_confirm);
    }
}

/// The `bevy_enhanced_input` context for gamepad/keyboard directional-navigation UI — see
/// `menu_controls()`. Lives on its own entity, separate from `controls::PlayerControls` (which
/// only exists once a player character has spawned — see `player_character.rs`, and stays focused
/// on gameplay actions, not UI navigation). Two independent call sites spawn one of these, each
/// tagged with its own lifetime-matching `DespawnOnExit`: the main menu itself (below, scoped to
/// `GameState::MainMenu`) and `modal_menu.rs`'s pause menu (scoped to `ModalMenuState::Open`) —
/// they're never both alive at once, so reusing the same context type/bindings for both is safe.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub(crate) struct MenuControls;

fn spawn_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(GameState::MainMenu)));
}

pub(crate) fn menu_controls() -> impl Bundle {
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
                // Without this, confirming a button that causes `MenuControls` itself to
                // despawn-and-respawn on the very same input (e.g. `modal_menu.rs`'s "Main Menu"
                // button, closing the modal and dropping straight into a fresh main-menu
                // `MenuControls` with `Play` auto-focused) reads the still-held South/Enter as a
                // brand-new press on the new context's own `Press` condition (a fresh component,
                // so it has no memory of the input already being down) and immediately activates
                // whatever's newly focused. `require_reset` is `bevy_enhanced_input`'s built-in
                // fix for exactly this: it tracks the physical binding globally (not per-context),
                // so a still-held button stays ignored across a context respawn until it's
                // actually released. See `ActionSettings::require_reset`'s doc comment.
                ActionSettings {
                    require_reset: true,
                    ..default()
                },
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
                    on(quit_button)
                ),
            ]
        ]
        WorldAssetRoot("MenuBackground.glb#Scene0")
        DespawnOnExit::<GameState>(GameState::MainMenu)
    }
}

fn play_button(_event: On<Activate>, default_level: Res<DefaultLevel>, mut commands: Commands) {
    commands.trigger(LoadLevel {
        id: format!("levels/{}#Scene0", default_level.0),
    });
}

/// "Options"/"Credits" — stub buttons that exist to be navigable, not functional yet.
fn stub_button(_event: On<Activate>) {
    info!("not implemented yet");
}

fn quit_button(_event: On<Activate>, mut commands: Commands) {
    commands.write_message(AppExit::Success);
}
