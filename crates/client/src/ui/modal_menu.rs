use crate::controls::controls::{MouseSensitivity, return_to_main_menu};
use crate::controls::input_device::InputDeviceState;
use crate::dev::console::{FpsOverlayVisible, PhysicsGizmosVisible};
use crate::gameplay::player_character::ClientPrediction;
use crate::ui::hud::HudVisible;
use crate::ui::input_icons::{InputIcon, InputIconAtlases};
use crate::ui::markup::{LocaleSelection, menu_controls, template};
use crate::ui::menu_screen::{MenuScreenScope, OpenMenuScreens};
use crate::ui::nameplate::NameplatesVisible;
use crate::ui::quad_panel::quad_panel;
use crate::ui::ui::{language_selector, open_options_screen, options_context};
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_markup::prelude::*;
use bevy_xr_utils::tracking_utils::XrTrackedLeftGrip;
use p19_shared::game_state::{GameState, ModalMenuState, VRState};
use serde_json::{Value, json};
use std::f32::consts::FRAC_PI_2;

/// The in-game pause menu — see `game_state::ModalMenuState`. Opened/closed by
/// `controls::toggle_modal_menu` (Escape/`GamepadButton::Start`): a dimmed full-screen
/// `pause_menu.html` root ("Resume", "Options", "Main Menu"; Resume auto-focused) plus a
/// top-left `controls_tips.html` root listing every gameplay binding for the active input
/// device. "Options" opens the same options screen the main menu has
/// (`ui::open_options_screen`, a pause-scoped menu screen) — the mouse sensitivity slider lives
/// there now, not on the pause root. Also owns the VR in-game wrist panel (`wrist_game.html` on
/// a `quad_panel`), which keeps its single "Main Menu" button.
///
/// Signals: `pause.main-menu`, `pause.options`, `pause.resume`, `wrist-game.main-menu`.
pub struct ModalMenuPlugin;

impl Plugin for ModalMenuPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/pause_menu.html");
        embedded_asset!(app, "html/controls_tips.html");
        embedded_asset!(app, "html/wrist_game.html");
        app.define_html_element("input-icon", input_icon);
        app.add_systems(
            OnEnter(ModalMenuState::Open),
            (spawn_modal_menu, spawn_modal_menu_controls),
        );
        // Resets the modal back to `Closed` whenever gameplay ends, regardless of how — this is
        // the *only* place that happens; the "Main Menu" button (`return_to_main_menu`) doesn't
        // touch `ModalMenuState` itself, it just leaves `GameState::InGame`, which fires this.
        // Also covers any other way `InGame` might end (a future disconnect/kick path from the
        // server, say), so a later `Play` never starts with a stale `Open` state.
        app.add_systems(OnExit(GameState::InGame), close_modal_menu);
        for main_menu in ["pause.main-menu", "wrist-game.main-menu"] {
            app.on_html_click(main_menu, |_: In<ElementSignal>, commands: Commands| {
                return_to_main_menu(commands);
            });
        }
        app.on_html_click("pause.resume", resume);
        app.on_html_click("pause.options", open_pause_options);
        app.add_systems(
            Update,
            (
                refresh_controls_tips.run_if(state_changed::<InputDeviceState>),
                spawn_vr_in_game_wrist_panel
                    .run_if(in_state(GameState::InGame).and_then(in_state(VRState::VR))),
            ),
        );
    }
}

fn close_modal_menu(mut next_state: ResMut<NextState<ModalMenuState>>) {
    next_state.set(ModalMenuState::Closed);
}

/// `controls::PlayerControls` (active throughout `InGame`) has no UI navigation bindings, so the
/// pause menu spawns its own `MenuControls` context (d-pad/stick/arrows, South/Enter), scoped to
/// `ModalMenuState::Open`.
fn spawn_modal_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(ModalMenuState::Open)));
}

/// Marks the controls-tips root, re-rendered by `refresh_controls_tips` when the active input
/// device changes.
#[derive(Component)]
struct ControlsTips;

/// The pause modal's three roots: the `pause_menu.html` root (Resume / Options / Main Menu), the
/// options submenu's Language `Selector` (its state — the selected row — lives for one pause
/// session; a fresh one next time), and the controls-tips root.
fn spawn_modal_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    input_device: Res<State<InputDeviceState>>,
    atlases: Option<Res<InputIconAtlases>>,
    locale: Res<LocaleSelection>,
) {
    commands.spawn((
        template(&asset_server, "pause_menu.html"),
        TemplateContext::new(),
        HtmlModal,
        DespawnOnExit(ModalMenuState::Open),
    ));
    commands.spawn((
        language_selector(&locale.0),
        DespawnOnExit(ModalMenuState::Open),
    ));
    commands.spawn((
        ControlsTips,
        template(&asset_server, "controls_tips.html"),
        controls_tips_context(*input_device.get(), atlases.as_deref()),
        DespawnOnExit(ModalMenuState::Open),
    ));
}

/// `pause.options`: the options screen as a submenu over the pause modal — the same
/// `options.html` the main menu opens, so both expose the same settings. Its root is marked
/// `OptionsScreen`, so `ui.rs`'s `update_options_screen` keeps it current every frame.
fn open_pause_options(
    _: In<ElementSignal>,
    open: OpenMenuScreens,
    prediction: Res<ClientPrediction>,
    mouse_sensitivity: Res<MouseSensitivity>,
    fps_overlay: Res<FpsOverlayVisible>,
    physics_debug: Res<PhysicsGizmosVisible>,
    nameplates: Res<NameplatesVisible>,
    hud: Res<HudVisible>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    open_options_screen(
        &mut commands,
        &asset_server,
        &open,
        options_context(
            *prediction,
            *mouse_sensitivity,
            *fps_overlay,
            *physics_debug,
            *nameplates,
            *hud,
        ),
        "pause-options",
        MenuScreenScope::PauseMenu,
    );
}

/// Every button of this module's surfaces. "Resume" closes the modal and hands control back to
/// gameplay — `GameState` never left `InGame` while the modal was up, so this is just
/// `ModalMenuState::Closed` plus re-locking the cursor, mirroring `controls::toggle_modal_menu`'s
/// close branch (see that function's doc comment for why the cursor is set directly rather than
/// via an `OnExit(ModalMenuState::Open)` system).
/// `pause.resume`: close the modal and hand the cursor back to the game.
fn resume(
    _: In<ElementSignal>,
    mut next_state: ResMut<NextState<ModalMenuState>>,
    // `Query`, not `Single<&mut …>` — windowless (`--mcp`) mode has no `CursorOptions` at all.
    mut cursor_options: Query<&mut CursorOptions>,
) {
    next_state.set(ModalMenuState::Closed);
    for mut options in &mut cursor_options {
        options.visible = false;
        options.grab_mode = CursorGrabMode::Locked;
    }
}

/// One controls-tips row: a binding `controls.rs`'s `player_controls()` sets up, with the inputs
/// it's bound to on each device — kept in sync with that list by hand (there's no single source
/// of truth `bevy_enhanced_input` bindings could be introspected from), but written as the actual
/// `KeyCode`s/`GamepadButton`s so a row visibly says which input it documents.
struct ControlTip {
    /// Fluent key of the label.
    key: &'static str,
    /// English fallback of the label.
    label: &'static str,
    keyboard_mouse: &'static [InputIcon],
    gamepad: &'static [InputIcon],
}

const CONTROL_TIPS: [ControlTip; 11] = [
    ControlTip {
        key: "hud-controls-move",
        label: "Move",
        keyboard_mouse: &[
            InputIcon::Key(KeyCode::KeyW),
            InputIcon::Key(KeyCode::KeyA),
            InputIcon::Key(KeyCode::KeyS),
            InputIcon::Key(KeyCode::KeyD),
        ],
        gamepad: &[InputIcon::LeftStick],
    },
    ControlTip {
        key: "hud-controls-look",
        label: "Look",
        keyboard_mouse: &[InputIcon::MouseMove],
        gamepad: &[InputIcon::RightStick],
    },
    ControlTip {
        key: "hud-controls-jump",
        label: "Jump",
        keyboard_mouse: &[InputIcon::Key(KeyCode::Space)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::South)],
    },
    ControlTip {
        key: "hud-controls-select",
        label: "Select",
        keyboard_mouse: &[InputIcon::Mouse(MouseButton::Left)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::RightThumb)],
    },
    ControlTip {
        key: "hud-controls-deselect",
        label: "Deselect",
        keyboard_mouse: &[InputIcon::Mouse(MouseButton::Right)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::LeftThumb)],
    },
    ControlTip {
        key: "hud-controls-attack",
        label: "Attack",
        keyboard_mouse: &[InputIcon::Key(KeyCode::KeyF)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::RightTrigger2)],
    },
    ControlTip {
        key: "hud-controls-kill",
        label: "Kill",
        keyboard_mouse: &[InputIcon::Key(KeyCode::KeyT)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::RightTrigger)],
    },
    ControlTip {
        key: "hud-controls-spawn-cube",
        label: "Spawn cube",
        keyboard_mouse: &[InputIcon::Key(KeyCode::KeyE)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::LeftTrigger)],
    },
    ControlTip {
        key: "hud-controls-spawn-npc",
        label: "Spawn NPC",
        keyboard_mouse: &[InputIcon::Key(KeyCode::KeyR)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::LeftTrigger2)],
    },
    ControlTip {
        key: "hud-controls-menu",
        label: "Menu",
        keyboard_mouse: &[InputIcon::Key(KeyCode::Escape)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::Start)],
    },
    ControlTip {
        key: "hud-controls-stats",
        label: "Stats",
        keyboard_mouse: &[InputIcon::Key(KeyCode::Tab)],
        gamepad: &[InputIcon::Gamepad(GamepadButton::Select)],
    },
];

/// `controls_tips.html`'s `rows` for `input_device`: each row's label key/fallback and the names
/// of its icons — only icons a loaded atlas has, so a row whose icons are all missing (no
/// atlases under `--no-common-assets`) renders label-only.
fn controls_tips_context(
    input_device: InputDeviceState,
    atlases: Option<&InputIconAtlases>,
) -> TemplateContext {
    let rows: Vec<Value> = CONTROL_TIPS
        .iter()
        .map(|tip| {
            let inputs = match input_device {
                InputDeviceState::KeyboardMouse => tip.keyboard_mouse,
                InputDeviceState::Gamepad => tip.gamepad,
            };
            let icons: Vec<&str> = inputs
                .iter()
                .filter_map(|input| input.name())
                .filter(|name| atlases.is_some_and(|atlases| atlases.contains(name)))
                .collect();
            json!({ "key": tip.key, "label": tip.label, "icons": icons })
        })
        .collect();
    TemplateContext::new().with("rows", &rows)
}

fn refresh_controls_tips(
    input_device: Res<State<InputDeviceState>>,
    atlases: Option<Res<InputIconAtlases>>,
    mut tips: Query<&mut TemplateContext, With<ControlsTips>>,
) {
    for mut context in &mut tips {
        *context = controls_tips_context(*input_device.get(), atlases.as_deref());
    }
}

/// `<div is="input-icon" data-icon="<name>">`: the glyph's atlas `ImageNode` (bevy_markup has
/// no `<img>`). Restyles keep it.
fn input_icon(
    icon: In<ElementConnected>,
    atlases: Option<Res<InputIconAtlases>>,
    mut commands: Commands,
) {
    if let Some(image) = icon.data("icon").and_then(|name| atlases?.image_node(name)) {
        commands.entity(icon.entity).insert(image);
    }
}

const IN_GAME_WRIST_PANEL_WIDTH: f32 = 0.18;
const IN_GAME_WRIST_PANEL_HEIGHT: f32 = 0.095;
const IN_GAME_WRIST_PANEL_TEXTURE_WIDTH: u32 = 420;
const IN_GAME_WRIST_PANEL_TEXTURE_HEIGHT: u32 = 220;

/// Marks the spawned in-game wrist panel so `spawn_vr_in_game_wrist_panel` doesn't spawn a second
/// one — see that system's doc comment for why it has to poll rather than spawn once on
/// `OnEnter(GameState::InGame)`.
#[derive(Component)]
struct VrInGameWristPanel;

/// Spawns a `quad_panel` with a single "Main Menu" button (`wrist_game.html`, signal
/// `wrist-game.main-menu`), attached to the left controller's tracked grip pose — the
/// always-available VR equivalent of the pause modal's own "Main Menu" button, for a player who
/// wants to quit to the main menu without first opening the pause modal.
///
/// Runs every frame while `GameState::InGame` and `VRState::VR`, rather than once on
/// `OnEnter(GameState::InGame)` — same reasoning as `ui::spawn_vr_main_menu_wrist_panel`: the
/// `XrTrackedLeftGrip`-marked entity this parents onto comes from a one-shot `Startup` system
/// (`vr_controllers::spawn_controller_cubes`) that itself depends on the real OpenXR session
/// having come up, which takes observable wall-clock time — an `OnEnter`-based spawn here would
/// race that and could easily find zero matching entities. Polling instead just tries again next
/// frame until the grip entity exists, then spawns exactly once (`VrInGameWristPanel`, checked
/// before spawning) and becomes a no-op afterward.
fn spawn_vr_in_game_wrist_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    left_grip: Query<Entity, With<XrTrackedLeftGrip>>,
    existing: Query<(), With<VrInGameWristPanel>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(left_grip) = left_grip.single() else {
        return; // XR tracking hasn't come up yet — try again next frame
    };

    let panel = quad_panel(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        IN_GAME_WRIST_PANEL_WIDTH,
        IN_GAME_WRIST_PANEL_HEIGHT,
        IN_GAME_WRIST_PANEL_TEXTURE_WIDTH,
        IN_GAME_WRIST_PANEL_TEXTURE_HEIGHT,
        (
            template(&asset_server, "wrist_game.html"),
            // Laser-pointer driven, like the main-menu wrist panel.
            HtmlNoFocus,
        ),
    );
    commands.spawn((
        panel,
        VrInGameWristPanel,
        // Same offset/rotation guess as `ui::spawn_vr_main_menu_wrist_panel` — see that system's
        // doc comment for why it's unverified on real hardware and how to nudge it.
        Transform::from_xyz(0.0, 0.12, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ChildOf(left_grip),
        DespawnOnExit(GameState::InGame),
    ));
}
