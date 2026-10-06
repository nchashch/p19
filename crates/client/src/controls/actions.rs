use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct AttackAction;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct KillAction;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Select;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Deselect;

#[derive(InputAction)]
#[action_output(bool)]
pub struct MainMenu;

/// Opens/closes the in-game pause menu (`modal_menu.rs`) — see `game_state::ModalMenuState`.
/// Bound to Escape/`GamepadButton::Start` (see `controls.rs`'s `player_controls()`); `MainMenu`
/// above is currently unbound to any input — its instant, no-confirmation disconnect was replaced
/// by this plus the modal's own "Main Menu" button.
#[derive(InputAction)]
#[action_output(bool)]
pub struct ToggleModalMenu;

/// Shows/hides `hud.rs`'s `DataFrame` debug panel (hidden by default). Bound to
/// Tab/`GamepadButton::Select` — see `controls.rs`'s `toggle_data_frame`.
#[derive(InputAction)]
#[action_output(bool)]
pub struct ToggleDataFrame;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Shoot;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SpawnNpcAction;

/// Directional input for menu navigation (gamepad d-pad/left stick, or arrow keys) — see
/// `ui::markup`'s `MenuControls`. Distinct from `Movement`: this drives `InputFocus` via
/// `bevy::ui::auto_directional_navigation`, not a character.
#[derive(InputAction)]
#[action_output(Vec2)]
pub struct UiNavigate;

/// "Activate the focused UI element" — gamepad South or Enter, see `ui::markup`'s
/// `MenuControls`. Emits the focused element's `data-on-click` signal, the same message a
/// pointer click produces.
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiConfirm;

/// "Close this screen" — Escape or gamepad East, bound only while a closable screen is open (the
/// credits, see `ui::credits`'s `CreditsControls`).
#[derive(InputAction)]
#[action_output(bool)]
pub struct UiCancel;
