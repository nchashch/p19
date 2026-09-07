use crate::assets::collections::CommonAssets;
use crate::ui::localization::{LocalizedText, localized};
use bevy::{
    color::palettes::css::{DARK_SLATE_GRAY, SLATE_GRAY, WHITE, WHITE_SMOKE},
    input_focus::InputFocus,
    prelude::*,
    text::FontSourceTemplate,
    ui::auto_directional_navigation::AutoDirectionalNavigation,
};
use bevy_fluent::prelude::Localization;
use fluent::FluentArgs;

/// Shared, screen-agnostic UI building blocks (`panel`, `button`, tooltips) used by both the main
/// menu (`ui.rs`) and the in-game HUD (`hud.rs`) — kept dependency-free of either so it stays a
/// leaf module.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(show_tooltip);
        app.add_observer(hide_tooltip);
        app.add_systems(Update, update_button_focus);
    }
}

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

/// `label_key` is a Fluent message key (see `assets/locales/`), not display text — `LocalizedText`
/// fills in the real label once localization is ready, using the key itself as the placeholder
/// shown for the frame or two before that. `font` is an already-cloned `Handle<Font>` (from
/// `CommonAssets.serif_font`), not `&CommonAssets` — `bsn!`'s generated `Scene` type must be
/// `'static`, and a borrowed `&CommonAssets` parameter makes edition-2024's default
/// return-position-`impl-Trait` lifetime capture pull that borrow's lifetime into `impl Scene`'s
/// hidden type, which doesn't satisfy that. An owned `Handle<Font>` has no such lifetime to fight.
pub fn button(width: Val, height: Val, label_key: &'static str) -> impl Scene {
    bsn! {
        Button
        // Makes the button a candidate for gamepad/keyboard directional navigation (see
        // `ui.rs`'s `MenuControls`) — edges are computed automatically from screen position,
        // no manual graph to maintain.
        AutoDirectionalNavigation
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
        on(activate_on_press)
        Children [(
            Text(label_key)
            LocalizedText(label_key)
            TextFont {
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

/// Fired on a `Button` when it's activated — a real pointer press, or gamepad/keyboard "confirm"
/// while it holds `InputFocus` (see `ui.rs`'s `on_ui_confirm`). Call sites observe this instead
/// of `Pointer<Press>` directly so both activation paths share one handler.
#[derive(EntityEvent, Clone)]
pub struct Activate {
    pub entity: Entity,
}

fn activate_on_press(press: On<Pointer<Press>>, mut commands: Commands) {
    commands.trigger(Activate {
        entity: press.entity,
    });
}

/// Highlights whichever `Button` currently holds `InputFocus` the same way `hover_button` does,
/// so gamepad/keyboard navigation has a visible cursor — reacts only to focus *changes*, so it
/// doesn't fight `hover_button`/`out_button` on every frame.
fn update_button_focus(
    focus: Res<InputFocus>,
    buttons: Query<Entity, With<Button>>,
    mut commands: Commands,
) {
    if !focus.is_changed() {
        return;
    }
    for entity in &buttons {
        let color = if focus.get() == Some(entity) {
            BUTTON_HOVERED_COLOR
        } else {
            BUTTON_COLOR
        };
        commands
            .entity(entity)
            .insert(BackgroundColor(color.into()));
    }
}

/// A value substituted into a `Tooltip`'s `.ftl` message — see `fluent_bundle::FluentValue`, which
/// this converts into at resolve time (`show_tooltip`). Kept as its own small enum instead of
/// storing `fluent::FluentArgs` directly on the component so `Tooltip` stays plain owned data
/// (`Default`/`Clone`, no borrowed lifetime) rather than something tied to a `FluentArgs<'a>`.
#[derive(Clone)]
pub enum TooltipArg {
    Number(f64),
    Text(String),
}

impl From<i32> for TooltipArg {
    fn from(value: i32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<f32> for TooltipArg {
    fn from(value: f32) -> Self {
        Self::Number(value as f64)
    }
}

impl From<&str> for TooltipArg {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

/// A tooltip shown on hover, resolved from a Fluent message key (see `assets/locales/`) rather than
/// a raw string — resolution happens lazily in `show_tooltip`, at hover time, not at spawn time, so
/// it never races localization still loading (unlike `LocalizedText`, which can spawn before the
/// locale folder finishes loading — a tooltip can't be hovered before the game is already running).
#[derive(Component, Clone, Default)]
pub struct Tooltip {
    key: &'static str,
    args: Vec<(&'static str, TooltipArg)>,
}

impl Tooltip {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            args: Vec::new(),
        }
    }

    /// Builds a `Tooltip` from an already-assembled arg list in one call — for passing a tooltip
    /// across a function boundary (e.g. into `hud.rs`'s `ability_slot`) and installing it as a
    /// component inside a `bsn!` block, which needs a call expression there, not a bare variable.
    pub fn with_args(key: &'static str, args: Vec<(&'static str, TooltipArg)>) -> Self {
        Self { key, args }
    }
}

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
    // `Option`, not a bare `Res` — confirmed by testing, not just theory: a `Pointer<Over>` can
    // fire (and did, in practice) on a `Tooltip`-bearing, `Panel`-adjacent entity that exists
    // before `GameState::AssetLoading` finishes and inserts `CommonAssets` (e.g. dev-console/FPS
    // overlay UI, which `console.rs` adds unconditionally, independent of `GameState`).
    common_assets: Option<Res<CommonAssets>>,
    localization: Option<Res<Localization>>,
    mut commands: Commands,
) {
    let Ok(_panel_entity) = panel.single() else {
        return;
    };
    let Ok((tip, above)) = tips.get(over.entity) else {
        return;
    };
    let Some(localization) = localization else {
        return;
    };
    let Some(common_assets) = common_assets else {
        return;
    };
    let mut fluent_args = FluentArgs::new();
    for (name, arg) in &tip.args {
        match arg {
            TooltipArg::Number(value) => fluent_args.set(*name, *value),
            TooltipArg::Text(value) => fluent_args.set(*name, value.clone()),
        }
    }
    let text = localized(&localization, tip.key, &fluent_args);
    let font = common_assets.serif_font.clone();
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
            Text::new(text),
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
