use std::time::Duration;

use avian3d::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::image::{CompressedImageFormatSupport, CompressedImageFormats};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use lightyear::avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use lightyear::prelude::*;
use shared::assets::SharedAssetsPlugin;
use shared::assets::level::LevelMetadataAssets;
use shared::game_state::ServerState;
use shared::inputs::SharedInputsPlugin;
use shared::mesh_primitive::SharedMeshPrimitivePlugin;
use shared::replication::SharedReplicationPlugin;

mod combat;
mod input;
mod level_state;
mod lobby;
mod networking;
mod rooms;
mod spawn;
mod tools;

use combat::ServerCombatPlugin;
use input::ServerInputPlugin;
use level_state::LevelStatePlugin;
use lobby::LobbyPlugin;
use rooms::GameRoomPlugin;
use spawn::ServerSpawnPlugin;
use tools::ServerToolsPlugin;

fn main() {
    App::new()
        .add_plugins((
            // `MinimalPlugins`' own `ScheduleRunnerPlugin` defaults to `RunMode::Loop { wait:
            // None }` — an unthrottled busy loop, no sleep between `App::update()` calls ever.
            // Confirmed live: an idle server with zero clients connected was burning ~200% CPU
            // (measured via /proc/<pid>/stat across 3 separate 5s windows) — the main thread
            // plus all 8 `Compute Task Pool` workers spinning continuously checking "is it tick
            // time yet", nothing to do with replication/networking (a connected, actively-moving
            // client added no measurable delta on top of that). `run_loop` caps `App::update()`
            // to this interval, sleeping only the remainder after accounting for actual work
            // time (`bevy_app::schedule_runner`'s own `wait - exe_time` calculation) — matching
            // `server::ServerPlugins`' own `tick_duration` below, so this doesn't change
            // simulation cadence, only how often the outer loop spins while there's nothing new
            // to do.
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
            // `RepliconPlugins` needs `States`, and log output needs a subscriber — both are
            // included in `DefaultPlugins` (which the client uses) but not in `MinimalPlugins`.
            StatesPlugin,
            // `lightyear_debug_log_plugin()` (from `lightyear_tools`, re-exported via
            // `lightyear::prelude` behind the `debug` feature): writes every
            // `lightyear_debug::*` structured trace event (`server_input_message_recv` —
            // the exact `BEIStateSequence` this server received per client per tick — plus
            // timeline/sync/message/replication events) as JSONL to `$LIGHTYEAR_DEBUG_FILE`,
            // and keeps them out of the normal human-readable log. No-ops (falls back to plain
            // logging) if that env var isn't set. `LightyearDebugPlugin` itself (the
            // metadata/schedule/role resource these trace events read from) is auto-added by
            // lightyear's own `SharedPlugin` once the `debug` feature is on. The `filter`
            // override is required, not cosmetic — these are `trace!`-level events, and Bevy's
            // default `LogPlugin` level is `INFO`; without a more-specific `lightyear_debug=trace`
            // directive they never reach any layer (including the JSONL one) at all, confirmed
            // live (empty output file until this was added).
            LogPlugin {
                filter: format!("{},lightyear_debug=trace", bevy::log::DEFAULT_FILTER),
                ..lightyear_debug_log_plugin()
            },
            TransformPlugin,
            // `server` has no `assets/` directory of its own — level geometry (and eventually
            // anything else the server needs, e.g. collider-relevant data) lives in
            // `client/assets/` (see AGENTS.md's note on why assets live inside `client/`), so
            // point the default filesystem asset source there instead of duplicating it.
            AssetPlugin {
                // file_path: "../client/assets".to_string(),
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
                    // NOT the default — see the matching comment above `PhysicsPlugins` in
                    // `client/src/main.rs`: ahoy's KCC authors `Transform` during fixed ticks,
                    // which only reaches `Position` (and thus replication) when the
                    // Transform→Position import is enabled.
                    sync_to_transform: true,
                },
                ..default()
            },
            SharedReplicationPlugin,
            SharedAssetsPlugin,
            // Registers the `lightyear_inputs_bei` input protocol (the replicated-BEI input
            // flow — see `shared/src/inputs.rs`). The server half of `InputPlugin` is
            // headless-safe: it registers `ServerInputPlugin<BEIStateSequence<C>>` and
            // disables BEI's own update systems on headless binaries, so it needs no
            // rendering and no real input devices. Note this outer tuple is now at Bevy's
            // 15-element `Plugins` limit — the next top-level plugin needs to join the nested
            // GLTF tuple below.
            SharedInputsPlugin,
            ServerCombatPlugin,
            ServerSpawnPlugin,

            // Loads `.glb` level geometry headlessly: `GltfPlugin` parses the file,
            // `WorldSerializationPlugin` instantiates it as a `WorldAssetRoot`/reflected entity
            // graph (the same mechanism `WorldInstanceReady` etc. rely on client-side), and
            // `SkeinPlugin` applies whatever reflected components (including `ColliderConstructor`)
            // are baked into the file's extras. None of these need rendering. `GameRoomPlugin`/
            // `LobbyPlugin` (room-based interest management, and replicating the available level
            // list while in the lobby room — see `rooms.rs`/`lobby.rs`) are nested in here too,
            // not because they're related, just because `add_plugins` only supports so many
            // top-level elements before it runs out of `Plugins` tuple impls.
            (
                bevy::gltf::GltfPlugin::default(),
                bevy::world_serialization::WorldSerializationPlugin,
                // `handle_brp: false`: `SkeinPlugin::default()`'s `handle_brp` is
                // `cfg!(debug_assertions)` — true in debug builds — so left at its default it
                // adds `RemotePlugin`/`RemoteHttpPlugin` itself, and `ServerToolsPlugin` below
                // (which unconditionally adds both, always-on regardless of profile — see its
                // own doc comment) then panics on the duplicate add. Same fix the client already
                // applies to its own `SkeinPlugin` (`client/src/main.rs`), for the same reason.
                bevy_skein::SkeinPlugin {
                    handle_brp: false,
                    ..default()
                },
                // Ahoy's KCC stack — the server-authoritative half of the M2 movement setup
                // (the client registers the same group; both sides simulate the player over
                // the same replicated-BEI input stream). Headless-safe: `InputPlugin` disables
                // BEI's raw-input systems on server-only builds, and the camera plugin's
                // observers simply never fire without camera entities.
                bevy_ahoy::prelude::AhoyPlugins::default(),
                // Accumulates the replicated `RotateCamera` action into the server-side
                // `CharacterLook` (see `input.rs`).
                ServerInputPlugin,
                LevelStatePlugin,
                GameRoomPlugin,
                LobbyPlugin,
                // BRP (localhost:15701, `--brp-port`) + MCP (15711, `--mcp-port`) debugging
                // surface — authoritative server state for desync debugging; see `tools.rs`.
                ServerToolsPlugin,
                // Registers `MeshPrimitive`'s reflection (shared with `client` — see
                // `shared::mesh_primitive`'s own doc comment) so this binary's `AppTypeRegistry`
                // recognizes the type too, whether or not anything on this side ever reacts to
                // it — only `client` currently does (a purely visual `Mesh3d`/`MeshMaterial3d`
                // spawn, no collider). Nested here for the same tuple-arity reason as everything
                // else in this group, not because it's related to GLTF/Skein specifically beyond
                // depending on the same reflection pipeline.
                SharedMeshPrimitivePlugin,
            ),
            networking::NetworkingPlugin,
        ))
        .init_state::<ServerState>()
        .add_loading_state(
            LoadingState::new(ServerState::Startup)
                .continue_to_state(ServerState::Lobby)
                .load_collection::<LevelMetadataAssets>(),
        )
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
