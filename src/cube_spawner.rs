use crate::GameState;
use bevy::prelude::*;

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_cube);
    }
}

#[derive(Event)]
pub struct SpawnCube;

pub fn spawn_cube(
    _event: On<SpawnCube>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cube_spawner: Query<Entity, With<CubeSpawner>>,
) {
    let Ok(cube_spawner) = cube_spawner.single() else {
        return;
    };
    commands.entity(cube_spawner).with_child((
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("Cube.glb"))),
        DespawnOnEnter(GameState::MainMenu),
    ));
}

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct Cube;

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct CubeSpawner;
