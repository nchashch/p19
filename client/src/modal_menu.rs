use crate::controls::return_to_main_menu;
use crate::game_state::{GameState, InputDeviceState, ModalMenuState};
use crate::input_icons::{
    GAMEPAD_LOOK_STICK_ICON_PNG, GAMEPAD_MOVE_STICK_ICON_PNG, MOUSE_MOVE_ICON_PNG,
    gamepad_button_icon_png, key_code_icon_png, mouse_button_icon_png,
};
use crate::localization::LocalizedText;
use crate::networking::PendingLevelId;
use crate::ui::menu_controls;
use crate::widgets::{Activate, SERIF_FONT, button, panel};
use bevy::color::palettes::css::WHITE;
use bevy::input_focus::AutoFocus;
use bevy::prelude::*;
use bevy::text::FontSourceTemplate;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_hanabi::ParticleEffect;
use bevy_quinnet::client::QuinnetClient;
use bevy_seedling::sample::SamplePlayer;

/// The in-game pause menu — see `game_state::ModalMenuState`. Opened/closed by
/// `controls::toggle_modal_menu` (Escape/`GamepadButton::Start`). Besides its two buttons, this is
/// also where the controls-help panel lives now (`controls_tips`/`gamepad_controls_tips`) — it
/// used to be part of the always-visible HUD (`hud.rs`), but a reference panel makes more sense
/// tucked behind the pause menu than permanently on screen during gameplay.
pub struct ModalMenuPlugin;

impl Plugin for ModalMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ModalMenuState::Open),
            (modal_menu_scene.spawn(), spawn_modal_menu_controls),
        );
        // Resets the modal back to `Closed` whenever gameplay ends, regardless of how — the "Main
        // Menu" button below already sets it directly, but this also covers e.g. a future
        // disconnect/kick path, so a later `Play` never starts with a stale `Open` state.
        app.add_systems(OnExit(GameState::InGame), close_modal_menu);
        app.add_systems(Update, update_controls_tips_visibility);
    }
}

fn close_modal_menu(mut next_state: ResMut<NextState<ModalMenuState>>) {
    next_state.set(ModalMenuState::Closed);
}

/// Without this, gamepad/keyboard directional navigation did nothing while the modal was open —
/// `ui.rs`'s `MenuControls` context (the thing that actually turns d-pad/stick/arrow presses into
/// `InputFocus` movement, and South/Enter into `Activate`) is normally only spawned for the main
/// menu, and `controls::PlayerControls` (active throughout `InGame`) has no `UiNavigate`/`UiConfirm`
/// bindings of its own — it's built for gameplay input, not UI. Spawning the same `MenuControls`
/// context here, scoped to `ModalMenuState::Open` instead of `GameState::MainMenu`, reuses
/// `ui.rs`'s existing `on_ui_navigate`/`on_ui_confirm` observers (they react to the action, not to
/// which entity/context fired it) rather than duplicating that binding set.
fn spawn_modal_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(ModalMenuState::Open)));
}

fn modal_menu_scene() -> impl SceneList {
    bsn_list![modal_menu()]
}

fn modal_menu() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))
        DespawnOnExit::<ModalMenuState>(ModalMenuState::Open)
        Children [
            controls_tips(),
            gamepad_controls_tips(),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(20),
                }
                Children [
                    (
                        button(px(220), px(60), "modal-menu-main-menu")
                        on(main_menu_button)
                    ),
                    (
                        button(px(220), px(60), "modal-menu-resume")
                        AutoFocus
                        on(resume_button)
                    ),
                ]
            )
        ]
    }
}

fn main_menu_button(
    _: On<Activate>,
    commands: Commands,
    client: ResMut<QuinnetClient>,
    pending_level_id: ResMut<PendingLevelId>,
    particle_effects: Query<Entity, With<ParticleEffect>>,
    sample_players: Query<Entity, With<SamplePlayer>>,
) {
    return_to_main_menu(
        commands,
        client,
        pending_level_id,
        particle_effects,
        sample_players,
    );
}

/// Closes the modal and hands control back to gameplay — `GameState` never left `InGame` while
/// the modal was up, so this is just `ModalMenuState::Closed` plus re-locking the cursor. Mirrors
/// `controls::toggle_modal_menu`'s close branch exactly (see that function's doc comment for why
/// the cursor is set directly here rather than via an `OnExit(ModalMenuState::Open)` system).
fn resume_button(
    _: On<Activate>,
    mut next_state: ResMut<NextState<ModalMenuState>>,
    mut cursor_options: Single<&mut CursorOptions>,
) {
    next_state.set(ModalMenuState::Closed);
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Locked;
}

/// Marks `controls_tips`'s root — shown only while `InputDeviceState` is `KeyboardMouse`. See
/// `update_controls_tips_visibility`.
#[derive(Component, Clone, Default)]
struct KeyboardMouseControlsTips;

/// Marks `gamepad_controls_tips`'s root — the mirror image of `KeyboardMouseControlsTips`, shown
/// only while `InputDeviceState` is `Gamepad`.
#[derive(Component, Clone, Default)]
struct GamepadControlsTips;

/// Both panels are always spawned as children of `modal_menu()` (they only exist at all while the
/// modal is open — there's no separate `HudVisible`-style toggle to combine here any more, unlike
/// when this lived in `hud.rs`), so this only needs to pick which one matches the currently active
/// input device.
fn update_controls_tips_visibility(
    input_device: Res<State<InputDeviceState>>,
    mut keyboard_mouse: Query<
        &mut Visibility,
        (
            With<KeyboardMouseControlsTips>,
            Without<GamepadControlsTips>,
        ),
    >,
    mut gamepad: Query<
        &mut Visibility,
        (
            With<GamepadControlsTips>,
            Without<KeyboardMouseControlsTips>,
        ),
    >,
) {
    let visibility = |show: bool| {
        if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    let show_keyboard_mouse = *input_device.get() == InputDeviceState::KeyboardMouse;
    let show_gamepad = *input_device.get() == InputDeviceState::Gamepad;
    for mut element_visibility in &mut keyboard_mouse {
        *element_visibility = visibility(show_keyboard_mouse);
    }
    for mut element_visibility in &mut gamepad {
        *element_visibility = visibility(show_gamepad);
    }
}

const CONTROLS_TIPS_ICON_SIZE: f32 = 40.0;
const CONTROLS_TIPS_ICON_GAP: f32 = 4.0;
const CONTROLS_TIPS_LABEL_FONT_SIZE: f32 = 16.0;

/// Top-left panel listing every keyboard/mouse binding `controls.rs`'s `player_controls()` sets up
/// — kept in sync with that list by hand, same as `console.ftl`'s `console-controls` text hint is;
/// there's no single source of truth `bevy_enhanced_input` bindings could be introspected from
/// automatically. Using the actual `KeyCode`s here (via `control_tip_keys`/`input_icons`) at least
/// makes *that* part self-documenting — a row visibly says `KeyCode::Escape`, not an opaque asset
/// path with nothing connecting it back to which key it's supposed to be. Mouse buttons/motion
/// aren't `KeyCode`, so `hud-controls-look`/`hud-controls-select` go through `control_tip_icons`
/// directly with a PNG path from `input_icons::mouse_button_icon_png`/`MOUSE_MOVE_ICON_PNG`
/// instead of a `key_code_icon_png` lookup. `mouse_move` (for `FpsCameraRotation`'s mouse-motion
/// binding) is included since it's a real, always-on control, even though it isn't a discrete
/// key/button press. See `input_icons`'s module doc comment for why this renders each icon as its
/// own PNG (`ImageNode`) rather than packing glyphs into a `Text` run with an icon font.
///
/// `position_type: Absolute` (escaping `modal_menu()`'s centered flex flow) is what lets this sit
/// at the top-left corner as a sibling of the centered button row instead of being squeezed into
/// the same flex line.
fn controls_tips() -> impl Scene {
    bsn! {
        KeyboardMouseControlsTips
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
        }
        Pickable::IGNORE
        Children[
            panel(px(300), px(600))
            Children [
                control_tip_keys(
                    &[KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD],
                    "hud-controls-move",
                ),
                control_tip_icons(vec![MOUSE_MOVE_ICON_PNG], "hud-controls-look"),
                control_tip_keys(&[KeyCode::Space], "hud-controls-jump"),
                control_tip_mouse_button(MouseButton::Left, "hud-controls-select"),
                control_tip_mouse_button(MouseButton::Right, "hud-controls-deselect"),
                control_tip_keys(&[KeyCode::KeyF], "hud-controls-attack"),
                control_tip_keys(&[KeyCode::KeyT], "hud-controls-kill"),
                control_tip_keys(&[KeyCode::KeyE], "hud-controls-spawn-cube"),
                control_tip_keys(&[KeyCode::KeyR], "hud-controls-spawn-npc"),
                control_tip_keys(&[KeyCode::Escape], "hud-controls-menu"),
                control_tip_keys(&[KeyCode::Tab], "hud-controls-stats"),
            ]
        ]
    }
}

/// The gamepad equivalent of `controls_tips` — same rows, same order, same labels, just Steam
/// Deck button/stick icons (`input_icons::gamepad_button_icon_png`) sourced from the actual
/// `GamepadButton`s `controls.rs`'s `player_controls()` binds, instead of `KeyCode`s. Shown
/// instead of `controls_tips` (never alongside it) once `InputDeviceState` says a gamepad is the
/// active device — see `update_controls_tips_visibility`.
fn gamepad_controls_tips() -> impl Scene {
    bsn! {
        GamepadControlsTips
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
        }
        Pickable::IGNORE
        Children[
            panel(px(300), px(600))
            Children [
                control_tip_icons(vec![GAMEPAD_MOVE_STICK_ICON_PNG], "hud-controls-move"),
                control_tip_icons(vec![GAMEPAD_LOOK_STICK_ICON_PNG], "hud-controls-look"),
                control_tip_gamepad_buttons(&[GamepadButton::South], "hud-controls-jump"),
                control_tip_gamepad_buttons(&[GamepadButton::RightThumb], "hud-controls-select"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftThumb], "hud-controls-deselect"),
                control_tip_gamepad_buttons(&[GamepadButton::RightTrigger2], "hud-controls-attack"),
                control_tip_gamepad_buttons(&[GamepadButton::RightTrigger], "hud-controls-kill"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftTrigger], "hud-controls-spawn-cube"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftTrigger2], "hud-controls-spawn-npc"),
                control_tip_gamepad_buttons(&[GamepadButton::Start], "hud-controls-menu"),
                control_tip_gamepad_buttons(&[GamepadButton::Select], "hud-controls-stats"),
            ]
        ]
    }
}

/// Same idea as `control_tip_keys`, for `GamepadButton`s instead of `KeyCode`s — any button the
/// pack doesn't cover is silently skipped, same contract as `input_icons::gamepad_button_icon_png`.
fn control_tip_gamepad_buttons(buttons: &[GamepadButton], label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = buttons
        .iter()
        .filter_map(|&button| gamepad_button_icon_png(button))
        .collect();
    control_tip_icons(icons, label_key)
}

/// Builds a `control_tip_icons` row directly from the `KeyCode`s a binding actually uses, via
/// `input_icons::key_code_icon_png` — any key the pack doesn't cover is silently skipped rather
/// than showing a broken image or panicking, so an unmapped key just quietly narrows the icon set
/// for that row instead of breaking it.
fn control_tip_keys(keys: &[KeyCode], label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = keys
        .iter()
        .filter_map(|&key| key_code_icon_png(key))
        .collect();
    control_tip_icons(icons, label_key)
}

/// Same idea as `control_tip_keys`, for the one mouse-button tip (`Select`) — not a `KeyCode`, so
/// it goes through `input_icons::mouse_button_icon_png` instead.
fn control_tip_mouse_button(button: MouseButton, label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = mouse_button_icon_png(button).into_iter().collect();
    control_tip_icons(icons, label_key)
}

/// One row: zero or more icon images side by side (e.g. `W A S D` as four separate `ImageNode`s,
/// left to right) followed by a localized label. `icons` is a `Vec` rather than a fixed-size slice
/// since a binding can use any number of keys, including zero if none of them mapped to an icon —
/// the row then just shows the label on its own instead of disappearing entirely, so a gap in
/// icon coverage stays visible/debuggable rather than silently dropping the whole tip.
fn control_tip_icons(icons: Vec<&'static str>, label_key: &'static str) -> impl Scene {
    let icons: Vec<_> = icons.into_iter().map(control_tip_icon).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        Pickable::IGNORE
        Children [
            {icons},
            (
                Text(label_key)
                LocalizedText(label_key)
                TextFont {
                    font: FontSourceTemplate::Handle(SERIF_FONT),
                    font_size: px(CONTROLS_TIPS_LABEL_FONT_SIZE),
                }
                TextColor(WHITE)
                Pickable::IGNORE
            ),
        ]
    }
}

/// A single icon image at the panel's fixed icon size — one `ImageNode` per key/button, laid out
/// in a row by `control_tip_icons`'s parent `Node` rather than packed into one `Text` the way the
/// font-glyph version did.
fn control_tip_icon(path: &'static str) -> impl Scene {
    bsn! {
        ImageNode { image: path }
        Node {
            width: px(CONTROLS_TIPS_ICON_SIZE),
            height: px(CONTROLS_TIPS_ICON_SIZE),
            margin: UiRect::right(px(CONTROLS_TIPS_ICON_GAP)),
        }
        Pickable::IGNORE
    }
}
