//! Maps `KeyCode`/`MouseButton`/`GamepadButton` to the matching icon in Kenney's CC0 "Input
//! Prompts" packs, rendered from Kenney's own pre-built texture atlas for each pack (a sheet PNG
//! together with a Sparrow/Starling-format XML manifest naming each icon's rect within it) rather
//! than one `ImageNode`/draw call per separate PNG — see `InputIconAtlases` below.
//!
//! This used to build its own atlas at runtime (via `bevy_image::TextureAtlasBuilder`, packing the
//! pack's ~85 individual per-icon PNGs) because a first pass over `assets_src/input_prompts/`
//! checked only for a `Spritesheet`-*named subdirectory* and found none. That check was wrong: the
//! pre-built sheet + XML manifest sit as loose files at the top level of each pack folder instead
//! (`assets_src/input_prompts/{Keyboard & Mouse,Steam Deck}/keyboard-&-mouse_sheet_default.{png,
//! xml}` / `steam-deck_sheet_default.{png,xml}`), copied into `assets/textures/input_prompts/` as
//! `{keyboard_mouse,steam_deck}_sheet.{png,xml}`. Using Kenney's own sheet means no runtime
//! packing pass and no risk of drifting from the pack's authored/tested layout.
//!
//! The XML is the "Sparrow"/"Starling" atlas format — a widely-supported TexturePacker export
//! preset (not a Kenney-specific thing): `<TextureAtlas imagePath="..."><SubTexture name="..."
//! x="" y="" width="" height=""/>...</TextureAtlas>`, one `SubTexture` per icon, named exactly the
//! same as the individual pack's own per-icon filenames minus extension (e.g. `"keyboard_a"`,
//! `"mouse_left"`) — `SparrowAtlasManifest`/`SparrowAtlasLoader` below parse it, via `quick-xml`,
//! into that same name → pixel-rect mapping.
//!
//! Kenney's own pack ships these as a single icon font too (`fonts/input_prompts/`); this module
//! used to wrap that instead of PNGs, but the PNGs turned out to have a real advantage the font
//! didn't: `mouse_left.png`/`mouse_right.png`/`mouse.png` are genuinely distinct images (a
//! highlighted button vs. a plain silhouette), whereas the font's `mouse_left`/`mouse_right`/
//! `mouse` glyphs are pixel-identical — a font-level limitation found while fixing that font's
//! winding-direction bug (see the font file's own history), not fixable by re-winding contours.

use bevy::asset::AssetLoader;
use bevy::asset::io::Reader;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use crate::assets::CommonAssets;
use crate::game_state::GameState;

pub struct InputIconsPlugin;

impl Plugin for InputIconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SparrowAtlasManifest>();
        app.init_asset_loader::<SparrowAtlasLoader>();
        // `OnEnter(GameState::MainMenu)`, not `OnEnter(ModalMenuState::Open)` (the only current
        // consumer, see `modal_menu.rs`) — finalizing both atlases once, up front, right when
        // `CommonAssets`'s sheets/manifests first become available, avoids a first-pause hitch.
        app.add_systems(OnEnter(GameState::MainMenu), finalize_input_icon_atlases);
    }
}

/// A parsed Sparrow/Starling atlas manifest: every named icon's pixel rect within its sheet
/// image, keyed exactly as its `SubTexture name="..."` attribute reads (matching the pack's own
/// per-icon filenames minus extension). Not consumed directly outside this module — see
/// `finalize_icon_atlas`, which turns one of these plus its sheet `Handle<Image>` into an
/// `IconAtlas`.
#[derive(Asset, TypePath)]
pub struct SparrowAtlasManifest {
    frames: HashMap<Box<str>, URect>,
}

/// Parses the Sparrow/Starling XML format (see this module's doc comment) via `quick-xml`'s
/// pull-based `Reader` — the format has no nesting worth a full DOM, just a flat run of
/// self-closing `<SubTexture .../>` elements, so a straight event loop is enough.
#[derive(Default, TypePath)]
struct SparrowAtlasLoader;

impl AssetLoader for SparrowAtlasLoader {
    type Asset = SparrowAtlasManifest;
    type Settings = ();
    type Error = SparrowAtlasError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<SparrowAtlasManifest, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let text = std::str::from_utf8(&bytes)?;
        Ok(SparrowAtlasManifest {
            frames: parse_sparrow_atlas(text)?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["xml"]
    }
}

/// The actual XML→rects parsing, factored out of `SparrowAtlasLoader::load` so it's testable
/// without needing a `dyn Reader`/`LoadContext` (confirmed against the real committed
/// `keyboard_mouse_sheet.xml`/`steam_deck_sheet.xml` while writing this — 243 `SubTexture`
/// entries parsed correctly for the keyboard/mouse pack, matching a direct `grep -c` count).
fn parse_sparrow_atlas(text: &str) -> Result<HashMap<Box<str>, URect>, SparrowAtlasError> {
    let mut frames = HashMap::default();
    let mut xml = XmlReader::from_str(text);
    loop {
        match xml.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == "SubTexture" => {
                let (mut name, mut x, mut y, mut width, mut height) =
                    (None, None, None, None, None);
                for attr in tag.attributes() {
                    let attr = attr?;
                    let value = attr.value.as_ref();
                    match attr.key.as_ref() {
                        "name" => name = Some(value.to_owned()),
                        "x" => x = value.parse::<u32>().ok(),
                        "y" => y = value.parse::<u32>().ok(),
                        "width" => width = value.parse::<u32>().ok(),
                        "height" => height = value.parse::<u32>().ok(),
                        _ => {}
                    }
                }
                if let (Some(name), Some(x), Some(y), Some(width), Some(height)) =
                    (name, x, y, width, height)
                {
                    frames.insert(
                        name.into_boxed_str(),
                        URect::new(x, y, x + width, y + height),
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(frames)
}

#[derive(Debug)]
enum SparrowAtlasError {
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    Xml(quick_xml::Error),
    XmlAttr(quick_xml::events::attributes::AttrError),
}

impl std::fmt::Display for SparrowAtlasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read atlas manifest: {err}"),
            Self::Utf8(err) => write!(f, "atlas manifest is not valid UTF-8: {err}"),
            Self::Xml(err) => write!(f, "failed to parse atlas manifest XML: {err}"),
            Self::XmlAttr(err) => write!(f, "failed to parse atlas manifest XML attribute: {err}"),
        }
    }
}

impl std::error::Error for SparrowAtlasError {}

impl From<std::io::Error> for SparrowAtlasError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
impl From<std::str::Utf8Error> for SparrowAtlasError {
    fn from(err: std::str::Utf8Error) -> Self {
        Self::Utf8(err)
    }
}
impl From<quick_xml::Error> for SparrowAtlasError {
    fn from(err: quick_xml::Error) -> Self {
        Self::Xml(err)
    }
}
impl From<quick_xml::events::attributes::AttrError> for SparrowAtlasError {
    fn from(err: quick_xml::events::attributes::AttrError) -> Self {
        Self::XmlAttr(err)
    }
}

/// Both atlases, finalized once by `finalize_input_icon_atlases` from `CommonAssets`'s sheet
/// images + parsed manifests.
#[derive(Resource)]
pub struct InputIconAtlases {
    keyboard_mouse: IconAtlas,
    steam_deck: IconAtlas,
}

/// One atlas: the sheet image + a `TextureAtlasLayout` built from its manifest's rects, plus each
/// icon's index into that layout, keyed by name (e.g. `"keyboard_a"`, `"mouse_left"`).
struct IconAtlas {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
    indices: HashMap<Box<str>, usize>,
}

impl IconAtlas {
    fn get(&self, name: &str) -> Option<usize> {
        self.indices.get(name).copied()
    }

    fn image_node(&self, index: usize) -> ImageNode {
        ImageNode::from_atlas_image(
            self.image.clone(),
            TextureAtlas {
                layout: self.layout.clone(),
                index,
            },
        )
    }
}

/// Builds one `IconAtlas` from an already-loaded sheet image + parsed manifest — `image`'s pixel
/// size becomes the layout's overall size (the Sparrow XML format doesn't carry that itself, only
/// per-icon rects), and each manifest entry gets a fresh index assigned in iteration order.
///
/// Each rect's Y also gets flipped here (`size.y` is only known now, not at parse time) —
/// confirmed by direct pixel inspection that this XML's `y` is measured bottom-up, not top-down
/// like the rest of the rect (`x`, `width`, `height`) and like raster image rows: taking the
/// `keyboard_t`/`keyboard_backspace` entries at face value pointed at "T" and a left-arrow icon
/// respectively, not "T" and "BACKSPACE" — cropping at `sheet_height - cell_height - xml_y`
/// instead landed on the correct glyph for every entry checked.
fn finalize_icon_atlas(
    image: Handle<Image>,
    manifest: &SparrowAtlasManifest,
    images: &Assets<Image>,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> IconAtlas {
    let size = images
        .get(&image)
        .expect("CommonAssets guarantees the sheet image is already loaded")
        .size();
    let mut layout = TextureAtlasLayout::new_empty(size);
    let mut indices = HashMap::default();
    for (name, rect) in &manifest.frames {
        let flipped = URect::new(
            rect.min.x,
            size.y - rect.max.y,
            rect.max.x,
            size.y - rect.min.y,
        );
        indices.insert(name.clone(), layout.textures.len());
        layout.textures.push(flipped);
    }
    IconAtlas {
        image,
        layout: layouts.add(layout),
        indices,
    }
}

fn finalize_input_icon_atlases(
    mut commands: Commands,
    common_assets: Res<CommonAssets>,
    images: Res<Assets<Image>>,
    manifests: Res<Assets<SparrowAtlasManifest>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // `CommonAssets` guarantees both manifests are already loaded by the time
    // `GameState::MainMenu` is entered (that's the whole point of `GameState::AssetLoading`
    // blocking on the collection) — `None` here would mean that guarantee broke.
    let keyboard_mouse_manifest = manifests
        .get(&common_assets.keyboard_mouse_atlas_manifest)
        .expect("keyboard/mouse atlas manifest should already be loaded");
    let steam_deck_manifest = manifests
        .get(&common_assets.steam_deck_atlas_manifest)
        .expect("Steam Deck atlas manifest should already be loaded");
    commands.insert_resource(InputIconAtlases {
        keyboard_mouse: finalize_icon_atlas(
            common_assets.keyboard_mouse_atlas_image.clone(),
            keyboard_mouse_manifest,
            &images,
            &mut layouts,
        ),
        steam_deck: finalize_icon_atlas(
            common_assets.steam_deck_atlas_image.clone(),
            steam_deck_manifest,
            &images,
            &mut layouts,
        ),
    });
}

/// Which pack an `Icon` came from — a plain `Copy` enum (no `Handle`s, no template machinery)
/// deliberately, so it (and `Icon`, which pairs it with a plain `usize` index) can pass through
/// `bsn!` scene-building as ordinary component data — see `modal_menu.rs`'s `PendingIcon` for why
/// that matters (an already-built `ImageNode`/`TextureAtlas` value can't).
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum IconPack {
    #[default]
    KeyboardMouse,
    SteamDeck,
}

/// A resolved icon: which pack it's from, plus its index into that pack's atlas layout. Call
/// `resolve` (with the same `InputIconAtlases`) to turn this into an actual `ImageNode`.
#[derive(Clone, Copy, Default)]
pub struct Icon {
    pub pack: IconPack,
    pub index: usize,
}

impl Icon {
    pub fn resolve(self, atlases: &InputIconAtlases) -> ImageNode {
        let atlas = match self.pack {
            IconPack::KeyboardMouse => &atlases.keyboard_mouse,
            IconPack::SteamDeck => &atlases.steam_deck,
        };
        atlas.image_node(self.index)
    }
}

/// Looks up the icon for `key`, if this pack has one. `None` for keys the pack doesn't cover (most
/// notably: no numpad, media, or `Super`/Windows-key icons in this set) — callers decide their own
/// fallback (skip the icon, fall back to text, etc.) rather than this silently substituting
/// something that isn't actually `key`.
pub fn key_code_icon(atlases: &InputIconAtlases, key: KeyCode) -> Option<Icon> {
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
    Some(Icon {
        pack: IconPack::KeyboardMouse,
        index: atlases.keyboard_mouse.get(name)?,
    })
}

/// Looks up the icon for a mouse button. Not a `KeyCode`, so it's a separate function/match rather
/// than another arm above — see the module doc comment. `MouseButton::Middle` has no icon in this
/// pack (checked directly — `mouse_middle.png` doesn't exist alongside `mouse_left.png`/
/// `mouse_right.png`), so it falls through to `None` like any other uncovered key.
pub fn mouse_button_icon(atlases: &InputIconAtlases, button: MouseButton) -> Option<Icon> {
    let name = match button {
        MouseButton::Left => "mouse_left",
        MouseButton::Right => "mouse_right",
        _ => return None,
    };
    Some(Icon {
        pack: IconPack::KeyboardMouse,
        index: atlases.keyboard_mouse.get(name)?,
    })
}

/// The mouse-motion icon (not tied to any `MouseButton` press) — used for always-on look/aim
/// controls. Exposed as its own function rather than folded into `mouse_button_icon` since there's
/// only ever one of these, unlike buttons.
pub fn mouse_move_icon(atlases: &InputIconAtlases) -> Icon {
    Icon {
        pack: IconPack::KeyboardMouse,
        index: atlases
            .keyboard_mouse
            .get("mouse_move")
            .expect("mouse_move.png is always present in the keyboard/mouse pack"),
    }
}

/// Looks up the icon for a gamepad button, using Steam Deck's own button icons — this project's
/// primary handheld/gamepad target (see the project's own vision notes) — rather than a
/// generic/Xbox/PlayStation set. `bevy`'s `GamepadButton` names describe an Xbox-style layout
/// (`South`/`East`/`North`/`West`), which the Steam Deck's face buttons also use physically
/// (A/B/X/Y), so the mapping is direct. Only the variants `controls.rs`'s `player_controls()`
/// actually binds are covered — `None` for the rest, same "let the caller decide" contract as
/// `key_code_icon`.
pub fn gamepad_button_icon(atlases: &InputIconAtlases, button: GamepadButton) -> Option<Icon> {
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
    Some(Icon {
        pack: IconPack::SteamDeck,
        index: atlases.steam_deck.get(name)?,
    })
}

/// The left-stick "move" and right-stick "look" icons (not tied to any `GamepadButton` press) —
/// same idea as `mouse_move_icon`.
pub fn gamepad_move_stick_icon(atlases: &InputIconAtlases) -> Icon {
    Icon {
        pack: IconPack::SteamDeck,
        index: atlases
            .steam_deck
            .get("steamdeck_stick_l")
            .expect("steamdeck_stick_l.png is always present in the Steam Deck pack"),
    }
}
pub fn gamepad_look_stick_icon(atlases: &InputIconAtlases) -> Icon {
    Icon {
        pack: IconPack::SteamDeck,
        index: atlases
            .steam_deck
            .get("steamdeck_stick_r")
            .expect("steamdeck_stick_r.png is always present in the Steam Deck pack"),
    }
}
