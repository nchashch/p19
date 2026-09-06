//! `CommonAssets` is the single manifest for every asset the client needs regardless of which
//! level is loaded — models, sounds, the skybox, UI fonts, and the two input-prompt icon packs.
//! It's loaded once, up front, via `bevy_asset_loader`'s `LoadingState` (see `main.rs`'s
//! `GameState::AssetLoading` — the app doesn't reach `MainMenu` until this collection is fully
//! loaded), so every other module reads a pre-loaded `Handle`/lookup out of `Res<CommonAssets>`
//! instead of calling `asset_server.load(...)` with a hardcoded path at the point of use.
//!
//! `LevelAssets` below is a separate, not-yet-wired-up sketch for the future per-level dynamic
//! asset manifest (see the project's own design discussion on sharing a level-packaging mechanism
//! between `client` and `server`) — it isn't touched by this module's loading state and has no
//! consumers yet.
//!
//! Deliberately *not* included here: `locales/`'s `.ftl` files. `bevy_fluent`'s
//! `LocalizationBuilder::build` needs the raw `Handle<LoadedFolder>` itself (to look up the
//! folder's contents via `Res<Assets<LoadedFolder>>>` and group files by locale subfolder), but
//! `bevy_asset_loader`'s folder-collection support (`collection(...)` on a `Handle<LoadedFolder>`
//! field) only ever exposes the folder's *contents* as an exploded `Vec`/`HashMap` of individual
//! file handles, never the folder handle itself — confirmed directly against the derive macro's
//! codegen (`bevy_asset_loader_derive::assets::AssetField::Folder`). There's no attribute
//! combination that produces what `bevy_fluent` actually needs, so `localization.rs` keeps its
//! own manual `asset_server.load_folder("locales")` + polling instead.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use bevy_asset_loader::mapped::AssetFileStem;
use bevy_asset_loader::prelude::*;
use bevy_seedling::sample::AudioSample;

#[derive(AssetCollection, Resource)]
pub struct CommonAssets {
    /// The `Cube` prop's world-asset scene (`cube_spawner.rs`) — a `Handle<WorldAsset>`, not
    /// `Handle<Gltf>`, since that's what `WorldAssetRoot` (Skein/world-serialization's headless-safe
    /// spawn mechanism) actually wraps; see `shared`/CLAUDE.md's "Server" section for why this
    /// path exists alongside plain GLTF loading at all.
    #[asset(path = "models/Cube.glb#Scene0")]
    pub cube_world: Handle<WorldAsset>,
    /// The player/NPC rig's world-asset scene (`npc_spawner.rs`, `player_character.rs`) — same
    /// `WorldAsset` reasoning as `cube_world`.
    #[asset(path = "models/rig.glb#Scene0")]
    pub rig_world: Handle<WorldAsset>,
    /// The *same* `rig.glb`, loaded again as a plain `Handle<Gltf>` — `animation.rs` needs this
    /// one specifically, to read `named_animations` out of `Res<Assets<Gltf>>` and build the
    /// shared `AnimationGraph`; `WorldAsset` doesn't expose that.
    #[asset(path = "models/rig.glb")]
    pub rig_gltf: Handle<Gltf>,
    /// The main menu's background scene (`ui.rs`'s `main_menu`).
    #[asset(path = "models/MenuBackground.glb#Scene0")]
    pub menu_background: Handle<WorldAsset>,

    #[asset(path = "audio/crunch.wav")]
    pub crunch: Handle<AudioSample>,
    #[asset(path = "audio/explosion.wav")]
    pub explosion: Handle<AudioSample>,

    /// See `scripts/hdri_to_skybox.py`'s doc comment for how this KTX2 cubemap is built.
    #[asset(path = "skyboxes/night_sky_clean_bc6h.ktx2")]
    pub skybox: Handle<Image>,

    #[asset(path = "fonts/serif/IBMPlexSerif-Regular.ttf")]
    pub serif_font: Handle<Font>,

    /// Every icon PNG in Kenney's keyboard/mouse "Input Prompts" pack, keyed by filename minus
    /// extension (`AssetFileStem` — e.g. `"keyboard_a"`, `"mouse_left"`, `"mouse_move"`), matching
    /// the same lookup keys `input_icons.rs`'s `icon_png!` macro used to hardcode as path strings.
    #[asset(
        path = "textures/input_prompts/keyboard_mouse",
        collection(typed, mapped)
    )]
    pub keyboard_mouse_icons: HashMap<AssetFileStem, Handle<Image>>,
    /// Same idea as `keyboard_mouse_icons`, for the Steam Deck button/stick icon pack.
    #[asset(path = "textures/input_prompts/steam_deck", collection(typed, mapped))]
    pub steam_deck_icons: HashMap<AssetFileStem, Handle<Image>>,
}

#[derive(AssetCollection, Resource)]
struct LevelAssets {
    #[asset(key = "level")]
    level: Handle<Gltf>,
    #[asset(key = "skybox")]
    skybox: Handle<Image>,
}
