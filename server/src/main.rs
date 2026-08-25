use avian3d::prelude::*;
use bevy::prelude::*;
use shared::character_controller::CharacterControllerPlugin;
use shared::combat::SharedCombatPlugin;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            PhysicsPlugins::default(),
            SharedCombatPlugin,
            CharacterControllerPlugin,
        ))
        // avian3d's collider cache reads `AssetEvent<Mesh>` (for mesh-derived colliders) even
        // though the server never renders — normally registered by rendering plugins the headless
        // server doesn't have, so it needs registering directly instead.
        .init_asset::<Mesh>()
        .run();
}
