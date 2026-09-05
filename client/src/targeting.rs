use avian3d::prelude::*;
use bevy::color::palettes::css::{DARK_SLATE_GRAY, GRAY, WHITE};
use bevy::prelude::*;
use bevy_mod_outline::{AsyncWorldInheritOutline, OutlinePlugin, OutlineVolume};
use shared::player::Selectable;

use crate::game_state::VRState;
use crate::player_character::LocalPlayer;

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LocalPlayer(None));
        app.add_plugins(OutlinePlugin::JUMP_FLOOD);
        app.add_systems(
            Update,
            (
                // VR has its own hover/select input (`vr_controllers::update_vr_pointers`, one
                // raycast per controller) — the screen-center crosshair ray makes no sense once
                // there's no fixed "center of the screen" the player is actually looking through.
                raycast_from_center.run_if(in_state(VRState::Desktop)),
                update_outline_hovered_selected,
                deselect_when_out_of_range,
            ),
        );
        app.add_observer(add_outline_component);
        app.insert_resource(Hovered(None));
        app.insert_resource(Selected(None));
    }
}

#[derive(Resource)]
pub struct Hovered(pub Option<(Entity, f32)>);

#[derive(Resource)]
pub struct Selected(pub Option<Entity>);

pub const SELECT_RANGE: f32 = 50.0;

/// The crosshair ray: straight out from the center of the screen, through the active camera —
/// shared by `raycast_from_center` (world-entity hover/select, via Avian's `SpatialQuery`) and
/// `npc_ui_quad`'s pointer-driving system (UI-on-a-quad hover/click, via `bevy_picking`'s
/// `MeshRayCast` — Avian's raycast hits don't carry a UV coordinate, so that one can't reuse
/// `SpatialQuery` the way this module does). Both should look at the exact same point on screen,
/// so this is computed once here rather than duplicated.
pub(crate) fn screen_center_ray(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    window: &Window,
) -> Option<Ray3d> {
    let screen_origin = window.size() / 2.0;
    // Screen space -> world ray. `None` if the camera has no usable viewport/projection this frame.
    camera
        .viewport_to_world(camera_transform, screen_origin)
        .ok()
}

fn raycast_from_center(
    local_player: Res<LocalPlayer>,
    spatial_query: SpatialQuery,
    camera_query: Query<(&Camera, &GlobalTransform), With<IsDefaultUiCamera>>,
    window_query: Query<&Window>,
    selectables: Query<(), With<Selectable>>,
    mut hovered: ResMut<Hovered>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };

    let Some(ray) = screen_center_ray(camera, camera_transform, window) else {
        return;
    };

    if let Some(hit) = spatial_query.cast_ray(
        ray.origin,
        ray.direction, // already a Dir3
        f32::MAX,      // max distance
        true,          // treat shapes as solid (hit registers if origin is inside)
        &SpatialQueryFilter::from_excluded_entities(match local_player.0 {
            Some(local_player) => vec![local_player],
            None => vec![],
        }),
    ) {
        if selectables.contains(hit.entity) {
            hovered.0 = Some((hit.entity, hit.distance));
        } else {
            hovered.0 = None;
        }
    } else {
        hovered.0 = None;
    }
}

fn deselect_when_out_of_range(
    global_transform: Query<&GlobalTransform>,
    local_player: Res<LocalPlayer>,
    mut selected: ResMut<Selected>,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    if let Some(selected_entity) = selected.0 {
        let Ok(player_transform) = global_transform.get(local_player) else {
            return;
        };
        let Ok(selected_transform) = global_transform.get(selected_entity) else {
            return;
        };
        let distance = player_transform
            .translation()
            .distance(selected_transform.translation());
        if distance > SELECT_RANGE {
            selected.0 = None;
        }
    }
}

/// AsyncWorldInheritOutline only starts inheriting once the *same* entity's own WorldAssetRoot
/// has finished loading (it checks WorldInstance on itself, not on an ancestor) — so this reacts
/// to WorldAssetRoot being added anywhere, and outlines it if it's Selectable itself or a
/// descendant of a Selectable ancestor (e.g. Npc, whose model lives on a child entity spawned
/// after — and so already linked to — its Selectable parent).
fn add_outline_component(
    add: On<Add, WorldAssetRoot>,
    mut commands: Commands,
    selectables: Query<(), With<Selectable>>,
    ancestors: Query<&ChildOf>,
) {
    let entity = add.entity;
    let is_outlinable = selectables.contains(entity)
        || ancestors
            .iter_ancestors(entity)
            .any(|ancestor| selectables.contains(ancestor));
    if is_outlinable {
        commands.entity(entity).insert((
            OutlineVolume {
                visible: false,
                width: 4.0,
                colour: Color::srgb(1.0, 1.0, 1.0),
            },
            AsyncWorldInheritOutline::default(),
        ));
    }
}

fn update_outline_hovered_selected(
    children: Query<&Children>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    mut outlines: Query<&mut OutlineVolume>,
) {
    for mut outline in outlines.iter_mut() {
        outline.visible = false;
    }
    if let Some((hovered, distance)) = hovered.0 {
        for entity in std::iter::once(hovered).chain(children.iter_descendants(hovered)) {
            if let Ok(mut outline) = outlines.get_mut(entity) {
                if distance < SELECT_RANGE {
                    outline.colour = GRAY.into();
                    outline.visible = true;
                } else {
                    outline.colour = DARK_SLATE_GRAY.into();
                    outline.visible = true;
                }
            }
        }
    }
    if let Some(selected) = selected.0 {
        for entity in std::iter::once(selected).chain(children.iter_descendants(selected)) {
            if let Ok(mut outline) = outlines.get_mut(entity) {
                outline.colour = WHITE.into();
                outline.visible = true;
            }
        }
    }
}
