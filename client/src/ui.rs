use crate::actions::{UiConfirm, UiNavigate};
use crate::add_observers_run_if;
use crate::assets::CommonAssets;
use crate::events::LoadLevel;
use crate::game_state::{GameState, VRState};
use crate::hud::HudPlugin;
use crate::networking::DefaultLevel;
use crate::quad_panel::quad_panel;
use crate::widgets::{Activate, Tooltip, WidgetsPlugin, button, panel};
use bevy::{
    input_focus::{AutoFocus, InputFocus, directional_navigation::DirectionalNavigationPlugin},
    prelude::*,
    ui::auto_directional_navigation::AutoDirectionalNavigator,
};
use bevy_enhanced_input::prelude::{Press, *};
use bevy_fluent::prelude::Locale;
use bevy_xr_utils::tracking_utils::XrTrackedLeftGrip;
use chill_bevy_console::console_closed;
use std::f32::consts::FRAC_PI_2;
use unic_langid::{LanguageIdentifier, langid};

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((WidgetsPlugin, HudPlugin, DirectionalNavigationPlugin));
        app.add_input_context::<MenuControls>();
        app.init_resource::<LanguageMenuOpen>();
        app.add_systems(
            OnEnter(GameState::MainMenu),
            (spawn_menu_controls, reset_language_menu),
        );
        app.add_systems(
            Update,
            (
                update_language_options_visibility,
                spawn_vr_main_menu_wrist_panel
                    .run_if(in_state(GameState::MainMenu).and_then(in_state(VRState::VR))),
            ),
        );
        add_observers_run_if!(app, console_closed, on_ui_navigate, on_ui_confirm);
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
            context.spawn((
                Action::<UiConfirm>::new(),
                // Without this, confirming a button that causes `MenuControls` itself to
                // despawn-and-respawn on the very same input (e.g. `modal_menu.rs`'s "Main Menu"
                // button, closing the modal and dropping straight into a fresh main-menu
                // `MenuControls` with `Play` auto-focused) reads the still-held South/Enter as a
                // brand-new press on the new context's own `Press` condition (a fresh component,
                // so it has no memory of the input already being down) and immediately activates
                // whatever's newly focused. `require_reset` is `bevy_enhanced_input`'s built-in
                // fix for exactly this: it tracks the physical binding globally (not per-context),
                // so a still-held button stays ignored across a context respawn until it's
                // actually released. See `ActionSettings::require_reset`'s doc comment.
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

/// Moves `InputFocus` one step in the pushed direction — fires once per push (`Start`, not
/// `Fire`), since a stick held over the dead zone shouldn't keep re-navigating every frame.
fn on_ui_navigate(navigate: On<Start<UiNavigate>>, mut navigator: AutoDirectionalNavigator) {
    if let Ok(direction) = Dir2::new(navigate.value) {
        let _ = navigator.navigate(direction.into());
    }
}

/// "Presses" whichever UI element currently holds `InputFocus` — see `widgets::Activate`.
fn on_ui_confirm(_confirm: On<Start<UiConfirm>>, focus: Res<InputFocus>, mut commands: Commands) {
    if let Some(entity) = focus.get() {
        commands.trigger(Activate { entity });
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
pub(crate) fn main_menu_buttons() -> impl Scene {
    bsn! {
        panel(px(400), px(400))
        Children [
            (
                button(px(200), px(50), "main-menu-play")
                Tooltip::new("main-menu-play-tooltip")
                AutoFocus
                on(play_button)
            ),
            (
                button(px(200), px(50), "main-menu-options")
                Tooltip::new("main-menu-options-tooltip")
                on(stub_button)
            ),
            (
                button(px(200), px(50), "main-menu-credits")
                Tooltip::new("main-menu-credits-tooltip")
                on(stub_button)
            ),
            (
                button(px(200), px(50), "main-menu-quit")
                Tooltip::new("main-menu-quit-tooltip")
                on(quit_button)
            ),
            language_picker(),
        ]
    }
}

/// Whether the language options popup (below) is showing — a plain resource rather than a
/// `States` type since this is a small, purely-cosmetic toggle local to one panel, not something
/// anything else needs to branch on (see `game_state.rs`'s states for the "worth a states machine"
/// bar this doesn't clear). Reset on every `OnEnter(GameState::MainMenu)` so a menu left open
/// before leaving (e.g. hitting `Play` without picking a language) doesn't reappear pre-opened the
/// next time the main menu spawns fresh.
#[derive(Resource, Default)]
struct LanguageMenuOpen(bool);

fn reset_language_menu(mut open: ResMut<LanguageMenuOpen>) {
    open.0 = false;
}

/// Tags the options popup so `update_language_options_visibility` can find it without needing to
/// thread an entity reference through from `toggle_language_menu`.
#[derive(Component, Clone, Default)]
struct LanguageOptionsPanel;

/// Which locale a language-option button switches to — read directly off the entity `Activate`
/// fires on (see `select_language`), the same "look up a component on `activate.entity`" pattern
/// `input_icons.rs`'s `PendingIcon` uses for a similar per-entity-payload problem.
#[derive(Component, Clone, Default)]
struct LocaleOption(LanguageIdentifier);

/// The "Language" button plus its (initially hidden) options popup. `position_type: Relative` on
/// the wrapping `Node` is what lets the popup's own `position_type: Absolute` anchor directly below
/// the button instead of relative to the whole screen.
fn language_picker() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Relative,
        }
        Children [
            (
                button(px(200), px(50), "main-menu-language")
                on(toggle_language_menu)
            ),
            language_options_panel(),
        ]
    }
}

/// Each option's label is the language's own name in its own script (`"English"`, `"Русский"`),
/// deliberately *not* run through a real localization key — `button()` always attaches
/// `LocalizedText`, but `localized()` (see `localization.rs`) falls back to the raw key string
/// when no message matches, which these labels never do in any locale. That's relied on
/// intentionally here: a language picker should show every option in its own language regardless
/// of which language is currently active, not translate "Русский" into whatever's selected now.
fn language_options_panel() -> impl Scene {
    bsn! {
        LanguageOptionsPanel
        Visibility::Hidden
        Node {
            position_type: PositionType::Absolute,
            top: percent(100),
            left: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
        }
        Children [
            (
                button(px(200), px(40), "English")
                LocaleOption(langid!("en-US"))
                on(select_language)
            ),
            (
                button(px(200), px(40), "Русский")
                LocaleOption(langid!("ru-RU"))
                on(select_language)
            ),
        ]
    }
}

fn toggle_language_menu(_: On<Activate>, mut open: ResMut<LanguageMenuOpen>) {
    open.0 = !open.0;
}

fn select_language(
    activate: On<Activate>,
    options: Query<&LocaleOption>,
    mut locale: ResMut<Locale>,
    mut open: ResMut<LanguageMenuOpen>,
) {
    let Ok(option) = options.get(activate.entity) else {
        return;
    };
    locale.requested = option.0.clone();
    open.0 = false;
}

fn update_language_options_visibility(
    open: Res<LanguageMenuOpen>,
    mut panels: Query<&mut Visibility, With<LanguageOptionsPanel>>,
) {
    let visibility = if open.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut panel_visibility in &mut panels {
        *panel_visibility = visibility;
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

fn play_button(_event: On<Activate>, default_level: Res<DefaultLevel>, mut commands: Commands) {
    commands.trigger(LoadLevel {
        id: format!("levels/{}#Scene0", default_level.0),
    });
}

/// "Options"/"Credits" — stub buttons that exist to be navigable, not functional yet.
fn stub_button(_event: On<Activate>) {
    info!("not implemented yet");
}

fn quit_button(_event: On<Activate>, mut commands: Commands) {
    commands.write_message(AppExit::Success);
}
