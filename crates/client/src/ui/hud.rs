//! The in-game HUD: the crosshair (always shown), the data frame debug panel (hidden by default,
//! toggled by `Tab`/`GamepadButton::Select` — see [`DataFrameVisible`]), and the ability hotbar
//! (with its GCD cooldown-sweep overlay; currently not spawned — see [`HOTBAR_ENABLED`]). Each is
//! its own `HtmlUi` root (`html/crosshair.html`, `html/data_frame.html`, `html/hotbar.html`).
//! The controls-tips panel is part of the pause modal (`modal_menu.rs`).

use crate::controls::targeting::{Hovered, SELECT_RANGE, Selected};
use crate::gameplay::player_character::LocalPlayer;
use crate::ui::markup;
use bevy::{
    asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::*,
    shader::ShaderRef,
};
use bevy_markup::prelude::*;
use p19_shared::combat::{ATTACK_RANGE, DAMAGE, GCD_DURATION, Gcd, HitPoints};
use p19_shared::game_state::GameState;
use serde_json::{Value, json};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/crosshair.html");
        embedded_asset!(app, "html/data_frame.html");
        embedded_asset!(app, "html/hotbar.html");
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
        app.add_observer(on_crosshair_built)
            .add_observer(on_hotbar_built);
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

/// Toggled by the console's `hud` command (`console.rs`): master-hides every [`HudElement`] root
/// (crosshair, hotbar) and the data frame. Doesn't cover `nameplate.rs`'s `NameplatesVisible` —
/// a separate toggle for a separate, non-HUD surface.
#[derive(Resource)]
pub struct HudVisible(pub bool);

/// Marks the HUD roots shown exactly while [`HudVisible`] is set. The data frame isn't one — it
/// also needs [`DataFrameVisible`].
#[derive(Component, Clone, Default)]
struct HudElement;

/// Unconditional (no `is_changed()` guard): a level reload respawns the roots while
/// `HudVisible` may still be `false`, and their default `Visibility::Inherited` must be
/// corrected too.
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
        element_visibility.set_if_neq(visibility);
    }
}

/// Toggled by `Tab`/`GamepadButton::Select` (`controls::toggle_data_frame`) — hidden by default,
/// unlike `HudVisible`. ANDed with `HudVisible` (the console's `hud` command still master-hides
/// the data frame) without forcing it visible the instant the HUD is shown.
#[derive(Resource)]
pub struct DataFrameVisible(pub bool);

/// The data frame's root and the values it was last rendered with — see `update_data_frame`.
#[derive(Component)]
struct DataFrame {
    values: Value,
}

/// Unconditional for the same reason as `update_hud_visibility`.
fn update_data_frame_visibility(
    hud_visible: Res<HudVisible>,
    data_frame_visible: Res<DataFrameVisible>,
    mut panels: Query<&mut Visibility, With<DataFrame>>,
) {
    let visibility = if hud_visible.0 && data_frame_visible.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut panel_visibility in &mut panels {
        panel_visibility.set_if_neq(visibility);
    }
}

/// The ability hotbar is left out of the HUD while testing the crosshair-only look; flip to
/// re-enable it.
const HOTBAR_ENABLED: bool = false;

/// Spawns the HUD roots on entering `GameState::InGame`.
pub fn spawn_in_game_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    let values = data_frame_values((0, 0), false, Value::Null, Value::Null);
    commands.spawn((
        markup::template(&asset_server, "data_frame.html"),
        template_context(&values),
        DataFrame { values },
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            right: px(0),
            ..default()
        },
        Visibility::Hidden,
        DespawnOnExit(GameState::InGame),
    ));
    commands.spawn((
        HudElement,
        Crosshair,
        markup::template(&asset_server, "crosshair.html"),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
        DespawnOnExit(GameState::InGame),
    ));
    if HOTBAR_ENABLED {
        commands.spawn((
            HudElement,
            Hotbar,
            markup::template(&asset_server, "hotbar.html"),
            TemplateContext::new()
                .with("damage", &DAMAGE)
                .with("attack_range", &ATTACK_RANGE),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(HOTBAR_BOTTOM_PADDING),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(GameState::InGame),
        ));
    }
}

/// A `TemplateContext` holding every key of the JSON object `values`.
fn template_context(values: &Value) -> TemplateContext {
    let mut context = TemplateContext::new();
    if let Value::Object(map) = values {
        for (key, value) in map {
            context.insert(key.clone(), value);
        }
    }
    context
}

/// Marks the crosshair root.
#[derive(Component)]
struct Crosshair;

/// The plain dot shown while the GCD is ready — hidden in favor of [`CrosshairGcdRing`] while it
/// runs. See `update_crosshair_gcd`.
#[derive(Component)]
struct CrosshairDot;

/// The radial GCD progress ring (`crosshair_gcd.wgsl`) shown in place of the dot while the GCD
/// runs; overlaps the dot in the crosshair's one grid cell.
#[derive(Component)]
struct CrosshairGcdRing;

/// Attaches the ring's material on every (re)build. Unpickability and the dot's roundness are
/// CSS (`.crosshair`, `.crosshair-dot`).
fn on_crosshair_built(
    built: On<HtmlUiBuilt>,
    crosshairs: Query<(), With<Crosshair>>,
    elements: HtmlElements,
    handle: Res<CrosshairGcdMaterialHandle>,
    mut commands: Commands,
) {
    let root = built.entity;
    if !crosshairs.contains(root) {
        return;
    }
    if let Some(ring) = elements.by_id(root, "crosshair-ring") {
        commands.entity(ring).insert((
            CrosshairGcdRing,
            MaterialNode(handle.0.clone()),
            Visibility::Hidden,
        ));
    }
    if let Some(dot) = elements.by_id(root, "crosshair-dot") {
        commands.entity(dot).insert(CrosshairDot);
    }
}

/// Marks the hotbar root.
#[derive(Component)]
struct Hotbar;

const HOTBAR_BOTTOM_PADDING: f32 = 20.0;

/// Attaches the shared GCD sweep to every slot's `.hotbar-gcd` overlay cell.
fn on_hotbar_built(
    built: On<HtmlUiBuilt>,
    hotbars: Query<(), With<Hotbar>>,
    elements: HtmlElements,
    handle: Res<GcdOverlayMaterialHandle>,
    mut commands: Commands,
) {
    if !hotbars.contains(built.entity) {
        return;
    }
    for overlay in elements.by_class(built.entity, "hotbar-gcd") {
        commands.entity(overlay).insert(MaterialNode(handle.0.clone()));
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

/// Swaps the crosshair between the plain dot (GCD ready) and the radial progress ring (GCD
/// running), and keeps the ring's fill in sync while it's shown — a per-frame component sync on
/// the built elements, never a re-render.
fn update_crosshair_gcd(
    local_player: Res<LocalPlayer>,
    player: Query<&Gcd>,
    handle: Res<CrosshairGcdMaterialHandle>,
    mut materials: ResMut<Assets<CrosshairGcdMaterial>>,
    mut dots: Query<&mut Visibility, (With<CrosshairDot>, Without<CrosshairGcdRing>)>,
    mut rings: Query<&mut Visibility, (With<CrosshairGcdRing>, Without<CrosshairDot>)>,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Ok(gcd) = player.get(local_player) else {
        return;
    };
    let running = !gcd.0.is_finished();
    let (dot_visibility, ring_visibility) = if running {
        (Visibility::Hidden, Visibility::Inherited)
    } else {
        (Visibility::Inherited, Visibility::Hidden)
    };
    for mut visibility in &mut dots {
        visibility.set_if_neq(dot_visibility);
    }
    for mut visibility in &mut rings {
        visibility.set_if_neq(ring_visibility);
    }
    if running && let Some(mut material) = materials.get_mut(&handle.0) {
        material.covered = Vec4::splat(gcd.0.fraction_remaining());
    }
}

/// The data frame's template variables: one Fluent args map per line (`*_args`), `hovered`/
/// `selected` `null` when there's nothing to show. Floats are pre-formatted as displayed, so
/// comparing two of these tells whether the panel would look different.
fn data_frame_values(hp: (i32, i32), grounded: bool, hovered: Value, selected: Value) -> Value {
    json!({
        "hp_args": { "hp": hp.0, "max_hp": hp.1 },
        "damage_args": { "damage": DAMAGE },
        "attack_range_args": { "range": ATTACK_RANGE },
        "select_range_args": { "range": SELECT_RANGE },
        "gcd_args": { "seconds": GCD_DURATION },
        "grounded_args": { "grounded": if grounded { "true" } else { "false" } },
        "hovered": hovered,
        "selected": selected,
    })
}

/// Re-renders the data frame while it's shown and its displayed values changed — never while
/// hidden, never for an identical frame.
fn update_data_frame(
    hud_visible: Res<HudVisible>,
    data_frame_visible: Res<DataFrameVisible>,
    local_player: Res<LocalPlayer>,
    mut frames: Query<(&mut DataFrame, &mut TemplateContext)>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    player: Query<(
        &HitPoints,
        &GlobalTransform,
        &bevy_ahoy::CharacterControllerState,
    )>,
    global_transforms: Query<&GlobalTransform>,
    hit_points: Query<&HitPoints>,
    names: Query<&Name>,
) {
    if !(hud_visible.0 && data_frame_visible.0) {
        return;
    }
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Ok((player_hit_points, player_transform, state)) = player.get(local_player) else {
        return;
    };

    let hovered = match hovered.0 {
        Some((entity, distance)) => json!({
            "entity_args": { "entity": format!("{entity:?}") },
            "distance_args": { "distance": format!("{distance:.2}") },
        }),
        None => Value::Null,
    };

    let selected = match selected.0 {
        Some(entity) => {
            let distance = global_transforms.get(entity).ok().map(|transform| {
                let distance = (transform.translation() - player_transform.translation()).length();
                json!({ "distance": format!("{distance:.2}") })
            });
            let name = names
                .get(entity)
                .ok()
                .map(|name| json!({ "name": name.as_str() }));
            let hp = hit_points
                .get(entity)
                .ok()
                .map(|target| json!({ "hp": target.hit_points, "max_hp": target.max_hit_points }));
            json!({
                "entity_args": { "entity": format!("{entity:?}") },
                "distance_args": distance,
                "name_args": name,
                "hp_args": hp,
            })
        }
        None => Value::Null,
    };

    let values = data_frame_values(
        (
            player_hit_points.hit_points,
            player_hit_points.max_hit_points,
        ),
        state.grounded.is_some(),
        hovered,
        selected,
    );
    for (mut frame, mut context) in &mut frames {
        if frame.values != values {
            *context = template_context(&values);
            frame.values = values.clone();
        }
    }
}
