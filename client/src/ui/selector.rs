//! A generic, reusable "selector" popup widget: a fixed-size paginated window over a
//! caller-supplied list of options (gamepad/keyboard/mouse-wheel navigation, a discrete
//! scrollbar, `AutoDirectionalNavigation` locked to the popup while open), firing a `UiSelected`
//! event when one is chosen rather than baking in any one domain's own selection logic. Built by
//! generalizing `ui.rs`'s original language-picker popup — see this module's own doc comments for
//! the design reasoning that carried over (paginated-window-over-continuous-scrolling, why
//! `AutoDirectionalNavigation` gets stripped from other buttons while a popup is open, etc.).
//!
//! # Using it
//!
//! Build one option per row with [`SelectorOption`] (a label plus a `payload` closure that
//! inserts whatever components the caller wants onto the slot entity that ends up showing it —
//! e.g. `LocaleOption` for a language, or a stub marker for a not-yet-real setting), spawn the
//! popup with [`selector_popup`] as a sibling of a plain toggle button (see `ui.rs`'s
//! `language_picker()`/`options_picker()` for the exact wrapping shape — a `position_type:
//! Relative` `Node` around `[toggle_button, selector_popup(options)]`), then react to
//! [`UiSelected`] in a system of your own that reads whichever payload component it's after off
//! `UiSelected.entity` — the same "resolve identity from the entity an event fires on" pattern
//! this project already uses elsewhere (`AttackAttempt`, the original `select_language`).
//!
//! There's no `Entity` back-reference wiring needed between the toggle button and its popup: this
//! module finds "my sibling popup" / "my owning popup" via plain hierarchy walks
//! (`iter_ancestors`/`iter_descendants`) at the point it's needed, rather than pre-resolving and
//! storing a reference at construction time — this project's own established convention for
//! hierarchy lookups (see `AGENTS.md`'s "Notable conventions" section), and it sidesteps needing
//! `bsn!`'s `#label`/`EntityTemplate` cross-reference mechanism, which this module's own
//! `payload` closures (see below) already can't use anyway.
//!
//! # Scope note
//!
//! The visible row count (`SELECTOR_VISIBLE_ROWS`) is a single module-level constant shared by
//! every selector instance, not a per-instance parameter — `bsn!`'s `Children [...]` needs a
//! literal list, so a genuinely per-instance row count would need either a dynamically-built
//! `SceneList` (unverified whether a plain `Vec<impl Scene>` implements it the way `bsn_list!`'s
//! macro-expanded lists do) or a real templated-props mechanism. Given every current use wants the
//! same 5 rows anyway, this was a deliberate scope cut rather than something worth the risk right
//! now — revisit if a consumer ever genuinely needs a different count.

use bevy::ecs::system::EntityCommands;
use bevy::feathers::{
    controls::FeathersButton,
    theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText},
    tokens,
};
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::auto_directional_navigation::AutoDirectionalNavigation;
use bevy::ui_widgets::Activate;

/// How many rows a selector popup shows at once — see this module's own "Scope note" doc comment
/// for why this is a single shared constant rather than a per-instance parameter.
pub const SELECTOR_VISIBLE_ROWS: usize = 5;

pub struct SelectorPlugin;

impl Plugin for SelectorPlugin {
    fn build(&self, app: &mut App) {
        // `toggle_selector`/`select_option` are deliberately *not* registered globally here — each
        // toggle button/row attaches them per-entity via bsn's `on(...)` at its own spawn site
        // (`selector_popup`/`selector_row`, and `ui.rs`'s `language_picker`/`options_picker`),
        // same as `scroll_selector` below. Registering both here *as well* was a real bug, not a
        // style choice: `Activate` reached two independent observer instances per press (the
        // per-entity one and this global one), each with its own separate debounce state, which is
        // exactly what defeated `is_duplicate_activate`'s dedup.
        app.add_systems(
            Update,
            (
                update_selector_slots,
                update_selector_scrollbar,
                update_selector_visibility,
                lock_navigation_while_selector_open,
            ),
        );
    }
}

/// Marks a selector popup's root panel entity. `pub` (unlike this module's other internal state
/// components) specifically so a caller can find "the `Selector` among my own children" to seed
/// its real options after spawning — see `selector_popup`'s doc comment for why that has to
/// happen as a separate step, and `set_selector_options`.
#[derive(Component, Clone, Default)]
pub struct Selector;

#[derive(Component, Default, Clone, Copy)]
pub(crate) struct SelectorOpen(bool);

/// Index into this panel's `SelectorEntries` of the entry shown in the topmost slot. `pub(crate)`
/// (unlike this module's other internal state) because `ui.rs`'s `on_ui_navigate` needs to query
/// it directly when calling `selector_navigate_fallback`.
#[derive(Component, Default, Clone, Copy)]
pub(crate) struct SelectorWindowStart(usize);

/// Absolute index of whichever option was last actually selected — `None` until the first
/// selection. Lets reopening resume where you left off instead of always starting at the top.
#[derive(Component, Default, Clone, Copy)]
pub(crate) struct SelectorLastSelected(Option<usize>);

/// One selectable row: its display label, and a closure that applies this option's payload to
/// whichever slot entity currently shows it. Not a plain `Bundle`: `Bundle` isn't object-safe, so
/// a caller-supplied list of heterogeneous option payloads (a locale here, a stub label
/// elsewhere) needs a closure instead of a trait object — the standard Rust workaround.
pub struct SelectorOption {
    pub label: String,
    pub payload: Box<dyn Fn(&mut EntityCommands) + Send + Sync>,
}

/// The full ordered list of options a selector panel pages through. A component (one per panel),
/// not a `Resource` — multiple independent selector instances (language, options, ...) need
/// independent state, and a `Resource` is a singleton. Deliberately *not* set via `bsn!`'s own
/// templated-value mechanism (`{expr}`, the way e.g. `SelectorSlot(index)` is): that requires
/// `Clone` (see this module's own hard-won discovery — every component embedded in a `bsn!` block
/// needs it, even bare marker structs), and `SelectorOption::payload`'s `Box<dyn Fn>` genuinely
/// can't be `Clone`. `selector_popup` spawns this empty (`#[derive(Default)]`, no `Clone` needed
/// for that path — constructing a fresh `Default` value never calls `.clone()`) and
/// `set_selector_options` fills it in afterward as a plain `Commands` insert.
/// `pub(crate)` for the same reason as `SelectorWindowStart` — `ui.rs`'s navigate-fallback call
/// needs to query it directly.
#[derive(Component, Default)]
pub(crate) struct SelectorEntries(Vec<SelectorOption>);

/// Overwrites a selector panel's option list — the only way to actually populate one, since
/// `bsn!`'s own templated-value mechanism can't hold `SelectorOption::payload`'s `Box<dyn Fn>`
/// (see `selector_popup`'s doc comment). Typical use: tag the *wrapper* node around
/// `[toggle_button, selector_popup()]` with your own domain-specific marker (e.g.
/// `LanguagePicker`), then in a system reacting to `Added<YourMarker>`, find the `Selector` among
/// that wrapper's own children and call this on it:
///
/// ```rust,ignore
/// fn seed_language_options(
///     mut commands: Commands,
///     wrappers: Query<&Children, Added<LanguagePicker>>,
///     panels: Query<Entity, With<selector::Selector>>,
/// ) {
///     for children in &wrappers {
///         if let Some(&panel) = children.iter().find(|&&e| panels.contains(e)) {
///             selector::set_selector_options(&mut commands, panel, language_options());
///         }
///     }
/// }
/// ```
pub fn set_selector_options(commands: &mut Commands, panel: Entity, options: Vec<SelectorOption>) {
    commands.entity(panel).insert(SelectorEntries(options));
}

/// Fired when a selector option is actually chosen (clicked, or Enter/gamepad-confirmed while
/// focused) — `entity` is the *slot* entity that was activated, matching this project's existing
/// "resolve identity from the entity an event fires on" convention. A downstream system reads
/// whatever payload component that option's `SelectorOption::payload` closure inserted onto it.
#[derive(EntityEvent, Clone)]
pub struct UiSelected {
    pub entity: Entity,
}

/// Tags one of a selector panel's `SELECTOR_VISIBLE_ROWS` fixed-position row entities. `0` is the
/// topmost slot. `update_selector_slots` is what actually gives a slot its content; the slot
/// entity itself never moves or gets despawned/respawned as the window scrolls. `pub(crate)` for
/// the same reason as `SelectorWindowStart` — `ui.rs`'s navigate-fallback call needs it.
#[derive(Component, Default, Clone, Copy)]
pub(crate) struct SelectorSlot(usize);

#[derive(Component, Clone, Default)]
struct SelectorScrollbarTrack;

#[derive(Component, Clone, Default)]
struct SelectorScrollbarThumb;

/// Attach to any other navigable entity (e.g. this project's own main-menu Connect/Quit buttons,
/// or another selector's own toggle button) that should lose `AutoDirectionalNavigation` while
/// *any* selector popup is open — see `lock_navigation_while_selector_open`'s doc comment.
#[derive(Component, Clone, Copy, Default)]
pub struct LockedWhileSelectorOpen;

/// Spawns a selector popup, initially with no options — call `set_selector_options` afterward to
/// actually populate it (typically from a system reacting to a domain-specific marker you've put
/// on the *wrapper* node around `[toggle_button, selector_popup()]`, then finding the `pub
/// Selector` among that wrapper's children — see `set_selector_options`'s own doc comment for a
/// worked example). Options can't be set inline here: `bsn!` requires every embedded component to
/// be `Clone` (confirmed the hard way — even bare marker structs need it), and
/// `SelectorOption::payload`'s `Box<dyn Fn>` genuinely can't be. Spawn this as a sibling of a
/// plain toggle button under a shared `position_type: Relative` wrapper — see this module's own
/// top-of-file doc comment for the exact shape.
pub fn selector_popup() -> impl Scene {
    bsn! {
        Selector
        SelectorOpen(false)
        SelectorWindowStart(0)
        SelectorLastSelected(None)
        Visibility::Hidden
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(210), // to the right of a 200px-wide toggle button + a small gap
            width: px(224),
            // Row, not Column: the rows box and the scrollbar sit side by side as ordinary flex
            // siblings, not an absolutely-positioned overlay reaching into reserved padding — a
            // plain flex child structurally can't render outside its container's content box, so
            // this can't overflow the panel regardless of exact sizing.
            flex_direction: FlexDirection::Row,
            column_gap: px(6),
            border: px(1),
            border_radius: px(3),
            padding: UiRect::all(px(6)),
        }
        ThemeBorderColor(tokens::GROUP_BODY_BORDER)
        ThemeBackgroundColor(tokens::WINDOW_BG)
        on(scroll_selector)
        Children [
            (
                // Plain, unstyled grouping box for the row slots — no background/border of its
                // own, just a `Column` flex container so the outer `Row` has one clean child for
                // "the rows" instead of splicing all the slots directly in alongside the
                // scrollbar.
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                }
                Children [
                    selector_row(0),
                    selector_row(1),
                    selector_row(2),
                    selector_row(3),
                    selector_row(4),
                ]
            ),
            (
                // A plain flex child, not absolutely positioned — its height comes for free from
                // the outer `Row`'s default `align_items: Stretch`, matching its only sibling
                // (the rows box) instead of needing manual top/bottom math.
                SelectorScrollbarTrack
                Node {
                    width: px(4),
                    border_radius: px(2),
                }
                ThemeBackgroundColor(tokens::SCROLLBAR_BG)
                Children [(
                    SelectorScrollbarThumb
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        right: px(0),
                        border_radius: px(2),
                    }
                    ThemeBackgroundColor(tokens::SCROLLBAR_THUMB)
                )]
            ),
        ]
    }
}

/// One of a selector popup's fixed-position row slots. Doesn't attach `LocalizedText`: a slot's
/// displayed label changes at runtime as the window scrolls (`update_selector_slots`), and
/// `LocalizedText`'s own sync system would fight a direct `Text` write on every unrelated
/// locale-resource change — same reasoning the original language-only version already settled on.
fn selector_row(index: usize) -> impl Scene {
    bsn! {
        @FeathersButton
        AutoDirectionalNavigation
        SelectorSlot(index)
        Node {
            width: px(200),
        }
        on(select_option)
        Children [(
            Text("")
            ThemedText
        )]
    }
}

/// Rebinds each of a selector panel's fixed row slots to whichever `SelectorEntries` entry
/// `SelectorWindowStart` currently maps it to. A slot whose mapped index is past the end of
/// `SelectorEntries` (only possible if a list ever shrinks below `SELECTOR_VISIBLE_ROWS` entries)
/// is hidden instead of left showing stale content.
fn update_selector_slots(
    selectors: Query<
        (Entity, &SelectorWindowStart, &SelectorEntries),
        Or<(Changed<SelectorWindowStart>, Changed<SelectorEntries>)>,
    >,
    children_query: Query<&Children>,
    mut slots: Query<(&SelectorSlot, &Children, &mut Visibility)>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
) {
    for (panel, window, entries) in &selectors {
        for descendant in children_query.iter_descendants(panel) {
            let Ok((slot, slot_children, mut visibility)) = slots.get_mut(descendant) else {
                continue;
            };
            match entries.0.get(window.0 + slot.0) {
                Some(option) => {
                    *visibility = Visibility::Inherited;
                    (option.payload)(&mut commands.entity(descendant));
                    for &child in slot_children {
                        if let Ok(mut text) = texts.get_mut(child) {
                            text.0 = option.label.clone();
                        }
                    }
                }
                None => *visibility = Visibility::Hidden,
            }
        }
    }
}

/// Sizes and positions each selector panel's scrollbar thumb from its own
/// `SelectorWindowStart`/`SelectorEntries` — standard scrollbar math (thumb height proportional
/// to visible/total, thumb position proportional to window start/max start), driven by whole-item
/// counts instead of pixel scroll ranges. Re-runs every frame rather than gating on `Changed<_>`:
/// cheap, and avoids depending on the track's `ComputedNode` already being valid on the exact
/// frame the window changes (layout can lag a frame behind a resource/component write).
fn update_selector_scrollbar(
    selectors: Query<(&SelectorWindowStart, &SelectorEntries)>,
    parents: Query<&ChildOf>,
    children_query: Query<&Children>,
    tracks: Query<(Entity, &ComputedNode), With<SelectorScrollbarTrack>>,
    mut thumbs: Query<&mut Node, With<SelectorScrollbarThumb>>,
) {
    for (track_entity, track_computed) in &tracks {
        let Some(panel) = parents
            .iter_ancestors(track_entity)
            .find(|entity| selectors.contains(*entity))
        else {
            continue;
        };
        let Ok((window, entries)) = selectors.get(panel) else {
            continue;
        };
        let Some(&thumb_entity) = children_query
            .get(track_entity)
            .ok()
            .and_then(|children| children.first())
        else {
            continue;
        };
        let Ok(mut thumb_node) = thumbs.get_mut(thumb_entity) else {
            continue;
        };
        let track_height = track_computed.size().y * track_computed.inverse_scale_factor;
        if track_height <= 0.0 {
            continue;
        }
        let total = entries.0.len().max(1);
        let visible_fraction = (SELECTOR_VISIBLE_ROWS as f32 / total as f32).min(1.0);
        let thumb_height = (track_height * visible_fraction).max(8.0);
        let max_start = total.saturating_sub(SELECTOR_VISIBLE_ROWS).max(1) as f32;
        let scroll_fraction = window.0 as f32 / max_start;
        let thumb_top = (track_height - thumb_height).max(0.0) * scroll_fraction;
        thumb_node.height = px(thumb_height);
        thumb_node.top = px(thumb_top);
    }
}

/// Pages a selector popup by one entry per wheel notch. Attached to the whole panel (bubbles up
/// from any descendant — `Pointer<Scroll>` propagates through the hierarchy the same as
/// `Pointer<Press>`/`Pointer<Click>`), not just the scrollbar's thin track: requiring the cursor
/// to sit precisely over a 4px-wide bar would be an unreasonably small target. Sign convention
/// matches `bevy_ui_widgets::scrollarea::scrollarea_on_scroll`'s own handling (positive
/// `scroll.y` = wheel up = earlier content). Focus follows the direction scrolled — scrolling
/// down moves it onto the bottommost visible slot (landing on the real last item once the window
/// can't advance further), scrolling up onto the topmost visible slot (landing on the real first
/// item at the start) — the same "focus follows the direction you're heading" idea
/// `selector_navigate_fallback` already uses for the keyboard/gamepad paging fallback at the
/// list's edges.
fn scroll_selector(
    mut scroll: On<Pointer<Scroll>>,
    parents: Query<&ChildOf>,
    mut selectors: Query<(&mut SelectorWindowStart, &SelectorEntries)>,
    children_query: Query<&Children>,
    slots: Query<&SelectorSlot>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    scroll.propagate(false);
    let Some(panel) = core::iter::once(scroll.entity)
        .chain(parents.iter_ancestors(scroll.entity))
        .find(|entity| selectors.contains(*entity))
    else {
        return;
    };
    let Ok((mut window, entries)) = selectors.get_mut(panel) else {
        return;
    };
    let max_start = entries.0.len().saturating_sub(SELECTOR_VISIBLE_ROWS);
    let target_slot_index = if scroll.y < 0.0 && window.0 < max_start {
        window.0 += 1;
        SELECTOR_VISIBLE_ROWS - 1
    } else if scroll.y > 0.0 && window.0 > 0 {
        window.0 -= 1;
        0
    } else {
        return;
    };
    if let Some(entity) = children_query
        .iter_descendants(panel)
        .find(|&entity| slots.get(entity).map(|slot| slot.0) == Ok(target_slot_index))
    {
        focus.set(entity, FocusCause::Navigated);
        focus_visible.0 = true;
    }
}

/// Toggles whichever selector popup is the *sibling* of the clicked button (see this module's
/// top-of-file doc comment for the expected `[toggle_button, selector_popup(...)]` wrapping
/// shape) — register via `on(selector::toggle_selector)` on a caller-built toggle button, same as
/// any other button handler in this project.
///
/// Opening also positions the paginated window and moves focus: reopens wherever
/// `SelectorLastSelected` last left off (`select_option` records it) — the selected entry becomes
/// the topmost slot again (clamped so the window never scrolls past the actual end of the list),
/// and that slot gets focus. The very first time a popup is ever opened, `SelectorLastSelected` is
/// still `None`, so this falls back to `window = 0` / slot `0` — always-topmost.
pub fn toggle_selector(
    activate: On<Activate>,
    parents: Query<&ChildOf>,
    children_query: Query<&Children>,
    mut selectors: Query<(
        &mut SelectorOpen,
        &mut SelectorWindowStart,
        &SelectorLastSelected,
        &SelectorEntries,
    )>,
    slots: Query<&SelectorSlot>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(&ChildOf(parent)) = parents.get(activate.entity) else {
        return;
    };
    let Ok(siblings) = children_query.get(parent) else {
        return;
    };
    let Some(panel) = siblings.iter().find(|&entity| selectors.contains(entity)) else {
        return;
    };
    let Ok((mut open, mut window, last_selected, entries)) = selectors.get_mut(panel) else {
        return;
    };
    open.0 = !open.0;
    info!("after toggle popup open = {}", open.0);
    if open.0 {
        let max_start = entries.0.len().saturating_sub(SELECTOR_VISIBLE_ROWS);
        let target_index = last_selected.0.unwrap_or(0);
        window.0 = target_index.min(max_start);
        let target_slot_index = target_index - window.0;
        if let Some(entity) = children_query
            .iter_descendants(panel)
            .find(|&entity| slots.get(entity).map(|slot| slot.0) == Ok(target_slot_index))
        {
            focus.set(entity, FocusCause::Navigated);
        }
    }
}

/// Handles a row actually being chosen: records it as the panel's `SelectorLastSelected`, closes
/// the popup, returns focus to the toggle button (the *other* child of the shared wrapper — see
/// this module's top-of-file doc comment for that shape), and fires `UiSelected` for whatever
/// domain-specific system wants to react to the actual choice (e.g. this project's own language
/// selection, reading `LocaleOption` off `UiSelected.entity`).
fn select_option(
    activate: On<Activate>,
    parents: Query<&ChildOf>,
    children_query: Query<&Children>,
    slots: Query<&SelectorSlot>,
    mut selectors: Query<(
        &mut SelectorOpen,
        &SelectorWindowStart,
        &mut SelectorLastSelected,
    )>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok(slot) = slots.get(activate.entity) else {
        return;
    };
    let Some(panel) = parents
        .iter_ancestors(activate.entity)
        .find(|entity| selectors.contains(*entity))
    else {
        return;
    };
    let Ok((mut open, window, mut last_selected)) = selectors.get_mut(panel) else {
        return;
    };
    last_selected.0 = Some(window.0 + slot.0);
    open.0 = false;
    // The toggle button is the *other* child of the panel's own parent wrapper — see this
    // module's top-of-file doc comment for that shape (`[toggle_button, selector_popup(...)]`,
    // exactly two children).
    if let Ok(&ChildOf(wrapper)) = parents.get(panel)
        && let Ok(siblings) = children_query.get(wrapper)
        && let Some(toggle_button) = siblings.iter().find(|&entity| entity != panel)
    {
        focus.set(toggle_button, FocusCause::Navigated);
    }
    commands.trigger(UiSelected {
        entity: activate.entity,
    });
}

/// While *any* selector popup is open, strips `AutoDirectionalNavigation` from every
/// `LockedWhileSelectorOpen`-tagged entity (re-inserting it once every popup is closed again), so
/// directional navigation is locked to the open popup's own rows and can't escape into the rest
/// of the menu. `AutoDirectionalNavigation` is explicitly documented as z-index/layer-agnostic —
/// it treats every entity that has the component as one flat set regardless of which UI "layer"
/// it's actually part of — and the component's own doc comment calls out "remove the component
/// when the layer is hidden" as the intended workaround for exactly this.
///
/// Shows/hides each selector panel to match its own `SelectorOpen` — `selector_popup()` spawns it
/// `Visibility::Hidden`, so this is what actually reveals it once toggled open (and re-hides it on
/// close, whether that came from `toggle_selector` or `select_option`).
fn update_selector_visibility(
    mut panels: Query<(&SelectorOpen, &mut Visibility), Changed<SelectorOpen>>,
) {
    for (open, mut visibility) in &mut panels {
        *visibility = if open.0 {
            info!("Selector visible");
            Visibility::Visible
        } else {
            info!("Selector hidden");
            Visibility::Hidden
        };
    }
}

fn lock_navigation_while_selector_open(
    changed: Query<(), Changed<SelectorOpen>>,
    all_open: Query<&SelectorOpen>,
    lockable: Query<Entity, With<LockedWhileSelectorOpen>>,
    mut commands: Commands,
) {
    if changed.is_empty() {
        return;
    }
    let any_open = all_open.iter().any(|open| open.0);
    for entity in &lockable {
        if any_open {
            commands
                .entity(entity)
                .remove::<AutoDirectionalNavigation>();
        } else {
            commands
                .entity(entity)
                .insert(AutoDirectionalNavigation::default());
        }
    }
}

/// Attempts the language-popup-style paging fallback for a failed directional-navigation attempt:
/// if `focus` is a selector's topmost/bottommost slot specifically, pages that selector's window
/// by one entry instead of leaving the caller to just treat it as a dead end. Returns `true` if it
/// handled the step (so the caller can flip `InputFocusVisible`, same as a normal successful
/// navigate) — every other "no neighbor" case (a plain menu button hitting the edge of the whole
/// menu) should be left as a real stop by the caller.
pub fn selector_navigate_fallback(
    octant: CompassOctant,
    focus: Entity,
    parents: &Query<&ChildOf>,
    slots: &Query<&SelectorSlot>,
    selectors: &mut Query<(&mut SelectorWindowStart, &SelectorEntries)>,
) -> bool {
    let Ok(slot) = slots.get(focus) else {
        return false;
    };
    let Some(panel) = parents
        .iter_ancestors(focus)
        .find(|entity| selectors.contains(*entity))
    else {
        return false;
    };
    let Ok((mut window, entries)) = selectors.get_mut(panel) else {
        return false;
    };
    match octant {
        CompassOctant::North if slot.0 == 0 && window.0 > 0 => {
            window.0 -= 1;
            true
        }
        CompassOctant::South
            if slot.0 == SELECTOR_VISIBLE_ROWS - 1
                && window.0 + SELECTOR_VISIBLE_ROWS < entries.0.len() =>
        {
            window.0 += 1;
            true
        }
        _ => false,
    }
}
