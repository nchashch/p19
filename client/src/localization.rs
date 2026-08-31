use bevy::asset::{LoadState, LoadedFolder};
use bevy::prelude::*;
use bevy_fluent::prelude::*;
use fluent_content::Content;
use unic_langid::langid;

/// Loads `.ftl` locale assets under `assets/locales/` and exposes localized strings to the rest of
/// the client via the `LocalizedText` marker component, reactively (`sync_localized_text`) rather
/// than as a one-shot lookup at spawn time — the main menu's scene spawns on `OnEnter(MainMenu)`,
/// which runs on the very first frame, before the locale folder has had a chance to finish loading,
/// and a future runtime locale switch needs the same "re-apply everything" pass regardless.
pub struct LocalizationPlugin;

impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Locale::new(langid!("en-US")).with_default(langid!("en-US")));
        app.add_plugins(FluentPlugin);
        app.add_systems(Startup, load_locales);
        app.add_systems(Update, (build_localization, sync_localized_text));
    }
}

#[derive(Resource)]
struct LocaleFolder(Handle<LoadedFolder>);

fn load_locales(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(LocaleFolder(asset_server.load_folder("locales")));
}

fn build_localization(
    mut commands: Commands,
    folder: Option<Res<LocaleFolder>>,
    asset_server: Res<AssetServer>,
    localization_builder: LocalizationBuilder,
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
    commands.insert_resource(localization_builder.build(&folder.0));
    commands.remove_resource::<LocaleFolder>();
}

/// Marks a `Text` entity to have its content set from `key`'s message in the current `Localization`
/// — both once localization first becomes available and again on every later change (e.g. a locale
/// switch). `key` doubles as the placeholder shown for the one or two frames before localization is
/// ready, since asset loading is never instant even for these tiny `.ftl` files.
#[derive(Component, Clone, Copy, Default)]
pub struct LocalizedText(pub &'static str);

fn sync_localized_text(
    localization: Option<Res<Localization>>,
    mut texts: Query<(&LocalizedText, &mut Text)>,
    added: Query<(), Added<LocalizedText>>,
) {
    let Some(localization) = localization else {
        return;
    };
    if !localization.is_changed() && added.is_empty() {
        return;
    }
    for (localized, mut text) in &mut texts {
        if let Some(content) = localization.content(localized.0) {
            text.0 = content;
        }
    }
}
