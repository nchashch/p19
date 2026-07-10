use crate::game_state::GameState;
use crate::player_character::{DESPAWN_RANGE, Hovered, SELECT_RANGE, Selected};
use crate::{LevelScene, cube_spawner::Cube};
use bevy::color::palettes::css::{BLUE, GRAY, RED};
use bevy::{
    color::palettes::css::{BLACK, DARK_SLATE_GRAY, SLATE_GRAY, WHITE, WHITE_SMOKE},
    prelude::*,
    text::FontSourceTemplate,
};
use bevy_mod_outline::OutlineVolume;

pub struct PrototypeUiPlugin;

impl Plugin for PrototypeUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(show_tooltip);
        app.add_observer(hide_tooltip);
        app.add_systems(Update, (update_console, update_outline_hovered_selected));
    }
}

pub fn main_menu_scene() -> impl SceneList {
    bsn_list![main_menu()]
}

fn main_menu() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Start,
        }
        Children [
            panel(px(400), px(400))
            Children [
                (
                    button("play")
                    Tooltip("Start the game.")
                    on(play_button)
                ),
            ]
        ]
        WorldAssetRoot("MenuBackground.glb#Scene0")
        DespawnOnExit::<GameState>(GameState::MainMenu)
    }
}

pub fn in_game_scene() -> impl SceneList {
    bsn_list![ui()]
}

fn ui() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::End,
            justify_content: JustifyContent::Start,
        }
        Children[
            panel(px(400), px(400))
            Children [
                (
                    Text("[WASD] move")
                ),
                (
                    Text("[RMB] rotate camera")
                ),
                (
                    Text("[LMB] select")
                ),
                (
                    Text("[ESC] deselect")
                ),
                (
                    Text("[E] spawn cube")
                ),
                (
                    Text("[T] despawn selected")
                ),
                (
                    Text("[R] respawn")
                ),
                (
                    Text("[Q] despawn all cubes")
                ),
                (
                    Text("")
                    Console
                ),
            ]
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

#[derive(Component, Clone, Default)]
struct Panel;

fn panel(width: Val, height: Val) -> impl Scene {
    bsn! {
        Panel
        Node {
            width: width,
            height: height,
            border: px(2),
            border_radius: px(3),
            margin: UiRect::axes(px(50), px(50)),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            column_gap: px(10),
            padding: px(10),
        }
        BorderColor::from(PANEL_BORDER_COLOR)
        BackgroundColor(PANEL_COLOR)
    }
}

fn button(label: &str) -> impl Scene {
    bsn! {
        Button
        Node {
            width: px(200),
            height: px(65),
            border: px(2),
            border_radius: px(3),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BorderColor::from(BUTTON_BORDER_COLOR)
        BackgroundColor(BUTTON_COLOR)
        on(hover_button)
        on(out_button)
        Children [(
            Text(label)
            TextFont {
                font: FontSourceTemplate::Handle(SERIF_FONT),
                font_size: px(BUTTON_TEXT_FONT_SIZE),
            }
            TextColor(BUTTON_TEXT_COLOR)
        )]
    }
}

const MONO_FONT: &str = "fonts/mono/IBMPlexMono-Regular.ttf";
const SERIF_FONT: &str = "fonts/serif/IBMPlexSerif-Regular.ttf";

const PANEL_BORDER_COLOR: Srgba = WHITE_SMOKE;
const PANEL_COLOR: Srgba = BLACK;
const BUTTON_BORDER_COLOR: Srgba = WHITE_SMOKE;
const BUTTON_COLOR: Srgba = DARK_SLATE_GRAY;
const BUTTON_HOVERED_COLOR: Srgba = SLATE_GRAY;
const BUTTON_TEXT_COLOR: Srgba = WHITE;
const BUTTON_TEXT_FONT_SIZE: f32 = 33.0;

fn play_button(_event: On<Pointer<Press>>, mut commands: Commands) {
    commands.set_state(GameState::Loading);
}

fn menu_button(_event: On<Pointer<Press>>, mut commands: Commands) {
    commands.set_state(GameState::MainMenu);
}

fn despawn_button(
    _event: On<Pointer<Press>>,
    mut commands: Commands,
    cubes: Query<Entity, With<Cube>>,
) {
    for cube in cubes {
        commands.entity(cube).despawn();
    }
}

fn hover_button(event: On<Pointer<Over>>, mut commands: Commands) {
    commands
        .entity(event.entity)
        .insert(BackgroundColor(BUTTON_HOVERED_COLOR.into()));
}

fn out_button(event: On<Pointer<Out>>, mut commands: Commands) {
    commands
        .entity(event.entity)
        .insert(BackgroundColor(BUTTON_COLOR.into()));
}

#[derive(Component, Clone, Default)]
struct Tooltip(String);

#[derive(Component)]
struct TooltipUi; // marks the spawned tooltip so we can find/despawn it

fn show_tooltip(
    over: On<Pointer<Over>>,
    tips: Query<&Tooltip>,
    panel: Query<Entity, With<Panel>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(panel_entity) = panel.single() else {
        return;
    };
    let Ok(tip) = tips.get(over.entity) else {
        return;
    };
    let font = asset_server.load(SERIF_FONT);
    commands.entity(over.entity).with_child((
        TooltipUi,
        Node {
            position_type: PositionType::Absolute, // escape flex flow, free to overlap
            left: px(210),
            top: px(33),
            padding: UiRect::all(px(6)),
            ..default()
        },
        Pickable::IGNORE,
        GlobalZIndex(1000), // draw above all other UI
        BackgroundColor(TOOLTIP_BACKGROUND_COLOR.into()),
        children![(
            Text::new(tip.0.clone()),
            TextColor(Color::WHITE),
            Pickable::IGNORE,
            TextFont {
                font: FontSource::Handle(font),
                font_size: FontSize::Px(TOOLTIP_TEXT_FONT_SIZE),
                ..Default::default()
            }
        )],
    ));
}

const TOOLTIP_BACKGROUND_COLOR: Srgba = DARK_SLATE_GRAY;
const TOOLTIP_TEXT_FONT_SIZE: f32 = 24.;

fn hide_tooltip(_out: On<Pointer<Out>>, q: Query<Entity, With<TooltipUi>>, mut commands: Commands) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[derive(Component, Clone, Default, Debug)]
struct Console;

fn update_console(
    mut query: Query<&mut Text, With<Console>>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
) {
    if !hovered.is_changed() && !selected.is_changed() {
        return;
    }
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    text.0 = format!("Hovered: {:?}\nSelected: {:?}", hovered.0, selected.0);
}

fn update_outline_hovered_selected(
    query: Query<(Entity, &mut OutlineVolume)>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
) {
    if !hovered.is_changed() {
        return;
    }
    for (entity, mut outline) in query {
        if let Some((hovered_entity, distance)) = hovered.0 {
            if selected.0 == Some(entity) {
                outline.colour = WHITE.into();
                outline.visible = true;
            } else if hovered_entity == entity {
                if distance < SELECT_RANGE {
                    outline.colour = GRAY.into();
                    outline.visible = true;
                } else {
                    outline.colour = DARK_SLATE_GRAY.into();
                    outline.visible = true;
                }
            } else {
                outline.visible = false;
            }
        }
    }
}
