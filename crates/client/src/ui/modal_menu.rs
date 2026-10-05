use crate::controls::controls::return_to_main_menu;
use crate::controls::input_device::InputDeviceState;
use crate::ui::input_icons::{InputIcon, InputIconAtlases};
use crate::ui::markup::{UiNavModal, menu_controls, template};
use crate::ui::quad_panel::quad_panel;
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
/// `pause_menu.html` root ("Main Menu" above "Resume", Resume auto-focused) plus a top-left
/// `controls_tips.html` root listing every gameplay binding for the active input device. Also
/// owns the VR in-game wrist panel (`wrist_game.html` on a `quad_panel`).
///
/// Signals: `pause.main-menu`, `pause.resume`, `wrist-game.main-menu`.
pub struct ModalMenuPlugin;

impl Plugin for ModalMenuPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/pause_menu.html");
        embedded_asset!(app, "html/controls_tips.html");
        embedded_asset!(app, "html/wrist_game.html");
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
        app.add_systems(
            Update,
            (
                handle_modal_menu_signals,
                refresh_controls_tips.run_if(state_changed::<InputDeviceState>),
                spawn_vr_in_game_wrist_panel
                    .run_if(in_state(GameState::InGame).and_then(in_state(VRState::VR))),
            ),
        );
    }
}

/// Above the HUD; the controls tips sit one above the dimmed pause root.
const PAUSE_MENU_Z: i32 = 100;

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

fn spawn_modal_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    input_device: Res<State<InputDeviceState>>,
    atlases: Option<Res<InputIconAtlases>>,
) {
    commands.spawn((
        template(&asset_server, "pause_menu.html"),
        UiNavModal,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(PAUSE_MENU_Z),
        DespawnOnExit(ModalMenuState::Open),
    ));
    commands
        .spawn((
            ControlsTips,
            template(&asset_server, "controls_tips.html"),
            controls_tips_context(*input_device.get(), atlases.as_deref()),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                ..default()
            },
            GlobalZIndex(PAUSE_MENU_Z + 1),
            Pickable::IGNORE,
            DespawnOnExit(ModalMenuState::Open),
        ))
        .observe(attach_controls_tip_icons);
}

/// Every button of this module's surfaces. "Resume" closes the modal and hands control back to
/// gameplay — `GameState` never left `InGame` while the modal was up, so this is just
/// `ModalMenuState::Closed` plus re-locking the cursor, mirroring `controls::toggle_modal_menu`'s
/// close branch (see that function's doc comment for why the cursor is set directly rather than
/// via an `OnExit(ModalMenuState::Open)` system).
fn handle_modal_menu_signals(
    mut signals: MessageReader<ElementSignal>,
    mut next_state: ResMut<NextState<ModalMenuState>>,
    // `Query`, not `Single<&mut …>` — windowless (`--mcp`) mode has no `CursorOptions` at all.
    mut cursor_options: Query<&mut CursorOptions>,
    mut commands: Commands,
) {
    for signal in signals.read() {
        if signal.trigger != SignalTrigger::Click {
            continue;
        }
        match signal.name.as_ref() {
            "pause.main-menu" | "wrist-game.main-menu" => return_to_main_menu(commands.reborrow()),
            "pause.resume" => {
                next_state.set(ModalMenuState::Closed);
                for mut options in &mut cursor_options {
                    options.visible = false;
                    options.grab_mode = CursorGrabMode::Locked;
                }
            }
            _ => {}
        }
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

/// Installs the atlas `ImageNode` on each `controls-tip-icon` element (class `icon-<name>`)
/// after every (re)build — bevy_markup has no `<img>`. Restyles keep it (app state; bevy_markup
/// bug_0018).
fn attach_controls_tip_icons(
    built: On<HtmlUiBuilt>,
    elements: HtmlElements,
    atlases: Option<Res<InputIconAtlases>>,
    mut commands: Commands,
) {
    let Some(atlases) = atlases else {
        return;
    };
    for (entity, element) in elements.iter(built.entity) {
        let Some(name) = element
            .classes
            .iter()
            .find_map(|class| class.strip_prefix("icon-"))
        else {
            continue;
        };
        if let Some(image) = atlases.image_node(name) {
            commands.entity(entity).insert(image);
        }
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
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
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
