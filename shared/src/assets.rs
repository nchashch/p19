use bevy::{asset::AssetPath, prelude::*, tasks::futures_lite};

pub fn asset_exists(asset_server: &AssetServer, id: &str) -> bool {
    let asset_path = AssetPath::parse(id);
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        return false; // no such asset source (e.g. bad source prefix)
    };
    futures_lite::future::block_on(source.reader().read(asset_path.path())).is_ok()
}
