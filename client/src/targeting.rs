use crate::{add_observers_run_if, player_character::PlayerCharacter};
use avian3d::prelude::*;
use bevy::color::palettes::css::{DARK_SLATE_GRAY, GRAY, WHITE};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_mod_outline::{AsyncWorldInheritOutline, OutlinePlugin, OutlineVolume};
use chill_bevy_console::console_closed;

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(OutlinePlugin::JUMP_FLOOD);
        app.add_systems(
            Update,
            (
                raycast_from_center,
                update_outline_hovered_selected,
                deselect_when_out_of_range,
            ),
        );
        app.add_observer(add_outline_component);
        app.insert_resource(Hovered(None));
        app.insert_resource(Selected(None));
        add_observers_run_if!(app, console_closed, select, deselect);
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Select;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Deselect;

#[derive(Resource)]
pub struct Hovered(pub Option<(Entity, f32)>);

#[derive(Resource)]
pub struct Selected(pub Option<Entity>);

pub const SELECT_RANGE: f32 = 50.0;

fn raycast_from_center(
    player_collider_entity: Query<Entity, (With<PlayerCharacter>, With<Collider>)>,
    spatial_query: SpatialQuery,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    window_query: Query<&Window>,
    mut hovered: ResMut<Hovered>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok(player_collider_entity) = player_collider_entity.single() else {
        return;
    };

    /*
    // Center of the screen in logical (not physical) pixels.
    let screen_origin = if disable_fps_camera_control.0 {
        window.cursor_position().unwrap_or(Vec2::ZERO)
    } else {
        return;
        // window.size() / 2.0
    };
    */

    let screen_origin = window.size() / 2.0;

    // Screen space -> world ray. Returns Err if the camera has no usable
    // viewport/projection this frame.
    let Ok(ray) = camera.viewport_to_world(camera_transform, screen_origin) else {
        return;
    };

    if let Some(hit) = spatial_query.cast_ray(
        ray.origin,
        ray.direction, // already a Dir3
        f32::MAX,      // max distance
        true,          // treat shapes as solid (hit registers if origin is inside)
        &SpatialQueryFilter::from_excluded_entities([player_collider_entity]),
    ) {
        hovered.0 = Some((hit.entity, hit.distance));
    } else {
        hovered.0 = None;
    }
}

fn select(
    _event: On<Fire<Select>>,
    hovered: Res<Hovered>,
    mut selected: ResMut<Selected>,
    query: Query<Entity, With<Selectable>>,
) {
    if let Some((entity, distance)) = hovered.0 {
        if query.get(entity).is_ok() {
            if distance < SELECT_RANGE {
                selected.0 = Some(entity);
            }
        }
    }
}

fn deselect(_event: On<Fire<Deselect>>, mut selected: ResMut<Selected>) {
    selected.0 = None;
}

fn deselect_when_out_of_range(
    player_transform: Single<&GlobalTransform, With<PlayerCharacter>>,
    selected_transform: Query<&GlobalTransform>,
    mut selected: ResMut<Selected>,
) {
    if let Some(selected_entity) = selected.0 {
        let Ok(selected_transform) = selected_transform.get(selected_entity) else {
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

#[derive(Component)]
pub struct Selectable;

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
