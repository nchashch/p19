//! The credits screen (`credits.html`): the main menu's Credits button opens a modal listing the
//! third-party assets the game uses — the same entries as `assets/CREDITS.md`, from [`CREDITS`].
//! A main-menu screen (`ui/menu_screen.rs`): Back, Escape or gamepad East close it.
//!
//! Signals: `main-menu.credits`.

use crate::ui::menu_screen::{OpenMenuScreens, open_menu_screen};
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy_markup::prelude::*;
use serde_json::json;

/// One third-party asset: an id (element ids, the Fluent key `credits-use-<id>`), its title and
/// author as published, and its source page. Keep in sync with `assets/CREDITS.md`.
struct Credit {
    id: &'static str,
    title: &'static str,
    author: &'static str,
    url: &'static str,
}

const CREDITS: [Credit; 3] = [
    Credit {
        id: "input-prompts",
        title: "Input Prompts",
        author: "Kenney",
        url: "kenney.nl/assets/input-prompts",
    },
    Credit {
        id: "night-sky",
        title: "Night Sky HDRI 012",
        author: "ambientCG",
        url: "ambientcg.com/view?id=NightSkyHDRI012",
    },
    Credit {
        id: "rubber-tiles",
        title: "Rubber Tiles",
        author: "Amal Kumar, Poly Haven",
        url: "polyhaven.com/a/rubber_tiles",
    },
];

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/credits.html");
        app.on_html_click("main-menu.credits", open_credits);
    }
}

fn open_credits(
    _: In<ElementSignal>,
    open: OpenMenuScreens,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let credits: Vec<serde_json::Value> = CREDITS
        .iter()
        .map(|credit| {
            json!({ "id": credit.id, "title": credit.title, "author": credit.author, "url": credit.url })
        })
        .collect();
    let _ = open_menu_screen(
        &mut commands,
        &asset_server,
        &open,
        "credits.html",
        TemplateContext::new().with("credits", &credits),
        "credits",
    );
}
