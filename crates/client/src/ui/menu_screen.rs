//! Main-menu screens: modals over the main menu opened by one of its buttons (Options, Credits).
//! [`open_menu_screen`] spawns the screen's `HtmlUi` root (`HtmlModal`, `.menu-screen-root` in
//! `theme.css`) plus its cancel context. A Back button (`data-on-click="menu-screen.back"`),
//! Escape or gamepad East closes it and returns focus to the button that opened it. With a
//! selector popup open on the screen, cancel closes the popup first.
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
use p19_shared::game_state::GameState;

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

/// Opens the screen `template` (an embedded `credits.html`, …) with `context`, unless a screen is
/// already open. `return_focus` is the `id` of the opening button.
pub fn open_menu_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    open: &OpenMenuScreens,
    template_name: &str,
    context: TemplateContext,
    return_focus: &'static str,
) {
    if !open.is_empty() {
        return;
    }
    commands.spawn((
        MenuScreen { return_focus },
        template(asset_server, template_name),
        context,
        // Focus stays inside while it's open; the main menu underneath can't be reached.
        HtmlModal,
        DespawnOnExit(GameState::MainMenu),
    ));
    // A separate entity, not a child of the root: an `HtmlUi` root's children belong to
    // bevy_markup, which removes anything it didn't build.
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
    fn cancel(mut self) {
        if self.screens.is_empty() {
            return;
        }
        if let Some((popup, popup_state)) = self.popups.iter().next() {
            self.commands.entity(popup).try_despawn();
            self.focus.set(popup_state.toggle, FocusCause::Navigated);
            return;
        }
        self.close_screen();
    }

    /// Closes the open screen (with any popup on it) and focuses the button that opened it.
    fn close_screen(mut self) {
        let mut return_focus = None;
        for (entity, screen) in &self.screens {
            self.commands.entity(entity).despawn();
            return_focus = Some(screen.return_focus);
        }
        let Some(return_focus) = return_focus else {
            return;
        };
        // Popups anchored inside the screen go with their toggle (`HtmlAnchor`).
        if let Some((button, _)) = self
            .elements
            .iter()
            .find(|(_, element)| element.id.as_deref() == Some(return_focus))
        {
            self.focus.set(button, FocusCause::Navigated);
        }
    }
}
