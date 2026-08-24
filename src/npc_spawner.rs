use avian3d::prelude::*;
use bevy::prelude::*;

use crate::game_state::GameState;

pub struct NpcSpawnerPlugin;

impl Plugin for NpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_npc);
    }
}

#[derive(Event)]
pub struct SpawnNpc;

#[derive(Component)]
pub struct NpcSpawner;

#[derive(Component)]
pub struct Npc;

pub fn spawn_npc(
    _event: On<SpawnNpc>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    npc_spawner: Query<&GlobalTransform, With<NpcSpawner>>,
) {
    for transform in npc_spawner {
        let translation = transform.translation();
        commands
            .spawn((
                Npc,
                Transform::from_translation(translation),
                DespawnOnEnter(GameState::MainMenu),
                RigidBody::Dynamic,
                Collider::capsule(0.4, 1.0),
                LockedAxes::new()
                    .lock_rotation_x()
                    .lock_rotation_y()
                    .lock_rotation_z(),
            ))
            .with_child((
                WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            ));
    }
}
