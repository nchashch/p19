//! Compile-time feature detection for optional embedded assets.
//!
//! `/client/assets` is gitignored (see `.gitignore`), so a fresh CI checkout has no font
//! files — but `include_bytes!` is a hard compile error on a missing file. When the TUI
//! panel's embedded font is present, this emits `--cfg has_tui_font`; `ui::tui_panel`
//! gates its `include_bytes!` on that and skips the panel entirely otherwise (matching
//! the `CommonAssets` optional-asset degradation pattern: no panic, no missing-file
//! build break, the rest of the app unaffected).

fn main() {
    let font = format!(
        "{}/assets/fonts/mono/IBMPlexMono-Regular.ttf",
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")
    );
    println!("cargo:rerun-if-changed={font}");
    if std::path::Path::new(&font).exists() {
        println!("cargo:rustc-cfg=has_tui_font");
    }
}
