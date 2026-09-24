use avian3d::prelude::*;
use bevy::feathers::{dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::winit::WinitPlugin;
use bevy::window::ExitCondition;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_ahoy::prelude::AhoyPlugins;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;
use lightyear::prelude::*;
use lightyear_avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use shared::assets::SharedAssetsPlugin;
use shared::inputs::SharedInputsPlugin;
use shared::mesh_primitive::SharedMeshPrimitivePlugin;
use shared::replication::SharedReplicationPlugin;
use std::time::Duration;

use controls::fps_controller::FpsControllerPlugin;
use controls::camera::{
    HeadlessUiCameraBootstrap, OffscreenRenderTarget, keep_ui_camera_drawn_last,
    maintain_default_ui_camera, retarget_cameras_to_offscreen,
};
use controls::input_device::InputDevicePlugin;
use dev::console::PConsolePlugin;
use gameplay::cube_spawner::CubeSpawnerPlugin;
use gameplay::player_character::PlayerCharacterPlugin;
use presentation::mesh_primitive::ClientMeshPrimitivePlugin;
use presentation::particles::ParticleEffectsPlugin;
use shared::game_state::{GameState, GameStatePlugin};

use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

mod assets;
mod config;
mod controls;
mod dev;
mod events;
mod gameplay;
mod lifecycle;
mod presentation;
mod ui;

/// Registers each `$observer` with `$app`, gated behind `$condition` (e.g.
/// `chill_bevy_console::console_closed`, to suppress gameplay observers while the
/// dev console is open).
macro_rules! add_observers_run_if {
    ($app:expr, $condition:expr, $($observer:expr),+ $(,)?) => {
        $( $app.add_observer($observer.run_if($condition)); )+
    };
}
pub(crate) use add_observers_run_if;

use crate::lifecycle::lobby::LobbyPlugin;
use crate::{
    controls::vr_controllers::VrControllersPlugin, gameplay::npc_spawner::NpcSpawnerPlugin,
    lifecycle::loading::LoadingPlugin, presentation::animation::PAnimationPlugin,
    ui::input_icons::InputIconsPlugin, ui::localization::LocalizationPlugin,
    ui::modal_menu::ModalMenuPlugin, ui::nameplate::NameplatePlugin,
    ui::npc_ui_quad::NpcUiQuadPlugin, ui::quad_panel::QuadPanelPlugin,
};

fn main() {
    App::new().add_plugins(Prototype19).run();
}

/// Global UI scale factor — see `bevy::ui::UiScale`'s own doc comment (a plain multiplier applied
/// on top of the window's scale factor, affecting every `bevy_ui` node uniformly). Not yet
/// per-platform/settings-driven (e.g. a Deck-vs-desktop default, or a real options-menu slider) —
/// just a single hardcoded constant for now.
const UI_SCALE: f32 = 1.0;

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        // Decided *before* anything else below — which plugin group even gets added is a
        // one-time choice at build time, long before any `Startup` system (including
        // `networking::load_client_config`, which reads the *rest* of `config.toml` the normal
        // way, via the `AssetServer`) could run. See `is_vr_enabled_presync`'s doc comment for why
        // this can't just reuse that later, `AssetServer`-based path.
        let vr_enabled = config::is_vr_enabled_presync();
        // `--mcp` (or config.toml's `mcp = true`): run as a headless agent host — no window at
        // all, every camera rendered into an offscreen texture, the tool API (BRP + MCP)
        // serving localhost. Same pre-sync reasoning as `vr_enabled` above. Incompatible with
        // VR (the XR swapchain needs a session, and this mode's purpose is display-less hosts);
        // `--mcp` wins when both are set.
        let mcp_headless = config::is_mcp_mode_presync() && !vr_enabled;
        // `--no-common-assets`: barest boot for fully-plaintext playtest asset roots — no
        // `CommonAssets` collection load at all (see the branch at the bottom of this method).
        let no_common_assets = config::is_no_common_assets_presync();

        if vr_enabled {
            app.add_plugins(add_xr_plugins(
                DefaultPlugins.build().disable::<PipelinedRenderingPlugin>(),
            ));
        } else if mcp_headless {
            // The headless-renderer pattern (bevy's own `headless_renderer` example): no winit
            // at all — `ScheduleRunnerPlugin` drives the frame loop, no window is ever created
            // (so this runs on display-less hosts too, with a software Vulkan driver such as
            // lavapipe), and every camera renders into [`OffscreenRenderTarget`] via the
            // retarget system below. UI renders into the same texture (the UI camera is a
            // normal camera).
            app.add_plugins(
                DefaultPlugins.build()
                    .disable::<WinitPlugin>()
                    .disable::<PipelinedRenderingPlugin>()
                    .set(WindowPlugin {
                        primary_window: None,
                        exit_condition: ExitCondition::DontExit,
                        ..default()
                    }),
            )
            .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)));
        let offscreen_target = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            // 1280×800 — the Steam Deck's native (800p) resolution, this project's primary
            // target platform: agent captures see exactly what a Deck player would, including
            // the 16:10 aspect ratio (menu layout / camera framing differ from 16:9).
            OffscreenRenderTarget::new(1280, 800, &mut images)
        };
        app.insert_resource(offscreen_target)
            // A camera for UI that exists before any player/menu-background camera does —
            // otherwise bevy_ui has nothing to render the main menu onto until the level's
            // cameras arrive. The retarget system aims it at the offscreen texture.
            // `HeadlessUiCameraBootstrap` marks it specifically (distinct from `player_camera()`,
            // which also carries `IsDefaultUiCamera`) so `maintain_default_ui_camera` can hand
            // the marker back and forth between them instead of letting both hold it at once —
            // see `retarget_cameras_to_offscreen`'s doc comment for why that ambiguity is the
            // real cause of the menu/lobby-UI-render-order bug, not an ordering problem on its
            // own.
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((Camera2d, IsDefaultUiCamera, HeadlessUiCameraBootstrap));
            })
            .add_systems(
                Update,
                (
                    retarget_cameras_to_offscreen,
                    maintain_default_ui_camera,
                    keep_ui_camera_drawn_last,
                )
                    .chain(),
            );
            // A prior version of this file stripped `Skybox`/`TemporalAntiAliasing`/
            // `ScreenSpaceAmbientOcclusion` from every camera here, believing the loaded KTX2
            // (BC6H) skybox specifically killed offscreen rendering (silently, no wgpu error) and
            // that TAA/SSAO did the same to the player camera. Confirmed by testing (once
            // `retarget_cameras_to_offscreen`'s `target_info` bug — see that function's doc
            // comment — was actually fixed) that this was never a separate bug: every one of
            // those "kills" was the same camera never having a resolved render target, so nothing
            // it carried could render either. With `target_info` fixed, the skybox/TAA/SSAO all
            // render correctly unstripped; no headless-specific carve-out needed here at all.
        } else {
            app.add_plugins(DefaultPlugins);
        }

        app.add_plugins((
            bevy::feathers::FeathersPlugins,
            PAnimationPlugin,
            PConsolePlugin,
            LoadingPlugin,
            LocalizationPlugin,
            SeedlingPlugins,
            ParticleEffectsPlugin,
            LobbyPlugin,
            // Same distinctive-extension reasoning as `Level` above — `assets::controller::Controller`
            // (e.g. `assets/controllers/player.controller.ron`) is a plain data asset, loaded
            // independent of `bevy_asset_loader`'s dynamic-asset manifests. A distinctive compound
            // extension, same convention `bevy_asset_loader` itself uses for `"assets.ron"` (see
            // `assets::collections`'s `common_assets.assets.ron`/`Level.assets.ron`) rather than
            // bare `"ron"` — not just for symmetry: a bare `"ron"` registration only avoids
            // ambiguity with other RON-based asset types as long as every call site stays
            // explicitly typed (`AssetServer::load::<T>(path)`, never `load_untyped`). A
            // distinctive extension per type sidesteps that structurally instead of relying on it.
            RonAssetPlugin::<assets::controller::Controller>::new(&["controller.ron"]),
            // `handle_brp: false`: DevToolsPlugin owns the BRP server so `--brp-port` applies in
            // dev builds too (Skein's default-add is hardwired to port 15702). Skein's own
            // presets endpoint still registers in its `finish` because DevToolsPlugin adds
            // `RemotePlugin` (hence the `RemoteMethods` resource) before `finish` runs.
            SkeinPlugin {
                handle_brp: false,
                ..Default::default()
            },
            // Full Avian simulation runs client-side now, same as the server — the client is no
            // longer just holding colliders for spatial queries while waiting on replicated
            // `Transform`s. The server remains authoritative (`LightyearAvianPlugin` below
            // reconciles local simulation against the replicated `Position`/`Rotation` it
            // receives), but `RigidBody::Dynamic` cubes/NPCs now predict their own motion locally
            // between replication ticks instead of only moving when a new server `Transform`
            // arrives, and `PlayerCharacter` movement is unaffected either way since it's driven
            // directly by `character_controller.rs`'s kinematic move-and-slide, not the solver.
            //
            // `PhysicsTransformPlugin`/`PhysicsInterpolationPlugin` are disabled because
            // `LightyearAvianPlugin` takes over `Position`/`Rotation` <-> `Transform`
            // synchronization and frame interpolation itself — running both at once is exactly
            // the footgun `lightyear_avian3d`'s own docs warn against (see its module doc
            // comment), not something specific to this project. Mirrors `server/src/main.rs`'s
            // identical `PhysicsPlugins` setup.
            //
            // `sync_to_transform: true` (NOT the default) makes `Transform` the authoring API
            // during fixed ticks — `Transform` is imported into `Position` before physics,
            // and copied back after. This is required by ahoy: its KCC writes **`Transform`**
            // only (`run_kcc`'s final write), and with the import disabled every ahoy-driven
            // character froze at `Position (0,0,0)` while `LinearVelocity` accumulated
            // unboundedly (`CustomPositionIntegration` deliberately excludes KCC bodies from
            // avian's own velocity integration, so nothing else moves them either). It also
            // fixes player spawning: `player()` positions the character via `Transform` only.
            // The import is safe for this setup: it only touches `With<RigidBody>` entities
            // (interpolated remote entities don't carry `RigidBody` client-side) and only on
            // `Changed<Transform>` with a real value difference.
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
            // `PredictionPlugin` hard-depends on one of lightyear's own input plugins
            // (`lightyear_inputs_native`/`_bei`/`_leafwing`) having already initialized
            // `LastConfirmedInput` — without one, `reset_input_rollback_tracker` panics
            // ("Resource does not exist: LastConfirmedInput") the moment a connection starts.
            // The ahoy/prediction migration's M0 resolves that the idiomatic way:
            // `lightyear_inputs_bei` (BEI is ahoy's native input layer) is registered via
            // `shared::inputs::SharedInputsPlugin` below, so the plugin runs un-disabled.
            client::ClientPlugins {
                tick_duration: Duration::from_secs_f32(1.0 / 60.0),
            },
            LightyearAvianPlugin {
                replication_mode: AvianReplicationMode::Position {
                    // NOT the default — see the comment above `PhysicsPlugins`: ahoy's KCC
                    // authors `Transform` during fixed ticks, which only reaches `Position`
                    // (and thus the visual/camera sync in PostUpdate) when the
                    // Transform→Position import is enabled.
                    sync_to_transform: true,
                },
                ..default()
            },
            (
                SharedReplicationPlugin,
                SharedAssetsPlugin,
                SharedInputsPlugin,
                SharedMeshPrimitivePlugin,
            ),
            (
                CubeSpawnerPlugin,
                NpcSpawnerPlugin,
                NpcUiQuadPlugin,
                PlayerCharacterPlugin,
                ClientMeshPrimitivePlugin,
                // `bevy_ahoy` (Bevy + Avian + BEI kinematic character controller) — wired in
                // ahead of the actual controller migration, deliberately inert for now: no
                // entity carries ahoy's `CharacterController` and none of its own input actions
                // are bound yet (`controls/actions.rs`'s existing `Movement`/`Jump` still feed
                // the old send-a-network-message path), so every system this group registers
                // simply matches no entities until that changes. Both prerequisites are already
                // registered: `PhysicsPlugins` (outer tuple, above) and `EnhancedInputPlugin`
                // (added by `PlayerControlsPlugin`, via `PlayerCharacterPlugin` directly above).
                // Sits in this inner tuple only because the outer one is already at Bevy's
                // 15-element `Plugins` tuple-impl limit. Default schedule: `FixedPostUpdate`.
                AhoyPlugins::default(),
                FpsControllerPlugin,
                GameStatePlugin { vr_enabled },
                InputDevicePlugin,
                NameplatePlugin,
                ui::ui::PrototypeUiPlugin,
                lifecycle::networking::NetworkingPlugin,
                ModalMenuPlugin,
                QuadPanelPlugin,
                InputIconsPlugin,
            ),
        ));

        // `OxrSessionConfig`/`HandGizmosPlugin`/`VrControllersPlugin` are all meaningless (and, for
        // `VrControllersPlugin`, actively broken — its `Startup`/`On<PlayerSpawned>` systems use
        // `Single<Entity, With<XrTrackingRoot>>`, an entity that only exists once the XR plugins
        // above actually ran, so a bare `Single` query on it panics under plain `DefaultPlugins`)
        // without an active XR session, so they're only added when `vr_enabled` is true, not just
        // toggled off via a `run_if` inside them.
        if vr_enabled {
            app.insert_resource(OxrSessionConfig {
                blend_mode_preference: vec![EnvironmentBlendMode::OPAQUE],
                ..default()
            })
            .add_plugins((
                bevy_mod_xr::hand_debug_gizmos::HandGizmosPlugin,
                VrControllersPlugin,
            ));
        }

        #[cfg(feature = "dev-tools")]
        // The agent/QA tool API (ADR 0009): BRP on 127.0.0.1:15702 + the MCP wrapper on
        // 15703 — dev-only (the client is untrusted; a tool API in it is a cheat surface).
        app.add_plugins(dev::tool_api::DevToolsPlugin);

        app.insert_resource(UiTheme(create_dark_theme()))
            .insert_resource(UiScale(UI_SCALE))
            .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 100.,
                ..default()
            })
            .register_type::<ColliderConstructor>()
            .init_asset::<assets::character::Character>()
            .register_asset_loader(assets::character::CharacterAssetLoader)
            .add_systems(
                OnEnter(GameState::MainMenu),
                (
                    ui::ui::spawn_main_menu,
                    assets::collections::override_default_font,
                ),
            )
            .add_systems(
                Update,
                assets::collections::override_feathers_button_font
                    .run_if(resource_exists::<assets::collections::CommonAssets>),
            );

        if no_common_assets {
            // The `--no-common-assets` barest boot: no `LoadingState` is registered (so no
            // manifest is read, and nothing blocks on asset completion) — instead a placeholder
            // collection (dangling world handles, `None` furniture; see its doc comment) is
            // inserted up front and `Startup` performs the `AssetLoading → MainMenu` transition
            // the loading state's completion normally would. UI spawns against the placeholder
            // and every consumer degrades via the `None` furniture fields; world content still
            // arrives via `ClientWorldAsset`s, which load by path, not through this manifest.
            app.insert_resource(assets::collections::CommonAssets::placeholder()).add_systems(
                Startup,
                |mut next_state: ResMut<NextState<GameState>>| {
                    next_state.set(GameState::MainMenu);
                },
            );
        } else {
            app.add_loading_state(
                LoadingState::new(GameState::AssetLoading)
                    .continue_to_state(GameState::MainMenu)
                    .with_dynamic_assets_file::<StandardDynamicAssetCollection>(
                        "collections/common_assets.assets.ron",
                    )
                    .load_collection::<assets::collections::CommonAssets>(),
            );
        }
    }
}
