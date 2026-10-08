//! The main menu (`main_menu.html`): Connect, Options (stub selector), Credits (stub), Quit and
//! Language (selector). The same template, with `wrist = true`, is the VR main-menu wrist panel;
//! signals from both are handled by [`handle_main_menu_signals`].

use crate::assets::collections::CommonAssets;
use crate::controls::controls::MouseSensitivity;
use crate::events::Connect;
use crate::gameplay::player_character::ClientPrediction;
use crate::ui::credits;
use crate::ui::hud::{HudPlugin, HudVisible};
use crate::ui::markup::{self, menu_controls, LocaleSelection};
use crate::ui::menu_screen::{self, MenuScreenScope, OpenMenuScreens, open_menu_screen};
use crate::ui::nameplate::NameplatesVisible;
use crate::ui::quad_panel::quad_panel;
use crate::ui::selector::{self, Selector, SelectorOption, SelectorPicked};
use crate::ui::slider::{self, SliderChange, SliderInput};
use avian3d::prelude::{PhysicsDebugPlugin, PhysicsGizmos};
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::{asset::embedded_asset, prelude::*};
use bevy_markup::prelude::*;
use bevy_xr_utils::tracking_utils::XrTrackedLeftGrip;
use p19_shared::game_state::{GameState, VRState};
use std::f32::consts::FRAC_PI_2;

/// The language selector's key; its toggles (`options.html`, the VR wrist panel's
/// `main_menu.html`) name it in `data-selector`.
const LANGUAGE_SELECTOR: &str = "main-menu.language";

/// Selectable languages: locale id and the language's own name in its own script — deliberately
/// not localized, so every option is readable whichever language is active. Fonts are the
/// system's (`markup::register_ui_fonts`): Japanese renders only if an installed font covers it
/// (ADR 0016).
const LANGUAGES: [(&str, &str); 3] = [
    ("en-US", "English"),
    ("ru-RU", "Русский"),
    ("ja-JP", "日本語"),
];

/// The mouse sensitivity slider's key (`options.html`'s `data-slider`).
const MOUSE_SENSITIVITY_SLIDER: &str = "options.mouse-sensitivity";

/// The mouse sensitivity slider's range (multipliers of the base mouse-look speed), its
/// keyboard/gamepad step, and the precision values are kept at.
const MOUSE_SENSITIVITY_MIN: f32 = 0.1;
const MOUSE_SENSITIVITY_MAX: f32 = 3.0;
const MOUSE_SENSITIVITY_STEP: f32 = 0.05;
const MOUSE_SENSITIVITY_PRECISION: f32 = 0.01;

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/main_menu.html");
        embedded_asset!(app, "html/options.html");
        // The options screen's visibility toggles' engine side. The FPS overlay's plugin needs
        // render-side `Assets<ShaderBuffer>`, absent in `--no-render` mode — skipped there (its
        // toggle still works: it flips `FpsOverlayVisible`, whose applier has no config to
        // write). The system's monospace font (no font files ship with the game). avian's
        // `PhysicsDebugPlugin` registers the `PhysicsGizmos` gizmo group that
        // `apply_physics_gizmos` writes (`config_mut` panics on an unregistered group).
        if !crate::config::is_no_render_presync() {
            app.add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    text_config: TextFont {
                        font: FontSource::Monospace,
                        ..FpsOverlayConfig::default().text_config
                    },
                    ..default()
                },
            });
        }
        app.add_plugins(PhysicsDebugPlugin::default())
            .insert_resource(FpsOverlayVisible::default())
            .insert_resource(PhysicsGizmosVisible::default())
            .register_type::<FpsOverlayVisible>()
            .register_type::<PhysicsGizmosVisible>()
            .add_systems(Update, (apply_fps_overlay, apply_physics_gizmos));
        app.add_plugins((
            HudPlugin,
            selector::SelectorPlugin,
            slider::SliderPlugin,
            menu_screen::MenuScreenPlugin,
            credits::CreditsPlugin,
        ));
        app.add_systems(OnEnter(GameState::MainMenu), spawn_menu_controls);
        app.on_html_click(
            "main-menu.connect",
            |_: In<ElementSignal>, mut commands: Commands| {
                commands.trigger(Connect);
            },
        )
        .on_html_click("main-menu.options", open_options)
        .on_html_click("options.prediction", toggle_prediction)
        .on_html_click("options.fps-overlay", toggle_fps_overlay)
        .on_html_click("options.physics-debug", toggle_physics_debug)
        .on_html_click("options.nameplates", toggle_nameplates)
        .on_html_click("options.hud", toggle_hud)
        .on_html_click(
            "main-menu.quit",
            |_: In<ElementSignal>, mut commands: Commands| {
                commands.write_message(AppExit::Success);
            },
        );
        app.add_systems(
            Update,
            (
                handle_main_menu_picks,
                apply_slider_input,
                update_options_screen,
                spawn_vr_main_menu_wrist_panel
                    .run_if(in_state(GameState::MainMenu).and_then(in_state(VRState::VR))),
            ),
        );
    }
}

fn spawn_menu_controls(mut commands: Commands) {
    commands.spawn((menu_controls(), DespawnOnExit(GameState::MainMenu)));
}

/// The desktop main menu (bottom-left panel over the menu background scene) and its selectors.
/// The background is its own entity: an `HtmlUi` root's children belong to the pipeline.
pub fn spawn_main_menu(
    mut commands: Commands,
    common_assets: Res<CommonAssets>,
    asset_server: Res<AssetServer>,
    locale: Res<LocaleSelection>,
) {
    commands.spawn((
        WorldAssetRoot(common_assets.menu_background.clone()),
        DespawnOnExit(GameState::MainMenu),
    ));
    commands.spawn((
        markup::template(&asset_server, "main_menu.html"),
        TemplateContext::new().with("wrist", &false),
        DespawnOnExit(GameState::MainMenu),
    ));
    let current = locale.0.clone();
    commands.spawn((
        language_selector(&current),
        DespawnOnExit(GameState::MainMenu),
    ));
}

/// The language picker's `Selector` — options in their own scripts (deliberately not localized;
/// see `LANGUAGES`), the current language selected. Spawned by the main menu (with the menu) and
/// by the pause menu (with the modal), so the options screen's Language toggle works on both;
/// picks from either flow through `handle_main_menu_picks`.
pub fn language_selector(current: &str) -> Selector {
    let languages = LANGUAGES
        .iter()
        .map(|(id, name)| SelectorOption::literal(*id, *name))
        .collect();
    Selector::new(LANGUAGE_SELECTOR, languages)
        .with_selected(LANGUAGES.iter().position(|(id, _)| *id == current))
}

/// The options screen's template context (main menu and pause menu alike; written every frame by
/// `update_options_screen` — an identical render does nothing). `prediction`, `fps_overlay`,
/// `physics_debug`, `nameplates` and `hud` are the toggle labels' enabled args; the slider values
/// come from [`mouse_sensitivity_context`].
pub fn options_context(
    prediction: ClientPrediction,
    mouse_sensitivity: MouseSensitivity,
    fps_overlay: FpsOverlayVisible,
    physics_debug: PhysicsGizmosVisible,
    nameplates: NameplatesVisible,
    hud: HudVisible,
) -> TemplateContext {
    mouse_sensitivity_context(TemplateContext::new(), mouse_sensitivity)
        .with("prediction", &prediction.0)
        .with("fps_overlay", &fps_overlay.0)
        .with("physics_debug", &physics_debug.0)
        .with("nameplates", &nameplates.0)
        .with("hud", &hud.0)
}

/// Opens the options screen (`options.html`) — the main menu's and the pause menu's submenu
/// alike — and marks its root so `update_options_screen` keeps it current. `return_focus` is the
/// `id` of the opening button.
pub fn open_options_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    open: &OpenMenuScreens,
    context: TemplateContext,
    return_focus: &'static str,
    scope: MenuScreenScope,
) {
    if let Some(root) = open_menu_screen(commands, asset_server, open, "options.html", context, return_focus, scope) {
        commands.entity(root).insert(OptionsScreen);
    }
}

/// The main menu's options screen root (`.menu-screen-root` over it); the pause menu's submenu
/// root is marked the same, so `update_options_screen` keeps both current.
#[derive(Component)]
pub struct OptionsScreen;

/// `main-menu.options`: the options screen — the language picker, the mouse sensitivity slider,
/// the client-side prediction toggle and the four visibility toggles.
fn open_options(
    _: In<ElementSignal>,
    open: OpenMenuScreens,
    prediction: Res<ClientPrediction>,
    mouse_sensitivity: Res<MouseSensitivity>,
    fps_overlay: Res<FpsOverlayVisible>,
    physics_debug: Res<PhysicsGizmosVisible>,
    nameplates: Res<NameplatesVisible>,
    hud: Res<HudVisible>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    open_options_screen(
        &mut commands,
        &asset_server,
        &open,
        options_context(
            *prediction,
            *mouse_sensitivity,
            *fps_overlay,
            *physics_debug,
            *nameplates,
            *hud,
        ),
        "options",
        MenuScreenScope::MainMenu,
    );
}

/// The options screen's FPS-overlay toggle state. Reflected, so BRP can toggle it too; the
/// options screens flip it (the console that shared it is gone), and `apply_fps_overlay` copies
/// it into Bevy's `FpsOverlayConfig` (absent under `--no-render`, where the overlay plugin is
/// skipped). Not persisted.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Resource)]
pub struct FpsOverlayVisible(pub bool);

/// The options screen's physics-debug-gizmos toggle state — same pattern as
/// [`FpsOverlayVisible`], applied into avian's `PhysicsGizmos` group. Not persisted.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Resource)]
pub struct PhysicsGizmosVisible(pub bool);

/// Copies [`FpsOverlayVisible`] into Bevy's `FpsOverlayConfig` — the overlay and its frame-time
/// graph together. Written only on change.
fn apply_fps_overlay(
    visible: Res<FpsOverlayVisible>,
    config: Option<ResMut<FpsOverlayConfig>>,
) {
    if !visible.is_changed() {
        return;
    }
    let Some(mut config) = config else {
        return;
    };
    config.enabled = visible.0;
    config.frame_time_graph_config.enabled = visible.0;
}

/// Copies [`PhysicsGizmosVisible`] into avian's `PhysicsGizmos` group. Written only on change.
fn apply_physics_gizmos(
    visible: Res<PhysicsGizmosVisible>,
    mut store: ResMut<GizmoConfigStore>,
) {
    if !visible.is_changed() {
        return;
    }
    store.config_mut::<PhysicsGizmos>().0.enabled = visible.0;
}

/// Adds what the `ui.mouse_sensitivity` slider component renders (the options screen's slider,
/// main menu and pause menu alike): `mouse_sensitivity`, the value as shown, and
/// `mouse_sensitivity_percent`, its position on the slider — rounded to a tenth of a percent:
/// finer is invisible, and an identical render is free.
pub fn mouse_sensitivity_context(
    context: TemplateContext,
    mouse_sensitivity: MouseSensitivity,
) -> TemplateContext {
    let fraction = (mouse_sensitivity.0 - MOUSE_SENSITIVITY_MIN)
        / (MOUSE_SENSITIVITY_MAX - MOUSE_SENSITIVITY_MIN);
    context
        .with("mouse_sensitivity", &format!("{:.2}", mouse_sensitivity.0))
        .with(
            "mouse_sensitivity_percent",
            &format!("{:.1}", fraction.clamp(0.0, 1.0) * 100.0),
        )
}

/// Applies the mouse sensitivity slider's input (options screen and pause menu alike): a pointer position maps linearly onto
/// `MOUSE_SENSITIVITY_MIN..=MAX`, a step moves by `MOUSE_SENSITIVITY_STEP`; both are clamped
/// and kept at `MOUSE_SENSITIVITY_PRECISION`.
fn apply_slider_input(
    mut input: MessageReader<SliderInput>,
    mut mouse_sensitivity: ResMut<MouseSensitivity>,
) {
    for input in input
        .read()
        .filter(|input| input.key == MOUSE_SENSITIVITY_SLIDER)
    {
        let value = match input.change {
            SliderChange::Set(fraction) => {
                MOUSE_SENSITIVITY_MIN + fraction * (MOUSE_SENSITIVITY_MAX - MOUSE_SENSITIVITY_MIN)
            }
            SliderChange::Step(steps) => {
                mouse_sensitivity.0 + steps as f32 * MOUSE_SENSITIVITY_STEP
            }
        };
        let value = (value.clamp(MOUSE_SENSITIVITY_MIN, MOUSE_SENSITIVITY_MAX)
            / MOUSE_SENSITIVITY_PRECISION)
            .round()
            * MOUSE_SENSITIVITY_PRECISION;
        mouse_sensitivity.set_if_neq(MouseSensitivity(value));
    }
}

/// `options.prediction`: flips [`ClientPrediction`] (applies from the next Play).
fn toggle_prediction(_: In<ElementSignal>, mut prediction: ResMut<ClientPrediction>) {
    prediction.0 = !prediction.0;
    info!(enabled = prediction.0, "client-side prediction");
}

/// Keeps every open options screen showing the current settings (written every frame; an
/// identical render does nothing).
fn update_options_screen(
    prediction: Res<ClientPrediction>,
    mouse_sensitivity: Res<MouseSensitivity>,
    fps_overlay: Res<FpsOverlayVisible>,
    physics_debug: Res<PhysicsGizmosVisible>,
    nameplates: Res<NameplatesVisible>,
    hud: Res<HudVisible>,
    mut screens: Query<&mut TemplateContext, With<OptionsScreen>>,
) {
    for mut context in &mut screens {
        *context = options_context(
            *prediction,
            *mouse_sensitivity,
            *fps_overlay,
            *physics_debug,
            *nameplates,
            *hud,
        );
    }
}

/// `options.fps-overlay`: flips [`FpsOverlayVisible`] (`apply_fps_overlay` writes Bevy's
/// config).
fn toggle_fps_overlay(_: In<ElementSignal>, mut visible: ResMut<FpsOverlayVisible>) {
    visible.0 = !visible.0;
}

/// `options.physics-debug`: flips [`PhysicsGizmosVisible`].
fn toggle_physics_debug(_: In<ElementSignal>, mut visible: ResMut<PhysicsGizmosVisible>) {
    visible.0 = !visible.0;
}

/// `options.nameplates`: flips `NameplatesVisible` (nameplate.rs consumes it directly).
fn toggle_nameplates(_: In<ElementSignal>, mut visible: ResMut<NameplatesVisible>) {
    visible.0 = !visible.0;
}

/// `options.hud`: flips `HudVisible` (hud.rs consumes it directly).
fn toggle_hud(_: In<ElementSignal>, mut visible: ResMut<HudVisible>) {
    visible.0 = !visible.0;
}

/// Language picks — the options screen (main menu or pause submenu) and the VR wrist panel
/// alike.
fn handle_main_menu_picks(
    mut picked: MessageReader<SelectorPicked>,
    mut locale: ResMut<LocaleSelection>,
) {
    for pick in picked
        .read()
        .filter(|pick| pick.selector == LANGUAGE_SELECTOR)
    {
        if LANGUAGES.iter().any(|(id, _)| *id == pick.value) {
            if locale.0 != pick.value {
                locale.0 = pick.value.clone();
            }
        } else {
            warn!("unknown language option {:?}", pick.value);
        }
    }
}

const WRIST_PANEL_SIZE: f32 = 0.16;
const WRIST_PANEL_TEXTURE_SIZE: u32 = 420;

/// Marks the spawned wrist panel so `spawn_vr_main_menu_wrist_panel` spawns it only once.
#[derive(Component)]
struct VrMainMenuWristPanel;

/// The main menu on a `quad_panel` attached to the left controller's grip, for VR players
/// without a desktop mouse (laser-pointer driven, so `HtmlNoFocus`). The offset/rotation is an
/// unverified guess at "back of the wrist, angled toward the face".
///
/// Polls every frame in `MainMenu` + `VR` instead of running on `OnEnter(MainMenu)`: that fires
/// on the first frame, long before the OpenXR session is up and the `XrTrackedLeftGrip` entity
/// exists, so a one-shot spawn found nothing and never retried.
fn spawn_vr_main_menu_wrist_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
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
        (
            markup::template(&asset_server, "main_menu.html"),
            TemplateContext::new().with("wrist", &true),
            HtmlNoFocus,
        ),
    );
    commands.spawn((
        panel,
        VrMainMenuWristPanel,
        // 0.12m clears `vr_controllers::spawn_controller_cubes`'s debug cube (centered on this
        // same grip pose, 0.12 long) whichever way "up" turns out to be; `quad_panel`'s material
        // renders both sides for the same reason.
        Transform::from_xyz(0.0, 0.12, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ChildOf(left_grip),
        DespawnOnExit(GameState::MainMenu),
    ));
}
