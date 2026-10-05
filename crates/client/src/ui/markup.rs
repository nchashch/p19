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
//!   messages (bevy_markup). Gamepad South / Enter ([`UiConfirm`]) emits the *same* message for the
//!   focused element, so every surface handles one input path: a `MessageReader<ElementSignal>`
//!   matching on `name`. Directional navigation (`MenuControls`: d-pad, arrows, left stick, with
//!   press-and-hold auto-repeat) moves `InputFocus` between the `data-on-click` elements of
//!   [`UiNav`] roots; a [`UiNavModal`] root confines it. Focus survives rebuilds (restored by
//!   element `id`), falls back to the root's `autofocus`-classed element, and is drawn as an
//!   `Outline` while `InputFocusVisible`.
//! - **Tooltips.** `data-on-enter="tooltip" data-on-leave="tooltip"` plus a `data-with` carrying
//!   `"tooltip"` (a Fluent key) and optional `"tooltip_args"`/`"tooltip_above"` shows a tooltip
//!   beside the element while the pointer is over it.
//!
//! bevy_markup's CSS subset has no positioning, `z-index`, `overflow` or `border-color`:
//! anything that must sit at a screen position is its own `HtmlUi` root whose `Node`
//! (`position_type`, `left`/`top`, …) and `GlobalZIndex` the spawning code sets.

use crate::add_observers_run_if;
use crate::assets::collections::CommonAssets;
use crate::controls::actions::{UiConfirm, UiNavigate};
use bevy::asset::embedded_asset;
use bevy::{
    input_focus::{
        FocusCause, InputFocus, InputFocusVisible,
        directional_navigation::DirectionalNavigationPlugin,
    },
    math::CompassOctant,
    platform::collections::HashMap,
    prelude::*,
    ui::{
        ComputedUiTargetCamera, UiGlobalTransform,
        auto_directional_navigation::{AutoDirectionalNavigation, AutoDirectionalNavigator},
    },
};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_fluent::prelude::Locale;
use bevy_markup::prelude::*;
use chill_bevy_console::console_closed;
use serde_json::Value;
use std::borrow::Cow;

pub struct MarkupPlugin;

impl Plugin for MarkupPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BevyMarkupPlugin);
        if !app.is_plugin_added::<DirectionalNavigationPlugin>() {
            app.add_plugins(DirectionalNavigationPlugin);
        }
        embedded_asset!(app, "html/theme.css");
        embedded_asset!(app, "html/tooltip.html");
        app.add_input_context::<MenuControls>()
            .init_resource::<UiNavigateHold>()
            .init_resource::<FocusMemory>()
            .init_resource::<LocaleBundles>()
            .add_systems(Startup, load_default_stylesheet)
            .add_systems(
                Update,
                (
                    register_ui_fonts.run_if(resource_added::<CommonAssets>),
                    sync_active_locale,
                    repeat_ui_navigate_while_held.run_if(console_closed),
                    focus_on_pointer_click,
                    show_tooltips,
                    remember_focus,
                ),
            )
            .add_systems(
                PostUpdate,
                (sync_navigation, repair_focus, update_focus_ring)
                    .chain()
                    .after(HtmlUiSystems::Build)
                    .before(bevy::ui::UiSystems::Prepare),
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

/// Opt-in on an `HtmlUi` root: its `data-on-click` elements take part in gamepad/keyboard
/// directional navigation and focus.
#[derive(Component, Clone, Copy, Default)]
pub struct UiNav;

/// An `HtmlUi` root that confines directional navigation to itself while it exists and is
/// visible (popups, the pause menu). With several, the highest `GlobalZIndex` wins.
#[derive(Component, Clone, Copy, Default)]
#[require(UiNav)]
pub struct UiNavModal;

/// Fired on the focused element when directional navigation finds no neighbour in `octant` —
/// lets a paginated list (the selector popup) page instead of stopping at its edge.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct UiNavigateEdge {
    pub entity: Entity,
    pub octant: CompassOctant,
}

/// Whether `signals` declares a `data-on-click` hook.
pub fn is_clickable(signals: &ElementSignals) -> bool {
    signals
        .0
        .iter()
        .any(|binding| binding.trigger == SignalTrigger::Click)
}

/// The `HtmlUi` root `entity` belongs to (itself included).
fn html_root(
    entity: Entity,
    parents: &Query<&ChildOf>,
    roots: &Query<(), With<HtmlUi>>,
) -> Option<Entity> {
    std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .find(|&ancestor| roots.contains(ancestor))
}

/// Gives exactly the clickable elements of the active navigation scope
/// `AutoDirectionalNavigation`: every visible [`UiNav`] root, or only the top [`UiNavModal`]
/// while one is visible.
fn sync_navigation(
    nav_roots: Query<
        (
            Entity,
            &InheritedVisibility,
            Has<UiNavModal>,
            Option<&GlobalZIndex>,
        ),
        (With<UiNav>, With<HtmlUi>),
    >,
    elements: Query<(Entity, &ElementSignals, Has<AutoDirectionalNavigation>)>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<HtmlUi>>,
    mut commands: Commands,
) {
    let modal = nav_roots
        .iter()
        .filter(|(_, visible, modal, _)| *modal && visible.get())
        .max_by_key(|(_, _, _, z)| z.map_or(0, |z| z.0))
        .map(|(entity, ..)| entity);
    for (entity, signals, navigable) in &elements {
        let wanted = is_clickable(signals)
            && html_root(entity, &parents, &roots).is_some_and(|root| match modal {
                Some(modal) => root == modal,
                None => nav_roots
                    .get(root)
                    .is_ok_and(|(_, visible, ..)| visible.get()),
            });
        if wanted && !navigable {
            commands
                .entity(entity)
                .insert(AutoDirectionalNavigation::default());
        } else if !wanted && navigable {
            commands
                .entity(entity)
                .remove::<AutoDirectionalNavigation>();
        }
    }
}

/// The last valid focus target: its `HtmlUi` root and element `id`, so a rebuild of that root
/// (which replaces every child entity) can put focus back on the same element.
#[derive(Resource, Default)]
struct FocusMemory {
    root: Option<Entity>,
    id: Option<String>,
}

fn remember_focus(
    focus: Res<InputFocus>,
    elements: Query<&HtmlElement, With<AutoDirectionalNavigation>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<HtmlUi>>,
    mut memory: ResMut<FocusMemory>,
) {
    let Some(entity) = focus.get() else {
        return;
    };
    let Ok(element) = elements.get(entity) else {
        return;
    };
    memory.root = html_root(entity, &parents, &roots);
    memory.id = element.id.clone();
}

/// Keeps `InputFocus` on a navigable element. When it points at nothing navigable (despawned
/// by a rebuild, outside a just-opened modal, never set): the remembered element in the same
/// root by `id`, else the first `autofocus`-classed element in scope, else the first navigable
/// element. With nothing navigable at all (in-game), focus is cleared.
fn repair_focus(
    mut focus: ResMut<InputFocus>,
    memory: Res<FocusMemory>,
    navigable: Query<(Entity, &HtmlElement), With<AutoDirectionalNavigation>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<HtmlUi>>,
) {
    if focus.get().is_some_and(|entity| navigable.contains(entity)) {
        return;
    }
    let remembered = memory.id.as_deref().and_then(|id| {
        navigable.iter().find_map(|(entity, element)| {
            (element.id.as_deref() == Some(id)
                && html_root(entity, &parents, &roots) == memory.root)
                .then_some(entity)
        })
    });
    let target = remembered
        .or_else(|| {
            navigable
                .iter()
                .find_map(|(entity, element)| element.has_class("autofocus").then_some(entity))
        })
        .or_else(|| navigable.iter().map(|(entity, _)| entity).min());
    match target {
        Some(entity) => focus.set(entity, FocusCause::Navigated),
        None => {
            if focus.get().is_some() {
                focus.clear();
            }
        }
    }
}

/// Pointer clicks move focus to the clicked element (so gamepad navigation continues from
/// there) and hide the focus ring, like a browser.
fn focus_on_pointer_click(
    mut signals: MessageReader<ElementSignal>,
    navigable: Query<(), With<AutoDirectionalNavigation>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    for signal in signals.read() {
        if signal.trigger == SignalTrigger::Click
            && signal.position.is_some()
            && navigable.contains(signal.target)
        {
            focus.set(signal.target, FocusCause::Pressed);
            focus_visible.0 = false;
        }
    }
}

const FOCUS_RING_COLOR: Color = Color::srgb(0.98, 0.78, 0.30);

/// Marks the element currently drawing the focus ring.
#[derive(Component)]
struct FocusRing;

fn update_focus_ring(
    focus: Res<InputFocus>,
    focus_visible: Res<InputFocusVisible>,
    ringed: Query<Entity, With<FocusRing>>,
    navigable: Query<(), With<AutoDirectionalNavigation>>,
    mut commands: Commands,
) {
    let wanted = focus
        .get()
        .filter(|&entity| focus_visible.0 && navigable.contains(entity));
    for entity in &ringed {
        if Some(entity) != wanted {
            commands.entity(entity).remove::<(FocusRing, Outline)>();
        }
    }
    if let Some(entity) = wanted
        && !ringed.contains(entity)
    {
        commands.entity(entity).insert((
            FocusRing,
            Outline {
                width: Val::Px(2.0),
                offset: Val::Px(2.0),
                color: FOCUS_RING_COLOR,
            },
        ));
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

/// One navigation step: moves focus, or reports the dead end as [`UiNavigateEdge`]. Sets
/// `InputFocusVisible` — directional navigation is what shows the ring.
fn navigate_step(
    octant: CompassOctant,
    navigator: &mut AutoDirectionalNavigator,
    focus_visible: &mut InputFocusVisible,
    commands: &mut Commands,
) {
    focus_visible.0 = true;
    if navigator.navigate(octant).is_err()
        && let Some(entity) = navigator.input_focus()
    {
        commands.trigger(UiNavigateEdge { entity, octant });
    }
}

fn on_ui_navigate(
    navigate: On<Start<UiNavigate>>,
    mut navigator: AutoDirectionalNavigator,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut hold: ResMut<UiNavigateHold>,
    mut commands: Commands,
) {
    let Ok(direction) = Dir2::new(navigate.value) else {
        return;
    };
    let octant = CompassOctant::from(direction);
    navigate_step(octant, &mut navigator, &mut focus_visible, &mut commands);
    hold.direction = Some(octant);
    hold.next_repeat = UI_NAVIGATE_HOLD_DELAY;
}

fn on_ui_navigate_complete(_complete: On<Complete<UiNavigate>>, mut hold: ResMut<UiNavigateHold>) {
    hold.direction = None;
}

fn repeat_ui_navigate_while_held(
    time: Res<Time>,
    mut hold: ResMut<UiNavigateHold>,
    mut navigator: AutoDirectionalNavigator,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut commands: Commands,
) {
    let Some(octant) = hold.direction else {
        return;
    };
    hold.next_repeat -= time.delta_secs();
    if hold.next_repeat > 0.0 {
        return;
    }
    navigate_step(octant, &mut navigator, &mut focus_visible, &mut commands);
    hold.next_repeat = UI_NAVIGATE_REPEAT_INTERVAL;
}

/// Gamepad South / Enter: emits the focused element's `data-on-click` signal, exactly what a
/// pointer click on it emits (minus `position`).
fn on_ui_confirm(
    _confirm: On<Start<UiConfirm>>,
    focus: Res<InputFocus>,
    elements: Query<(&ElementSignals, Option<&HtmlElement>), With<AutoDirectionalNavigation>>,
    mut writer: MessageWriter<ElementSignal>,
) {
    let Some(target) = focus.get() else {
        return;
    };
    let Ok((signals, element)) = elements.get(target) else {
        return;
    };
    for binding in signals
        .0
        .iter()
        .filter(|binding| binding.trigger == SignalTrigger::Click)
    {
        writer.write(ElementSignal {
            name: Cow::Owned(binding.name.clone()),
            trigger: SignalTrigger::Click,
            target,
            element: element.cloned().unwrap_or_default(),
            payload: binding.payload.clone(),
            position: None,
        });
    }
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
