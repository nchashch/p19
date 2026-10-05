//! A terminal-styled demo panel in the main menu's top-right corner (`html/tui_panel.html`):
//! a title, an elapsed-time readout and a gauge oscillating off it, proving the UI renders live.
//! Both are template values written every frame (the gauge as `style="width: …%"`): bevy_markup
//! updates the panel in place, only when the rounded values change. Non-interactive: no
//! `data-on-click` hooks, and the whole panel ignores picking.

use crate::ui::markup;
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy_markup::prelude::*;
use p19_shared::game_state::GameState;

pub struct TuiPanelPlugin;

impl Plugin for TuiPanelPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/tui_panel.html");
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu_tui_panel);
        app.add_systems(
            Update,
            update_main_menu_tui_panel.run_if(in_state(GameState::MainMenu)),
        );
    }
}

/// The panel's root.
#[derive(Component)]
struct MainMenuTuiPanel;

/// `tui_panel.html`'s variables at `elapsed` seconds: the whole seconds and the gauge fill in
/// whole percent, oscillating off the elapsed time.
fn panel_context(context: &mut TemplateContext, elapsed: f32) {
    context.insert("seconds", &(elapsed as u32));
    context.insert("gauge", &((elapsed.sin() + 1.0) / 2.0 * 100.0).round());
}

fn spawn_main_menu_tui_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    let mut context = TemplateContext::new();
    panel_context(&mut context, time.elapsed_secs());
    commands.spawn((
        MainMenuTuiPanel,
        markup::template(&asset_server, "tui_panel.html"),
        context,
        DespawnOnExit(GameState::MainMenu),
    ));
}

fn update_main_menu_tui_panel(
    time: Res<Time>,
    mut panels: Query<&mut TemplateContext, With<MainMenuTuiPanel>>,
) {
    for mut context in &mut panels {
        panel_context(&mut context, time.elapsed_secs());
    }
}
