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

use bevy::feathers::font_styles::InheritableFont;
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
    /// `None` (the consumers degrade to Bevy's built-in defaults: the embedded default font,
    /// no skybox pass, no icon quads, no sample playback). The normal `client/assets` manifest
    /// lists every key, so production behavior is unchanged.
    #[asset(key = "crunch", optional)]
    pub crunch: Option<Handle<AudioSample>>,
    #[asset(key = "explosion", optional)]
    pub explosion: Option<Handle<AudioSample>>,

    /// See `scripts/hdri_to_skybox.py`'s doc comment for how this KTX2 cubemap is built.
    #[asset(key = "skybox", optional)]
    pub skybox: Option<Handle<Image>>,

    /// The client's main UI font — IosevkaSlabQP (a slab-serif face), replacing IBM Plex Serif.
    /// Field name kept as `serif_font` rather than renamed to match: "slab serif" is still a
    /// serif, and every call site (`widgets.rs`, `hud.rs`, `npc_ui_quad.rs`,
    /// `override_default_font` below) already reads `serif_font` for "the UI font," not
    /// specifically IBM Plex — renaming would've been a pure churn edit with no behavior change.
    #[asset(key = "serif_font", optional)]
    pub serif_font: Option<Handle<Font>>,

    /// Noto Sans JP — kept loaded purely so `parley` (Bevy 0.19's text-shaping stack) has a
    /// CJK-capable font actually registered in its font collection to fall back to for glyphs
    /// `serif_font` doesn't cover (confirmed via `bevy_text::font::load_font_assets_into_font_collection`'s
    /// own source: *every* loaded `Font` asset gets registered into the same `parley::fontique`
    /// collection shaping draws from, whether or not any `TextFont` ever names it directly — this
    /// field's own `Handle` never needs to be read anywhere else). Replaces relying on
    /// `system_font_discovery` (a host-machine-dependent CJK font, not a shipped one) for the
    /// `ja-JP` locale added alongside it — see `ui.rs`'s `language_options`.
    #[asset(key = "noto_sans_jp_font", optional)]
    pub noto_sans_jp_font: Option<Handle<Font>>,

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
            serif_font: None,
            noto_sans_jp_font: None,
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

/// Overrides Bevy's own built-in default font (`AssetId::<Font>::default()` — what any
/// `TextFont`/`FontSource` left at its `#[default]` resolves to, e.g. plain `Text` with no font
/// set) with `CommonAssets.serif_font`, so the fallback matches this project's own UI font
/// instead of Bevy's embedded FiraMono. `bevy_text::TextPlugin` (part of `DefaultPlugins`) seeds
/// that same slot once, in its own `build()`, with the embedded font — this just overwrites it
/// afterward with a real asset already in `Assets<Font>`.
///
/// **Does not cover `bevy_feathers` widgets** — despite an earlier version of this doc comment
/// claiming otherwise. Confirmed by reading `bevy_feathers::controls::button`'s source:
/// `FeathersButton` always inserts a real, explicit `font_styles::InheritableFont { font:
/// fonts::REGULAR, .. }` pointing at feathers' own embedded Fira Sans — it never reads
/// `Handle::default()` at all, so patching the default id here has no effect on it whatsoever.
/// See `override_feathers_button_font`, below, for the actual fix for that.
///
/// Not a `Startup` system, despite the name suggesting one — `CommonAssets` doesn't exist until
/// `GameState::AssetLoading`'s `LoadingState` finishes, well after `Startup` runs (same ordering
/// constraint as `npc_ui_quad.rs`'s `setup_npc_ui_quad`, which hit exactly this as a real panic:
/// `Res<CommonAssets>` "resource does not exist"). `OnEnter(GameState::MainMenu)` is the earliest
/// point `Res<CommonAssets>` is guaranteed to exist.
pub fn override_default_font(common_assets: Res<CommonAssets>, mut fonts: ResMut<Assets<Font>>) {
    // `None` = the playtest-assets mode omitted the font keys; keep Bevy's embedded default.
    let Some(serif_font) = &common_assets.serif_font else {
        return;
    };
    let Some(font) = fonts.get(serif_font).cloned() else {
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

/// The actual fix for `bevy_feathers` widgets not picking up `override_default_font`'s patch —
/// see that function's doc comment for why it doesn't reach them. Overwrites
/// `InheritableFont.font` with `CommonAssets.serif_font` the instant one is inserted, on whatever
/// entity it lands on — `FeathersButton` is the only widget this project currently spawns that
/// carries one, but this isn't scoped to buttons specifically, since nothing about
/// `InheritableFont` itself is button-specific.
///
/// A continuous `Update` system, not a one-shot `OnEnter(GameState::MainMenu)` system like
/// `override_default_font` — feathers widgets keep getting spawned well after the main menu
/// (lobby, the pause modal, `selector` popups, ...), and each one inserts its own fresh
/// `InheritableFont` at spawn time that needs the same overwrite.
///
/// `.run_if(resource_exists::<CommonAssets>)` in `main.rs` is load-bearing, not defensive
/// boilerplate — confirmed by a real panic: unlike `override_default_font` (which only ever runs
/// once `CommonAssets` is guaranteed to exist, on `OnEnter(GameState::MainMenu)`), this system
/// runs every frame from `Startup`, well before `GameState::AssetLoading`'s `LoadingState`
/// finishes — `Res<CommonAssets>` panics (a hard parameter-validation failure, not a query that
/// just comes up empty) on every one of those early frames without the guard.
///
/// Re-`insert`s the whole component via `Commands` rather than writing through `&mut
/// InheritableFont` directly — confirmed by testing that the `&mut` version compiles and runs but
/// silently has no visible effect: `bevy_feathers::font_styles::on_changed_font` (the observer
/// that actually propagates `InheritableFont.font` down into a real `TextFont` on `ThemedText`
/// descendants) only reacts to `On<Insert, InheritableFont>`, which a plain mutable-query write
/// never triggers — inserts/hook-fired triggers and ordinary `DerefMut` component writes are
/// different things in Bevy, and only the former re-fires insert observers. A real `.insert()`
/// call — even one that just replaces an already-present component's value — does re-trigger
/// `On<Insert, _>`, which is what actually gets the corrected font propagated.
pub fn override_feathers_button_font(
    mut commands: Commands,
    fonts: Query<(Entity, &InheritableFont), Added<InheritableFont>>,
    common_assets: Res<CommonAssets>,
) {
    for (entity, font) in &fonts {
        // `None` (playtest-assets mode): leave the feathers default font alone.
        let Some(serif_font) = &common_assets.serif_font else {
            return;
        };
        commands.entity(entity).insert(InheritableFont {
            font: serif_font.clone(),
            ..font.clone()
        });
    }
}
