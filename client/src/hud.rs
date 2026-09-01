use crate::game_state::GameState;
use crate::localization::localized;
use crate::player_character::LocalPlayer;
use crate::targeting::{Hovered, SELECT_RANGE, Selected};
use crate::widgets::{
    PANEL_BORDER_COLOR, PANEL_COLOR, SERIF_FONT, Tooltip, TooltipAbove, TooltipArg, panel,
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

/// The always-visible in-game HUD: the `DataFrame` debug panel, the ability hotbar (with its GCD
/// cooldown-sweep overlay), and the crosshair.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<GcdOverlayMaterial>::default());
        app.add_systems(Startup, setup_gcd_overlay_material);
        app.add_observer(add_gcd_overlay);
        app.add_systems(Update, (update_data_frame, update_gcd_overlay));
    }
}

pub fn in_game_scene() -> impl SceneList {
    bsn_list![data_frame(), hotbar(), crosshair(),]
}

const CROSSHAIR_SIZE: f32 = 8.0;

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
                    width: px(CROSSHAIR_SIZE),
                    height: px(CROSSHAIR_SIZE),
                    border_radius: px(CROSSHAIR_SIZE / 2.0),
                }
                BackgroundColor(WHITE)
                Pickable::IGNORE
            ),
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

fn data_frame() -> impl Scene {
    bsn! {
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

fn hotbar() -> impl Scene {
    bsn! {
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
            ),
            ability_slot(
                "K", "t", "hud-kill-tooltip",
                vec![("range", ATTACK_RANGE.into())],
            ),
            ability_slot("N", "r", "hud-spawn-npc-tooltip", vec![]),
            ability_slot("C", "e", "hud-spawn-cube-tooltip", vec![]),
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
) -> impl Scene {
    bsn! {
        hotbar_slot()
        Tooltip::with_args(tooltip_key, tooltip_args)
        TooltipAbove(HOTBAR_SLOT_SIZE)
        Children [
            (
                Text(ability_letter)
                TextFont {
                    font: FontSourceTemplate::Handle(SERIF_FONT),
                    font_size: px(ABILITY_LETTER_FONT_SIZE),
                }
                TextColor(WHITE)
                Pickable::IGNORE
            ),
            (
                Text(hotkey_letter)
                TextFont {
                    font: FontSourceTemplate::Handle(SERIF_FONT),
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
        let distance_to_selected = global_transforms.get(entity).ok().map(|selected_transform| {
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
