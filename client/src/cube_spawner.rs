use bevy::prelude::*;
use bevy_replicon::prelude::ClientTriggerExt;
use shared::client_events::SpawnCubeRequest;
use shared::cube_spawner::{Cube, CubeSpawner};
use shared::level::LevelRoot;
use shared::player::Selectable;
use shared::server_events::CubeSpawned;

use crate::events::SpawnCube;
use crate::fps_controller::FpsCamera;

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_cube);
        app.add_systems(Update, decorate_cubes);
    }
}

fn request_spawn_cube(
    _event: On<SpawnCube>,
    fps_camera: Query<&FpsCamera>,
    cube_spawner: Single<&GlobalTransform, With<CubeSpawner>>,
    mut commands: Commands,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let aim_direction = Vec3::Z.rotate_x(fps_camera.pitch).rotate_y(fps_camera.yaw);
    commands.client_trigger(SpawnCubeRequest {
        transform: cube_spawner.compute_transform(),
        aim_direction,
    });
}

fn decorate_cubes(
    cubes: Query<Entity, (With<Cube>, Without<Decorated>)>,
    level_root: Single<Entity, With<LevelRoot>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for cube in cubes {
        commands
            .entity(cube)
            .insert((Visibility::default(), Decorated, ChildOf(*level_root)))
            .with_child(WorldAssetRoot(asset_server.load("Cube.glb#Scene0")));
    }
}

#[derive(Component)]
pub struct Decorated;
