//! The client's UI layer on top of `bevy_markup`: every UI surface is an `HtmlUi` (a Tera
//! template + CSS + Fluent) whose built children are plain Bevy UI. This module owns what all of
//! them share:
//!
//! - **Assets.** Templates and the stylesheet are compiled into the binary (`embedded_asset!`,
//!   files in `src/ui/html/`), so the UI is versioned with the code and present under every asset
//!   root (`BEVY_ASSET_ROOT` playtest sets, `--no-common-assets`). Each surface module registers
//!   its own templates; [`template`] loads one. `html/theme.css` is the single
//!   `DefaultStylesheet` for every surface.
//! - **Fonts / locale.** `FontFamilies` gets the UI font from `CommonAssets` once loaded (CSS
//!   `font-family: serif`); `ActiveLocale` follows bevy_fluent's `Locale` resource (the language
//!   picker writes `Locale`), loading `locales/<id>/main.ftl.yml`.
//! - **Interaction.** Pointer clicks on `data-on-click` elements arrive as `ElementSignal`
//!   messages (bevy_markup), and so does gamepad South / Enter ([`UiConfirm`] → bevy_markup's
//!   `HtmlFocus::activate` on the focused element), so every surface handles one input path: a
//!   `MessageReader<ElementSignal>` matching on `name`. Focus itself is bevy_markup's (browser
//!   style: `data-on-click` elements are focusable, the `autofocus` attribute takes the initial
//!   focus, focus survives rebuilds by `id`, `HtmlModal` roots confine it, `HtmlNoFocus` roots
//!   never take it, `:focus-visible` + `outline` in `theme.css` draw the ring). This module only
//!   binds the input: `MenuControls` (d-pad, arrows, left stick, South/Enter) drives
//!   `HtmlFocus::navigate`, with press-and-hold auto-repeat.
//! - **Tooltips.** `data-on-enter="tooltip" data-on-leave="tooltip"` plus a `data-with` carrying
//!   `"tooltip"` (a Fluent key) and optional `"tooltip_args"`/`"tooltip_above"` shows a tooltip
//!   beside the element while the pointer is over it.
//!
//! bevy_markup's CSS doesn't reach the `HtmlUi` root itself: each root's screen placement
//! (`Node`), cross-root stacking (`GlobalZIndex`) and `Pickable` are set by the spawning code.

use crate::add_observers_run_if;
use crate::assets::collections::CommonAssets;
use crate::controls::actions::{UiConfirm, UiNavigate};
use bevy::asset::embedded_asset;
use bevy::{
    math::CompassOctant,
    platform::collections::HashMap,
    prelude::*,
    ui::{ComputedUiTargetCamera, UiGlobalTransform},
};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_fluent::prelude::Locale;
use bevy_markup::prelude::*;
use chill_bevy_console::console_closed;
use serde_json::Value;

pub struct MarkupPlugin;

impl Plugin for MarkupPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BevyMarkupPlugin);
        embedded_asset!(app, "html/theme.css");
        embedded_asset!(app, "html/tooltip.html");
        app.add_input_context::<MenuControls>()
            .init_resource::<UiNavigateHold>()
            .init_resource::<LocaleBundles>()
            .add_systems(Startup, load_default_stylesheet)
            .add_systems(
                Update,
                (
                    register_ui_fonts.run_if(resource_added::<CommonAssets>),
                    sync_active_locale,
                    repeat_ui_navigate_while_held.run_if(console_closed),
                    show_tooltips,
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

fn load_default_stylesheet(mut stylesheet: ResMut<DefaultStylesheet>, assets: Res<AssetServer>) {
    *stylesheet = DefaultStylesheet::new(assets.load(embedded_path("theme.css")));
}

/// The CSS name of the UI font; `theme.css` also maps the `serif` generic to it.
const UI_FONT_FAMILY: &str = "Iosevka Slab QP";

/// Registers `CommonAssets`' UI font faces. Absent faces (`--no-common-assets`) leave the
/// family unregistered, so CSS falls back to Bevy's default font.
fn register_ui_fonts(common_assets: Res<CommonAssets>, mut fonts: ResMut<FontFamilies>) {
    let Some(regular) = common_assets.serif_font.clone() else {
        return;
    };
    let mut faces = FontFaces::new(regular);
    if let Some(bold) = common_assets.serif_bold_font.clone() {
        faces = faces.with_bold(bold);
    }
    fonts
        .insert(UI_FONT_FAMILY, faces)
        .set_generic(GenericFamily::Serif, UI_FONT_FAMILY);
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
) {
    let Ok(direction) = Dir2::new(navigate.value) else {
        return;
    };
    let octant = CompassOctant::from(direction);
    focus.navigate(octant);
    hold.direction = Some(octant);
    hold.next_repeat = UI_NAVIGATE_HOLD_DELAY;
}

fn on_ui_navigate_complete(_complete: On<Complete<UiNavigate>>, mut hold: ResMut<UiNavigateHold>) {
    hold.direction = None;
}

fn repeat_ui_navigate_while_held(
    time: Res<Time>,
    mut hold: ResMut<UiNavigateHold>,
    mut focus: HtmlFocus,
) {
    let Some(octant) = hold.direction else {
        return;
    };
    hold.next_repeat -= time.delta_secs();
    if hold.next_repeat > 0.0 {
        return;
    }
    focus.navigate(octant);
    hold.next_repeat = UI_NAVIGATE_REPEAT_INTERVAL;
}

/// Gamepad South / Enter: activates the focused element — the same `ElementSignal` a pointer
/// click on it emits (minus `position`).
fn on_ui_confirm(_confirm: On<Start<UiConfirm>>, mut focus: HtmlFocus) {
    focus.activate();
}

/// Signal name of the tooltip hooks (`data-on-enter` / `data-on-leave`).
pub const TOOLTIP_SIGNAL: &str = "tooltip";

/// The tooltip root shown for `element`.
#[derive(Component)]
struct Tooltip {
    element: Entity,
}

const TOOLTIP_GAP: f32 = 8.0;
const TOOLTIP_Z: i32 = 1000;

/// Spawns a tooltip root beside the hovered element on `tooltip` enter, despawns it on leave.
/// Rendered on the element's own UI camera (quad panels render to textures).
fn show_tooltips(
    mut signals: MessageReader<ElementSignal>,
    tooltips: Query<(Entity, &Tooltip)>,
    elements: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        Option<&ComputedUiTargetCamera>,
    )>,
    cameras: Query<&Camera>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for signal in signals
        .read()
        .filter(|signal| signal.name == TOOLTIP_SIGNAL)
    {
        match signal.trigger {
            SignalTrigger::Enter => {
                let Some(key) = signal.payload.get("tooltip").and_then(Value::as_str) else {
                    continue;
                };
                let Ok((node, transform, target)) = elements.get(signal.target) else {
                    continue;
                };
                let camera = target.and_then(ComputedUiTargetCamera::get);
                // Physical px → UI px (window scale × `UiScale`), the space `Node` offsets use.
                let scale = node.inverse_scale_factor;
                let size = node.size() * scale;
                let top_left = transform.translation * scale - size / 2.0;
                let above = signal
                    .payload
                    .get("tooltip_above")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let viewport_height = camera
                    .and_then(|camera| cameras.get(camera).ok())
                    .and_then(Camera::physical_viewport_size)
                    .map_or(0.0, |size| size.y as f32 * scale);
                let position = if above {
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(top_left.x),
                        bottom: px(viewport_height - top_left.y + TOOLTIP_GAP),
                        ..default()
                    }
                } else {
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(top_left.x + size.x + TOOLTIP_GAP),
                        top: px(top_left.y),
                        ..default()
                    }
                };
                let args = signal
                    .payload
                    .get("tooltip_args")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                let mut root = commands.spawn((
                    Tooltip {
                        element: signal.target,
                    },
                    template(&asset_server, "tooltip.html"),
                    TemplateContext::new().with("key", key).with("args", &args),
                    position,
                    GlobalZIndex(TOOLTIP_Z),
                    Pickable::IGNORE,
                ));
                if let Some(camera) = camera {
                    root.insert(UiTargetCamera(camera));
                }
            }
            SignalTrigger::Leave => {
                for (entity, tooltip) in &tooltips {
                    if tooltip.element == signal.target {
                        commands.entity(entity).despawn();
                    }
                }
            }
            _ => {}
        }
    }
    // A tooltip whose element was despawned without a leave (its root despawned) goes too.
    for (entity, tooltip) in &tooltips {
        if elements.get(tooltip.element).is_err() {
            commands.entity(entity).despawn();
        }
    }
}
