//! A terminal-styled demo panel in the main menu's top-right corner (`html/tui_panel.html`):
//! a title, an elapsed-time readout and a gauge oscillating off it, proving the UI renders live.
//! The gauge fill is a per-frame `Node.width` update on the built element; the readout's seconds
//! are written every frame and rebuild only when the displayed second changes. Non-interactive: no `data-on-click` hooks, and
//! the whole panel ignores picking.

use crate::ui::markup;
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy_markup::prelude::*;
use p19_shared::game_state::GameState;

pub struct TuiPanelPlugin;

impl Plugin for TuiPanelPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/tui_panel.html");
        app.add_observer(on_tui_panel_built);
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

/// The gauge's fill element.
#[derive(Component)]
struct TuiGaugeFill;

const PANEL_MARGIN: f32 = 20.0;

fn spawn_main_menu_tui_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    commands.spawn((
        MainMenuTuiPanel,
        markup::template(&asset_server, "tui_panel.html"),
        TemplateContext::new().with("seconds", &(time.elapsed_secs() as u32)),
        Node {
            position_type: PositionType::Absolute,
            top: px(PANEL_MARGIN),
            right: px(PANEL_MARGIN),
            ..default()
        },
        Pickable::IGNORE,
        DespawnOnExit(GameState::MainMenu),
    ));
}

/// Also sets the new fill's width right away: the rebuild happens after this frame's
/// `update_main_menu_tui_panel`, and the CSS width would otherwise show for a frame.
fn on_tui_panel_built(
    built: On<HtmlUiBuilt>,
    panels: Query<(), With<MainMenuTuiPanel>>,
    elements: HtmlElements,
    time: Res<Time>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
) {
    if !panels.contains(built.entity) {
        return;
    }
    if let Some(fill) = elements.by_id(built.entity, "tui-gauge-fill") {
        commands.entity(fill).insert(TuiGaugeFill);
        if let Ok(mut node) = nodes.get_mut(fill) {
            node.width = gauge_width(time.elapsed_secs());
        }
    }
}

/// The gauge fill's width, oscillating off the elapsed time.
fn gauge_width(elapsed: f32) -> Val {
    percent((elapsed.sin() + 1.0) / 2.0 * 100.0)
}

/// Sets the gauge every frame and writes the readout's whole seconds (bevy_markup rebuilds only
/// when the rendered text changes, once a second).
fn update_main_menu_tui_panel(
    time: Res<Time>,
    mut panels: Query<&mut TemplateContext, With<MainMenuTuiPanel>>,
    mut fills: Query<&mut Node, With<TuiGaugeFill>>,
) {
    let elapsed = time.elapsed_secs();
    for mut fill in &mut fills {
        fill.width = gauge_width(elapsed);
    }
    for mut context in &mut panels {
        context.insert("seconds", &(elapsed as u32));
    }
}
