use crate::assets::collections::CommonAssets;
use crate::controls::targeting::{Hovered, SELECT_RANGE, Selected};
use crate::gameplay::player_character::LocalPlayer;
use crate::ui::localization::localized;
use crate::ui::widgets::{
    PANEL_BORDER_COLOR, PANEL_COLOR, Tooltip, TooltipAbove, TooltipArg, panel,
};
use bevy::{
    color::palettes::css::{WHITE, WHITE_SMOKE},
    prelude::*,
    reflect::TypePath,
    render::render_resource::*,
    shader::ShaderRef,
    text::FontSourceTemplate,
};
use bevy_fluent::prelude::Localization;
use fluent::FluentArgs;
use shared::character_controller::Grounded;
use shared::combat::{ATTACK_RANGE, DAMAGE, GCD_DURATION, Gcd, HitPoints};
use shared::game_state::GameState;

/// The in-game HUD: the crosshair (always visible), the `DataFrame` debug panel (hidden by
/// default, toggled by `Tab`/`GamepadButton::Select` — see `DataFrameVisible`), and the ability
/// hotbar (with its GCD cooldown-sweep overlay). The controls-tips panel used to live here too —
/// it's now part of the pause modal instead (see `modal_menu.rs`'s `controls_tips`).
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HudVisible(true));
        app.insert_resource(DataFrameVisible(false));
        app.add_plugins((
            UiMaterialPlugin::<GcdOverlayMaterial>::default(),
            UiMaterialPlugin::<CrosshairGcdMaterial>::default(),
        ));
        app.add_systems(
            Startup,
            (setup_gcd_overlay_material, setup_crosshair_gcd_material),
        );
        app.add_observer(add_gcd_overlay);
        app.add_observer(add_crosshair_gcd_ring);
        app.add_systems(
            Update,
            (
                update_data_frame,
                update_gcd_overlay,
                update_crosshair_gcd,
                update_hud_visibility,
                update_data_frame_visibility,
            ),
        );
    }
}

/// Toggled by the console's `hud` command (`console.rs`) — sets `Visibility` on every top-level
/// `HudElement` root, which (being ordinary UI `Node` hierarchies, unlike `nameplate.rs`'s
/// non-child-parented nameplates) hides every descendant for free via `InheritedVisibility`
/// propagation. Doesn't cover `nameplate.rs`'s `NameplatesVisible` — that's a separate toggle for
/// a separate, non-HUD UI surface. Also read directly by `update_controls_tips_visibility` — the
/// two `controls_tips` panels (see below) aren't tagged `HudElement` themselves, since their
/// visibility already depends on a *second* condition (`InputDeviceState`) that would fight this
/// system if both tried to drive the same `Visibility` independently.
#[derive(Resource)]
pub struct HudVisible(pub bool);

/// Marks `in_game_scene`'s `HudVisible`-only top-level roots (currently just `hotbar`, disabled
/// while testing — see `in_game_scene`) so `update_hud_visibility` can find and toggle all of them
/// without needing a single common parent. `data_frame` is deliberately *not* tagged with this any
/// more — see `DataFrameVisible`.
#[derive(Component, Clone, Default)]
struct HudElement;

/// Deliberately unconditional (no `is_changed()` guard) — a `HudElement` can spawn *after* the
/// last toggle (e.g. reloading the level respawns `in_game_scene`'s roots while `HudVisible` is
/// still `false` from an earlier toggle), and that new entity's default `Visibility::Inherited`
/// would never get corrected to match if this only reacted to `HudVisible` changing. The element
/// count here is tiny, so running every frame is cheap.
fn update_hud_visibility(
    hud_visible: Res<HudVisible>,
    mut elements: Query<&mut Visibility, With<HudElement>>,
) {
    let visibility = if hud_visible.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut element_visibility in &mut elements {
        *element_visibility = visibility;
    }
}

/// Toggled by `Tab`/`GamepadButton::Select` (`controls::toggle_data_frame`) — hidden by default,
/// unlike `HudVisible`. Kept as its own resource/condition rather than folding the `DataFrame`
/// panel into `HudElement`, the same reasoning `update_controls_tips_visibility` used to combine
/// `HudVisible` with a second condition before that panel moved to `modal_menu.rs`: this needs to
/// AND with `HudVisible` (the console's `hud` command should still master-hide it) without forcing
/// it visible the instant the HUD is shown.
#[derive(Resource)]
pub struct DataFrameVisible(pub bool);

/// Marks `data_frame`'s root — see `DataFrameVisible`/`update_data_frame_visibility`.
#[derive(Component, Clone, Default)]
struct DataFramePanel;

/// Combines `HudVisible` and `DataFrameVisible` the same way `update_controls_tips_visibility`
/// used to combine `HudVisible` and `InputDeviceState` — see `DataFrameVisible`'s doc comment.
/// Deliberately unconditional (no `is_changed()` guard) for the same reason as
/// `update_hud_visibility`: a level reload can respawn `DataFramePanel` while both toggles are
/// still whatever they were left at.
fn update_data_frame_visibility(
    hud_visible: Res<HudVisible>,
    data_frame_visible: Res<DataFrameVisible>,
    mut panels: Query<&mut Visibility, With<DataFramePanel>>,
) {
    let visibility = if hud_visible.0 && data_frame_visible.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut panel_visibility in &mut panels {
        *panel_visibility = visibility;
    }
}

/// A real system (not the usual `some_scene.spawn()` adapter, which only works for a zero-arg
/// `Fn() -> impl SceneList`) since `data_frame()` needs `Res<CommonAssets>` for its `TextFont` —
/// see `ui.rs`'s `spawn_main_menu` for the same pattern.
pub fn spawn_in_game_scene(mut commands: Commands, common_assets: Res<CommonAssets>) {
    commands.spawn_scene_list(in_game_scene(&common_assets));
}

// `hotbar()` is left out of the scene below (rather than deleted) while testing the minimalist
// crosshair-only look — see `update_crosshair_gcd`. Add `hotbar(common_assets),` back to
// re-enable it.
fn in_game_scene(common_assets: &CommonAssets) -> impl SceneList {
    bsn_list![data_frame(common_assets), crosshair()]
}

const CROSSHAIR_SIZE: f32 = 8.0;
/// Outer diameter of the GCD progress ring — bigger than the dot it replaces so the ring has room
/// to read clearly around it (see `crosshair_gcd.wgsl`'s `RING_THICKNESS`).
const CROSSHAIR_GCD_RING_SIZE: f32 = 22.0;

/// Marks the plain dot shown while the GCD is ready — hidden in favor of `CrosshairGcdRing` while
/// it's running. See `update_crosshair_gcd`.
#[derive(Component, Clone, Default)]
struct CrosshairDot;

/// Marks the radial GCD progress overlay (`crosshair_gcd.wgsl`) shown in place of the dot while
/// the GCD is running. Sized to `CROSSHAIR_GCD_RING_SIZE` and absolutely positioned so it overlays
/// the dot's wrapper node rather than participating in its flex layout.
#[derive(Component, Clone, Default)]
struct CrosshairGcdRing;

fn crosshair() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Pickable::IGNORE
        Children [
            (
                Node {
                    width: px(CROSSHAIR_GCD_RING_SIZE),
                    height: px(CROSSHAIR_GCD_RING_SIZE),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                Pickable::IGNORE
                Children [
                    (
                        CrosshairGcdRing
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                        }
                        Visibility::Hidden
                        Pickable::IGNORE
                    ),
                    (
                        CrosshairDot
                        Node {
                            width: px(CROSSHAIR_SIZE),
                            height: px(CROSSHAIR_SIZE),
                            border_radius: px(CROSSHAIR_SIZE / 2.0),
                        }
                        BackgroundColor(WHITE)
                        Pickable::IGNORE
                    ),
                ]
            ),
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

fn data_frame(common_assets: &CommonAssets) -> impl Scene {
    let font = common_assets.serif_font.clone();
    bsn! {
        DataFramePanel
        Visibility::Hidden
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::End,
        }
        Children[
            panel(px(400), px(400))
            Children [
                (
                    Text("")
                    TextFont {
                        font: FontSourceTemplate::Handle(font),
                    }
                    DataFrame
                ),
            ]
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

const HOTBAR_SLOT_SIZE: f32 = 64.0;
const HOTBAR_SLOT_GAP: f32 = 4.0;
const HOTBAR_BOTTOM_PADDING: f32 = 20.0;
const ABILITY_LETTER_FONT_SIZE: f32 = 28.0;
const HOTKEY_LETTER_FONT_SIZE: f32 = 14.0;

fn hotbar(common_assets: &CommonAssets) -> impl Scene {
    bsn! {
        HudElement
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::End,
            justify_content: JustifyContent::Center,
            column_gap: px(HOTBAR_SLOT_GAP),
            padding: UiRect::bottom(px(HOTBAR_BOTTOM_PADDING)),
        }
        Children [
            ability_slot(
                "A", "f", "hud-attack-tooltip",
                vec![("damage", DAMAGE.into()), ("range", ATTACK_RANGE.into())],
                common_assets,
            ),
            ability_slot(
                "K", "t", "hud-kill-tooltip",
                vec![("range", ATTACK_RANGE.into())],
                common_assets,
            ),
            ability_slot("N", "r", "hud-spawn-npc-tooltip", vec![], common_assets),
            ability_slot("C", "e", "hud-spawn-cube-tooltip", vec![], common_assets),
            hotbar_slot(), hotbar_slot(), hotbar_slot(), hotbar_slot(),
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

#[derive(Component, Clone, Default)]
struct HotbarSlot;

fn hotbar_slot() -> impl Scene {
    bsn! {
        HotbarSlot
        Node {
            width: px(HOTBAR_SLOT_SIZE),
            height: px(HOTBAR_SLOT_SIZE),
            border: px(2),
            border_radius: px(3),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BorderColor::from(PANEL_BORDER_COLOR)
        BackgroundColor(PANEL_COLOR)
    }
}

/// A hotbar slot with a big ability-name letter (centered) and a small hotkey letter (bottom-right
/// corner) — currently just text standing in for real icons, since no icon/inventory asset system
/// exists yet.
fn ability_slot(
    ability_letter: &str,
    hotkey_letter: &str,
    tooltip_key: &'static str,
    tooltip_args: Vec<(&'static str, TooltipArg)>,
    common_assets: &CommonAssets,
) -> impl Scene {
    let font = common_assets.serif_font.clone();
    let hotkey_font = font.clone();
    bsn! {
        hotbar_slot()
        Tooltip::with_args(tooltip_key, tooltip_args)
        TooltipAbove(HOTBAR_SLOT_SIZE)
        Children [
            (
                Text(ability_letter)
                TextFont {
                    font: FontSourceTemplate::Handle(font),
                    font_size: px(ABILITY_LETTER_FONT_SIZE),
                }
                TextColor(WHITE)
                Pickable::IGNORE
            ),
            (
                Text(hotkey_letter)
                TextFont {
                    font: FontSourceTemplate::Handle(hotkey_font),
                    font_size: px(HOTKEY_LETTER_FONT_SIZE),
                }
                TextColor(WHITE_SMOKE)
                Node {
                    position_type: PositionType::Absolute,
                    right: px(3),
                    bottom: px(1),
                }
                Pickable::IGNORE
            ),
        ]
    }
}

/// Radial cooldown-sweep overlay material for hotbar slots — see `assets/shaders/gcd_overlay.wgsl`.
/// `covered` is the fraction of the GCD still remaining (1.0 = just triggered, 0.0 = ready), shared
/// by every slot since the GCD is global.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct GcdOverlayMaterial {
    #[uniform(0)]
    covered: Vec4,
}

impl UiMaterial for GcdOverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/gcd_overlay.wgsl".into()
    }
}

#[derive(Resource)]
struct GcdOverlayMaterialHandle(Handle<GcdOverlayMaterial>);

fn setup_gcd_overlay_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<GcdOverlayMaterial>>,
) {
    commands.insert_resource(GcdOverlayMaterialHandle(materials.add(
        GcdOverlayMaterial {
            covered: Vec4::ZERO,
        },
    )));
}

/// Attaches the (shared) GCD overlay to every hotbar slot as it spawns — reactive rather than
/// baked into the `bsn!` scene itself, since building the overlay needs `Assets<GcdOverlayMaterial>`.
fn add_gcd_overlay(
    added: On<Add, HotbarSlot>,
    handle: Res<GcdOverlayMaterialHandle>,
    mut commands: Commands,
) {
    commands.entity(added.entity).with_child((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        Pickable::IGNORE,
        MaterialNode(handle.0.clone()),
    ));
}

fn update_gcd_overlay(
    local_player: Res<LocalPlayer>,
    player: Query<&Gcd>,
    handle: Res<GcdOverlayMaterialHandle>,
    mut materials: ResMut<Assets<GcdOverlayMaterial>>,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Ok(gcd) = player.get(local_player) else {
        return;
    };
    let Some(mut material) = materials.get_mut(&handle.0) else {
        return;
    };
    material.covered = Vec4::splat(gcd.0.fraction_remaining());
}

/// Radial GCD progress-ring material for the crosshair — see `assets/shaders/crosshair_gcd.wgsl`.
/// A separate `UiMaterial` type from `GcdOverlayMaterial` even though the uniform shape is
/// identical, since `UiMaterial::fragment_shader()` is per-type, not per-instance — the crosshair
/// needs its own shader (a thin ring, not a filled-square pie wipe).
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct CrosshairGcdMaterial {
    #[uniform(0)]
    covered: Vec4,
}

impl UiMaterial for CrosshairGcdMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/crosshair_gcd.wgsl".into()
    }
}

#[derive(Resource)]
struct CrosshairGcdMaterialHandle(Handle<CrosshairGcdMaterial>);

fn setup_crosshair_gcd_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<CrosshairGcdMaterial>>,
) {
    commands.insert_resource(CrosshairGcdMaterialHandle(materials.add(
        CrosshairGcdMaterial {
            covered: Vec4::ZERO,
        },
    )));
}

/// Attaches the `MaterialNode` to the ring as it spawns — same reason as `add_gcd_overlay`: the
/// handle needs `Assets<CrosshairGcdMaterial>`, which isn't available inside the `bsn!` scene.
fn add_crosshair_gcd_ring(
    added: On<Add, CrosshairGcdRing>,
    handle: Res<CrosshairGcdMaterialHandle>,
    mut commands: Commands,
) {
    commands
        .entity(added.entity)
        .insert(MaterialNode(handle.0.clone()));
}

/// Swaps the crosshair between the plain dot (GCD ready) and the radial progress ring (GCD
/// running), and keeps the ring's fill in sync while it's shown. Toggling `Visibility` rather than
/// despawning/respawning either node keeps this a plain per-frame state sync, matching
/// `update_gcd_overlay`'s style.
fn update_crosshair_gcd(
    local_player: Res<LocalPlayer>,
    player: Query<&Gcd>,
    handle: Res<CrosshairGcdMaterialHandle>,
    mut materials: ResMut<Assets<CrosshairGcdMaterial>>,
    mut dot: Query<&mut Visibility, (With<CrosshairDot>, Without<CrosshairGcdRing>)>,
    mut ring: Query<&mut Visibility, (With<CrosshairGcdRing>, Without<CrosshairDot>)>,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Ok(gcd) = player.get(local_player) else {
        return;
    };
    let running = !gcd.0.is_finished();
    if let Ok(mut dot_visibility) = dot.single_mut() {
        *dot_visibility = if running {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if let Ok(mut ring_visibility) = ring.single_mut() {
        *ring_visibility = if running {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Some(mut material) = materials.get_mut(&handle.0) {
        material.covered = Vec4::splat(gcd.0.fraction_remaining());
    }
}

#[derive(Component, Clone, Default, Debug)]
struct DataFrame;

fn update_data_frame(
    local_player: Res<LocalPlayer>,
    localization: Option<Res<Localization>>,
    mut query: Query<&mut Text, With<DataFrame>>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    player: Query<(&HitPoints, &GlobalTransform, Has<Grounded>)>,
    global_transforms: Query<&GlobalTransform>,
    hit_points: Query<&HitPoints>,
    name: Query<&Name>,
    mut last_grounded: Local<Option<bool>>,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Ok((player_hit_points, player_global_transform, is_grounded)) = player.get(local_player)
    else {
        return;
    };
    // Localization loads asynchronously (see `localization.rs`) and isn't guaranteed ready by the
    // time this first runs — skip until it is rather than showing raw `.ftl` keys.
    let Some(localization) = localization else {
        return;
    };

    // `Grounded` is added/removed every jump/landing, not just when `Hovered`/`Selected` change —
    // without tracking it here too, the displayed status would only refresh coincidentally.
    // `localization.is_changed()` covers the one frame it goes from not-ready to ready.
    let grounded_changed = *last_grounded != Some(is_grounded);
    if !hovered.is_changed()
        && !selected.is_changed()
        && !grounded_changed
        && !localization.is_changed()
    {
        return;
    }
    *last_grounded = Some(is_grounded);

    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let mut value = localized(&localization, "hud-console-hint", &FluentArgs::new());
    value += "\n\n";

    let mut hp_args = FluentArgs::new();
    hp_args.set("hp", player_hit_points.hit_points);
    hp_args.set("max_hp", player_hit_points.max_hit_points);
    value += &localized(&localization, "hud-hp", &hp_args);
    value += "\n";

    let mut damage_args = FluentArgs::new();
    damage_args.set("damage", DAMAGE);
    value += &localized(&localization, "hud-damage", &damage_args);
    value += "\n";

    let mut attack_range_args = FluentArgs::new();
    attack_range_args.set("range", ATTACK_RANGE);
    value += &localized(&localization, "hud-attack-range", &attack_range_args);
    value += "\n";

    let mut select_range_args = FluentArgs::new();
    select_range_args.set("range", SELECT_RANGE);
    value += &localized(&localization, "hud-select-range", &select_range_args);
    value += "\n";

    let mut gcd_args = FluentArgs::new();
    gcd_args.set("seconds", GCD_DURATION);
    value += &localized(&localization, "hud-gcd", &gcd_args);
    value += "\n";

    let mut grounded_args = FluentArgs::new();
    grounded_args.set("grounded", if is_grounded { "true" } else { "false" });
    value += &localized(&localization, "hud-grounded", &grounded_args);
    value += "\n\n";

    match hovered.0 {
        Some((entity, distance)) => {
            let mut entity_args = FluentArgs::new();
            entity_args.set("entity", format!("{entity:?}"));
            value += &localized(&localization, "hud-hovered", &entity_args);
            value += "\n";

            let mut distance_args = FluentArgs::new();
            distance_args.set("distance", format!("{distance:.2}"));
            value += &localized(&localization, "hud-distance", &distance_args);
            value += "\n\n";
        }
        None => {
            value += &localized(&localization, "hud-hovered-none", &FluentArgs::new());
            value += "\n";
            value += &localized(&localization, "hud-distance-none", &FluentArgs::new());
            value += "\n\n";
        }
    }

    if let Some(entity) = selected.0 {
        let distance_to_selected = global_transforms
            .get(entity)
            .ok()
            .map(|selected_transform| {
                (selected_transform.compute_transform().translation
                    - player_global_transform.compute_transform().translation)
                    .length()
            });

        let mut selected_args = FluentArgs::new();
        selected_args.set("entity", format!("{entity:?}"));
        value += &localized(&localization, "hud-selected", &selected_args);
        value += "\n";

        match distance_to_selected {
            Some(distance_to_selected) => {
                let mut distance_args = FluentArgs::new();
                distance_args.set("distance", format!("{distance_to_selected:.2}"));
                value += &localized(&localization, "hud-distance", &distance_args);
            }
            None => value += &localized(&localization, "hud-distance-none", &FluentArgs::new()),
        }
        value += "\n";

        if let Ok(name) = name.get(entity) {
            let mut name_args = FluentArgs::new();
            name_args.set("name", name.as_str());
            value += &localized(&localization, "hud-name", &name_args);
            value += "\n";
        }
        if let Ok(target_hit_points) = hit_points.get(entity) {
            let mut hp_args = FluentArgs::new();
            hp_args.set("hp", target_hit_points.hit_points);
            hp_args.set("max_hp", target_hit_points.max_hit_points);
            value += &localized(&localization, "hud-hp", &hp_args);
            value += "\n";
        }
    }

    text.0 = value;
}
