use crate::add_observers_run_if;
use crate::assets::collections::CommonAssets;
use crate::controls::actions::{UiConfirm, UiConfirmEnter, UiNavigate};
use crate::events::Connect;
use crate::ui::hud::HudPlugin;
use crate::ui::localization::LocalizedText;
use crate::ui::quad_panel::quad_panel;
use crate::ui::selector;
use crate::ui::widgets::{Activate as LegacyActivate, Tooltip, WidgetsPlugin};
use bevy::{
    ecs::system::EntityCommands,
    feathers::{
        controls::{ButtonVariant, FeathersButton},
        theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText},
        tokens,
    },
    input_focus::{
        AutoFocus, InputFocus, InputFocusVisible,
        directional_navigation::DirectionalNavigationPlugin,
    },
    math::CompassOctant,
    prelude::*,
    ui::auto_directional_navigation::{AutoDirectionalNavigation, AutoDirectionalNavigator},
    ui_widgets::Activate,
};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_fluent::prelude::Locale;
use bevy_xr_utils::tracking_utils::XrTrackedLeftGrip;
use chill_bevy_console::console_closed;
use p19_shared::game_state::{GameState, VRState};
use std::f32::consts::FRAC_PI_2;
use unic_langid::langid;

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            WidgetsPlugin,
            HudPlugin,
            DirectionalNavigationPlugin,
            selector::SelectorPlugin,
        ));
        app.add_input_context::<MenuControls>();
        app.init_resource::<UiNavigateHold>();
        app.add_systems(OnEnter(GameState::MainMenu), spawn_menu_controls);
        app.add_systems(
            Update,
            (
                seed_language_options,
                seed_options_menu,
                repeat_ui_navigate_while_held.run_if(console_closed),
                spawn_vr_main_menu_wrist_panel
                    .run_if(in_state(GameState::MainMenu).and_then(in_state(VRState::VR))),
            ),
        );
        add_observers_run_if!(
            app,
            console_closed,
            on_ui_navigate,
            on_ui_confirm,
            on_ui_confirm_enter
        );
        app.add_observer(on_ui_navigate_complete);
        app.add_observer(apply_selected_language);
        app.add_observer(apply_selected_option);
    }
}

/// The `bevy_enhanced_input` context for gamepad/keyboard directional-navigation UI — see
/// `menu_controls()`. Lives on its own entity, separate from `controls::PlayerControls` (which
/// only exists once a player character has spawned — see `player_character.rs`, and stays focused
/// on gameplay actions, not UI navigation). Two independent call sites spawn one of these, each
/// tagged with its own lifetime-matching `DespawnOnExit`: the main menu itself (below, scoped to
/// `GameState::MainMenu`) and `modal_menu.rs`'s pause menu (scoped to `ModalMenuState::Open`) —
/// they're never both alive at once, so reusing the same context type/bindings for both is safe.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub(crate) struct MenuControls;

fn spawn_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(GameState::MainMenu)));
}

pub(crate) fn menu_controls() -> impl Bundle {
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
                    kind: DeadZoneKind::Radial, // circular; correct for a stick
                    lower_threshold: 0.15,      // below this magnitude → zero
                    upper_threshold: 1.0,       // above this → clamped to 1, rescaled between
                },
                Bindings::spawn(Axial::left_stick()),
            ));
            // `require_reset` on both: without it, confirming a button that causes `MenuControls`
            // itself to despawn-and-respawn on the very same input (e.g. `modal_menu.rs`'s "Main
            // Menu" button, closing the modal and dropping straight into a fresh main-menu
            // `MenuControls` with `Play` auto-focused) reads the still-held South/Enter as a
            // brand-new press on the new context's own `Press` condition (a fresh component, so
            // it has no memory of the input already being down) and immediately activates
            // whatever's newly focused. `require_reset` is `bevy_enhanced_input`'s built-in fix
            // for exactly this: it tracks the physical binding globally (not per-context), so a
            // still-held button stays ignored across a context respawn until it's actually
            // released. See `ActionSettings::require_reset`'s doc comment.
            //
            // Two separate actions, not one bound to both inputs — see `UiConfirm`'s own doc
            // comment for why: `on_ui_confirm`/`on_ui_confirm_enter` need to react differently.
            context.spawn((
                Action::<UiConfirm>::new(),
                ActionSettings {
                    require_reset: true,
                    ..default()
                },
                Press::new(1.0),
                bindings![GamepadButton::South],
            ));
            context.spawn((
                Action::<UiConfirmEnter>::new(),
                ActionSettings {
                    require_reset: true,
                    ..default()
                },
                Press::new(1.0),
                bindings![KeyCode::Enter],
            ));
        })),
    )
}

/// Delay, in seconds, before a held `UiNavigate` direction (gamepad stick/D-pad, keyboard arrows)
/// starts auto-repeating — see `UiNavigateHold`/`repeat_ui_navigate_while_held`.
const UI_NAVIGATE_HOLD_DELAY: f32 = 0.4;

/// Interval, in seconds, between auto-repeat steps once past `UI_NAVIGATE_HOLD_DELAY` —
/// deliberately much shorter than the initial delay, for "quickly scrolling" once it kicks in.
const UI_NAVIGATE_REPEAT_INTERVAL: f32 = 0.08;

/// Tracks a currently-held `UiNavigate` direction for press-and-hold auto-repeat. `None` while
/// nothing is held. Seeded by `on_ui_navigate` on the initial press (with `next_repeat` set to
/// `UI_NAVIGATE_HOLD_DELAY`), advanced and consumed by `repeat_ui_navigate_while_held`, and
/// cleared by `on_ui_navigate_complete` on release. A resource rather than `Local` state on either
/// system, since `Start`/`Complete` (observers) and the repeat timer (a plain `Update` system)
/// need to share it across three separate systems.
#[derive(Resource, Default)]
struct UiNavigateHold {
    direction: Option<CompassOctant>,
    /// Seconds remaining until the next auto-repeat step.
    next_repeat: f32,
}

/// Moves `InputFocus` one step in `octant`, including a `selector` popup's paginated-window
/// fallback (`selector::selector_navigate_fallback`) — shared between `on_ui_navigate` (the
/// initial press) and `repeat_ui_navigate_while_held` (auto-repeat while held) so both go through
/// identical logic.
///
/// Also sets `InputFocusVisible`, same as `bevy_input_focus`'s own `handle_tab_navigation` does
/// for literal Tab presses — confirmed by tracing `bevy_feathers::focus`'s `FocusIndicator`
/// system: it gates the focus-ring outline on `InputFocusVisible`, which is otherwise *only* ever
/// flipped `true` by Tab-key handling (and `false` by a mouse click) inside `bevy_input_focus`
/// itself. Nothing about `AutoDirectionalNavigator`-driven navigation (this project's own gamepad/
/// arrow-key path) touches it, so without this, focus genuinely moves — confirmed by logging
/// `InputFocus` across a whole traversal — but the ring never renders, reading as "navigation
/// doesn't work" even though it does.
fn navigate_step(
    octant: CompassOctant,
    navigator: &mut AutoDirectionalNavigator,
    focus_visible: &mut InputFocusVisible,
    parents: &Query<&ChildOf>,
    slots: &Query<&selector::SelectorSlot>,
    selectors: &mut Query<(
        &mut selector::SelectorWindowStart,
        &selector::SelectorEntries,
    )>,
) {
    if navigator.navigate(octant).is_ok() {
        focus_visible.0 = true;
        return;
    }
    let Some(focus) = navigator.input_focus() else {
        return;
    };
    if selector::selector_navigate_fallback(octant, focus, parents, slots, selectors) {
        focus_visible.0 = true;
    }
}

fn on_ui_navigate(
    navigate: On<Start<UiNavigate>>,
    mut navigator: AutoDirectionalNavigator,
    mut focus_visible: ResMut<InputFocusVisible>,
    parents: Query<&ChildOf>,
    slots: Query<&selector::SelectorSlot>,
    mut selectors: Query<(
        &mut selector::SelectorWindowStart,
        &selector::SelectorEntries,
    )>,
    mut hold: ResMut<UiNavigateHold>,
) {
    let Ok(direction) = Dir2::new(navigate.value) else {
        return;
    };
    let octant: CompassOctant = direction.into();
    navigate_step(
        octant,
        &mut navigator,
        &mut focus_visible,
        &parents,
        &slots,
        &mut selectors,
    );
    hold.direction = Some(octant);
    hold.next_repeat = UI_NAVIGATE_HOLD_DELAY;
}

/// Clears the held-direction state on release. Deliberately registered *ungated* (see
/// `main.rs`'s `add_observers_run_if!` convention doc comment) — if this were gated behind
/// `console_closed` and the console opened while a direction was held, the `Complete` event could
/// fire while suppressed, leaving `UiNavigateHold` stuck as `Some` and the repeat system
/// spuriously auto-repeating navigation the player already released.
fn on_ui_navigate_complete(_complete: On<Complete<UiNavigate>>, mut hold: ResMut<UiNavigateHold>) {
    hold.direction = None;
}

/// Auto-repeats `UiNavigateHold`'s currently-held direction after `UI_NAVIGATE_HOLD_DELAY`, then
/// every `UI_NAVIGATE_REPEAT_INTERVAL` — "hold down/up to quickly scroll" (a selector popup in
/// particular, though this applies to any `AutoDirectionalNavigation` target). A plain `Update`
/// system rather than reacting to `Fire<UiNavigate>` (which would fire every frame the input
/// stays active): the hold/release edges alone (`on_ui_navigate`/`on_ui_navigate_complete`) are
/// enough to know *whether* a direction is held, and timing the repeats here keeps that timing
/// logic in one place rather than re-deriving it from a continuous stream of `Fire` events.
fn repeat_ui_navigate_while_held(
    time: Res<Time>,
    mut hold: ResMut<UiNavigateHold>,
    mut navigator: AutoDirectionalNavigator,
    mut focus_visible: ResMut<InputFocusVisible>,
    parents: Query<&ChildOf>,
    slots: Query<&selector::SelectorSlot>,
    mut selectors: Query<(
        &mut selector::SelectorWindowStart,
        &selector::SelectorEntries,
    )>,
) {
    let Some(octant) = hold.direction else {
        return;
    };
    hold.next_repeat -= time.delta_secs();
    if hold.next_repeat > 0.0 {
        return;
    }
    navigate_step(
        octant,
        &mut navigator,
        &mut focus_visible,
        &parents,
        &slots,
        &mut selectors,
    );
    hold.next_repeat = UI_NAVIGATE_REPEAT_INTERVAL;
}

/// "Presses" whichever UI element currently holds `InputFocus`, for gamepad South specifically —
/// see `on_ui_confirm_enter` for the literal-Enter counterpart and why they're two separate
/// handlers now, not one. Fires both `widgets::Activate` (`LegacyActivate` here, for the
/// hand-rolled `widgets::button()` used by the HUD and the pause modal) and `bevy::ui_widgets`'s
/// own `Activate` (for the `FeathersButton`-based main menu/lobby/selector popups) — safe to fire
/// both unconditionally here: `bevy_ui_widgets` has no native gamepad handling at all (confirmed
/// by reading `bevy_ui_widgets::button`'s source — it only reacts to `KeyCode::Enter`/`Space` and
/// pointer events), so there's no second path for a gamepad press to collide with.
fn on_ui_confirm(_confirm: On<Start<UiConfirm>>, focus: Res<InputFocus>, mut commands: Commands) {
    if let Some(entity) = focus.get() {
        commands.trigger(LegacyActivate { entity });
        commands.trigger(Activate { entity });
    }
}

/// The literal-Enter counterpart to `on_ui_confirm` — fires only `LegacyActivate`, deliberately
/// *not* the real `bevy::ui_widgets::Activate`. `bevy_ui_widgets`'s own `button_on_key_event`
/// already reacts to a focused `FeathersButton`'s Enter/Space natively, so synthesizing a second
/// real `Activate` here double-fired it on every Enter press. That used to be harmless for a
/// plain toggle (the same entity just re-ran its own idempotent handler a second time), but is a
/// real bug for `selector::toggle_selector` specifically: it moves `InputFocus` into the popup as
/// a side effect of the *first* fire, so the *second*, native fire — which reads focus fresh —
/// landed on the row that focus had just moved to instead of re-hitting the toggle button,
/// immediately selecting and closing the popup on the very press that opened it. Confirmed via
/// user testing: one Enter press on "Options" opened the popup, then that same press logged a
/// stub option as selected and closed it again. `LegacyActivate` still needs firing
/// unconditionally here — the hand-rolled `widgets::button()` (HUD, pause modal) carries none of
/// `bevy_ui_widgets`'s own marker components, so it never receives that native path regardless of
/// which key was pressed, and still needs this bridge for Enter to do anything at all.
fn on_ui_confirm_enter(
    _confirm: On<Start<UiConfirmEnter>>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if let Some(entity) = focus.get() {
        commands.trigger(LegacyActivate { entity });
    }
}

/// A real system (not the usual `some_scene.spawn()` adapter, which only works for a zero-arg
/// `Fn() -> impl SceneList`) since `main_menu()`'s `WorldAssetRoot` needs `Res<CommonAssets>` —
/// see `modal_menu.rs`'s `spawn_modal_menu` for the same pattern.
pub fn spawn_main_menu(mut commands: Commands, common_assets: Res<CommonAssets>) {
    commands.spawn_scene_list(bsn_list![main_menu(&common_assets)]);
}

fn main_menu(common_assets: &CommonAssets) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Start,
        }
        Children [ main_menu_buttons() ]
        WorldAssetRoot({common_assets.menu_background.clone()})
        DespawnOnExit::<GameState>(GameState::MainMenu)
    }
}

/// The actual Play/Options/Credits/Quit button panel — factored out of `main_menu()` so
/// `spawn_vr_main_menu_wrist_panel` can put the exact same UI (content, handlers, tooltips) on a
/// VR wrist-mounted `quad_panel`, not just a re-styled lookalike. Deliberately *not* including
/// `main_menu()`'s fullscreen `Node`/`WorldAssetRoot` background — those only make sense for the
/// desktop window, not a small texture on someone's wrist.
///
/// Uses `bevy::feathers` (`FeathersButton`, theme tokens) in place of this project's own
/// hand-rolled `widgets::panel()`/`widgets::button()`. Hover/press/disabled color states, the
/// focus ring, and the pointer cursor all come from feathers for free (`bevy_feathers::controls::
/// button`'s `update_button_styles` + `focus::FocusIndicator`) instead of this project's own
/// `hover_button`/`out_button`/`update_button_focus`. `widgets.rs` itself is untouched — `hud.rs`
/// and `modal_menu.rs` still use it — so this only affects the main menu (and its VR wrist-panel
/// copy). `AutoDirectionalNavigation` still needs adding by hand per button: `FeathersButton`
/// doesn't include it, since gamepad/keyboard D-pad navigation (`MenuControls`, above) is this
/// project's own thing, not a `bevy_feathers` concept.
///
/// "Options" and "Language" are both `selector` popups (`client/src/ui/selector.rs`) now, not
/// plain buttons — see `options_picker()`/`language_picker()`.
pub(crate) fn main_menu_buttons() -> impl Scene {
    bsn! {
        Node {
            width: px(400),
            height: px(400),
            border: px(1),
            border_radius: px(3),
            margin: UiRect::axes(px(50), px(50)),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: px(10),
        }
        ThemeBorderColor(tokens::GROUP_BODY_BORDER)
        ThemeBackgroundColor(tokens::WINDOW_BG)
        Children [
            (
                menu_button("main-menu-connect", ButtonVariant::Primary)
                selector::LockedWhileSelectorOpen
                Tooltip::new("main-menu-connect-tooltip")
                AutoFocus
                on(connect_button)
            ),
            options_picker(),
            (
                menu_button("main-menu-credits", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                Tooltip::new("main-menu-credits-tooltip")
                on(stub_button)
            ),
            (
                menu_button("main-menu-quit", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                Tooltip::new("main-menu-quit-tooltip")
                on(quit_button)
            ),
            language_picker(),
        ]
    }
}

/// A `FeathersButton` sized/labeled for this menu — `label_key` is a Fluent message key (see
/// `assets/locales/`), routed through this project's own `LocalizedText` exactly like the old
/// `widgets::button()` did (feathers has no localization concept of its own). `ThemedText` is
/// what makes the label actually pick up `FeathersButton`'s themed text color/font — it's a
/// propagation target, not automatic (see `bevy_feathers::theme`'s `HierarchyPropagatePlugin`
/// registrations). Only `width` is overridden on top of `@FeathersButton`'s own `Node` (mirrors
/// `bevy_feathers::controls::button::FeathersToolButton`'s identical partial-override shape) —
/// height is left at feathers' own `size::ROW_HEIGHT` rather than forcing the old fixed 50px.
pub fn menu_button(label_key: &'static str, variant: ButtonVariant) -> impl Scene {
    bsn! {
        @FeathersButton {
            @variant: {variant},
        }
        AutoDirectionalNavigation
        Node {
            width: px(200),
        }
        Children [(
            Text(label_key)
            LocalizedText(label_key)
            ThemedText
        )]
    }
}

/// Which locale a language-selector row switches to (see `apply_selected_language`) — inserted
/// onto whichever slot entity currently shows a given language by that option's own
/// `selector::SelectorOption::payload` closure (see `language_options`), not set once at spawn
/// time: `client/src/ui/selector.rs`'s slots are fixed, reused entities whose content gets rebound
/// as its paginated window scrolls.
#[derive(Component, Clone, Default)]
struct LocaleOption(unic_langid::LanguageIdentifier);

/// Tags the wrapper around `[toggle button, selector popup]` for the language picker, so
/// `seed_language_options` can find *this* selector specifically (as opposed to
/// `options_picker()`'s) to hand it its real option list — see `selector::set_selector_options`'s
/// own doc comment for why seeding has to be a separate step from spawning.
#[derive(Component, Clone, Default)]
struct LanguagePicker;

/// The "Language" button plus its (initially empty/hidden) selector popup. `position_type:
/// Relative` on the wrapping `Node` is what lets the popup's own `position_type: Absolute` anchor
/// directly to the right of the button instead of relative to the whole screen — see
/// `selector::selector_popup`'s own doc comment for the exact positioning.
fn language_picker() -> impl Scene {
    bsn! {
        LanguagePicker
        Node {
            position_type: PositionType::Relative,
        }
        Children [
            (
                menu_button("main-menu-language", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                on(selector::toggle_selector)
            ),
            selector::selector_popup(),
        ]
    }
}

/// The full set of selectable languages. Each option's label is the language's own name in its
/// own script (`"English"`, `"Русский"`, `"日本語"`), deliberately *not* run through a real
/// localization key (`selector`'s row slots skip `LocalizedText` entirely, for the same reason
/// the original language-only popup did — a slot's label changes at runtime as the window
/// scrolls, and `LocalizedText`'s own sync system would fight that). A language picker should
/// show every option in its own language regardless of which language is currently active, not
/// translate "日本語" into whatever's selected now.
///
/// `ja-JP` in particular exists to exercise CJK rendering — see `client/assets/locales/ja-JP/`'s
/// real translations. Worth knowing before trusting what it looks like: the client's own UI font
/// (`CommonAssets.serif_font`, IBM Plex Serif) covers Latin/Cyrillic only, so this relies on
/// `bevy`'s `system_font_discovery` feature (added to the workspace `Cargo.toml` alongside this)
/// falling back to a CJK-capable font already installed on the machine running the client — real,
/// but machine-dependent verification, not a shipped-game font solution. Bundling an actual CJK
/// font asset is the real fix if this needs to work on a machine without one installed.
fn language_options() -> Vec<selector::SelectorOption> {
    let mut options = vec![];
    options.push(selector::SelectorOption {
        label: "English".to_string(),
        payload: Box::new(|entity: &mut EntityCommands| {
            entity.insert(LocaleOption(langid!("en-US")));
        }),
    });
    options.push(selector::SelectorOption {
        label: "Русский".to_string(),
        payload: Box::new(|entity: &mut EntityCommands| {
            entity.insert(LocaleOption(langid!("ru-RU")));
        }),
    });
    options.push(selector::SelectorOption {
        label: "日本語".to_string(),
        payload: Box::new(|entity: &mut EntityCommands| {
            entity.insert(LocaleOption(langid!("ja-JP")));
        }),
    });
    options
}

/// Hands the language selector its real option list once it exists — see
/// `selector::set_selector_options`'s own doc comment for why this has to be a separate,
/// post-spawn step rather than a parameter to `selector::selector_popup()` directly.
fn seed_language_options(
    mut commands: Commands,
    wrappers: Query<&Children, Added<LanguagePicker>>,
    panels: Query<Entity, With<selector::Selector>>,
) {
    for children in &wrappers {
        if let Some(panel) = children.iter().find(|&entity| panels.contains(entity)) {
            selector::set_selector_options(&mut commands, panel, language_options());
        }
    }
}

/// Reacts to a language actually being picked — reads `LocaleOption` straight off
/// `selected.entity`, same "resolve identity from the entity an event fires on" pattern the old,
/// language-specific `select_language` already used, just now behind `selector::UiSelected`
/// instead of a bespoke `Activate` handler. No-ops for any other selector's `UiSelected` (e.g.
/// `options_picker()`'s stub rows, which carry `OptionChoice`, not `LocaleOption`) — the payload
/// component itself is what disambiguates which selector this event came from.
fn apply_selected_language(
    selected: On<selector::UiSelected>,
    options: Query<&LocaleOption>,
    mut locale: ResMut<Locale>,
) {
    if let Ok(option) = options.get(selected.entity) {
        locale.requested = option.0.clone();
    }
}

/// Tags the wrapper around `[toggle button, selector popup]` for the stub "Options" picker — see
/// `LanguagePicker`'s doc comment for why this exists (letting `seed_options_menu` find *this*
/// selector specifically).
#[derive(Component, Clone, Default)]
struct OptionsPicker;

/// The "Options" button plus its (initially empty/hidden) selector popup — the same
/// `selector::selector_popup()` widget `language_picker()` uses, with stub content instead of a
/// real setting (there isn't one yet — see `stub_options`).
fn options_picker() -> impl Scene {
    bsn! {
        OptionsPicker
        Node {
            position_type: PositionType::Relative,
        }
        Children [
            (
                menu_button("main-menu-options", ButtonVariant::default())
                selector::LockedWhileSelectorOpen
                Tooltip::new("main-menu-options-tooltip")
                on(selector::toggle_selector)
            ),
            selector::selector_popup(),
        ]
    }
}

/// Payload for the stub "Options" selector's rows — just a display label for now
/// (`apply_selected_option` only logs it), not a real setting. Exists mainly to prove
/// `selector.rs`'s generic widget actually works for something other than `LocaleOption` — swap
/// for real settings once there are any.
#[derive(Component, Clone, Default)]
struct OptionChoice(String);

fn stub_options() -> Vec<selector::SelectorOption> {
    [
        "Stub Option A",
        "Stub Option B",
        "Stub Option C",
        "Stub Option D",
        "Stub Option E",
        "Stub Option F",
    ]
    .into_iter()
    .map(|label| selector::SelectorOption {
        label: label.to_string(),
        payload: Box::new(move |entity: &mut EntityCommands| {
            entity.insert(OptionChoice(label.to_string()));
        }),
    })
    .collect()
}

/// Hands the "Options" selector its stub option list once it exists — see
/// `seed_language_options`'s identical shape and `selector::set_selector_options`'s own doc
/// comment for why this has to be a separate, post-spawn step.
fn seed_options_menu(
    mut commands: Commands,
    wrappers: Query<&Children, Added<OptionsPicker>>,
    panels: Query<Entity, With<selector::Selector>>,
) {
    for children in &wrappers {
        if let Some(panel) = children.iter().find(|&entity| panels.contains(entity)) {
            selector::set_selector_options(&mut commands, panel, stub_options());
        }
    }
}

/// "Not implemented yet" — matches `stub_button`'s existing spirit, just naming which stub option
/// was actually picked. No-ops for any other selector's `UiSelected` (see
/// `apply_selected_language`'s identical reasoning).
fn apply_selected_option(selected: On<selector::UiSelected>, options: Query<&OptionChoice>) {
    if let Ok(choice) = options.get(selected.entity) {
        info!("selected option: {} (not implemented yet)", choice.0);
    }
}

const WRIST_PANEL_SIZE: f32 = 0.16;
const WRIST_PANEL_TEXTURE_SIZE: u32 = 420;

/// Spawns the exact same `main_menu_buttons()` UI, on a `quad_panel` attached to the left
/// controller's tracked grip pose, so VR players can use the main menu without a desktop mouse —
/// the offset/rotation here is a rough guess at "resting on the back of the wrist, angled up
/// toward the face," not something verified in a headset (the vendored `bevy_xr_utils` fork this
/// project uses only tracks grip pose, with no documented orientation convention — see
/// `vr_controllers`'s module doc comment); nudge it if it reads wrong on real hardware.
/// Marks the spawned wrist panel so `spawn_vr_main_menu_wrist_panel` doesn't spawn a second one —
/// see that system's doc comment for why it has to poll rather than spawn once on `OnEnter`.
#[derive(Component)]
struct VrMainMenuWristPanel;

/// Runs every frame while `GameState::MainMenu` and `VRState::VR`, rather than once on
/// `OnEnter(GameState::MainMenu)` — confirmed by testing, not theory: `OnEnter(MainMenu)` fires on
/// literally the first frame (`MainMenu` is `GameState`'s `#[default]`), which is long before the
/// real OpenXR session handshake has had time to run (that takes observable wall-clock time — see
/// the `IDLE`/`READY`/`SYNCHRONIZED`/.../`FOCUSED` log lines `bevy_mod_openxr` prints as it
/// progresses). `vr_controllers::spawn_controller_cubes` (which creates the
/// `XrTrackedLeftGrip`-marked entity this parents onto) is itself a one-shot `Startup` system, so
/// if *it* also runs before the session/`XrTrackingRoot` exist, that entity never gets created at
/// all — `OnEnter(MainMenu)`-based spawning here found zero matching entities every time and
/// silently never spawned anything. Polling instead means this simply tries again next frame until
/// the grip entity exists, then spawns exactly once (`VrMainMenuWristPanel`) and leaves itself a
/// no-op afterward.
fn spawn_vr_main_menu_wrist_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // common_assets: Res<CommonAssets>,
    left_grip: Query<Entity, With<XrTrackedLeftGrip>>,
    existing: Query<(), With<VrMainMenuWristPanel>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(left_grip) = left_grip.single() else {
        return; // XR tracking hasn't come up yet — try again next frame
    };

    let panel = quad_panel(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        WRIST_PANEL_SIZE,
        WRIST_PANEL_SIZE,
        WRIST_PANEL_TEXTURE_SIZE,
        WRIST_PANEL_TEXTURE_SIZE,
        main_menu_buttons(),
    );
    commands.spawn((
        panel,
        VrMainMenuWristPanel,
        // 0.12m clears `vr_controllers::spawn_controller_cubes`'s debug cube (a
        // `Cuboid::new(0.08, 0.08, 0.12)` centered on this same grip pose) regardless of which way
        // "up" actually turns out to be for it, so the panel doesn't render half-clipped inside
        // that cube even if this offset direction guess is wrong. `quad_panel`'s material also
        // renders both sides for the same reason (see its own doc comment) — between the two,
        // this should be visible somewhere near the hand even before the exact angle is verified.
        Transform::from_xyz(0.0, 0.12, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ChildOf(left_grip),
        DespawnOnExit(GameState::MainMenu),
    ));
}

fn connect_button(_event: On<Activate>, mut commands: Commands) {
    commands.trigger(Connect);
}

/// "Credits" — a stub button that exists to be navigable, not functional yet.
fn stub_button(_event: On<Activate>) {
    info!("not implemented yet");
}

fn quit_button(_event: On<Activate>, mut commands: Commands) {
    commands.write_message(AppExit::Success);
}
