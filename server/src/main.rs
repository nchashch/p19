use std::time::Duration;

use avian3d::prelude::*;
use bevy::image::{CompressedImageFormatSupport, CompressedImageFormats};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use lightyear::avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use lightyear::prelude::*;
use shared::character_controller::CharacterControllerPlugin;
use shared::replication::SharedReplicationPlugin;

mod combat;
mod level_state;
mod networking;
mod spawn;

use combat::ServerCombatPlugin;
use level_state::LevelStatePlugin;
use spawn::ServerSpawnPlugin;

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
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
            server::ServerPlugins {
                tick_duration: Duration::from_secs_f32(1.0 / 60.0),
            },
            LightyearAvianPlugin {
                replication_mode: AvianReplicationMode::Position {
                    sync_to_transform: false,
                }, // default
                ..default()
            },
            // RepliconPlugins,
            // RepliconQuinnetPlugins,
            SharedReplicationPlugin,
            ServerCombatPlugin,
            ServerSpawnPlugin,
            CharacterControllerPlugin,
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
                LevelStatePlugin,
            ),
            networking::NetworkingPlugin,
        ))
        // .init_state::<ServerState>()
        // avian3d's collider cache reads `AssetEvent<Mesh>` (for mesh-derived colliders) even
        // though the server never renders — normally registered by rendering plugins the headless
        // server doesn't have, so it needs registering directly instead. `Image` needs the same
        // treatment: `GltfLoader` allocates `Handle<Image>`s for material textures regardless of
        // whether anything ever samples them, and panics if the asset type was never initialized
        // (normally `ImagePlugin`'s job, which the server doesn't have either).
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        // Without a real render device, `GltfPlugin::finish()` has no `CompressedImageFormatSupport`
        // to read and falls back to `CompressedImageFormats::NONE` — which forces any KTX2/UASTC
        // texture's transcode down `bevy_image::ktx2`'s uncompressed `Rgba8Unorm` fallback path
        // instead of a real block-compressed target. That fallback path has a real upstream bug
        // (confirmed against `bevy_image-0.19.0`'s source): it slices the *source* UASTC bytes using
        // a size computed for the *target* `Rgba8Unorm` bytes (4 bytes/pixel vs UASTC's 1 byte/pixel
        // equivalent), so it panics with a slice-out-of-range error on any real KTX2 texture rather
        // than just running slower. Claiming `BC` support here is a total fiction — the server never
        // renders or samples the resulting `Image` at all — but it's harmless and routes the
        // transcode down the BC7 branch instead, which doesn't hit the bug (BC7's block size happens
        // to match UASTC's own, unlike `Rgba8Unorm`'s).
        .insert_resource(CompressedImageFormatSupport(CompressedImageFormats::BC))
        // Needed for Skein to reflect `ColliderConstructor` off level geometry onto entities —
        // mirrors the same registration in `client/src/main.rs`.
        .register_type::<ColliderConstructor>()
        .run();
}
