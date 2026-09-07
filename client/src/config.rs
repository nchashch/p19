use std::path::PathBuf;

use bevy::asset::AssetPath;
use bevy::prelude::*;
use futures_lite::io::AsyncReadExt;
use serde::Deserialize;

use crate::lifecycle::networking::ServerAddress;

/// The subset of `assets/config.toml` this binary cares about. `vr` defaults to `false` via
/// `#[serde(default)]` rather than being required like `server_ip`, so a `config.toml` predating
/// this field (or one that just doesn't care to set it) doesn't fail to parse at all — unlike
/// `server_ip`, where a mistake means the value simply doesn't get applied (see
/// `load_client_config`'s doc comment).
#[derive(Deserialize)]
struct ClientConfig {
    server_ip: String,
    #[serde(default)]
    vr: bool,
}

pub fn is_vr_enabled_presync() -> bool {
    let base_path = if let Ok(root) = std::env::var("BEVY_ASSET_ROOT") {
        PathBuf::from(root)
    } else if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(manifest_dir)
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(ToOwned::to_owned))
            .unwrap_or_default()
    };
    let Ok(contents) = std::fs::read_to_string(base_path.join("assets/config.toml")) else {
        return false;
    };
    toml::from_str::<ClientConfig>(&contents)
        .map(|config| config.vr)
        .unwrap_or(false)
}

/// Overwrites `ServerAddress`'s hardcoded fallback with `assets/config.toml`'s `server_ip`, if the
/// file exists and parses — read the same way `loading.rs`'s `load_level` checks a level id
/// exists (`AssetServer`'s default source reader, blocked on), rather than `std::fs` — a
/// `CARGO_MANIFEST_DIR`-relative path would break once this ships as a packaged build with no
/// source tree next to it, whereas the asset source resolves relative to whatever
/// `AssetPlugin.file_path` the running binary was actually configured with. A missing file is
/// fine (this repo's own `assets/config.toml` aside, nothing requires one to exist) and just
/// keeps the hardcoded fallback; a file that exists but fails to parse is `warn!`ed instead, since
/// that's more likely a real mistake worth noticing.
pub(crate) fn load_client_config(
    asset_server: Res<AssetServer>,
    mut server_address: ResMut<ServerAddress>,
) {
    let asset_path = AssetPath::parse("config.toml");
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        return;
    };
    let Ok(mut reader) = futures_lite::future::block_on(source.reader().read(asset_path.path()))
    else {
        return;
    };
    let mut contents = String::new();
    if let Err(err) = futures_lite::future::block_on(reader.read_to_string(&mut contents)) {
        warn!("load_client_config: failed to read config.toml ({err})");
        return;
    }
    match toml::from_str::<ClientConfig>(&contents) {
        Ok(config) => {
            server_address.0 = config.server_ip;
        }
        Err(err) => warn!("load_client_config: failed to parse config.toml ({err})"),
    }
}
