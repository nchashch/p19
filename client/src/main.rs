use avian3d::prelude::*;
use bevy::feathers::{dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_ahoy::prelude::AhoyPlugins;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;
use lightyear::prelude::*;
use lightyear_avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use shared::assets::SharedAssetsPlugin;
use shared::replication::SharedReplicationPlugin;
use std::time::Duration;

use controls::fps_controller::FpsControllerPlugin;
use controls::input_device::InputDevicePlugin;
use dev::console::PConsolePlugin;
use gameplay::cube_spawner::CubeSpawnerPlugin;
use gameplay::player_character::PlayerCharacterPlugin;
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
const UI_SCALE: f32 = 2.0;

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        // Decided *before* anything else below — which plugin group even gets added is a
        // one-time choice at build time, long before any `Startup` system (including
        // `networking::load_client_config`, which reads the *rest* of `config.toml` the normal
        // way, via the `AssetServer`) could run. See `is_vr_enabled_presync`'s doc comment for why
        // this can't just reuse that later, `AssetServer`-based path.
        let vr_enabled = config::is_vr_enabled_presync();

        if vr_enabled {
            app.add_plugins(add_xr_plugins(
                DefaultPlugins.build().disable::<PipelinedRenderingPlugin>(),
            ));
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
            SkeinPlugin::default(),
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
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsTransformPlugin>()
                .disable::<PhysicsInterpolationPlugin>(),
            // `PredictionPlugin` is on by default but unconditionally assumes one of lightyear's
            // own input plugins (`lightyear_inputs_native`/`_bei`/`_leafwing`, none of which this
            // project uses — `Movement`/`Jump` are sent as plain `MessageSender` messages, not
            // through lightyear's input-replication system) has already initialized
            // `LastConfirmedInput`. Without that, `reset_input_rollback_tracker` panics
            // ("Resource does not exist: LastConfirmedInput") the moment a connection starts.
            // Disabling it outright matches this project's deliberate "no client-side prediction
            // yet" design (see AGENTS.md's top-of-file gap note) rather than wiring up an input
            // protocol this project doesn't otherwise need.
            client::ClientPlugins {
                tick_duration: Duration::from_secs_f32(1.0 / 60.0),
            }
            .build()
            .disable::<lightyear::prediction::plugin::PredictionPlugin>(),
            LightyearAvianPlugin {
                replication_mode: AvianReplicationMode::Position {
                    sync_to_transform: false,
                }, // default
                ..default()
            },
            (SharedReplicationPlugin, SharedAssetsPlugin),
            (
                CubeSpawnerPlugin,
                NpcSpawnerPlugin,
                NpcUiQuadPlugin,
                PlayerCharacterPlugin,
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
            .add_loading_state(
                LoadingState::new(GameState::AssetLoading)
                    .continue_to_state(GameState::MainMenu)
                    .with_dynamic_assets_file::<StandardDynamicAssetCollection>(
                        "collections/common_assets.assets.ron",
                    )
                    .load_collection::<assets::collections::CommonAssets>(),
            )
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
    }
}
