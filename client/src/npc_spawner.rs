use bevy::prelude::*;
use bevy_replicon::prelude::ClientTriggerExt;
use shared::client_events::SpawnNpcRequest;
use shared::level::LevelRoot;
use shared::npc_spawner::{Npc, NpcSpawner};

use crate::cube_spawner::Decorated;
use crate::events::SpawnNpc;
use crate::npc_ui_quad::NpcUiQuad;

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

/// Height above the NPC's own origin the UI quad sits at — roughly head height for the `rig.glb`
/// model spawned alongside it (see the `-0.9` offset that model's own child gets below, which
/// puts its feet at the NPC's origin).
const UI_QUAD_HEIGHT: f32 = 1.8;

fn decorate_npcs(
    cubes: Query<Entity, (With<Npc>, Without<Decorated>)>,
    level_root: Single<Entity, With<LevelRoot>>,
    asset_server: Res<AssetServer>,
    npc_ui_quad: Res<NpcUiQuad>,
    mut commands: Commands,
) {
    for cube in cubes {
        commands
            .entity(cube)
            .insert((Visibility::default(), Decorated, ChildOf(*level_root)))
            .with_child((
                WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            ))
            .with_child((
                Mesh3d(npc_ui_quad.mesh.clone()),
                MeshMaterial3d(npc_ui_quad.material.clone()),
                // No billboard system on this child — it's a plain `ChildOf`-parented transform,
                // so it turns with the NPC exactly like `rig.glb` above does, rather than always
                // facing the camera the way `nameplate.rs`'s screen-space nameplates do.
                Transform::from_translation(Vec3::new(0.0, UI_QUAD_HEIGHT, 0.0)),
            ));
    }
}
