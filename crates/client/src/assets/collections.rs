//! `CommonAssets` is the single manifest for every asset the client needs regardless of which
//! level is loaded — models, sounds, the skybox and the two input-prompt icon packs (no fonts:
//! the UI uses the system's, see `ui::markup::register_ui_fonts`).
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
//! command, or eventually a level-select UI). `crates/client/src/loading.rs`'s `load_level` registers the
//! chosen `.ron` file into `DynamicAssetCollections<GameState>` for `GameState::Loading`
//! immediately before transitioning into it, instead of it being fixed via
//! `.with_dynamic_assets_file(...)` up front — see that module for the full flow, including why
//! resolving `LevelAssets.level` doesn't yet mean the level itself is ready to play.
//!
//! Deliberately *not* included here: `locales/`. The language bundles are bevy_markup's own
//! asset: `sync_active_locale` (`ui::markup`) loads only the selected language's
//! `locales/<id>/main.ftl.ron` and swaps it at runtime, so a `CommonAssets` folder entry (which
//! always loads everything) would be the wrong shape for it.

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
    /// spawn mechanism) actually wraps; see `shared`/AGENTS.md's "Server" section for why this
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
    #[asset(key = "lobby_background")]
    pub lobby_background: Handle<WorldAsset>,

    /// Furniture fields are `Option` so an asset manifest can omit them entirely — that's the
    /// `--no-common-assets`/playtest-assets mode: the playtest's own
    /// `collections/common_assets.assets.ron` lists only the world keys, and these come back
    /// `None` (the consumers degrade to Bevy's built-in defaults: no skybox pass, no icon
    /// quads, no sample playback). The normal `assets/client` manifest
    /// lists every key, so production behavior is unchanged.
    #[asset(key = "crunch", optional)]
    pub crunch: Option<Handle<AudioSample>>,
    #[asset(key = "explosion", optional)]
    pub explosion: Option<Handle<AudioSample>>,

    /// See `scripts/hdri_to_skybox.py`'s doc comment for how this KTX2 cubemap is built.
    #[asset(key = "skybox", optional)]
    pub skybox: Option<Handle<Image>>,

    /// Kenney's own pre-built texture atlas for the keyboard/mouse "Input Prompts" pack — a single
    /// sheet PNG plus a Sparrow/Starling-format XML manifest (`input_icons::SparrowAtlasManifest`
    /// parses it) naming each icon's pixel rect within the sheet. Loading these two instead of the
    /// pack's ~85 individual per-icon PNGs is deliberate — see `input_icons.rs`'s module doc
    /// comment for why building our own atlas at runtime from the individual files was dropped in
    /// favor of this.
    #[asset(key = "keyboard_mouse_atlas_image", optional)]
    pub keyboard_mouse_atlas_image: Option<Handle<Image>>,
    #[asset(key = "keyboard_mouse_atlas_manifest", optional)]
    pub keyboard_mouse_atlas_manifest: Option<Handle<SparrowAtlasManifest>>,
    /// Same idea as `keyboard_mouse_atlas_image`/`_manifest`, for the Steam Deck button/stick pack.
    #[asset(key = "steam_deck_atlas_image", optional)]
    pub steam_deck_atlas_image: Option<Handle<Image>>,
    #[asset(key = "steam_deck_atlas_manifest", optional)]
    pub steam_deck_atlas_manifest: Option<Handle<SparrowAtlasManifest>>,
}

impl CommonAssets {
    /// The `--no-common-assets` barest-boot mode's stand-in for a real collection load (see
    /// `main.rs`): no manifest is read at all, so the world keys get dangling handles
    /// (`Handle::default()` — the "default asset" id; the background/menu worlds simply never
    /// resolve, which is the point) and every furniture field is `None`, exactly as if a
    /// manifest had omitted them — every consumer's degradation path is the same one the
    /// `optional` fields above already implement.
    pub fn placeholder() -> Self {
        Self {
            cube_world: Handle::default(),
            rig_world: Handle::default(),
            rig_gltf: Handle::default(),
            menu_background: Handle::default(),
            lobby_background: Handle::default(),
            crunch: None,
            explosion: None,
            skybox: None,
            keyboard_mouse_atlas_image: None,
            keyboard_mouse_atlas_manifest: None,
            steam_deck_atlas_image: None,
            steam_deck_atlas_manifest: None,
        }
    }
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

// Add an `avian3d` collider to specify the trigger area.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct UnloadTrigger {
    collections: Vec<String>, // e.g. "collections/levels/dungeon.assets.ron"
}

// Add an `avian3d` collider to specify the trigger area.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct LoadTrigger {
    collections: Vec<String>, // e.g. "collections/levels/dungeon.assets.ron"
}

// Add an `avian3d` collider to specify the trigger area.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct DespawnTrigger {
    ids: Vec<String>,
}

// Add an `avian3d` collider to specify the trigger area.
#[derive(Component, Reflect)]
#[reflect(Component)]
struct SpawnTrigger {
    ids: Vec<String>,
}
