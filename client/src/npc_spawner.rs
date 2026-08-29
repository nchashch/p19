use crate::targeting::Selectable;
use bevy::prelude::*;
use shared::client_events::SpawnNpcRequest;
use shared::level::LevelRoot;
use shared::player::PlayerCharacter;
use shared::server_events::NpcSpawned;

use crate::events::SpawnNpc;

pub struct NpcSpawnerPlugin;

impl Plugin for NpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_npc);
        app.add_observer(on_npc_spawned);
    }
}

fn request_spawn_npc(
    _event: On<SpawnNpc>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(caster) = player.single() else {
        return;
    };
    commands.trigger(SpawnNpcRequest { caster });
}

fn on_npc_spawned(
    spawned: On<NpcSpawned>,
    asset_server: Res<AssetServer>,
    level_root: Single<Entity, With<LevelRoot>>,
    mut commands: Commands,
) {
    commands
        .entity(spawned.entity)
        .insert((Visibility::default(), Selectable, ChildOf(*level_root)))
        .with_child((
            WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
            Transform::from_translation(Vec3::new(0.0, -0.9, 0.0))
                .with_rotation(Quat::from_rotation_y(spawned.facing_yaw)),
        ));
}
