use crate::game_state::GameState;
use crate::player_character::PlayerCharacter;
use crate::targeting::{Hovered, SELECT_RANGE, Selected};
use crate::widgets::{PANEL_BORDER_COLOR, PANEL_COLOR, SERIF_FONT, Tooltip, TooltipAbove, panel};
use bevy::{
    color::palettes::css::{WHITE, WHITE_SMOKE},
    prelude::*,
    reflect::TypePath,
    render::render_resource::*,
    shader::ShaderRef,
    text::FontSourceTemplate,
};
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

const CROSSHAIR_SIZE: f32 = 4.0;

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
    let attack_description = format!(
        "Attack the selected target for {DAMAGE} damage (range {ATTACK_RANGE:.0}m). Shares the global cooldown."
    );
    let kill_description = format!(
        "Instantly kill the selected target (range {ATTACK_RANGE:.0}m). Shares the global cooldown."
    );
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
            ability_slot("A", "f", attack_description),
            ability_slot("K", "t", kill_description),
            ability_slot(
                "N", "r",
                "Spawn an NPC at the spawn point. Shares the global cooldown.".to_string(),
            ),
            ability_slot(
                "C", "e",
                "Spawn a cube, launched in the direction you're aiming. Shares the global cooldown.".to_string(),
            ),
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
fn ability_slot(ability_letter: &str, hotkey_letter: &str, description: String) -> impl Scene {
    bsn! {
        hotbar_slot()
        Tooltip(description)
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
    player: Query<&Gcd, With<PlayerCharacter>>,
    handle: Res<GcdOverlayMaterialHandle>,
    mut materials: ResMut<Assets<GcdOverlayMaterial>>,
) {
    let Ok(gcd) = player.single() else {
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
    mut query: Query<&mut Text, With<DataFrame>>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    player: Query<(&HitPoints, &GlobalTransform), With<PlayerCharacter>>,
    global_transforms: Query<&GlobalTransform>,
    hit_points: Query<&HitPoints>,
    name: Query<&Name>,
) {
    if !hovered.is_changed() && !selected.is_changed() {
        return;
    }
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    let Ok((player_hit_points, player_global_transform)) = player.single() else {
        return;
    };
    text.0 = format!(
        "Press ~ for console\n\nHP: {}/{}\nDamage: {}\nAttack range: {}m\nSelect range: {}m\nGCD: {}s\n\n",
        player_hit_points.hit_points,
        player_hit_points.max_hit_points,
        DAMAGE,
        ATTACK_RANGE,
        SELECT_RANGE,
        GCD_DURATION,
    );

    if let Some((entity, distance)) = hovered.0 {
        text.0 += &format!("Hovered: {:?}\nDistance: {:.2}\n\n", entity, distance);
    } else {
        text.0 += &format!("Hovered: n/a\nDistance: n/a\n\n");
    }

    if let Some(entity) = selected.0 {
        let distance_to_selected = {
            match global_transforms.get(entity) {
                Ok(selected_global_transform) => Some(
                    (selected_global_transform.compute_transform().translation
                        - player_global_transform.compute_transform().translation)
                        .length(),
                ),
                Err(_) => None,
            }
        };
        text.0 += &format!("Selected: {:?}\n", entity);
        match distance_to_selected {
            Some(distance_to_selected) => {
                text.0 += &format!("Distance: {:.2}\n", distance_to_selected);
            }
            None => text.0 += &format!("Distance: n/a\n"),
        };
        if let Some(name) = name.get(entity).ok() {
            text.0 += &format!("Name: {name}\n");
        }
        if let Some(hit_points) = hit_points.get(entity).ok() {
            text.0 += &format!(
                "HP: {}/{}\n",
                hit_points.hit_points, hit_points.max_hit_points
            );
        }
    }
}
