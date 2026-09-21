use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, AssetPath};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::{Deserialize, Serialize};

/*
TODO: Build this validator:

It would make sense to add in a dev tool that would validate level manifest + .glb files for this --
a kind of utility that would validate a .level.ron file, so for this kind of file:

#[derive(Asset, TypePath, Deserialize)]
pub struct Level {
  pub name: String,
  // .glb file containing player spawner to use when loading this level.
  pub entry: String,
  // Collections of assets to preload before spawning in.
  pub preload_collections: Vec<String>,
}

It would read in the entry .glb file, actually load it in -- and then in the ECS check that it
contains an entity with a PlayerSpawner component that is within load_range of PreloadBeacons
covering all collections in preload_collections of that level manifest.

TODO: Perhaps implement this as a special "dev" mode that outputs an overlay with all the level authoring validation errors.
*/

/// A level manifest — e.g. `assets/levels/start.level.ron` — loaded via `LevelAssetLoader` below,
/// not `bevy_common_assets`' generic `RonAssetPlugin` any more: `model`/`skybox` are real,
/// dependency-tracked handles rather than bare paths, and `Handle<T>` has no meaningful
/// `Deserialize` impl of its own (there's no path to resolve *against* until `LoadContext` is
/// available, which plain serde deserialization never gets) — see `LevelRon` below for the
/// on-disk shape these are resolved *from*. `Level` itself only loads once `model`/`skybox` have
/// both finished loading too (`AssetEvent::LoadedWithDependencies`, standard Bevy asset-dependency
/// behavior), since `LevelAssetLoader::load` obtains each of these via `LoadContext::load` rather
/// than leaving them as strings to be resolved by hand later. Mirrors `assets::character::Character`
/// exactly — see that module's doc comments for the fuller reasoning.
#[derive(Asset, TypePath, Debug, Clone, Deserialize, Serialize)]
pub struct Level {
    /// An `.ftl` message key (see `assets/locales/`), not display text.
    pub name: String,
    pub model: AssetPath<'static>,
    pub skybox: AssetPath<'static>,
}

/// The plain RON shape `.level.ron` files are actually authored in — just paths (which may use
/// Bevy's own `path#Label` sub-asset syntax, e.g. picking one named scene out of a `.glb` that has
/// several) — `LevelAssetLoader` below resolves each into a real handle. Deliberately a separate,
/// private type from `Level`, for the same reason `assets::character::CharacterRon` is separate
/// from `Character`.
#[derive(Deserialize)]
struct LevelRon {
    name: String,
    model: String,
    skybox: String,
}

#[derive(Default, TypePath)]
pub struct LevelAssetLoader;

impl AssetLoader for LevelAssetLoader {
    type Asset = Level;
    type Settings = ();
    type Error = LevelAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: LevelRon = ron::de::from_bytes(&bytes)?;
        // Each `load_context.load(...)` call is what actually turns a plain path into a real,
        // dependency-tracked `Handle<T>` — this is the mechanism a generic `RonAssetPlugin`
        // can't give you, since it never gets `LoadContext` access at all. Either path may use
        // `path#Label` sub-asset syntax (e.g. `"level.glb#Scene0"`) exactly like a normal
        // `asset_server.load(...)` call would — `AssetPath`'s parsing doesn't care which called it.
        Ok(Level {
            name: raw.name,
            model: raw.model.into(),
            skybox: raw.skybox.into(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["level.ron"]
    }
}

/// Same hand-rolled `Display`/`Error`/`From` pattern as `assets::character::CharacterAssetLoaderError`
/// (and `SparrowAtlasError` in `ui/input_icons.rs`) — no `thiserror` dependency in this codebase.
#[derive(Debug)]
pub enum LevelAssetLoaderError {
    Io(std::io::Error),
    Ron(ron::error::SpannedError),
}

impl std::fmt::Display for LevelAssetLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read level asset: {err}"),
            Self::Ron(err) => write!(f, "failed to parse level asset: {err}"),
        }
    }
}

impl std::error::Error for LevelAssetLoaderError {}

impl From<std::io::Error> for LevelAssetLoaderError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<ron::error::SpannedError> for LevelAssetLoaderError {
    fn from(err: ron::error::SpannedError) -> Self {
        Self::Ron(err)
    }
}

#[derive(AssetCollection, Resource)]
pub struct LevelMetadataAssets {
    #[asset(path = "levels", collection(typed))]
    pub levels: Vec<Handle<Level>>,
}

// This component is supposed to be attached to "logical" entities on the server.
// Then the client would load in the visuals or something else referenced by this component.
#[derive(Component, Deserialize, Serialize, Reflect)]
#[reflect(Component)]
pub struct ClientWorldAsset {
    pub asset_path: String,
}

// TODO: Implement this -- so it works in a similar way to ClientWorldAsset.
#[derive(Component, Deserialize, Serialize, Reflect)]
#[reflect(Component)]
pub struct Skybox {
    pub asset_path: String,
}

// This is a marker component to mark entities that must be replicated from the server to the client
// in the Skein authored .glb files on the server with the server logic.
#[derive(Component, Deserialize, Serialize, Reflect)]
#[reflect(Component)]
pub struct ClientReplicate;
