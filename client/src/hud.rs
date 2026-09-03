use crate::game_state::{GameState, InputDeviceState};
use crate::input_icons::{
    GAMEPAD_LOOK_STICK_ICON_PNG, GAMEPAD_MOVE_STICK_ICON_PNG, MOUSE_MOVE_ICON_PNG,
    gamepad_button_icon_png, key_code_icon_png, mouse_button_icon_png,
};
use crate::localization::{LocalizedText, localized};
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
        app.insert_resource(HudVisible(true));
        app.add_plugins(UiMaterialPlugin::<GcdOverlayMaterial>::default());
        app.add_systems(Startup, setup_gcd_overlay_material);
        app.add_observer(add_gcd_overlay);
        app.add_systems(
            Update,
            (
                update_data_frame,
                update_gcd_overlay,
                update_hud_visibility,
                update_controls_tips_visibility,
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

/// Marks each of `in_game_scene`'s top-level roots that only depend on `HudVisible`
/// (`data_frame`/`hotbar`/`crosshair`) so `update_hud_visibility` can find and toggle all of them
/// without needing a single common parent — they're independent root entities (see
/// `in_game_scene`'s `bsn_list!`), not siblings under one `Node`. The two `controls_tips` panels
/// are deliberately *not* tagged with this — see `update_controls_tips_visibility`.
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

/// Marks `controls_tips`'s root — shown only while `HudVisible` *and* `InputDeviceState` is
/// `KeyboardMouse`. See `update_controls_tips_visibility`.
#[derive(Component, Clone, Default)]
struct KeyboardMouseControlsTips;

/// Marks `gamepad_controls_tips`'s root — the mirror image of `KeyboardMouseControlsTips`, shown
/// only while `InputDeviceState` is `Gamepad`.
#[derive(Component, Clone, Default)]
struct GamepadControlsTips;

/// Combines two independent conditions (`HudVisible` and `InputDeviceState`) into the visibility
/// of whichever controls-tips panel matches the currently active input device — kept as its own
/// system rather than folding these two panels into `update_hud_visibility`'s plain `HudElement`
/// handling, since that system only knows about one condition (`HudVisible`) and would otherwise
/// force both panels visible together the instant the HUD is shown, regardless of which device is
/// actually in use.
fn update_controls_tips_visibility(
    hud_visible: Res<HudVisible>,
    input_device: Res<State<InputDeviceState>>,
    mut keyboard_mouse: Query<
        &mut Visibility,
        (
            With<KeyboardMouseControlsTips>,
            Without<GamepadControlsTips>,
        ),
    >,
    mut gamepad: Query<
        &mut Visibility,
        (
            With<GamepadControlsTips>,
            Without<KeyboardMouseControlsTips>,
        ),
    >,
) {
    let visibility = |show: bool| {
        if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    let show_keyboard_mouse =
        hud_visible.0 && *input_device.get() == InputDeviceState::KeyboardMouse;
    let show_gamepad = hud_visible.0 && *input_device.get() == InputDeviceState::Gamepad;
    for mut element_visibility in &mut keyboard_mouse {
        *element_visibility = visibility(show_keyboard_mouse);
    }
    for mut element_visibility in &mut gamepad {
        *element_visibility = visibility(show_gamepad);
    }
}

pub fn in_game_scene() -> impl SceneList {
    bsn_list![
        data_frame(),
        hotbar(),
        crosshair(),
        controls_tips(),
        gamepad_controls_tips(),
    ]
}

const CONTROLS_TIPS_ICON_SIZE: f32 = 40.0;
const CONTROLS_TIPS_ICON_GAP: f32 = 4.0;
const CONTROLS_TIPS_LABEL_FONT_SIZE: f32 = 16.0;

/// Top-left panel listing every keyboard/mouse binding `controls.rs`'s `player_controls()` sets up
/// — kept in sync with that list by hand, same as `console.ftl`'s `console-controls` text hint is;
/// there's no single source of truth `bevy_enhanced_input` bindings could be introspected from
/// automatically. Using the actual `KeyCode`s here (via `control_tip_keys`/`input_icons`) at least
/// makes *that* part self-documenting — a row visibly says `KeyCode::Escape`, not an opaque asset
/// path with nothing connecting it back to which key it's supposed to be. Mouse buttons/motion
/// aren't `KeyCode`, so `hud-controls-look`/`hud-controls-select` go through `control_tip_icons`
/// directly with a PNG path from `input_icons::mouse_button_icon_png`/`MOUSE_MOVE_ICON_PNG`
/// instead of a `key_code_icon_png` lookup. `mouse_move` (for `FpsCameraRotation`'s mouse-motion
/// binding) is included since it's a real, always-on control, even though it isn't a discrete
/// key/button press. See `input_icons`'s module doc comment for why this renders each icon as its
/// own PNG (`ImageNode`) rather than packing glyphs into a `Text` run with an icon font.
fn controls_tips() -> impl Scene {
    bsn! {
        KeyboardMouseControlsTips
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
        }
        Children[
            panel(px(300), px(600))
            Children [
                control_tip_keys(
                    &[KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD],
                    "hud-controls-move",
                ),
                control_tip_icons(vec![MOUSE_MOVE_ICON_PNG], "hud-controls-look"),
                control_tip_keys(&[KeyCode::Space], "hud-controls-jump"),
                control_tip_mouse_button(MouseButton::Left, "hud-controls-select"),
                control_tip_mouse_button(MouseButton::Right, "hud-controls-deselect"),
                control_tip_keys(&[KeyCode::KeyF], "hud-controls-attack"),
                control_tip_keys(&[KeyCode::KeyT], "hud-controls-kill"),
                control_tip_keys(&[KeyCode::KeyE], "hud-controls-spawn-cube"),
                control_tip_keys(&[KeyCode::KeyR], "hud-controls-spawn-npc"),
                control_tip_keys(&[KeyCode::Escape], "hud-controls-main-menu"),
            ]
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

/// The gamepad equivalent of `controls_tips` — same rows, same order, same labels, just Steam
/// Deck button/stick icons (`input_icons::gamepad_button_icon_png`) sourced from the actual
/// `GamepadButton`s `controls.rs`'s `player_controls()` binds, instead of `KeyCode`s. Shown
/// instead of `controls_tips` (never alongside it) once `InputDeviceState` says a gamepad is the
/// active device — see `update_controls_tips_visibility`.
fn gamepad_controls_tips() -> impl Scene {
    bsn! {
        GamepadControlsTips
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
        }
        Children[
            panel(px(300), px(600))
            Children [
                control_tip_icons(vec![GAMEPAD_MOVE_STICK_ICON_PNG], "hud-controls-move"),
                control_tip_icons(vec![GAMEPAD_LOOK_STICK_ICON_PNG], "hud-controls-look"),
                control_tip_gamepad_buttons(&[GamepadButton::South], "hud-controls-jump"),
                control_tip_gamepad_buttons(&[GamepadButton::RightThumb], "hud-controls-select"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftThumb], "hud-controls-deselect"),
                control_tip_gamepad_buttons(&[GamepadButton::RightTrigger2], "hud-controls-attack"),
                control_tip_gamepad_buttons(&[GamepadButton::RightTrigger], "hud-controls-kill"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftTrigger], "hud-controls-spawn-cube"),
                control_tip_gamepad_buttons(&[GamepadButton::LeftTrigger2], "hud-controls-spawn-npc"),
                control_tip_gamepad_buttons(&[GamepadButton::Start], "hud-controls-main-menu"),
            ]
        ]
        DespawnOnExit::<GameState>(GameState::InGame)
    }
}

/// Same idea as `control_tip_keys`, for `GamepadButton`s instead of `KeyCode`s — any button the
/// pack doesn't cover is silently skipped, same contract as `input_icons::gamepad_button_icon_png`.
fn control_tip_gamepad_buttons(buttons: &[GamepadButton], label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = buttons
        .iter()
        .filter_map(|&button| gamepad_button_icon_png(button))
        .collect();
    control_tip_icons(icons, label_key)
}

/// Builds a `control_tip_icons` row directly from the `KeyCode`s a binding actually uses, via
/// `input_icons::key_code_icon_png` — any key the pack doesn't cover is silently skipped rather
/// than showing a broken image or panicking, so an unmapped key just quietly narrows the icon set
/// for that row instead of breaking it.
fn control_tip_keys(keys: &[KeyCode], label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = keys
        .iter()
        .filter_map(|&key| key_code_icon_png(key))
        .collect();
    control_tip_icons(icons, label_key)
}

/// Same idea as `control_tip_keys`, for the one mouse-button tip (`Select`) — not a `KeyCode`, so
/// it goes through `input_icons::mouse_button_icon_png` instead.
fn control_tip_mouse_button(button: MouseButton, label_key: &'static str) -> impl Scene {
    let icons: Vec<&'static str> = mouse_button_icon_png(button).into_iter().collect();
    control_tip_icons(icons, label_key)
}

/// One row: zero or more icon images side by side (e.g. `W A S D` as four separate `ImageNode`s,
/// left to right) followed by a localized label. `icons` is a `Vec` rather than a fixed-size slice
/// since a binding can use any number of keys, including zero if none of them mapped to an icon —
/// the row then just shows the label on its own instead of disappearing entirely, so a gap in
/// icon coverage stays visible/debuggable rather than silently dropping the whole tip.
fn control_tip_icons(icons: Vec<&'static str>, label_key: &'static str) -> impl Scene {
    let icons: Vec<_> = icons.into_iter().map(control_tip_icon).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        Pickable::IGNORE
        Children [
            {icons},
            (
                Text(label_key)
                LocalizedText(label_key)
                TextFont {
                    font: FontSourceTemplate::Handle(SERIF_FONT),
                    font_size: px(CONTROLS_TIPS_LABEL_FONT_SIZE),
                }
                TextColor(WHITE)
                Pickable::IGNORE
            ),
        ]
    }
}

/// A single icon image at the panel's fixed icon size — one `ImageNode` per key/button, laid out
/// in a row by `control_tip_icons`'s parent `Node` rather than packed into one `Text` the way the
/// font-glyph version did.
fn control_tip_icon(path: &'static str) -> impl Scene {
    bsn! {
        ImageNode { image: path }
        Node {
            width: px(CONTROLS_TIPS_ICON_SIZE),
            height: px(CONTROLS_TIPS_ICON_SIZE),
            margin: UiRect::right(px(CONTROLS_TIPS_ICON_GAP)),
        }
        Pickable::IGNORE
    }
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
        HudElement
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
