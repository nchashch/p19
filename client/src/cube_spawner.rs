use crate::{
    fps_controller::FpsCamera, game_state::GameState, player_character::PlayerCharacter,
    targeting::Selectable,
};
use bevy::prelude::*;
use shared::cube_spawner::{CubeSpawned, SpawnCubeRequest};

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_cube);
        app.add_observer(on_cube_spawned);
    }
}

/// Client-local trigger (bound to input) — translated into a `SpawnCubeRequest` carrying the
/// player's current aim direction, since `shared` has no `FpsCamera` of its own.
#[derive(Event)]
pub struct SpawnCube;

fn request_spawn_cube(
    _event: On<SpawnCube>,
    fps_camera: Query<&FpsCamera>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let Ok(caster) = player.single() else {
        return;
    };
    let aim_direction = Vec3::Z.rotate_x(fps_camera.pitch).rotate_y(fps_camera.yaw);
    commands.trigger(SpawnCubeRequest {
        caster,
        aim_direction,
    });
}

fn on_cube_spawned(
    spawned: On<CubeSpawned>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.entity(spawned.entity).insert((
        Selectable,
        WorldAssetRoot(asset_server.load("Cube.glb#Scene0")),
        DespawnOnEnter(GameState::MainMenu),
    ));
}
