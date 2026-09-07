use bevy::prelude::*;
use bevy_replicon::prelude::ClientTriggerExt;
use shared::client_events::SpawnNpcRequest;
use shared::level::LevelRoot;
use shared::npc_spawner::{Npc, NpcSpawner};

use crate::assets::collections::CommonAssets;
use crate::events::SpawnNpc;
use crate::gameplay::cube_spawner::Decorated;
use crate::ui::npc_ui_quad::{NpcUiQuad, NpcUiQuadMesh};

pub struct NpcSpawnerPlugin;

impl Plugin for NpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(request_spawn_npc);
        // `NpcUiQuad` doesn't exist until `OnEnter(GameState::MainMenu)` runs
        // `npc_ui_quad::setup_npc_ui_quad` (needs `Res<CommonAssets>`, so it can't be `Startup`
        // any more — see that system's own doc comment) — guarded so this doesn't panic on a
        // hard `Res<NpcUiQuad>` validation failure during the brief `AssetLoading` window before
        // that first runs. No NPC ever exists before `InGame` anyway, well after `MainMenu`, so
        // this only ever skips frames that had nothing to decorate regardless.
        app.add_systems(Update, decorate_npcs.run_if(resource_exists::<NpcUiQuad>));
    }
}

fn request_spawn_npc(
    _event: On<SpawnNpc>,
    npc_spawner: Single<&GlobalTransform, With<NpcSpawner>>,
    mut commands: Commands,
) {
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
    common_assets: Res<CommonAssets>,
    npc_ui_quad: Res<NpcUiQuad>,
    mut commands: Commands,
) {
    for cube in cubes {
        commands
            .entity(cube)
            .insert((Visibility::default(), Decorated, ChildOf(*level_root)))
            .with_child((
                WorldAssetRoot(common_assets.rig_world.clone()),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            ))
            .with_child((
                Mesh3d(npc_ui_quad.mesh.clone()),
                MeshMaterial3d(npc_ui_quad.material.clone()),
                // Lets `npc_ui_quad::drive_npc_ui_quad_pointer` filter its `MeshRayCast` down to
                // just these, and (via this child's own `ChildOf`) resolve a hit back to which NPC
                // it belongs to.
                NpcUiQuadMesh,
                // No billboard system on this child — it's a plain `ChildOf`-parented transform,
                // so it turns with the NPC exactly like `rig.glb` above does, rather than always
                // facing the camera the way `nameplate.rs`'s screen-space nameplates do.
                Transform::from_translation(Vec3::new(0.0, UI_QUAD_HEIGHT, 0.0)),
            ));
    }
}
