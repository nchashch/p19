//! A minimal `bevy_tui_texture` + `ratatui` demo panel on the main menu — proves the crate is
//! correctly wired into this workspace (Bevy 0.19 / wgpu 29 / ratatui 0.30, matching this
//! project's own pins exactly) without touching any other UI. Renders as its own GPU texture on
//! a plain Bevy UI `Node`, entirely separate from the feathers-based button panel
//! (`ui::ui::main_menu_buttons`) — top-right corner, that panel's bottom-left, no overlap.
//!
//! Declarative spawn only (`TuiRequest::ui`, per the crate's own recommended pattern): no render
//! resources (`Assets<Image>`, `Device`/`Queue`, …) appear anywhere in this file — the plugin's
//! own `materialize_tui_requests` system does that work the frame after.
//!
//! **Font loading is deliberately `include_bytes!` + `Font::new`/`Fonts::new`
//! (`TuiFontSource::Ready`), not `TuiFontSource::Asset` via the `AssetServer`** — confirmed live,
//! not theoretical: `TerminalPlugin::build()` unconditionally registers its own
//! `TerminalFontAssetLoader` for the `.ttf` extension (no config flag to opt out), and loading
//! `fonts/mono/IBMPlexMono-Regular.ttf` through it — the exact same file `dev::console`'s
//! `ChillConsole` already loads as a real `Handle<bevy_text::font::Font>` — crashed the whole
//! app on startup with `bevy_asset`'s "target Handle<Font>'s TypeId does not match the TypeId of
//! this UntypedHandle", panicking inside `CommonAssets`' own (otherwise unrelated) loading-state
//! check. Embedding the font's bytes at compile time bypasses `AssetServer`/`AssetLoader`
//! entirely for this file's own font, sidestepping the conflict rather than just relocating it to
//! a different path (the loader registration itself is unconditional, so a different `.ttf` path
//! would still be reachable through the same competing loader).
//!
//! Non-interactive on purpose (`keyboard: false, mouse: false` in `TerminalConfig`): this
//! project's own gamepad/keyboard menu navigation (`ui::ui::MenuControls`,
//! `AutoDirectionalNavigation`) already owns the same raw input devices, and `TerminalInput`
//! stealing a share of it would risk exactly the kind of "two systems react to one input" bug
//! this codebase has hit before (`AGENTS.md`'s BEI "one consumer per input" note).

use bevy::prelude::*;
use bevy_tui_texture::prelude::*;
use bevy_tui_texture::{Font as TerminalFont, Fonts as TerminalFonts};
use ratatui::layout::Alignment;
use ratatui::style::{Color as RatatuiColor, Modifier, Style};
use ratatui::widgets::{Block, Gauge, Paragraph};
use p19_shared::game_state::GameState;
use std::sync::Arc;

pub struct TuiPanelPlugin;

impl Plugin for TuiPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TerminalPlugin::default());
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu_tui_panel);
        app.add_systems(
            Update,
            render_main_menu_tui_panel
                .in_set(TerminalSystemSet::Render)
                .run_if(in_state(GameState::MainMenu)),
        );
    }
}

/// Marks the demo panel's terminal entity, for both the render system below and
/// `DespawnOnExit::<GameState>` cleanup (implicit, via the entity itself carrying that
/// component — see the spawn call).
#[derive(Component)]
struct MainMenuTuiPanel;

const PANEL_COLS: u16 = 42;
const PANEL_ROWS: u16 = 13;
const FONT_SIZE_PX: u32 = 16;

/// Same font `dev::console`'s `ChillConsole` loads (there, through a real `AssetServer` call —
/// see this module's doc comment for why *this* file can't do the same). Embedded rather than
/// read at runtime so no `Handle`/load-state waiting is needed at all — the panel materializes
/// as soon as `TuiRequest::ui` is spawned.
///
/// `/client/assets` is gitignored, so CI checkouts have no font file; the build script
/// (`client/build.rs`) only emits `has_tui_font` when the file exists. Without it the panel
/// degrades to not spawning (below) — ratatui rasterizes glyphs from real TTF bytes itself,
/// so there is no "Bevy default font" fallback path into `TuiFontSource`.
#[cfg(has_tui_font)]
static IBM_PLEX_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/mono/IBMPlexMono-Regular.ttf");
#[cfg(not(has_tui_font))]
static IBM_PLEX_MONO_REGULAR: &[u8] = &[];

fn spawn_main_menu_tui_panel(mut commands: Commands) {
    // No embedded font (asset-less CI checkout) → skip the panel entirely. Nothing else in
    // the app reads it; the empty `&[]` above exists only so this file compiles.
    if IBM_PLEX_MONO_REGULAR.is_empty() {
        warn_once!(
            "TUI panel skipped: client/assets/fonts/mono/IBMPlexMono-Regular.ttf not found \
             (asset-less checkout); the menu renders without it"
        );
        return;
    }
    let font = TerminalFont::new(IBM_PLEX_MONO_REGULAR)
        .expect("embedded fonts/mono/IBMPlexMono-Regular.ttf must parse as a valid TTF");
    let fonts = Arc::new(TerminalFonts::new(font, FONT_SIZE_PX));

    commands.spawn((
        TuiRequest::ui(PANEL_COLS, PANEL_ROWS, fonts).with_config(TerminalConfig {
            keyboard: false,
            mouse: false,
            ..default()
        }),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            right: Val::Px(20.0),
            ..default()
        },
        MainMenuTuiPanel,
        DespawnOnExit(GameState::MainMenu),
    ));
}

/// Draws live content every frame so the panel visibly proves it's a real render, not a static
/// image: an elapsed-time readout and a gauge oscillating off it. `Query<&mut Tui, ...>` tolerates
/// the request not having materialized yet (`TuiRequest` → `Tui` takes one frame) — same "just try
/// again next frame" idiom the crate's own examples use.
fn render_main_menu_tui_panel(mut panels: Query<&mut Tui, With<MainMenuTuiPanel>>, time: Res<Time>) {
    let Ok(mut term) = panels.single_mut() else {
        return;
    };
    let elapsed = time.elapsed_secs();
    let ratio = ((elapsed.sin() + 1.0) / 2.0) as f64;

    term.draw(|frame| {
        let outer = Block::bordered()
            .title(" bevy_tui_texture + ratatui ")
            .border_style(Style::default().fg(RatatuiColor::LightCyan));
        let inner = outer.inner(frame.area());
        frame.render_widget(outer, frame.area());

        let rows =
            ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(3),
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(format!(
                "rendering on a Bevy UI Node\nas a GPU texture, live\nt = {elapsed:>6.1}s",
            ))
            .alignment(Alignment::Center)
            .style(Style::default().fg(RatatuiColor::White).add_modifier(Modifier::BOLD)),
            rows[0],
        );

        frame.render_widget(
            Gauge::default()
                .ratio(ratio)
                .label(format!("{:>3.0}%", ratio * 100.0))
                .gauge_style(Style::default().fg(RatatuiColor::Magenta)),
            rows[2],
        );
    });
}
