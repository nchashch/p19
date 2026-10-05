//! Screen-space nameplates (name + health bar, `html/nameplate.html`) above every entity with
//! `HitPoints`. Each plate is its own `HtmlUi` root, not parented to its target: its `Node` is
//! moved to the target's projected position every frame and it fades out with camera distance.
//! Only a name change re-renders a plate; position, fade and health fill are per-frame component
//! updates on the built nodes.

use crate::ui::markup;
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy_markup::prelude::*;
use p19_shared::combat::HitPoints;
use p19_shared::game_state::GameState;
use std::collections::HashSet;

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/nameplate.html");
        app.insert_resource(NameplatesVisible(false));
        app.add_observer(on_nameplate_built)
            .add_observer(on_nameplate_restyled);
        app.add_systems(
            Update,
            (
                spawn_nameplates.run_if(in_state(GameState::InGame)),
                track_nameplates,
            ),
        );
    }
}

/// Toggled by the console's `nameplates` command (`console.rs`) — a plain resource rather than
/// gating `spawn_nameplates`/despawning existing ones, so hiding/showing is instant and doesn't
/// lose/rebuild per-target state while toggled off.
#[derive(Resource)]
pub struct NameplatesVisible(pub bool);

/// A plate's root: its target and the state last applied to its built nodes, re-applied after a
/// rebuild or restyle (both reset the nodes to their CSS values).
#[derive(Component)]
struct Nameplate {
    target: Entity,
    offset: Vec3,
    name: String,
    alpha: f32,
    health_percent: f32,
}

/// The health bar's fill element.
#[derive(Component)]
struct HealthFill;

/// A built node's CSS colors, which the distance fade scales the alpha of.
#[derive(Component)]
struct FadeBase {
    text: Option<Color>,
    background: Option<Color>,
}

/// Distance from the camera at which the nameplate starts fading, and the distance at which
/// it's fully transparent (and hidden).
const FADE_START_DISTANCE: f32 = 15.0;
const FADE_END_DISTANCE: f32 = 30.0;

/// Polling (not an `On<Add, HitPoints>` observer): player characters' `HitPoints` arrive via
/// lightyear's initial replication sync, which doesn't reliably fire per-component `Add`
/// observers (see the repo conventions). Gated to `GameState::InGame` so plates never spawn for
/// lobby entities; the per-plate `DespawnOnExit(GameState::InGame)` cleans up on leaving.
fn spawn_nameplates(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    names: Query<&Name>,
    targets: Query<Entity, Added<HitPoints>>,
    nameplates: Query<&Nameplate>,
) {
    let existing: HashSet<Entity> = nameplates.iter().map(|plate| plate.target).collect();
    for target in &targets {
        // HitPoints and Name are always spawned together in the same bundle across the codebase,
        // but fall back gracefully rather than panicking if that's ever not the case.
        let Ok(name) = names.get(target) else {
            continue;
        };
        if existing.contains(&target) {
            continue;
        }
        commands.spawn((
            Nameplate {
                target,
                offset: Vec3::new(0.0, 1.5, 0.0),
                name: name.to_string(),
                alpha: 1.0,
                health_percent: 100.0,
            },
            markup::template(&asset_server, "nameplate.html"),
            TemplateContext::new().with("name", name.as_str()),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Pickable::IGNORE,
            // Spawn hidden — `track_nameplates` only reaches `Hidden` (via `NameplatesVisible`,
            // fade distance, or screen projection) one frame later, and an `Inherited` plate
            // would flash at the layout origin in that frame.
            Visibility::Hidden,
            DespawnOnExit(GameState::InGame),
        ));
    }
}

fn on_nameplate_built(
    built: On<HtmlUiBuilt>,
    nameplates: Query<&Nameplate>,
    elements: HtmlElements,
    children: Query<&Children>,
    mut nodes: Query<(
        &mut Node,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
    mut commands: Commands,
) {
    let Ok(nameplate) = nameplates.get(built.entity) else {
        return;
    };
    if let Some(fill) = elements.by_id(built.entity, "nameplate-fill") {
        commands.entity(fill).insert(HealthFill);
        if let Ok((mut node, ..)) = nodes.get_mut(fill) {
            node.width = percent(nameplate.health_percent);
        }
    }
    record_fade_bases(
        built.entity,
        nameplate.alpha,
        &children,
        &mut nodes,
        &mut commands,
    );
}

fn on_nameplate_restyled(
    restyled: On<HtmlUiRestyled>,
    nameplates: Query<&Nameplate>,
    children: Query<&Children>,
    fills: Query<(), With<HealthFill>>,
    mut nodes: Query<(
        &mut Node,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
    mut commands: Commands,
) {
    let Ok(nameplate) = nameplates.get(restyled.entity) else {
        return;
    };
    for entity in children.iter_descendants(restyled.entity) {
        if fills.contains(entity)
            && let Ok((mut node, ..)) = nodes.get_mut(entity)
        {
            node.width = percent(nameplate.health_percent);
        }
    }
    record_fade_bases(
        restyled.entity,
        nameplate.alpha,
        &children,
        &mut nodes,
        &mut commands,
    );
}

/// Records the freshly (re)styled CSS colors below `root` as [`FadeBase`]s and applies the
/// plate's current fade to them.
fn record_fade_bases(
    root: Entity,
    alpha: f32,
    children: &Query<&Children>,
    nodes: &mut Query<(
        &mut Node,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
    commands: &mut Commands,
) {
    for entity in children.iter_descendants(root) {
        let Ok((_, text, background)) = nodes.get_mut(entity) else {
            continue;
        };
        let base = FadeBase {
            text: text.as_ref().map(|color| color.0),
            background: background.as_ref().map(|color| color.0),
        };
        if let (Some(mut text), Some(base)) = (text, base.text) {
            text.0 = faded(base, alpha);
        }
        if let (Some(mut background), Some(base)) = (background, base.background) {
            background.0 = faded(base, alpha);
        }
        commands.entity(entity).insert(base);
    }
}

fn faded(base: Color, alpha: f32) -> Color {
    base.with_alpha(base.alpha() * alpha)
}

fn track_nameplates(
    mut commands: Commands,
    nameplates_visible: Res<NameplatesVisible>,
    // `With<IsDefaultUiCamera>` (not a bare `.single()`) — VR sessions add extra `Camera`
    // entities for the per-eye XR views (see `bevy_mod_openxr`), so a plain `(&Camera,
    // &GlobalTransform)` query stops matching exactly one camera once a headset is connected,
    // which made `.single()` fail and nameplates freeze at their unset default position.
    camera_query: Query<(&Camera, &GlobalTransform), With<IsDefaultUiCamera>>,
    targets: Query<(&GlobalTransform, &HitPoints, Option<&InheritedVisibility>)>,
    names: Query<&Name>,
    mut nameplates: Query<(
        Entity,
        &mut Nameplate,
        &mut Node,
        &mut Visibility,
        &mut TemplateContext,
    )>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, (With<HealthFill>, Without<Nameplate>)>,
    mut faded_nodes: Query<(
        &FadeBase,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (nameplate_entity, mut nameplate, mut node, mut visibility, mut context) in &mut nameplates
    {
        let Ok((target_transform, hit_points, target_visibility)) = targets.get(nameplate.target)
        else {
            // Target despawned (killed, or otherwise removed) — clean up after it.
            commands.entity(nameplate_entity).despawn();
            continue;
        };

        if !nameplates_visible.0 {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        // Nameplates aren't parented to their target, so they don't get `InheritedVisibility`
        // propagation through `ChildOf` for free — mirror it manually, so e.g. a
        // dead-but-not-yet-despawned target hidden via `Visibility::Hidden` doesn't leave its
        // nameplate floating over a corpse nobody can see. A target with no
        // `InheritedVisibility` at all (most character root entities) never opted into the
        // visibility system, so treat that as visible rather than despawning a live nameplate.
        if target_visibility.is_some_and(|visible| !visible.get()) {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        if let Ok(name) = names.get(nameplate.target)
            && nameplate.name != name.as_str()
        {
            nameplate.name = name.to_string();
            context.insert("name", name.as_str());
        }

        let world_pos = target_transform.translation() + nameplate.offset;
        let distance = camera_transform.translation().distance(world_pos);
        let alpha = 1.0
            - ((distance - FADE_START_DISTANCE) / (FADE_END_DISTANCE - FADE_START_DISTANCE))
                .clamp(0.0, 1.0);

        match camera.world_to_viewport(camera_transform, world_pos) {
            Ok(screen_pos) => {
                node.left = px(screen_pos.x);
                node.top = px(screen_pos.y);
                visibility.set_if_neq(if alpha <= 0.0 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
            }
            Err(_) => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }

        let health_percent =
            (hit_points.hit_points as f32 / hit_points.max_hit_points as f32 * 100.0).max(0.0);
        let health_changed = nameplate.health_percent != health_percent;
        let alpha_changed = nameplate.alpha != alpha;
        if !health_changed && !alpha_changed {
            continue;
        }
        nameplate.health_percent = health_percent;
        nameplate.alpha = alpha;
        for descendant in children.iter_descendants(nameplate_entity) {
            if health_changed && let Ok(mut fill_node) = fills.get_mut(descendant) {
                fill_node.width = percent(health_percent);
            }
            if alpha_changed && let Ok((base, text, background)) = faded_nodes.get_mut(descendant) {
                if let (Some(mut text), Some(base)) = (text, base.text) {
                    text.0 = faded(base, alpha);
                }
                if let (Some(mut background), Some(base)) = (background, base.background) {
                    background.0 = faded(base, alpha);
                }
            }
        }
    }
}
