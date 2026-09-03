//! Maps `KeyCode`/`MouseButton` to the matching icon PNG in Kenney's CC0 "Input Prompts"
//! keyboard/mouse pack (`assets_src/input_prompts/Keyboard & Mouse/Default/`, copied into
//! `assets/textures/input_prompts/keyboard_mouse/` under the pack's own filenames — not re-keyed
//! to the `KeyCode` variant name, so e.g. `KeyCode::Quote` maps to `keyboard_apostrophe.png`), so
//! call sites can build a row of icons directly from the same `KeyCode`s a binding actually uses
//! (see `hud.rs`'s `controls_tips`) instead of hand-copied asset-path strings with no visible
//! connection to which key they represent.
//!
//! The pack also ships these as a single icon font (`fonts/input_prompts/`); this module used to
//! wrap that instead, but the PNGs turned out to have a real advantage the font didn't:
//! `mouse_left.png`/`mouse_right.png`/`mouse.png` are genuinely distinct images (a highlighted
//! button vs. a plain silhouette), whereas the font's `mouse_left`/`mouse_right`/`mouse` glyphs
//! are pixel-identical — a font-level limitation found while fixing that font's winding-direction
//! bug (see the font file's own history), not fixable by re-winding contours.

use bevy::prelude::{GamepadButton, KeyCode, MouseButton};

/// Every path below lives under this directory (copied from
/// `assets_src/input_prompts/Keyboard & Mouse/Default/` under the pack's own filenames).
macro_rules! icon_png {
    ($name:literal) => {
        concat!("textures/input_prompts/keyboard_mouse/", $name, ".png")
    };
}

/// Looks up the icon-PNG asset path for `key`, if this pack has one. `None` for keys the pack
/// doesn't cover (most notably: no numpad, media, or `Super`/Windows-key icons in this set) —
/// callers decide their own fallback (skip the icon, fall back to text, etc.) rather than this
/// silently substituting something that isn't actually `key`.
pub fn key_code_icon_png(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::KeyA => icon_png!("keyboard_a"),
        KeyCode::KeyB => icon_png!("keyboard_b"),
        KeyCode::KeyC => icon_png!("keyboard_c"),
        KeyCode::KeyD => icon_png!("keyboard_d"),
        KeyCode::KeyE => icon_png!("keyboard_e"),
        KeyCode::KeyF => icon_png!("keyboard_f"),
        KeyCode::KeyG => icon_png!("keyboard_g"),
        KeyCode::KeyH => icon_png!("keyboard_h"),
        KeyCode::KeyI => icon_png!("keyboard_i"),
        KeyCode::KeyJ => icon_png!("keyboard_j"),
        KeyCode::KeyK => icon_png!("keyboard_k"),
        KeyCode::KeyL => icon_png!("keyboard_l"),
        KeyCode::KeyM => icon_png!("keyboard_m"),
        KeyCode::KeyN => icon_png!("keyboard_n"),
        KeyCode::KeyO => icon_png!("keyboard_o"),
        KeyCode::KeyP => icon_png!("keyboard_p"),
        KeyCode::KeyQ => icon_png!("keyboard_q"),
        KeyCode::KeyR => icon_png!("keyboard_r"),
        KeyCode::KeyS => icon_png!("keyboard_s"),
        KeyCode::KeyT => icon_png!("keyboard_t"),
        KeyCode::KeyU => icon_png!("keyboard_u"),
        KeyCode::KeyV => icon_png!("keyboard_v"),
        KeyCode::KeyW => icon_png!("keyboard_w"),
        KeyCode::KeyX => icon_png!("keyboard_x"),
        KeyCode::KeyY => icon_png!("keyboard_y"),
        KeyCode::KeyZ => icon_png!("keyboard_z"),

        KeyCode::Digit0 => icon_png!("keyboard_0"),
        KeyCode::Digit1 => icon_png!("keyboard_1"),
        KeyCode::Digit2 => icon_png!("keyboard_2"),
        KeyCode::Digit3 => icon_png!("keyboard_3"),
        KeyCode::Digit4 => icon_png!("keyboard_4"),
        KeyCode::Digit5 => icon_png!("keyboard_5"),
        KeyCode::Digit6 => icon_png!("keyboard_6"),
        KeyCode::Digit7 => icon_png!("keyboard_7"),
        KeyCode::Digit8 => icon_png!("keyboard_8"),
        KeyCode::Digit9 => icon_png!("keyboard_9"),

        KeyCode::F1 => icon_png!("keyboard_f1"),
        KeyCode::F2 => icon_png!("keyboard_f2"),
        KeyCode::F3 => icon_png!("keyboard_f3"),
        KeyCode::F4 => icon_png!("keyboard_f4"),
        KeyCode::F5 => icon_png!("keyboard_f5"),
        KeyCode::F6 => icon_png!("keyboard_f6"),
        KeyCode::F7 => icon_png!("keyboard_f7"),
        KeyCode::F8 => icon_png!("keyboard_f8"),
        KeyCode::F9 => icon_png!("keyboard_f9"),
        KeyCode::F10 => icon_png!("keyboard_f10"),
        KeyCode::F11 => icon_png!("keyboard_f11"),
        KeyCode::F12 => icon_png!("keyboard_f12"),

        KeyCode::ArrowUp => icon_png!("keyboard_arrow_up"),
        KeyCode::ArrowDown => icon_png!("keyboard_arrow_down"),
        KeyCode::ArrowLeft => icon_png!("keyboard_arrow_left"),
        KeyCode::ArrowRight => icon_png!("keyboard_arrow_right"),

        KeyCode::Space => icon_png!("keyboard_space"),
        KeyCode::Escape => icon_png!("keyboard_escape"),
        // The pack has two distinct enter-style icons (`keyboard_enter` and `keyboard_return`) —
        // `Return` (this one) is the main-keyboard key; `NumpadEnter` below gets the other.
        KeyCode::Enter => icon_png!("keyboard_return"),
        KeyCode::NumpadEnter => icon_png!("keyboard_enter"),
        KeyCode::Tab => icon_png!("keyboard_tab"),
        KeyCode::Backspace => icon_png!("keyboard_backspace"),
        KeyCode::Delete => icon_png!("keyboard_delete"),
        KeyCode::Insert => icon_png!("keyboard_insert"),
        KeyCode::Home => icon_png!("keyboard_home"),
        KeyCode::End => icon_png!("keyboard_end"),
        KeyCode::PageUp => icon_png!("keyboard_page_up"),
        KeyCode::PageDown => icon_png!("keyboard_page_down"),
        KeyCode::CapsLock => icon_png!("keyboard_capslock"),

        KeyCode::ShiftLeft | KeyCode::ShiftRight => icon_png!("keyboard_shift"),
        KeyCode::ControlLeft | KeyCode::ControlRight => icon_png!("keyboard_ctrl"),
        KeyCode::AltLeft | KeyCode::AltRight => icon_png!("keyboard_alt"),
        KeyCode::Comma => icon_png!("keyboard_comma"),
        KeyCode::Period => icon_png!("keyboard_period"),
        KeyCode::Semicolon => icon_png!("keyboard_semicolon"),
        KeyCode::Quote => icon_png!("keyboard_apostrophe"),
        KeyCode::Minus => icon_png!("keyboard_minus"),
        KeyCode::Equal => icon_png!("keyboard_equals"),
        KeyCode::BracketLeft => icon_png!("keyboard_bracket_open"),
        KeyCode::BracketRight => icon_png!("keyboard_bracket_close"),
        KeyCode::Backquote => icon_png!("keyboard_tilde"),

        _ => return None,
    })
}

/// Looks up the icon-PNG asset path for a mouse button. Not a `KeyCode`, so it's a separate
/// function/match rather than another arm above — see the module doc comment. `MouseButton::Middle`
/// has no icon in this pack (checked directly — `mouse_middle.png` doesn't exist alongside
/// `mouse_left.png`/`mouse_right.png`), so it falls through to `None` like any other uncovered key.
pub fn mouse_button_icon_png(button: MouseButton) -> Option<&'static str> {
    Some(match button {
        MouseButton::Left => icon_png!("mouse_left"),
        MouseButton::Right => icon_png!("mouse_right"),
        _ => return None,
    })
}

/// The mouse-motion icon (not tied to any `MouseButton` press) — used for always-on look/aim
/// controls. Exposed as its own constant rather than a `mouse_button_icon_png`-style function
/// since there's only ever one of these, unlike buttons.
pub const MOUSE_MOVE_ICON_PNG: &str = icon_png!("mouse_move");

/// Every path below lives under this directory (copied from
/// `assets_src/input_prompts/Steam Deck/Default/` under the pack's own filenames).
macro_rules! steam_deck_icon_png {
    ($name:literal) => {
        concat!("textures/input_prompts/steam_deck/", $name, ".png")
    };
}

/// Looks up the icon-PNG asset path for a gamepad button, using Steam Deck's own button icons —
/// this project's primary handheld/gamepad target (see the project's own vision notes) — rather
/// than a generic/Xbox/PlayStation set. `bevy`'s `GamepadButton` names describe an Xbox-style
/// layout (`South`/`East`/`North`/`West`), which the Steam Deck's face buttons also use physically
/// (A/B/X/Y), so the mapping is direct. Only the variants `controls.rs`'s `player_controls()`
/// actually binds are covered — `None` for the rest, same "let the caller decide" contract as
/// `key_code_icon_png`.
pub fn gamepad_button_icon_png(button: GamepadButton) -> Option<&'static str> {
    Some(match button {
        GamepadButton::South => steam_deck_icon_png!("steamdeck_button_a"),
        GamepadButton::East => steam_deck_icon_png!("steamdeck_button_b"),
        GamepadButton::North => steam_deck_icon_png!("steamdeck_button_y"),
        GamepadButton::West => steam_deck_icon_png!("steamdeck_button_x"),
        GamepadButton::LeftTrigger => steam_deck_icon_png!("steamdeck_button_l1"),
        GamepadButton::LeftTrigger2 => steam_deck_icon_png!("steamdeck_button_l2"),
        GamepadButton::RightTrigger => steam_deck_icon_png!("steamdeck_button_r1"),
        GamepadButton::RightTrigger2 => steam_deck_icon_png!("steamdeck_button_r2"),
        GamepadButton::LeftThumb => steam_deck_icon_png!("steamdeck_stick_l_press"),
        GamepadButton::RightThumb => steam_deck_icon_png!("steamdeck_stick_r_press"),
        // The Steam Deck's own naming for its two small menu-row buttons: "Options" (right side,
        // the Xbox-style Start/menu equivalent `bevy`'s `Start` maps onto) and "View" (left side,
        // the Xbox-style Select/back equivalent `bevy`'s `Select` maps onto).
        GamepadButton::Start => steam_deck_icon_png!("steamdeck_button_options"),
        GamepadButton::Select => steam_deck_icon_png!("steamdeck_button_view"),
        GamepadButton::DPadUp => steam_deck_icon_png!("steamdeck_dpad_up"),
        GamepadButton::DPadDown => steam_deck_icon_png!("steamdeck_dpad_down"),
        GamepadButton::DPadLeft => steam_deck_icon_png!("steamdeck_dpad_left"),
        GamepadButton::DPadRight => steam_deck_icon_png!("steamdeck_dpad_right"),

        _ => return None,
    })
}

/// The left-stick "move" and right-stick "look" icons (not tied to any `GamepadButton` press) —
/// same idea as `MOUSE_MOVE_ICON_PNG`.
pub const GAMEPAD_MOVE_STICK_ICON_PNG: &str = steam_deck_icon_png!("steamdeck_stick_l");
pub const GAMEPAD_LOOK_STICK_ICON_PNG: &str = steam_deck_icon_png!("steamdeck_stick_r");
