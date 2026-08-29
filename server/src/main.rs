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
            // `server` has no `assets/` directory of its own — level geometry (and eventually
            // anything else the server needs, e.g. collider-relevant data) lives in
            // `client/assets/` (see CLAUDE.md's note on why assets live inside `client/`), so
            // point the default filesystem asset source there instead of duplicating it.
            AssetPlugin {
                file_path: "../client/assets".to_string(),
                ..default()
            },
            PhysicsPlugins::default(),
            RepliconPlugins,
            RepliconQuinnetPlugins,
            SharedReplicationPlugin,
            SharedCombatPlugin,
            CharacterControllerPlugin,
            SharedCubeSpawnerPlugin,
            SharedNpcSpawnerPlugin,
            // Loads `.glb` level geometry headlessly: `GltfPlugin` parses the file,
            // `WorldSerializationPlugin` instantiates it as a `WorldAssetRoot`/reflected entity
            // graph (the same mechanism `WorldInstanceReady` etc. rely on client-side), and
            // `SkeinPlugin` applies whatever reflected components (including `ColliderConstructor`)
            // are baked into the file's extras. None of these need rendering. Nested in its own
            // tuple — `add_plugins` only supports so many top-level elements before it runs out
            // of `Plugins` tuple impls.
            (
                bevy::gltf::GltfPlugin::default(),
                bevy::world_serialization::WorldSerializationPlugin,
                bevy_skein::SkeinPlugin::default(),
            ),
            networking::NetworkingPlugin,
        ))
        // avian3d's collider cache reads `AssetEvent<Mesh>` (for mesh-derived colliders) even
        // though the server never renders — normally registered by rendering plugins the headless
        // server doesn't have, so it needs registering directly instead. `Image` needs the same
        // treatment: `GltfLoader` allocates `Handle<Image>`s for material textures regardless of
        // whether anything ever samples them, and panics if the asset type was never initialized
        // (normally `ImagePlugin`'s job, which the server doesn't have either).
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        // Needed for Skein to reflect `ColliderConstructor` off level geometry onto entities —
        // mirrors the same registration in `client/src/main.rs`.
        .register_type::<ColliderConstructor>()
        .run();
}
