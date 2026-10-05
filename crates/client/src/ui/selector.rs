//! A generic "selector" popup: a paginated window of [`SELECTOR_VISIBLE_ROWS`] rows over a
//! caller-supplied option list, with a discrete scrollbar, gamepad/keyboard paging at the window's
//! edges and mouse-wheel paging. Used by the main menu's Options/Language pickers and the lobby's
//! Level picker.
//!
//! # Using it
//!
//! 1. Spawn a [`Selector`] entity (scoped to the owning state) with a unique `key` (namespaced
//!    like signal names, e.g. `"lobby.level"`) and its options. Replace the options at any time
//!    with [`Selector::set_options`]; an open popup follows.
//! 2. Put a toggle element in the owner's template:
//!    `<div class="button" id="level" data-on-click="selector.toggle"
//!    data-with='{"selector": "lobby.level"}'>…</div>`. This module handles the click: it opens
//!    the popup just right of that element (on the element's UI camera, so VR quad panels work)
//!    or closes it if it's already open.
//! 3. Read [`SelectorPicked`] messages for your key; `value` is the picked
//!    [`SelectorOption::value`].
//!
//! The popup is its own `HtmlUi` root (`selector.html`) with [`HtmlModal`], so directional
//! navigation stays inside it while it's open. Visible rows are re-rendered whenever the window
//! pages; their element ids are per visible slot (`slot-0`..`slot-4`), so bevy_markup's focus restore
//! keeps focus on the same slot across the re-render. Opening resumes at the last picked entry
//! (it becomes the top row, clamped to the end of the list) and focuses it; picking closes the
//! popup and returns focus to the toggle. With no options, the popup shows a placeholder and
//! isn't modal (so gamepad navigation can still reach the toggle to close it).

use crate::ui::markup;
use bevy::{
    asset::embedded_asset,
    input_focus::{FocusCause, InputFocus, InputFocusVisible},
    math::CompassOctant,
    prelude::*,
    ui::{ComputedUiTargetCamera, UiGlobalTransform},
};
use bevy_markup::prelude::*;
use serde_json::{Value, json};

/// How many rows a selector popup shows at once.
pub const SELECTOR_VISIBLE_ROWS: usize = 5;

/// `data-on-click` name of a selector's toggle element; its `data-with` names the selector
/// (`{"selector": "<key>"}`).
pub const TOGGLE_SIGNAL: &str = "selector.toggle";
/// `data-on-click` name of a popup row (`selector.html`); `data-with` carries the option index.
const PICK_SIGNAL: &str = "selector.pick";

/// Popup root width in UI px (also used to keep the popup inside its viewport).
const POPUP_WIDTH: f32 = 280.0;
/// Horizontal gap between the toggle element and the popup.
const POPUP_GAP: f32 = 8.0;
/// Above every menu surface, below tooltips (`markup`'s `TOOLTIP_Z`).
const POPUP_Z: i32 = 900;

pub struct SelectorPlugin;

impl Plugin for SelectorPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/selector.html");
        app.add_message::<SelectorPicked>()
            // PostUpdate, after `markup`'s Update-time `focus_on_pointer_click` (which focuses the
            // clicked row): returning focus to the toggle must win over it. Before `Render`, so a
            // popup spawned or re-paged here builds this frame.
            .add_systems(
                PostUpdate,
                (handle_selector_signals, refresh_open_popups)
                    .chain()
                    .before(HtmlUiSystems::Render),
            )
            .add_observer(page_on_navigate_edge);
    }
}

/// How an option's label is shown.
#[derive(Clone, Debug)]
pub enum SelectorLabel {
    /// Shown as-is (e.g. a language's own name in its own script).
    Literal(String),
    /// A Fluent message key, localized like any other template text.
    Fluent(String),
}

/// One selectable entry.
#[derive(Clone, Debug)]
pub struct SelectorOption {
    /// Reported in [`SelectorPicked::value`] when picked.
    pub value: String,
    pub label: SelectorLabel,
}

impl SelectorOption {
    pub fn literal(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: SelectorLabel::Literal(label.into()),
        }
    }

    pub fn fluent(value: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: SelectorLabel::Fluent(key.into()),
        }
    }
}

/// A selector's identity, options and last pick. Lives on its own (non-UI) entity, so it
/// survives rebuilds of the template holding its toggle element.
#[derive(Component, Debug)]
pub struct Selector {
    key: &'static str,
    options: Vec<SelectorOption>,
    selected: Option<usize>,
}

impl Selector {
    pub fn new(key: &'static str, options: Vec<SelectorOption>) -> Self {
        Self {
            key,
            options,
            selected: None,
        }
    }

    /// Starts as if `index` had been picked (e.g. the current language), so the first opening
    /// resumes there.
    pub fn with_selected(mut self, index: Option<usize>) -> Self {
        self.selected = index.filter(|&index| index < self.options.len());
        self
    }

    pub fn key(&self) -> &'static str {
        self.key
    }

    /// Replaces the options (an open popup re-renders); a last pick past the new end is dropped.
    pub fn set_options(&mut self, options: Vec<SelectorOption>) {
        self.selected = self.selected.filter(|&index| index < options.len());
        self.options = options;
    }
}

/// An option was picked in the popup of the [`Selector`] whose key is `selector`.
#[derive(Message, Clone, Debug)]
pub struct SelectorPicked {
    pub selector: &'static str,
    pub value: String,
}

/// An open popup root.
#[derive(Component)]
struct SelectorPopup {
    selector: Entity,
    /// The toggle element that opened it (focus returns here on pick).
    toggle: Entity,
    /// Index of the option shown in the top row.
    window_start: usize,
    /// Visible slot to focus once the popup (re)builds.
    focus_slot: Option<usize>,
}

fn max_window_start(len: usize) -> usize {
    len.saturating_sub(SELECTOR_VISIBLE_ROWS)
}

fn slot_id(slot: usize) -> String {
    format!("slot-{slot}")
}

fn slot_of(element: &HtmlElement) -> Option<usize> {
    element.id.as_deref()?.strip_prefix("slot-")?.parse().ok()
}

/// `selector.html`'s variables: the visible rows and one scrollbar cell per option (`true`
/// inside the window — the thumb).
fn popup_context(selector: &Selector, window_start: usize) -> TemplateContext {
    let window = window_start..window_start + SELECTOR_VISIBLE_ROWS;
    let rows: Vec<Value> = selector
        .options
        .iter()
        .enumerate()
        .skip(window_start)
        .take(SELECTOR_VISIBLE_ROWS)
        .enumerate()
        .map(|(slot, (index, option))| {
            let (label, localized) = match &option.label {
                SelectorLabel::Literal(text) => (text, false),
                SelectorLabel::Fluent(key) => (key, true),
            };
            json!({ "slot": slot, "index": index, "label": label, "localized": localized })
        })
        .collect();
    let scrollbar: Vec<bool> = (0..selector.options.len())
        .map(|index| window.contains(&index))
        .collect();
    TemplateContext::new()
        .with("rows", &rows)
        .with("empty", &rows.is_empty())
        .with("scrollbar", &scrollbar)
}

/// Opens/closes popups on `selector.toggle` and handles `selector.pick`.
#[allow(clippy::too_many_arguments)]
fn handle_selector_signals(
    mut signals: MessageReader<ElementSignal>,
    mut selectors: Query<(Entity, &mut Selector)>,
    popups: Query<(Entity, &SelectorPopup)>,
    elements: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        Option<&ComputedUiTargetCamera>,
    )>,
    cameras: Query<&Camera>,
    parents: Query<&ChildOf>,
    asset_server: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    mut picked: MessageWriter<SelectorPicked>,
    mut commands: Commands,
) {
    for signal in signals
        .read()
        .filter(|signal| signal.trigger == SignalTrigger::Click)
    {
        match signal.name.as_ref() {
            TOGGLE_SIGNAL => {
                let Some(key) = signal.payload.get("selector").and_then(Value::as_str) else {
                    warn!("{TOGGLE_SIGNAL} without a \"selector\" key in data-with");
                    continue;
                };
                let Some((selector_entity, selector)) =
                    selectors.iter().find(|(_, selector)| selector.key == key)
                else {
                    warn!("{TOGGLE_SIGNAL}: no selector \"{key}\"");
                    continue;
                };
                let was_open = popups
                    .iter()
                    .any(|(_, popup)| popup.selector == selector_entity);
                // One popup at a time.
                for (popup, _) in &popups {
                    commands.entity(popup).try_despawn();
                }
                if was_open {
                    focus.set(signal.target, FocusCause::Navigated);
                    continue;
                }
                let Ok((node, transform, target)) = elements.get(signal.target) else {
                    continue;
                };
                let camera = target.and_then(ComputedUiTargetCamera::get);
                // Physical px → UI px, the space `Node` offsets use (as `markup`'s tooltips).
                let scale = node.inverse_scale_factor;
                let size = node.size() * scale;
                let top_left = transform.translation * scale - size / 2.0;
                let mut left = top_left.x + size.x + POPUP_GAP;
                if let Some(viewport) = camera
                    .and_then(|camera| cameras.get(camera).ok())
                    .and_then(Camera::physical_viewport_size)
                {
                    left = left.min(viewport.x as f32 * scale - POPUP_WIDTH).max(0.0);
                }
                let resume = selector.selected.unwrap_or(0);
                let window_start = resume.min(max_window_start(selector.options.len()));
                let mut popup = commands.spawn((
                    SelectorPopup {
                        selector: selector_entity,
                        toggle: signal.target,
                        window_start,
                        focus_slot: Some(resume - window_start),
                    },
                    markup::template(&asset_server, "selector.html"),
                    popup_context(selector, window_start),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(top_left.y),
                        width: px(POPUP_WIDTH),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    GlobalZIndex(POPUP_Z),
                ));
                popup.observe(focus_built_popup).observe(scroll_popup);
                if !selector.options.is_empty() {
                    popup.insert(HtmlModal);
                }
                if let Some(camera) = camera {
                    popup.insert(UiTargetCamera(camera));
                }
            }
            PICK_SIGNAL => {
                let Some(index) = signal
                    .payload
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|index| index as usize)
                else {
                    continue;
                };
                let Some((popup_entity, popup)) = parents
                    .iter_ancestors(signal.target)
                    .find_map(|ancestor| popups.get(ancestor).ok())
                else {
                    continue;
                };
                let Ok((_, mut selector)) = selectors.get_mut(popup.selector) else {
                    continue;
                };
                let Some(option) = selector.options.get(index) else {
                    continue;
                };
                picked.write(SelectorPicked {
                    selector: selector.key,
                    value: option.value.clone(),
                });
                selector.selected = Some(index);
                commands.entity(popup_entity).try_despawn();
                focus.set(popup.toggle, FocusCause::Navigated);
            }
            _ => {}
        }
    }
}

/// Keeps open popups consistent: closes one whose selector or toggle element is gone (state
/// exit, owner rebuild) and re-renders one whose options changed.
fn refresh_open_popups(
    selectors: Query<Ref<Selector>>,
    toggles: Query<(), With<HtmlElement>>,
    mut popups: Query<(
        Entity,
        &mut SelectorPopup,
        &mut TemplateContext,
        Has<HtmlModal>,
    )>,
    mut commands: Commands,
) {
    for (entity, mut popup, mut context, modal) in &mut popups {
        let Ok(selector) = selectors.get(popup.selector) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        if !toggles.contains(popup.toggle) {
            commands.entity(entity).try_despawn();
            continue;
        }
        if !selector.is_changed() {
            continue;
        }
        let len = selector.options.len();
        if len > 0 && !modal {
            // Options arrived while open: become modal, focusing the resume entry.
            let resume = selector.selected.unwrap_or(0);
            popup.window_start = resume.min(max_window_start(len));
            popup.focus_slot = Some(resume - popup.window_start);
            commands.entity(entity).insert(HtmlModal);
        } else {
            popup.window_start = popup.window_start.min(max_window_start(len));
            if len == 0 && modal {
                commands.entity(entity).remove::<HtmlModal>();
            }
        }
        *context = popup_context(&selector, popup.window_start);
    }
}

/// Focuses the slot the popup asked for once its rows exist.
fn focus_built_popup(
    built: On<HtmlUiBuilt>,
    mut popups: Query<&mut SelectorPopup>,
    elements: HtmlElements,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(mut popup) = popups.get_mut(built.entity) else {
        return;
    };
    let Some(slot) = popup.focus_slot.take() else {
        return;
    };
    if let Some(row) = elements.by_id(built.entity, &slot_id(slot)) {
        focus.set(row, FocusCause::Navigated);
    }
}

/// Moves `popup`'s window to `window_start` and focuses `slot` after the re-render.
fn page_popup(
    popup: &mut SelectorPopup,
    context: &mut TemplateContext,
    selector: &Selector,
    window_start: usize,
    slot: usize,
) {
    popup.window_start = window_start;
    popup.focus_slot = Some(slot);
    *context = popup_context(selector, window_start);
}

/// Gamepad/keyboard: navigating up from the top row or down from the bottom row pages the
/// window by one entry, focus staying on that row.
fn page_on_navigate_edge(
    edge: On<FocusEdge>,
    parents: Query<&ChildOf>,
    elements: Query<&HtmlElement>,
    selectors: Query<&Selector>,
    mut popups: Query<(&mut SelectorPopup, &mut TemplateContext)>,
) {
    let Some(slot) = elements.get(edge.entity).ok().and_then(slot_of) else {
        return;
    };
    let Some(root) = parents
        .iter_ancestors(edge.entity)
        .find(|&ancestor| popups.contains(ancestor))
    else {
        return;
    };
    let Ok((mut popup, mut context)) = popups.get_mut(root) else {
        return;
    };
    let Ok(selector) = selectors.get(popup.selector) else {
        return;
    };
    let start = popup.window_start;
    let window_start = match edge.direction {
        CompassOctant::North if start > 0 => start - 1,
        CompassOctant::South if start < max_window_start(selector.options.len()) => start + 1,
        _ => return,
    };
    page_popup(&mut popup, &mut context, selector, window_start, slot);
}

/// Mouse wheel over the popup (picking events bubble to the root): one entry per notch, focus
/// following the direction — the bottom row when scrolling down, the top row when scrolling up.
fn scroll_popup(
    scroll: On<Pointer<Scroll>>,
    selectors: Query<&Selector>,
    mut popups: Query<(&mut SelectorPopup, &mut TemplateContext)>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let Ok((mut popup, mut context)) = popups.get_mut(scroll.entity) else {
        return;
    };
    let Ok(selector) = selectors.get(popup.selector) else {
        return;
    };
    let start = popup.window_start;
    let (window_start, slot) = if scroll.y < 0.0 && start < max_window_start(selector.options.len())
    {
        (start + 1, SELECTOR_VISIBLE_ROWS - 1)
    } else if scroll.y > 0.0 && start > 0 {
        (start - 1, 0)
    } else {
        return;
    };
    page_popup(&mut popup, &mut context, selector, window_start, slot);
    focus_visible.0 = true;
}
