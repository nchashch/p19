use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_quinnet::RepliconQuinnetPlugins;
use shared::character_controller::CharacterControllerPlugin;
use shared::combat::SharedCombatPlugin;
use shared::cube_spawner::SharedCubeSpawnerPlugin;
use shared::npc_spawner::SharedNpcSpawnerPlugin;
use shared::replication::SharedReplicationPlugin;

mod networking;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            // `RepliconPlugins` needs `States`, and log output needs a subscriber — both are
            // included in `DefaultPlugins` (which the client uses) but not in `MinimalPlugins`.
            StatesPlugin,
            bevy::log::LogPlugin::default(),
            TransformPlugin,
            AssetPlugin::default(),
            PhysicsPlugins::default(),
            RepliconPlugins,
            RepliconQuinnetPlugins,
            SharedReplicationPlugin,
            SharedCombatPlugin,
            CharacterControllerPlugin,
            SharedCubeSpawnerPlugin,
            SharedNpcSpawnerPlugin,
            networking::NetworkingPlugin,
        ))
        // avian3d's collider cache reads `AssetEvent<Mesh>` (for mesh-derived colliders) even
        // though the server never renders — normally registered by rendering plugins the headless
        // server doesn't have, so it needs registering directly instead.
        .init_asset::<Mesh>()
        .run();
}
