//! The credits screen (`credits.html`): the main menu's Credits button opens a modal listing the
//! third-party assets the game uses — the same entries as `assets/CREDITS.md`, from [`CREDITS`].
//! Back (button), Escape or gamepad East close it and return focus to the Credits button.
//!
//! Signals: `main-menu.credits`, `credits.back`.

use crate::controls::actions::UiCancel;
use crate::ui::markup::template;
use bevy::asset::embedded_asset;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_markup::prelude::*;
use p19_shared::game_state::GameState;
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
        app.add_input_context::<CreditsControls>()
            .on_html_click("main-menu.credits", open_credits)
            .on_html_click(
                "credits.back",
                |_: In<ElementSignal>, close: CloseCredits| {
                    close.close();
                },
            )
            .add_observer(|_: On<Start<UiCancel>>, close: CloseCredits| close.close());
    }
}

/// The credits root and its input context (both despawned on close).
#[derive(Component)]
struct Credits;

/// The input context alive while the credits are open: `UiCancel` on Escape and gamepad East.
/// Its own context (not `MenuControls`) so Escape is bound only while there is something to
/// close — it never competes with gameplay's Escape binding (pause menu).
#[derive(Component)]
struct CreditsControls;

fn open_credits(
    _: In<ElementSignal>,
    open: Query<(), With<Credits>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if !open.is_empty() {
        return;
    }
    let credits: Vec<serde_json::Value> = CREDITS
        .iter()
        .map(|credit| {
            json!({ "id": credit.id, "title": credit.title, "author": credit.author, "url": credit.url })
        })
        .collect();
    commands.spawn((
        Credits,
        template(&asset_server, "credits.html"),
        TemplateContext::new().with("credits", &credits),
        // Focus stays inside while it's open; the main menu underneath can't be reached.
        HtmlModal,
        DespawnOnExit(GameState::MainMenu),
    ));
    // A separate entity, not a child of the root: an `HtmlUi` root's children belong to
    // bevy_markup, which removes anything it didn't build. Both carry `Credits`, so closing
    // despawns both.
    commands.spawn((
        Credits,
        CreditsControls,
        DespawnOnExit(GameState::MainMenu),
        actions!(
            CreditsControls[(
                Action::<UiCancel>::new(),
                bindings![KeyCode::Escape, GamepadButton::East],
            )]
        ),
    ));
}

/// Closes the credits (if open) and puts focus back on the main menu's Credits button.
#[derive(SystemParam)]
struct CloseCredits<'w, 's> {
    commands: Commands<'w, 's>,
    credits: Query<'w, 's, Entity, With<Credits>>,
    elements: Query<'w, 's, (Entity, &'static HtmlElement)>,
    focus: ResMut<'w, InputFocus>,
}

impl CloseCredits<'_, '_> {
    fn close(mut self) {
        let mut closed = false;
        for root in &self.credits {
            self.commands.entity(root).despawn();
            closed = true;
        }
        if closed
            && let Some((button, _)) = self
                .elements
                .iter()
                .find(|(_, element)| element.id.as_deref() == Some("credits"))
        {
            self.focus.set(button, FocusCause::Navigated);
        }
    }
}
