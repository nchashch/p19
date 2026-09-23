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
    #[serde(default)]
    mcp: bool,
}

/// `--mcp` (CLI) or `mcp = true` (config.toml) — runs the client as a **headless agent host**:
/// no window at all, all cameras rendered into an offscreen texture, and (with the `dev-tools`
/// cargo feature) the BRP + MCP tool API up on localhost. Must be decided *pre-sync* — it
/// changes which plugins the app is built with (no winit; `ScheduleRunnerPlugin` drives frames),
/// the same reason `is_vr_enabled_presync` is decided up front. See `docs/adr/0009`.
pub fn is_mcp_mode_presync() -> bool {
    // CLI first (explicit per-invocation intent), then the config file.
    if std::env::args().any(|arg| arg == "--mcp") {
        return true;
    }
    is_config_flag_enabled("mcp")
}

/// `--no-common-assets` (CLI-only): skips the whole `CommonAssets` collection load (no manifest
/// read at all) and boots with `CommonAssets::placeholder()` instead, transitioning straight to
/// `GameState::MainMenu` — the "barest boot" mode for playtest asset roots that carry no engine
/// furniture (no textures, fonts, sounds, skyboxes). Content then arrives only via the
/// `ClientWorldAsset` path, which loads by path rather than through the manifest. Like `--mcp`,
/// decided *pre-sync* (it changes whether the loading state is even registered); CLI-only since
/// it's a per-playtest-run choice, not a config-file property of an asset root.
pub fn is_no_common_assets_presync() -> bool {
    std::env::args().any(|arg| arg == "--no-common-assets")
}

/// Whether the game runs in desktop-VR mode (`vr = true` in `assets/config.toml`) — decided
/// *pre-sync* (which plugin group even gets added is a build-time choice); see `main.rs`.
pub fn is_vr_enabled_presync() -> bool {
    is_config_flag_enabled("vr")
}

/// Shared plumbing for the pre-sync boolean flags: the config file's `assets/config.toml`
/// resolved via the same base-path chain `bevy_asset` uses (`BEVY_ASSET_ROOT` →
/// `CARGO_MANIFEST_DIR` → the executable's directory), read with plain `std::fs` because this
/// runs before any asset source exists.
fn is_config_flag_enabled(flag: &str) -> bool {
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
    match toml::from_str::<ClientConfig>(&contents) {
        #[allow(clippy::bool_comparison)]
        Ok(config) => match flag {
            "vr" => config.vr == true,
            "mcp" => config.mcp == true,
            _ => false,
        },
        Err(_) => false,
    }
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
