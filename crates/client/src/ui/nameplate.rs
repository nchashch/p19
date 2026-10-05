//! Screen-space nameplates (name + health bar, `html/nameplate.html`) above every entity with
//! `HitPoints`. Each plate is its own `HtmlUi` root, not parented to its target: its `Node` is
//! moved to the target's projected position every frame (`left`/`top`, which CSS leaves alone).
//! Name, health fill (`style="width: …%"`) and the distance fade (`style="opacity: …"` on the
//! root) are template values, written every frame: bevy_markup updates the plate in place, and
//! only when the rendered output changes.

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
/// Reflected so the agent tool API (BRP `world.insert_resources`) can toggle it.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct NameplatesVisible(pub bool);

/// A plate's root: the entity it follows.
#[derive(Component)]
struct Nameplate {
    target: Entity,
    offset: Vec3,
}

/// Distance from the camera at which the nameplate starts fading, and the distance at which
/// it's fully transparent (and hidden).
const FADE_START_DISTANCE: f32 = 15.0;
const FADE_END_DISTANCE: f32 = 30.0;

/// `nameplate.html`'s variables. Fade and health are rounded to what's visible (1% steps), so a
/// plate only updates when it would look different.
fn nameplate_context(context: &mut TemplateContext, name: &str, alpha: f32, health: f32) {
    context.insert("name", name);
    context.insert("alpha", &((alpha * 100.0).round() / 100.0));
    context.insert("health", &health.round());
}

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
        let mut context = TemplateContext::new();
        nameplate_context(&mut context, name.as_str(), 1.0, 100.0);
        commands.spawn((
            Nameplate {
                target,
                offset: Vec3::new(0.0, 1.5, 0.0),
            },
            markup::template(&asset_server, "nameplate.html"),
            context,
            // Spawn hidden — `track_nameplates` only reaches `Hidden` (via `NameplatesVisible`,
            // fade distance, or screen projection) one frame later, and an `Inherited` plate
            // would flash at the layout origin in that frame.
            Visibility::Hidden,
            DespawnOnExit(GameState::InGame),
        ));
    }
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
        &Nameplate,
        &mut Node,
        &mut Visibility,
        &mut TemplateContext,
    )>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (nameplate_entity, nameplate, mut node, mut visibility, mut context) in &mut nameplates {
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

        let world_pos = target_transform.translation() + nameplate.offset;
        let distance = camera_transform.translation().distance(world_pos);
        let alpha = 1.0
            - ((distance - FADE_START_DISTANCE) / (FADE_END_DISTANCE - FADE_START_DISTANCE))
                .clamp(0.0, 1.0);

        match camera.world_to_viewport(camera_transform, world_pos) {
            Ok(screen_pos) if alpha > 0.0 => {
                node.left = px(screen_pos.x);
                node.top = px(screen_pos.y);
                visibility.set_if_neq(Visibility::Inherited);
            }
            _ => {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
        }

        let health =
            (hit_points.hit_points as f32 / hit_points.max_hit_points as f32 * 100.0).max(0.0);
        let name = names.get(nameplate.target).map_or("", Name::as_str);
        nameplate_context(&mut context, name, alpha, health);
    }
}
