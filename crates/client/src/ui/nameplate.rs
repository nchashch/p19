//! Screen-space nameplates (name + health bar, `html/nameplate.html`) above every entity with
//! `HitPoints`. Each plate is its own `HtmlUi` root, not parented to its target: bevy_markup's
//! `HtmlWorldAnchor` keeps it centered over the target's head (hidden behind the camera, off
//! screen or over an invisible target, despawned with it). Name, health fill (`style="width: …%"`),
//! the distance fade (`style="opacity: …"`, from `HtmlWorldAnchorView::distance`) and the
//! `NameplatesVisible` toggle (`hidden` class → `display: none`) are template values written every
//! frame: bevy_markup updates the plate in place, and only when the rendered output changes.

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
                update_nameplates,
            ),
        );
    }
}

/// Toggled by the console's `nameplates` command (`console.rs`) — a plain resource rather than
/// gating `spawn_nameplates`/despawning existing ones, so hiding/showing is instant and doesn't
/// lose/rebuild per-target state while toggled off.
/// Reflected so the agent tool API (BRP `world.insert_resources`) can toggle it.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct NameplatesVisible(pub bool);

/// Marks a nameplate root (its target is its `HtmlWorldAnchor`'s).
#[derive(Component)]
struct Nameplate;

/// Height above the target's origin the plate's bottom center sits at.
const NAMEPLATE_OFFSET: Vec3 = Vec3::new(0.0, 1.5, 0.0);

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
    context.insert("hidden", &(alpha <= 0.0));
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
    nameplates: Query<&HtmlWorldAnchor, With<Nameplate>>,
) {
    let existing: HashSet<Entity> = nameplates.iter().map(|anchor| anchor.target).collect();
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
        // Hidden until `update_nameplates` has measured the distance.
        nameplate_context(&mut context, name.as_str(), 0.0, 100.0);
        commands.spawn((
            Nameplate,
            markup::template(&asset_server, "nameplate.html"),
            context,
            HtmlWorldAnchor::new(target).with_offset(NAMEPLATE_OFFSET),
            DespawnOnExit(GameState::InGame),
        ));
    }
}

/// Name, health and fade for every plate, from its target and the anchor's measured distance.
fn update_nameplates(
    nameplates_visible: Res<NameplatesVisible>,
    targets: Query<(&HitPoints, Option<&Name>)>,
    mut nameplates: Query<
        (&HtmlWorldAnchor, &HtmlWorldAnchorView, &mut TemplateContext),
        With<Nameplate>,
    >,
) {
    for (anchor, view, mut context) in &mut nameplates {
        let Ok((hit_points, name)) = targets.get(anchor.target) else {
            continue;
        };
        let alpha = if nameplates_visible.0 {
            1.0 - ((view.distance - FADE_START_DISTANCE)
                / (FADE_END_DISTANCE - FADE_START_DISTANCE))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let health =
            (hit_points.hit_points as f32 / hit_points.max_hit_points as f32 * 100.0).max(0.0);
        nameplate_context(&mut context, name.map_or("", Name::as_str), alpha, health);
    }
}
