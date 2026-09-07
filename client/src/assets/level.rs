use bevy::prelude::*;
use serde::Deserialize;

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

/// A level manifest — e.g. `assets/levels/start.level.ron` — loaded directly as an asset via
/// `bevy_common_assets`' `RonAssetPlugin<Level>` (registered in `main.rs`), independent of
/// `bevy_asset_loader`'s own dynamic-asset manifests (see `assets::collections`).
///
/// Both fields are plain `String`s, not `AssetPath`/`Handle` themselves — resolving them is up to
/// whoever loads a `Level`:
/// - `name` is an `.ftl` message key (see `assets/locales/`), not display text.
/// - `collection` is the path to a `LevelAssets`-shaped dynamic-asset manifest (e.g.
///   `"collections/Level.assets.ron"`), meant to be registered into
///   `DynamicAssetCollections<GameState>` the same way `loading.rs::load_level` already does for
///   a manifest path handed to it directly.
#[derive(Asset, TypePath, Deserialize)]
pub struct Level {
    pub name: String,
    // .glb file containing player spawner to use when loading this level.
    pub entry: String,
}
