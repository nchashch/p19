//! Screens opened over a surface: modals with a Back button, opened by one of its buttons (the
//! main menu's Options/Credits, the pause menu's Options). [`open_menu_screen`] spawns the
//! screen's `HtmlUi` root (`HtmlModal`, `.menu-screen-root` in `theme.css`, above the pause
//! menu's own root) plus its cancel context. A Back button
//! (`data-on-click="menu-screen.back"`), Escape or gamepad East closes it and returns focus to
//! the button that opened it. With a selector popup open on the screen, cancel closes the popup
//! first.
//!
//! Escape is bound only on the main-menu scope: over the pause modal, Escape already belongs to
//! `ToggleModalMenu` (`controls.rs`), so the pause-scoped screen binds only gamepad East and
//! `toggle_modal_menu` closes the topmost screen itself (see [`close_topmost_screen`]).
//!
//! Signals: `menu-screen.back`.

use crate::controls::actions::UiCancel;
use crate::ui::markup::template;
use crate::ui::selector::SelectorPopup;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_markup::prelude::*;
use p19_shared::game_state::{GameState, ModalMenuState};

pub struct MenuScreenPlugin;

impl Plugin for MenuScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<MenuScreenControls>()
            .on_html_click(
                "menu-screen.back",
                |_: In<ElementSignal>, close: CloseMenuScreen| close.close_screen(),
            )
            .add_observer(|_: On<Start<UiCancel>>, close: CloseMenuScreen| close.cancel());
    }
}

/// An open screen's root and its input context (both despawned on close). `return_focus` is the
/// `id` of the main-menu button that opened it.
#[derive(Component)]
pub struct MenuScreen {
    return_focus: &'static str,
}

/// The input context alive while a screen is open: `UiCancel` on Escape and gamepad East. Its
/// own context (not `MenuControls`) so Escape is bound only while there is something to close.
#[derive(Component)]
struct MenuScreenControls;

/// Which surface the screen opens over: its root and cancel context despawn when that surface's
/// state exits (the main menu, or the pause modal closing).
#[derive(Clone, Copy, PartialEq)]
pub enum MenuScreenScope {
    /// Over the main menu.
    MainMenu,
    /// Over the in-game pause modal.
    PauseMenu,
}

/// Opens the screen `template` (an embedded `options.html`, …) with `context`, unless a screen is
/// already open. `return_focus` is the `id` of the button that opened it. Returns the screen's
/// root.
pub fn open_menu_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    open: &OpenMenuScreens,
    template_name: &str,
    context: TemplateContext,
    return_focus: &'static str,
    scope: MenuScreenScope,
) -> Option<Entity> {
    if !open.is_empty() {
        return None;
    }
    let root = match scope {
        MenuScreenScope::MainMenu => {
            let root = commands
                .spawn((
                    MenuScreen { return_focus },
                    template(asset_server, template_name),
                    context,
                    // Focus stays inside while it's open; the surface underneath can't be
                    // reached.
                    HtmlModal,
                    DespawnOnExit(GameState::MainMenu),
                ))
                .id();
            commands.spawn((
                MenuScreen { return_focus },
                MenuScreenControls,
                DespawnOnExit(GameState::MainMenu),
                actions!(
                    MenuScreenControls[(
                        Action::<UiCancel>::new(),
                        bindings![KeyCode::Escape, GamepadButton::East],
                    )]
                ),
            ));
            root
        }
        MenuScreenScope::PauseMenu => {
            let root = commands
                .spawn((
                    MenuScreen { return_focus },
                    template(asset_server, template_name),
                    context,
                    HtmlModal,
                    DespawnOnExit(ModalMenuState::Open),
                ))
                .id();
            // East only: Escape belongs to `ToggleModalMenu` (see the module doc).
            commands.spawn((
                MenuScreen { return_focus },
                MenuScreenControls,
                DespawnOnExit(ModalMenuState::Open),
                actions!(
                    MenuScreenControls[(
                        Action::<UiCancel>::new(),
                        bindings![GamepadButton::East],
                    )]
                ),
            ));
            root
        }
    };
    Some(root)
}

/// Closes the topmost layer — an open selector popup, else the (single) open screen — and
/// refocuses the button that opened it. Returns whether anything was closed. Free function so
/// `controls.rs`'s pause-modal Escape handler can close a pause-scoped screen without that
/// screen binding Escape itself.
pub fn close_topmost_screen(
    commands: &mut Commands,
    screens: &Query<(Entity, &MenuScreen)>,
    popups: &Query<(Entity, &SelectorPopup)>,
    elements: &Query<(Entity, &HtmlElement)>,
    focus: &mut InputFocus,
) -> bool {
    if screens.is_empty() {
        return false;
    }
    if let Some((popup, popup_state)) = popups.iter().next() {
        commands.entity(popup).try_despawn();
        focus.set(popup_state.toggle, FocusCause::Navigated);
        return true;
    }
    let mut return_focus = None;
    for (entity, screen) in screens {
        commands.entity(entity).despawn();
        return_focus = Some(screen.return_focus);
    }
    // Popups anchored inside the screen go with their toggle (`HtmlAnchor`).
    if let Some(return_focus) = return_focus
        && let Some((button, _)) = elements
            .iter()
            .find(|(_, element)| element.id.as_deref() == Some(return_focus))
    {
        focus.set(button, FocusCause::Navigated);
    }
    true
}

/// The open screens; [`open_menu_screen`] opens nothing while there is one.
pub type OpenMenuScreens<'w, 's> = Query<'w, 's, (), With<MenuScreen>>;

#[derive(SystemParam)]
struct CloseMenuScreen<'w, 's> {
    commands: Commands<'w, 's>,
    screens: Query<'w, 's, (Entity, &'static MenuScreen)>,
    popups: Query<'w, 's, (Entity, &'static SelectorPopup)>,
    elements: Query<'w, 's, (Entity, &'static HtmlElement)>,
    focus: ResMut<'w, InputFocus>,
}

impl CloseMenuScreen<'_, '_> {
    /// Escape / East: closes the topmost layer — an open selector popup, else the screen.
    fn cancel(self) {
        self.close_topmost();
    }

    /// Closes the open screen (with any popup on it) and focuses the button that opened it.
    fn close_screen(self) {
        self.close_topmost();
    }

    fn close_topmost(mut self) {
        close_topmost_screen(
            &mut self.commands,
            &self.screens,
            &self.popups,
            &self.elements,
            &mut self.focus,
        );
    }
}
