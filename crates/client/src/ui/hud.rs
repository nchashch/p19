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
        app.define_html_element("crosshair-gcd-ring", crosshair_gcd_ring)
            .define_html_element(
                "crosshair-dot",
                |dot: In<ElementConnected>, mut commands: Commands| {
                    commands.entity(dot.entity).insert(CrosshairDot);
                },
            )
            .define_html_element("gcd-overlay", gcd_overlay);
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

/// The data frame's root — see `update_data_frame`.
#[derive(Component)]
struct DataFrame;

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
    let values = data_frame_values((0, 0), Some(false), Value::Null, Value::Null);
    commands.spawn((
        markup::template(&asset_server, "data_frame.html"),
        template_context(&values),
        DataFrame,
        Visibility::Hidden,
        DespawnOnExit(GameState::InGame),
    ));
    commands.spawn((
        HudElement,
        markup::template(&asset_server, "crosshair.html"),
        DespawnOnExit(GameState::InGame),
    ));
    if HOTBAR_ENABLED {
        commands.spawn((
            HudElement,
            markup::template(&asset_server, "hotbar.html"),
            TemplateContext::new()
                .with("damage", &DAMAGE)
                .with("attack_range", &ATTACK_RANGE),
            DespawnOnExit(GameState::InGame),
        ));
    }
}

/// The plain dot shown while the GCD is ready — hidden in favor of [`CrosshairGcdRing`] while it
/// runs. See `update_crosshair_gcd`.
#[derive(Component)]
struct CrosshairDot;

/// The radial GCD progress ring (`crosshair_gcd.wgsl`) shown in place of the dot while the GCD
/// runs; overlaps the dot in the crosshair's one grid cell.
#[derive(Component)]
struct CrosshairGcdRing;

/// `<div is="crosshair-gcd-ring">`: the ring's material, hidden until the GCD runs.
/// Unpickability and the dot's roundness are CSS (`.crosshair`, `.crosshair-dot`).
fn crosshair_gcd_ring(
    ring: In<ElementConnected>,
    handle: Res<CrosshairGcdMaterialHandle>,
    mut commands: Commands,
) {
    commands.entity(ring.entity).insert((
        CrosshairGcdRing,
        MaterialNode(handle.0.clone()),
        Visibility::Hidden,
    ));
}

/// `<div is="gcd-overlay">`: a hotbar slot's share of the global GCD sweep.
fn gcd_overlay(
    overlay: In<ElementConnected>,
    handle: Res<GcdOverlayMaterialHandle>,
    mut commands: Commands,
) {
    commands
        .entity(overlay.entity)
        .insert(MaterialNode(handle.0.clone()));
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
/// `selected` `null` when there's nothing to show. Floats are pre-formatted as displayed, so an
/// unchanged panel renders identical HTML and bevy_markup skips the rebuild.
fn data_frame_values(
    hp: (i32, i32),
    grounded: Option<bool>,
    hovered: Value,
    selected: Value,
) -> Value {
    json!({
        "hp_args": { "hp": hp.0, "max_hp": hp.1 },
        "damage_args": { "damage": DAMAGE },
        "attack_range_args": { "range": ATTACK_RANGE },
        "select_range_args": { "range": SELECT_RANGE },
        "gcd_args": { "seconds": GCD_DURATION },
        "grounded_args": { "grounded": match grounded {
            Some(true) => "true",
            Some(false) => "false",
            None => "—",
        } },
        "hovered": hovered,
        "selected": selected,
    })
}

/// Writes the data frame's values while it's shown (never while hidden). bevy_markup rebuilds
/// only when the rendered panel actually differs.
fn update_data_frame(
    hud_visible: Res<HudVisible>,
    data_frame_visible: Res<DataFrameVisible>,
    local_player: Res<LocalPlayer>,
    mut frames: Query<&mut TemplateContext, With<DataFrame>>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    // No `CharacterControllerState` without prediction (no client-side KCC): grounded unknown.
    player: Query<(
        &HitPoints,
        &GlobalTransform,
        Option<&bevy_ahoy::CharacterControllerState>,
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
        state.map(|state| state.grounded.is_some()),
        hovered,
        selected,
    );
    for mut context in &mut frames {
        *context = template_context(&values);
    }
}

/// A `TemplateContext` holding every key of the JSON object `values`.
fn template_context(values: &Value) -> TemplateContext {
    bevy_markup::tera::Context::from_serialize(values)
        .expect("data frame values are a JSON object")
        .into()
}
