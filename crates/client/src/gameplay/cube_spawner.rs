use bevy::prelude::*;
use lightyear::prelude::*;
use p19_shared::client_events::SpawnCubeRequest;
use p19_shared::cube_spawner::{Cube, CubeSpawner};
use p19_shared::level::InGameRoot;
use p19_shared::replication::OrderedReliable;

use crate::assets::collections::CommonAssets;
use crate::controls::fps_controller::FpsCamera;
use crate::events::SpawnCube;

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
    mut sender: Single<&mut MessageSender<SpawnCubeRequest>>,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let aim_direction = fps_camera.forward();
    sender.send::<OrderedReliable>(SpawnCubeRequest {
        transform: cube_spawner.compute_transform(),
        aim_direction,
    });
}

fn decorate_cubes(
    cubes: Query<Entity, (With<Cube>, Without<Decorated>)>,
    level_root: Single<Entity, With<InGameRoot>>,
    common_assets: Res<CommonAssets>,
    mut commands: Commands,
) {
    for cube in cubes {
        commands
            .entity(cube)
            .insert((Visibility::default(), Decorated, ChildOf(*level_root)))
            .with_child(WorldAssetRoot(common_assets.cube_world.clone()));
    }
}

#[derive(Component)]
pub struct Decorated;
