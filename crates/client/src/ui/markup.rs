//! The client's UI layer on top of `bevy_markup`: every UI surface is an `HtmlUi` (a Tera
//! template + CSS + Fluent) whose built children are plain Bevy UI. This module owns what all of
//! them share:
//!
//! - **Assets.** Templates and the stylesheet are compiled into the binary (`embedded_asset!`,
//!   files in `src/ui/html/`), so the UI is versioned with the code and present under every asset
//!   root (`BEVY_ASSET_ROOT` playtest sets, `--no-common-assets`). Each surface module registers
//!   its own templates; [`template`] loads one. `html/theme.css` is the single
//!   `DefaultStylesheet` for every surface.
//! - **Fonts / locale.** The UI uses the system's fonts: [`register_ui_fonts`] maps the CSS
//!   generics `serif` / `sans-serif` / `monospace` to the host's families (Bevy's
//!   `system_font_discovery`; no font files ship with the game). `ActiveLocale` follows
//!   bevy_fluent's `Locale` resource (the language picker writes `Locale`), loading
//!   `locales/<id>/main.ftl.yml`.
//! - **Interaction.** Pointer clicks on `data-on-click` elements arrive as `ElementSignal`
//!   messages (bevy_markup), and so does gamepad South / Enter ([`UiConfirm`] → bevy_markup's
//!   `HtmlFocus::activate` on the focused element), so every surface handles one input path: a
//!   `MessageReader<ElementSignal>` matching on `name`. Focus itself is bevy_markup's (browser
//!   style: `data-on-click` elements are focusable, the `autofocus` attribute takes the initial
//!   focus, focus survives rebuilds by `id`, `HtmlModal` roots confine it, `HtmlNoFocus` roots
//!   never take it, `:focus-visible` + `outline` in `theme.css` draw the ring). This module only
//!   binds the input: `MenuControls` (d-pad, arrows, left stick, South/Enter) drives
//!   `HtmlFocus::navigate`, with press-and-hold auto-repeat.
//! - **Tooltips** are bevy_markup's: `data-tooltip` (a Fluent key) and optional
//!   `data-tooltip-args` (JSON) / `data-tooltip-placement="above"` show `tooltip.html` beside the
//!   element while the pointer is over it (`HtmlTooltips`, inserted here).
//!
//! Each template's `<html class="…-root">` places, stacks and (un)picks its `HtmlUi` root from
//! `theme.css` ("Roots"); overlays (tooltips, selector popups) are placed beside their element by
//! bevy_markup's `HtmlAnchor`.

use crate::add_observers_run_if;
use crate::controls::actions::{UiConfirm, UiNavigate};
use crate::ui::slider::{self, Slider, SliderInput};
use bevy::asset::embedded_asset;
use bevy::{math::CompassOctant, platform::collections::HashMap, prelude::*};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_fluent::prelude::Locale;
use bevy_markup::prelude::*;
use chill_bevy_console::console_closed;

pub struct MarkupPlugin;

impl Plugin for MarkupPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BevyMarkupPlugin);
        embedded_asset!(app, "html/theme.css");
        // Shared `{% component %}`s, included by the templates that use them.
        embedded_asset!(app, "html/components.html");
        embedded_asset!(app, "html/tooltip.html");
        app.add_input_context::<MenuControls>()
            .init_resource::<UiNavigateHold>()
            .init_resource::<LocaleBundles>()
            .add_systems(Startup, (load_default_stylesheet, register_ui_fonts))
            .add_systems(
                Update,
                (
                    sync_active_locale,
                    repeat_ui_navigate_while_held.run_if(console_closed),
                ),
            );
        add_observers_run_if!(app, console_closed, on_ui_navigate, on_ui_confirm);
        // Ungated: a release while the console is open must still clear the held direction, or
        // the repeat system would keep navigating after the console closes.
        app.add_observer(on_ui_navigate_complete);
    }
}

/// Asset path of an embedded template or stylesheet in `src/ui/html/` (registered with
/// `embedded_asset!(app, "html/<name>")` from a module in `src/ui/`).
pub fn embedded_path(name: &str) -> String {
    format!("embedded://p19_client/ui/html/{name}")
}

/// An `HtmlUi` for the embedded template `name` (e.g. `"main_menu.html"`).
pub fn template(asset_server: &AssetServer, name: &str) -> HtmlUi {
    HtmlUi::new(asset_server.load(embedded_path(name)))
}

/// The stylesheet, and bevy_markup's `data-tooltip` tooltips rendered from `tooltip.html`.
fn load_default_stylesheet(
    mut stylesheet: ResMut<DefaultStylesheet>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    *stylesheet = DefaultStylesheet::new(assets.load(embedded_path("theme.css")));
    commands.insert_resource(HtmlTooltips::new(
        assets.load(embedded_path("tooltip.html")),
    ));
}

/// Maps the CSS generic families `theme.css` uses to the system's fonts (Bevy's
/// `FontSource` generics, resolved by `system_font_discovery`): one source per family, the
/// system picks the bold and italic faces. Glyphs a family lacks (Japanese, say) fall back to
/// any installed font that has them. Nothing ships with the game, so the look follows the
/// player's system, and a locale renders only if some installed font covers its script.
fn register_ui_fonts(mut fonts: ResMut<FontFamilies>) {
    fonts
        .insert("System Serif", FontFaces::new(FontSource::Serif))
        .insert("System Sans", FontFaces::new(FontSource::SansSerif))
        .insert("System Mono", FontFaces::new(FontSource::Monospace))
        .set_generic(GenericFamily::Serif, "System Serif")
        .set_generic(GenericFamily::SansSerif, "System Sans")
        .set_generic(GenericFamily::Monospace, "System Mono");
}

/// Loaded locale bundles by language id, kept alive so switching back is instant.
#[derive(Resource, Default)]
struct LocaleBundles(HashMap<String, Handle<BundleAsset>>);

/// Points `ActiveLocale` at `Locale::requested`'s bundle whenever the requested language
/// changes (and once at startup).
fn sync_active_locale(
    locale: Res<Locale>,
    asset_server: Res<AssetServer>,
    mut bundles: ResMut<LocaleBundles>,
    mut active: ResMut<ActiveLocale>,
) {
    if !locale.is_changed() {
        return;
    }
    let id = locale.requested.to_string();
    let handle = bundles
        .0
        .entry(id.clone())
        .or_insert_with(|| asset_server.load(format!("locales/{id}/main.ftl.yml")))
        .clone();
    if active.0.as_ref() != Some(&handle) {
        active.set(handle);
    }
}

/// The `bevy_enhanced_input` context for gamepad/keyboard UI navigation. Spawn
/// [`menu_controls`] scoped to whatever UI state needs navigation (main menu, lobby, pause menu);
/// only one should be alive at a time.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct MenuControls;

pub fn menu_controls() -> impl Bundle {
    (
        MenuControls,
        Actions::<MenuControls>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
            context.spawn((
                Action::<UiNavigate>::new(),
                Bindings::spawn((Cardinal::dpad(),)),
            ));
            context.spawn((
                Action::<UiNavigate>::new(),
                Bindings::spawn((Cardinal::arrows(),)),
            ));
            context.spawn((
                Action::<UiNavigate>::new(),
                DeadZone {
                    kind: DeadZoneKind::Radial,
                    lower_threshold: 0.15,
                    upper_threshold: 1.0,
                },
                Bindings::spawn(Axial::left_stick()),
            ));
            // `require_reset`: a confirm that despawns and respawns `MenuControls` (the pause
            // menu's "Main Menu" → main menu) must not read the still-held South/Enter as a new
            // press on the fresh context. BEI tracks the reset per physical binding, globally.
            context.spawn((
                Action::<UiConfirm>::new(),
                ActionSettings {
                    require_reset: true,
                    ..default()
                },
                Press::new(1.0),
                bindings![GamepadButton::South, KeyCode::Enter],
            ));
        })),
    )
}

/// Delay before a held `UiNavigate` direction starts auto-repeating, and the repeat interval.
const UI_NAVIGATE_HOLD_DELAY: f32 = 0.4;
const UI_NAVIGATE_REPEAT_INTERVAL: f32 = 0.08;

/// The currently held `UiNavigate` direction, for press-and-hold auto-repeat.
#[derive(Resource, Default)]
struct UiNavigateHold {
    direction: Option<CompassOctant>,
    /// Seconds until the next repeat step.
    next_repeat: f32,
}

fn on_ui_navigate(
    navigate: On<Start<UiNavigate>>,
    mut focus: HtmlFocus,
    mut hold: ResMut<UiNavigateHold>,
    sliders: Query<&Slider>,
    mut slider_input: MessageWriter<SliderInput>,
) {
    let Ok(direction) = Dir2::new(navigate.value) else {
        return;
    };
    let octant = CompassOctant::from(direction);
    navigate_or_step(&mut focus, octant, &sliders, &mut slider_input);
    hold.direction = Some(octant);
    hold.next_repeat = UI_NAVIGATE_HOLD_DELAY;
}

/// Left/right on a focused slider steps it (`ui/slider.rs`); anything else moves focus.
fn navigate_or_step(
    focus: &mut HtmlFocus,
    octant: CompassOctant,
    sliders: &Query<&Slider>,
    slider_input: &mut MessageWriter<SliderInput>,
) {
    match slider::step_for(focus.focused(), octant, sliders) {
        Some(step) => {
            slider_input.write(step);
        }
        None => {
            focus.navigate(octant);
        }
    }
}

fn on_ui_navigate_complete(_complete: On<Complete<UiNavigate>>, mut hold: ResMut<UiNavigateHold>) {
    hold.direction = None;
}

fn repeat_ui_navigate_while_held(
    time: Res<Time>,
    mut hold: ResMut<UiNavigateHold>,
    mut focus: HtmlFocus,
    sliders: Query<&Slider>,
    mut slider_input: MessageWriter<SliderInput>,
) {
    let Some(octant) = hold.direction else {
        return;
    };
    hold.next_repeat -= time.delta_secs();
    if hold.next_repeat > 0.0 {
        return;
    }
    navigate_or_step(&mut focus, octant, &sliders, &mut slider_input);
    hold.next_repeat = UI_NAVIGATE_REPEAT_INTERVAL;
}

/// Gamepad South / Enter: activates the focused element — the same `ElementSignal` a pointer
/// click on it emits, its source the input that did it (`UiConfirm` doesn't say which of its
/// bindings fired, so the devices are asked; the tool API's device mocks count as the device).
fn on_ui_confirm(
    _confirm: On<Start<UiConfirm>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut focus: HtmlFocus,
) {
    let input = if keys.just_pressed(KeyCode::Enter) {
        ActivationInput::Key(KeyCode::Enter)
    } else if let Some((gamepad, _)) = gamepads
        .iter()
        .find(|(_, pad)| pad.just_pressed(GamepadButton::South))
    {
        ActivationInput::GamepadButton {
            gamepad,
            button: GamepadButton::South,
        }
    } else {
        ActivationInput::Other
    };
    focus.activate(input);
}
