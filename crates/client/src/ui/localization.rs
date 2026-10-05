use bevy::asset::{LoadState, LoadedFolder};
use bevy::prelude::*;
use bevy_fluent::prelude::*;
use fluent::FluentArgs;
use fluent_content::{Content, Request};
use unic_langid::langid;

/// The language selection (`Locale`, written by the main menu's language picker) and the
/// `Localization` the dev console formats its output with (`localized`). UI text doesn't go
/// through here: `HtmlUi`s resolve `data-l10n-id` against `ActiveLocale` (`ui::markup`), which
/// follows `Locale`.
pub struct LocalizationPlugin;

impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Locale::new(langid!("en-US")).with_default(langid!("en-US")));
        if !app.is_plugin_added::<FluentPlugin>() {
            app.add_plugins(FluentPlugin);
        }
        app.add_systems(Startup, load_locales);
        app.add_systems(Update, build_localization);
    }
}

#[derive(Resource)]
struct LocaleFolder(Handle<LoadedFolder>);

fn load_locales(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(LocaleFolder(asset_server.load_folder("locales")));
}

/// Rebuilds `Localization` once the locale folder first finishes loading, and again every time
/// `Locale.requested` changes afterward (the main menu's language picker) — this used to remove
/// `LocaleFolder` after the first build, a one-shot design that left a runtime locale switch
/// mutating `Locale` with nothing ever reading it again. `built` (not `locale.is_changed()` alone)
/// is what gates the *first* build: this system runs every frame from `Startup` on, so by the time
/// the folder actually finishes loading, `Locale`'s own initial-insert change tick has already been
/// "seen" by this system's own prior (early-returning) runs and no longer reads as changed.
fn build_localization(
    mut commands: Commands,
    folder: Option<Res<LocaleFolder>>,
    asset_server: Res<AssetServer>,
    locale: Res<Locale>,
    localization_builder: LocalizationBuilder,
    mut built: Local<bool>,
) {
    let Some(folder) = folder else {
        return;
    };
    if !matches!(
        asset_server.get_load_state(&folder.0),
        Some(LoadState::Loaded)
    ) {
        return;
    }
    if *built && !locale.is_changed() {
        return;
    }
    commands.insert_resource(localization_builder.build(&folder.0));
    *built = true;
}

/// Looks up `key`'s message in `localization`, formatted with `args` — falling back to the raw key
/// itself (rather than panicking or showing nothing) if a key is ever missing from the `.ftl` files.
/// Used by the dev console's command output (`dev::console`).
pub fn localized(localization: &Localization, key: &'static str, args: &FluentArgs) -> String {
    localization
        .content(Request::new(key).args(args))
        .unwrap_or_else(|| key.to_string())
}
