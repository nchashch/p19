use crate::{
    fps_controller::FpsCamera, loading::LevelRoot, player_character::PlayerCharacter,
    targeting::Selectable,
};
use bevy::prelude::*;
use shared::events::{CubeSpawned, SpawnCubeRequest};

use crate::events::SpawnCube;

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_cube);
        app.add_observer(on_cube_spawned);
    }
}

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
    level_root: Single<Entity, With<LevelRoot>>,
    mut commands: Commands,
) {
    commands.entity(spawned.entity).insert((
        Selectable,
        WorldAssetRoot(asset_server.load("Cube.glb#Scene0")),
        ChildOf(*level_root),
    ));
}
