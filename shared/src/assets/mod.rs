use bevy::{asset::AssetPath, prelude::*, tasks::futures_lite};

pub mod level;

pub struct SharedAssetsPlugin;

impl Plugin for SharedAssetsPlugin {
    fn build(&self, app: &mut App) {
        app
            // `assets::level::Level` (e.g. `assets/levels/start.level.ron`) now resolves
            // `model`/`skybox` into real handles via `LoadContext`, which `RonAssetPlugin`'s plain
            // `serde` deserialization can't give it — see `LevelAssetLoader`'s doc comment.
            .init_asset::<level::Level>()
            .register_asset_loader(level::LevelAssetLoader);
    }
}

pub fn asset_exists<'a>(asset_server: &AssetServer, asset_path: &AssetPath<'a>) -> bool {
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        return false; // no such asset source (e.g. bad source prefix)
    };
    futures_lite::future::block_on(source.reader().read(asset_path.path())).is_ok()
}
