//! Filesystem locations shared by both binaries: the workspace root and each side's asset
//! directory.
//!
//! Layout (development checkout):
//!
//! ```text
//! <workspace>/crates/{client,server,shared}/   source
//! <workspace>/assets/client/                  client asset root
//! <workspace>/assets/server/                  server asset root
//! <workspace>/assets/src/                     raw sources (.blend, packs); nothing loads from it
//! ```
//!
//! Assets are deliberately not inside the crate directories, so Bevy's default
//! `CARGO_MANIFEST_DIR`-relative `assets/` lookup does not apply. Every binary sets its
//! `AssetPlugin::file_path` from [`asset_dir`], and anything that reads asset-root files with
//! plain `std::fs` (config, network identity, TLS pins) must use the same function.

use std::path::{Path, PathBuf};

/// Which binary's asset directory to resolve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetSide {
    Client,
    Server,
}

impl AssetSide {
    fn dir_name(self) -> &'static str {
        match self {
            AssetSide::Client => "client",
            AssetSide::Server => "server",
        }
    }
}

/// The workspace root of the checkout this binary was built from (compile-time path).
///
/// Only meaningful on the build machine — use it for development-only locations (agent
/// screenshot staging, the dev asset fallback), never for anything a deployed build needs.
pub fn workspace_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` of this crate is `<workspace>/crates/shared`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The asset root for one side, resolved in this order:
///
/// 1. `BEVY_ASSET_ROOT` set → `$BEVY_ASSET_ROOT/assets` (Bevy's own convention; used by the
///    isolated playtest asset sets under `docs/agents/playtests/playtest_assets/` and as a
///    manual override).
/// 2. `<workspace>/assets/<side>` if it exists → the development checkout this binary was
///    built from (wins over 3 so stray `target/*/assets/` directories can't shadow it).
/// 3. `assets/` next to the executable → the deployed layout (e.g. the Steam Deck staging
///    directory `target/steamdeck/release/`, copied to a machine without the checkout).
pub fn asset_dir(side: AssetSide) -> PathBuf {
    if let Some(root) = std::env::var_os("BEVY_ASSET_ROOT") {
        return PathBuf::from(root).join("assets");
    }
    let dev = workspace_root().join("assets").join(side.dir_name());
    if dev.is_dir() {
        return dev;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("assets")))
        .unwrap_or(dev)
}
