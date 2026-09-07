//! `CommonAssets` is the single manifest for every asset the client needs regardless of which
//! level is loaded — models, sounds, the skybox, UI fonts, and the two input-prompt icon packs.
//! It's loaded once, up front, via `bevy_asset_loader`'s `LoadingState` (see `main.rs`'s
//! `GameState::AssetLoading` — the app doesn't reach `MainMenu` until this collection is fully
//! loaded), so every other module reads a pre-loaded `Handle`/lookup out of `Res<CommonAssets>`
//! instead of calling `asset_server.load(...)` with a hardcoded path at the point of use.
//!
//! Every field below is `#[asset(key = "...")]`, not `#[asset(path = "...")]` — the actual paths
//! live in `assets/collections/common_assets.assets.ron`, a dynamic asset collection file (`main.rs` registers
//! it via `.with_dynamic_assets_file::<StandardDynamicAssetCollection>(...)`, *before*
//! `.load_collection::<CommonAssets>()`). This is a real, separately-tracked loading phase, not
//! just a config file read: `bevy_asset_loader` loads and parses the `.ron` itself as a genuine
//! asset first (`InternalLoadingState::LoadingDynamicAssetCollections`, confirmed directly against
//! `loading_state.rs`), and only *then* resolves each `key` against the now-available table and
//! issues the real loads (`StandardDynamicAsset::File { path } => asset_server.load_untyped(path)`)
//! — the manifest itself never ends up as a live handle anywhere in `CommonAssets`, only the keys'
//! resolved targets do. Every entry here happens to be a plain `File(path: "...")` — the RON format
//! also supports `Image`/`Folder`/`Files`/`StandardMaterial`/`TextureAtlasLayout` variants with
//! their own extra fields, none of which this collection currently needs.
//!
//! `LevelAssets` below is a *separate* collection from `CommonAssets`, loaded through its own
//! `LoadingState` scoped to `GameState::Loading` (see `main.rs`) rather than `AssetLoading` — its
//! manifest isn't known at `main.rs`'s build time the way `common_assets.assets.ron` is, since
//! which level to load is a runtime choice (`config.toml`'s `level`, a console `load_level`
//! command, or eventually a level-select UI). `client/src/loading.rs`'s `load_level` registers the
//! chosen `.ron` file into `DynamicAssetCollections<GameState>` for `GameState::Loading`
//! immediately before transitioning into it, instead of it being fixed via
//! `.with_dynamic_assets_file(...)` up front — see that module for the full flow, including why
//! resolving `LevelAssets.level` doesn't yet mean the level itself is ready to play.
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
use bevy_asset_loader::prelude::*;
use bevy_seedling::sample::AudioSample;

use crate::ui::input_icons::SparrowAtlasManifest;

#[derive(AssetCollection, Resource)]
pub struct CommonAssets {
    /// The `Cube` prop's world-asset scene (`cube_spawner.rs`) — a `Handle<WorldAsset>`, not
    /// `Handle<Gltf>`, since that's what `WorldAssetRoot` (Skein/world-serialization's headless-safe
    /// spawn mechanism) actually wraps; see `shared`/CLAUDE.md's "Server" section for why this
    /// path exists alongside plain GLTF loading at all.
    #[asset(key = "cube_world")]
    pub cube_world: Handle<WorldAsset>,
    /// The player/NPC rig's world-asset scene (`npc_spawner.rs`, `player_character.rs`) — same
    /// `WorldAsset` reasoning as `cube_world`.
    #[asset(key = "rig_world")]
    pub rig_world: Handle<WorldAsset>,
    /// The *same* `rig.glb`, loaded again as a plain `Handle<Gltf>` — `animation.rs` needs this
    /// one specifically, to read `named_animations` out of `Res<Assets<Gltf>>` and build the
    /// shared `AnimationGraph`; `WorldAsset` doesn't expose that.
    #[asset(key = "rig_gltf")]
    pub rig_gltf: Handle<Gltf>,
    /// The main menu's background scene (`ui.rs`'s `main_menu`).
    #[asset(key = "menu_background")]
    pub menu_background: Handle<WorldAsset>,

    /// The lobby background scene.
    #[asset(key = "menu_background")]
    pub lobby_background: Handle<WorldAsset>,

    #[asset(key = "crunch")]
    pub crunch: Handle<AudioSample>,
    #[asset(key = "explosion")]
    pub explosion: Handle<AudioSample>,

    /// See `scripts/hdri_to_skybox.py`'s doc comment for how this KTX2 cubemap is built.
    #[asset(key = "skybox")]
    pub skybox: Handle<Image>,

    #[asset(key = "serif_font")]
    pub serif_font: Handle<Font>,

    /// Kenney's own pre-built texture atlas for the keyboard/mouse "Input Prompts" pack — a single
    /// sheet PNG plus a Sparrow/Starling-format XML manifest (`input_icons::SparrowAtlasManifest`
    /// parses it) naming each icon's pixel rect within the sheet. Loading these two instead of the
    /// pack's ~85 individual per-icon PNGs is deliberate — see `input_icons.rs`'s module doc
    /// comment for why building our own atlas at runtime from the individual files was dropped in
    /// favor of this.
    #[asset(key = "keyboard_mouse_atlas_image")]
    pub keyboard_mouse_atlas_image: Handle<Image>,
    #[asset(key = "keyboard_mouse_atlas_manifest")]
    pub keyboard_mouse_atlas_manifest: Handle<SparrowAtlasManifest>,
    /// Same idea as `keyboard_mouse_atlas_image`/`_manifest`, for the Steam Deck button/stick pack.
    #[asset(key = "steam_deck_atlas_image")]
    pub steam_deck_atlas_image: Handle<Image>,
    #[asset(key = "steam_deck_atlas_manifest")]
    pub steam_deck_atlas_manifest: Handle<SparrowAtlasManifest>,
}

#[derive(AssetCollection, Resource)]
pub struct PreloadCollection {
    #[asset(key = "armature_rigs", collection(typed, mapped))]
    pub armature_rigs: HashMap<AssetFileStem, Handle<Gltf>>,

    #[asset(key = "world_rigs", collection(typed, mapped))]
    pub world_rigs: HashMap<AssetFileStem, Handle<WorldAsset>>,

    #[asset(key = "skyboxes", collection(typed, mapped))]
    pub skyboxes: HashMap<AssetFileStem, Handle<Image>>,

    #[asset(key = "audio", collection(typed, mapped))]
    pub audio: HashMap<AssetFileStem, Handle<AudioSample>>,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct PreloadBeacon {
    collections: Vec<String>, // e.g. "collections/levels/dungeon.assets.ron"
    load_radius: f32,
    unload_radius: f32,
}

/// Overrides Bevy's own built-in default font (`AssetId::<Font>::default()` — what any
/// `TextFont`/`FontSource` left at its `#[default]` resolves to, e.g. `bevy_feathers` widgets or
/// plain `Text` with no font set) with `CommonAssets.serif_font`, so the fallback matches this
/// project's own UI font instead of Bevy's embedded FiraMono. `bevy_text::TextPlugin` (part of
/// `DefaultPlugins`) seeds that same slot once, in its own `build()`, with the embedded font —
/// this just overwrites it afterward with a real asset already in `Assets<Font>`.
///
/// Not a `Startup` system, despite the name suggesting one — `CommonAssets` doesn't exist until
/// `GameState::AssetLoading`'s `LoadingState` finishes, well after `Startup` runs (same ordering
/// constraint as `npc_ui_quad.rs`'s `setup_npc_ui_quad`, which hit exactly this as a real panic:
/// `Res<CommonAssets>` "resource does not exist"). `OnEnter(GameState::MainMenu)` is the earliest
/// point `Res<CommonAssets>` is guaranteed to exist.
pub fn override_default_font(common_assets: Res<CommonAssets>, mut fonts: ResMut<Assets<Font>>) {
    let Some(font) = fonts.get(&common_assets.serif_font).cloned() else {
        // Shouldn't happen — every handle in `CommonAssets` is guaranteed fully loaded by the
        // time the collection resource itself exists — but fail soft rather than panic/unwrap if
        // that guarantee is ever violated.
        warn!("override_default_font: CommonAssets.serif_font isn't loaded yet");
        return;
    };
    // `AssetId::<Font>::default()` is always the `Uuid` variant (see `Handle<A>::default()`),
    // and `Assets::insert`'s `Err` case only ever comes from the `Index` variant — this can't
    // actually fail, so there's nothing meaningful to do with the `Result`.
    let _ = fonts.insert(AssetId::<Font>::default(), font);
}
