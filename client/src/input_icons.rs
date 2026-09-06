//! Maps `KeyCode`/`MouseButton` to the matching icon image in Kenney's CC0 "Input Prompts"
//! keyboard/mouse pack (`assets_src/input_prompts/Keyboard & Mouse/Default/`, copied into
//! `assets/textures/input_prompts/keyboard_mouse/` under the pack's own filenames — not re-keyed
//! to the `KeyCode` variant name, so e.g. `KeyCode::Quote` maps to `keyboard_apostrophe.png`), so
//! call sites can build a row of icons directly from the same `KeyCode`s a binding actually uses
//! (see `modal_menu.rs`'s `controls_tips`) instead of hand-copied asset-path strings with no
//! visible connection to which key they represent.
//!
//! The pack also ships these as a single icon font (`fonts/input_prompts/`); this module used to
//! wrap that instead, but the PNGs turned out to have a real advantage the font didn't:
//! `mouse_left.png`/`mouse_right.png`/`mouse.png` are genuinely distinct images (a highlighted
//! button vs. a plain silhouette), whereas the font's `mouse_left`/`mouse_right`/`mouse` glyphs
//! are pixel-identical — a font-level limitation found while fixing that font's winding-direction
//! bug (see the font file's own history), not fixable by re-winding contours.
//!
//! Every lookup here goes through `assets::CommonAssets`'s `keyboard_mouse_icons`/
//! `steam_deck_icons` folder collections (`HashMap<AssetFileStem, Handle<Image>>` — the file's
//! name minus extension is the key, e.g. `"keyboard_a"`, matching what the old `icon_png!` macro
//! used to build as a path string) rather than returning a path for the caller to `asset_server
//! .load(...)` itself, so every icon is already resident by the time it's actually looked up.

use bevy::prelude::{GamepadButton, Handle, Image, KeyCode, MouseButton};

use crate::assets::CommonAssets;

/// Looks up the icon `Handle<Image>` for `key`, if this pack has one. `None` for keys the pack
/// doesn't cover (most notably: no numpad, media, or `Super`/Windows-key icons in this set) —
/// callers decide their own fallback (skip the icon, fall back to text, etc.) rather than this
/// silently substituting something that isn't actually `key`.
pub fn key_code_icon(common_assets: &CommonAssets, key: KeyCode) -> Option<Handle<Image>> {
    let name = match key {
        KeyCode::KeyA => "keyboard_a",
        KeyCode::KeyB => "keyboard_b",
        KeyCode::KeyC => "keyboard_c",
        KeyCode::KeyD => "keyboard_d",
        KeyCode::KeyE => "keyboard_e",
        KeyCode::KeyF => "keyboard_f",
        KeyCode::KeyG => "keyboard_g",
        KeyCode::KeyH => "keyboard_h",
        KeyCode::KeyI => "keyboard_i",
        KeyCode::KeyJ => "keyboard_j",
        KeyCode::KeyK => "keyboard_k",
        KeyCode::KeyL => "keyboard_l",
        KeyCode::KeyM => "keyboard_m",
        KeyCode::KeyN => "keyboard_n",
        KeyCode::KeyO => "keyboard_o",
        KeyCode::KeyP => "keyboard_p",
        KeyCode::KeyQ => "keyboard_q",
        KeyCode::KeyR => "keyboard_r",
        KeyCode::KeyS => "keyboard_s",
        KeyCode::KeyT => "keyboard_t",
        KeyCode::KeyU => "keyboard_u",
        KeyCode::KeyV => "keyboard_v",
        KeyCode::KeyW => "keyboard_w",
        KeyCode::KeyX => "keyboard_x",
        KeyCode::KeyY => "keyboard_y",
        KeyCode::KeyZ => "keyboard_z",

        KeyCode::Digit0 => "keyboard_0",
        KeyCode::Digit1 => "keyboard_1",
        KeyCode::Digit2 => "keyboard_2",
        KeyCode::Digit3 => "keyboard_3",
        KeyCode::Digit4 => "keyboard_4",
        KeyCode::Digit5 => "keyboard_5",
        KeyCode::Digit6 => "keyboard_6",
        KeyCode::Digit7 => "keyboard_7",
        KeyCode::Digit8 => "keyboard_8",
        KeyCode::Digit9 => "keyboard_9",

        KeyCode::F1 => "keyboard_f1",
        KeyCode::F2 => "keyboard_f2",
        KeyCode::F3 => "keyboard_f3",
        KeyCode::F4 => "keyboard_f4",
        KeyCode::F5 => "keyboard_f5",
        KeyCode::F6 => "keyboard_f6",
        KeyCode::F7 => "keyboard_f7",
        KeyCode::F8 => "keyboard_f8",
        KeyCode::F9 => "keyboard_f9",
        KeyCode::F10 => "keyboard_f10",
        KeyCode::F11 => "keyboard_f11",
        KeyCode::F12 => "keyboard_f12",

        KeyCode::ArrowUp => "keyboard_arrow_up",
        KeyCode::ArrowDown => "keyboard_arrow_down",
        KeyCode::ArrowLeft => "keyboard_arrow_left",
        KeyCode::ArrowRight => "keyboard_arrow_right",

        KeyCode::Space => "keyboard_space",
        KeyCode::Escape => "keyboard_escape",
        // The pack has two distinct enter-style icons (`keyboard_enter` and `keyboard_return`) —
        // `Return` (this one) is the main-keyboard key; `NumpadEnter` below gets the other.
        KeyCode::Enter => "keyboard_return",
        KeyCode::NumpadEnter => "keyboard_enter",
        KeyCode::Tab => "keyboard_tab",
        KeyCode::Backspace => "keyboard_backspace",
        KeyCode::Delete => "keyboard_delete",
        KeyCode::Insert => "keyboard_insert",
        KeyCode::Home => "keyboard_home",
        KeyCode::End => "keyboard_end",
        KeyCode::PageUp => "keyboard_page_up",
        KeyCode::PageDown => "keyboard_page_down",
        KeyCode::CapsLock => "keyboard_capslock",

        KeyCode::ShiftLeft | KeyCode::ShiftRight => "keyboard_shift",
        KeyCode::ControlLeft | KeyCode::ControlRight => "keyboard_ctrl",
        KeyCode::AltLeft | KeyCode::AltRight => "keyboard_alt",
        KeyCode::Comma => "keyboard_comma",
        KeyCode::Period => "keyboard_period",
        KeyCode::Semicolon => "keyboard_semicolon",
        KeyCode::Quote => "keyboard_apostrophe",
        KeyCode::Minus => "keyboard_minus",
        KeyCode::Equal => "keyboard_equals",
        KeyCode::BracketLeft => "keyboard_bracket_open",
        KeyCode::BracketRight => "keyboard_bracket_close",
        KeyCode::Backquote => "keyboard_tilde",

        _ => return None,
    };
    common_assets.keyboard_mouse_icons.get(name).cloned()
}

/// Looks up the icon `Handle<Image>` for a mouse button. Not a `KeyCode`, so it's a separate
/// function/match rather than another arm above — see the module doc comment.
/// `MouseButton::Middle` has no icon in this pack (checked directly — `mouse_middle.png` doesn't
/// exist alongside `mouse_left.png`/`mouse_right.png`), so it falls through to `None` like any
/// other uncovered key.
pub fn mouse_button_icon(
    common_assets: &CommonAssets,
    button: MouseButton,
) -> Option<Handle<Image>> {
    let name = match button {
        MouseButton::Left => "mouse_left",
        MouseButton::Right => "mouse_right",
        _ => return None,
    };
    common_assets.keyboard_mouse_icons.get(name).cloned()
}

/// The mouse-motion icon (not tied to any `MouseButton` press) — used for always-on look/aim
/// controls. Exposed as its own function rather than folded into `mouse_button_icon` since there's
/// only ever one of these, unlike buttons — but it's still a lookup against the same folder
/// collection, not a separately-declared `CommonAssets` field, so there's exactly one place
/// (`assets.rs`) that owns the whole keyboard/mouse icon set.
pub fn mouse_move_icon(common_assets: &CommonAssets) -> Handle<Image> {
    common_assets.keyboard_mouse_icons["mouse_move"].clone()
}

/// Looks up the icon `Handle<Image>` for a gamepad button, using Steam Deck's own button icons —
/// this project's primary handheld/gamepad target (see the project's own vision notes) — rather
/// than a generic/Xbox/PlayStation set. `bevy`'s `GamepadButton` names describe an Xbox-style
/// layout (`South`/`East`/`North`/`West`), which the Steam Deck's face buttons also use physically
/// (A/B/X/Y), so the mapping is direct. Only the variants `controls.rs`'s `player_controls()`
/// actually binds are covered — `None` for the rest, same "let the caller decide" contract as
/// `key_code_icon`.
pub fn gamepad_button_icon(
    common_assets: &CommonAssets,
    button: GamepadButton,
) -> Option<Handle<Image>> {
    let name = match button {
        GamepadButton::South => "steamdeck_button_a",
        GamepadButton::East => "steamdeck_button_b",
        GamepadButton::North => "steamdeck_button_y",
        GamepadButton::West => "steamdeck_button_x",
        GamepadButton::LeftTrigger => "steamdeck_button_l1",
        GamepadButton::LeftTrigger2 => "steamdeck_button_l2",
        GamepadButton::RightTrigger => "steamdeck_button_r1",
        GamepadButton::RightTrigger2 => "steamdeck_button_r2",
        GamepadButton::LeftThumb => "steamdeck_stick_l_press",
        GamepadButton::RightThumb => "steamdeck_stick_r_press",
        // The Steam Deck's own naming for its two small menu-row buttons: "Options" (right side,
        // the Xbox-style Start/menu equivalent `bevy`'s `Start` maps onto) and "View" (left side,
        // the Xbox-style Select/back equivalent `bevy`'s `Select` maps onto).
        GamepadButton::Start => "steamdeck_button_options",
        GamepadButton::Select => "steamdeck_button_view",
        GamepadButton::DPadUp => "steamdeck_dpad_up",
        GamepadButton::DPadDown => "steamdeck_dpad_down",
        GamepadButton::DPadLeft => "steamdeck_dpad_left",
        GamepadButton::DPadRight => "steamdeck_dpad_right",

        _ => return None,
    };
    common_assets.steam_deck_icons.get(name).cloned()
}

/// The left-stick "move" and right-stick "look" icons (not tied to any `GamepadButton` press) —
/// same idea as `mouse_move_icon`.
pub fn gamepad_move_stick_icon(common_assets: &CommonAssets) -> Handle<Image> {
    common_assets.steam_deck_icons["steamdeck_stick_l"].clone()
}
pub fn gamepad_look_stick_icon(common_assets: &CommonAssets) -> Handle<Image> {
    common_assets.steam_deck_icons["steamdeck_stick_r"].clone()
}
