use bevy::{
    color::palettes::css::{DARK_SLATE_GRAY, SLATE_GRAY, WHITE, WHITE_SMOKE},
    prelude::*,
    text::FontSourceTemplate,
};

/// Shared, screen-agnostic UI building blocks (`panel`, `button`, tooltips) used by both the main
/// menu (`ui.rs`) and the in-game HUD (`hud.rs`) — kept dependency-free of either so it stays a
/// leaf module.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(show_tooltip);
        app.add_observer(hide_tooltip);
    }
}

pub const SERIF_FONT: &str = "fonts/serif/IBMPlexSerif-Regular.ttf";

pub const PANEL_BORDER_COLOR: Srgba = WHITE_SMOKE;
pub const PANEL_COLOR: Srgba = DARK_SLATE_GRAY;
const BUTTON_BORDER_COLOR: Srgba = WHITE_SMOKE;
const BUTTON_COLOR: Srgba = DARK_SLATE_GRAY;
const BUTTON_HOVERED_COLOR: Srgba = SLATE_GRAY;
const BUTTON_TEXT_COLOR: Srgba = WHITE;
const BUTTON_TEXT_FONT_SIZE: f32 = 33.0;

#[derive(Component, Clone, Default)]
pub struct Panel;

pub fn panel(width: Val, height: Val) -> impl Scene {
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

pub fn button(width: Val, height: Val, label: &str) -> impl Scene {
    bsn! {
        Button
        Node {
            width,
            height,
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
pub struct Tooltip(pub String);

/// Positions the tooltip above the hovered entity instead of the default side offset — for
/// entities that sit at the bottom edge of the screen with no room below them (e.g. the HUD
/// hotbar). Carries the anchor's own height so this module doesn't need to know any caller's
/// layout constants — the caller passes its own size in (see `hud.rs`'s `ability_slot`).
#[derive(Component, Clone, Default)]
pub struct TooltipAbove(pub f32);

const TOOLTIP_GAP: f32 = 8.0;

#[derive(Component)]
struct TooltipUi; // marks the spawned tooltip so we can find/despawn it

fn show_tooltip(
    over: On<Pointer<Over>>,
    tips: Query<(&Tooltip, Option<&TooltipAbove>)>,
    panel: Query<Entity, With<Panel>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(_panel_entity) = panel.single() else {
        return;
    };
    let Ok((tip, above)) = tips.get(over.entity) else {
        return;
    };
    let font = asset_server.load(SERIF_FONT);
    let position = if let Some(above) = above {
        Node {
            position_type: PositionType::Absolute, // escape flex flow, free to overlap
            bottom: px(above.0 + TOOLTIP_GAP),
            left: px(0),
            max_width: px(240),
            padding: UiRect::all(px(6)),
            ..default()
        }
    } else {
        Node {
            position_type: PositionType::Absolute, // escape flex flow, free to overlap
            left: px(210),
            top: px(33),
            max_width: px(240),
            padding: UiRect::all(px(6)),
            ..default()
        }
    };
    commands.entity(over.entity).with_child((
        TooltipUi,
        position,
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
