use bevy::prelude::*;
use bevy_replicon::prelude::ClientTriggerExt;
use shared::client_events::SpawnNpcRequest;
use shared::level::LevelRoot;
use shared::npc_spawner::{Npc, NpcSpawner};
use shared::server_events::NpcSpawned;

use crate::cube_spawner::Decorated;
use crate::events::SpawnNpc;

pub struct NpcSpawnerPlugin;

impl Plugin for NpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_npc);
        app.add_systems(Update, decorate_npcs);
    }
}

fn request_spawn_npc(
    _event: On<SpawnNpc>,
    npc_spawner: Single<&GlobalTransform, With<NpcSpawner>>,
    mut commands: Commands,
) {
    info!("requested npc spawn");
    commands.client_trigger(SpawnNpcRequest {
        transform: npc_spawner.compute_transform(),
    });
}

fn decorate_npcs(
    cubes: Query<Entity, (With<Npc>, Without<Decorated>)>,
    level_root: Single<Entity, With<LevelRoot>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for cube in cubes {
        commands
            .entity(cube)
            .insert((Visibility::default(), Decorated, ChildOf(*level_root)))
            .with_child((
                WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            ));
    }
}
