//! Compile-time feature detection for optional embedded assets.
//!
//! `/assets/client` is gitignored (see `.gitignore`), so a fresh CI checkout has no font
//! files — but `include_bytes!` is a hard compile error on a missing file. When the TUI
//! panel's embedded font is present, this emits `--cfg has_tui_font`; `ui::tui_panel`
//! gates its `include_bytes!` on that and skips the panel entirely otherwise (matching
//! the `CommonAssets` optional-asset degradation pattern: no panic, no missing-file
//! build break, the rest of the app unaffected).

const FONT_PATH: &str = "../../assets/client/fonts/IosevkaSlabMono/IosevkaSlabMono-Regular.ttf";

fn main() {
    // `CARGO_MANIFEST_DIR` is `<workspace>/crates/client`; assets live in `<workspace>/assets/client`.
    let font = format!(
        "{}/{FONT_PATH}",
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")
    );
    println!("cargo:rustc-check-cfg=cfg(has_tui_font)");
    println!("cargo:rerun-if-changed={font}");
    if std::path::Path::new(&font).exists() {
        println!("cargo:rustc-cfg=has_tui_font");
    }
}
