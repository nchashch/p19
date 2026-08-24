use crate::{
    add_observers_run_if, cube_spawner::Selectable, fps_controller::DisableFpsCameraControl,
    player_character::PlayerCharacter,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use chill_bevy_console::console_closed;

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Hovered(None));
        app.insert_resource(Selected(None));
        app.add_systems(Update, raycast_from_center);
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
    disable_fps_camera_control: Res<DisableFpsCameraControl>,
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

    // Center of the screen in logical (not physical) pixels.
    let screen_origin = if disable_fps_camera_control.0 {
        window.cursor_position().unwrap_or(Vec2::ZERO)
    } else {
        return;
        // window.size() / 2.0
    };

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
