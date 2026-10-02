use bevy::asset::AssetLoader;
use bevy::asset::io::Reader;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;
use serde::{Deserialize, Serialize};

use crate::assets::controller::Controller;

// Component to mark the entity that is supposed to be populated with `Controller` components and
// data components for a character.
#[derive(Component)]
pub struct CharacterAssetRoot(Handle<Character>);

/// The resolved, ready-to-spawn-from form: every field a real, dependency-tracked handle rather
/// than a bare path — see `CharacterRon` below for the on-disk shape these are resolved *from*.
/// `Character` itself only loads once `rig`/`model`/`controller`/`stats` have all finished
/// loading too (`AssetEvent::LoadedWithDependencies`, standard Bevy asset-dependency behavior),
/// since `CharacterAssetLoader::load` obtains each of these via `LoadContext::load` rather than
/// leaving them as strings to be resolved by hand later.
#[derive(Asset, TypePath)]
pub struct Character {
    pub rig: Handle<Gltf>,
    pub model: Handle<WorldAsset>,
    pub controller: Handle<Controller>,
}

/// The plain RON shape `.character.ron` files are actually authored in — just paths (which may
/// use Bevy's own `path#Label` sub-asset syntax, e.g. picking one named scene out of a `.glb`
/// that has several) — `CharacterAssetLoader` below resolves each into a real handle. This is
/// deliberately a separate, private type from `Character`: `Handle<T>` has no meaningful
/// `Deserialize` impl of its own (there's no path to resolve *against* until `LoadContext` is
/// available, which plain serde deserialization never gets), so the loader has to go through this
/// intermediate string-only form first.
#[derive(Deserialize)]
struct CharacterRon {
    rig: String,
    model: String,
    controller: String,
}

#[derive(Default, TypePath)]
pub struct CharacterAssetLoader;

impl AssetLoader for CharacterAssetLoader {
    type Asset = Character;
    type Settings = ();
    type Error = CharacterAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: CharacterRon = ron::de::from_bytes(&bytes)?;
        // Each `load_context.load(...)` call is what actually turns a plain path into a real,
        // dependency-tracked `Handle<T>` — this is the mechanism a generic `RonAssetPlugin`
        // can't give you, since it never gets `LoadContext` access at all. Any of these paths may
        // use `path#Label` sub-asset syntax (e.g. `"rig.glb#Scene0"`) exactly like a normal
        // `asset_server.load(...)` call would — `AssetPath`'s parsing doesn't care which called it.
        Ok(Character {
            rig: load_context.load(raw.rig),
            model: load_context.load(raw.model),
            controller: load_context.load(raw.controller),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["character.ron"]
    }
}

/// Same hand-rolled `Display`/`Error`/`From` pattern as `SparrowAtlasError` in
/// `ui/input_icons.rs` — no `thiserror` dependency in this codebase.
#[derive(Debug)]
pub enum CharacterAssetLoaderError {
    Io(std::io::Error),
    Ron(ron::error::SpannedError),
}

impl std::fmt::Display for CharacterAssetLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read character asset: {err}"),
            Self::Ron(err) => write!(f, "failed to parse character asset: {err}"),
        }
    }
}

impl std::error::Error for CharacterAssetLoaderError {}

impl From<std::io::Error> for CharacterAssetLoaderError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<ron::error::SpannedError> for CharacterAssetLoaderError {
    fn from(err: ron::error::SpannedError) -> Self {
        Self::Ron(err)
    }
}
