use avian3d::prelude::*;
use bevy::feathers::{dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_replicon_quinnet::RepliconQuinnetPlugins;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;
use shared::replication::SharedReplicationPlugin;

use console::PConsolePlugin;
use cube_spawner::CubeSpawnerPlugin;
use fps_controller::FpsControllerPlugin;
use game_state::{GameState, GameStatePlugin};
use particles::ParticleEffectsPlugin;
use player_character::PlayerCharacterPlugin;

use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

mod actions;
mod animation;
mod camera;
mod combat;
mod console;
mod controls;
mod cube_spawner;
mod events;
mod fps_controller;
mod game_state;
mod hud;
mod input_icons;
mod loading;
mod localization;
mod nameplate;
mod networking;
mod npc_spawner;
mod particles;
mod player_character;
mod targeting;
mod ui;
mod vr_controllers;
mod widgets;

/// Registers each `$observer` with `$app`, gated behind `$condition` (e.g.
/// `chill_bevy_console::console_closed`, to suppress gameplay observers while the
/// dev console is open).
macro_rules! add_observers_run_if {
    ($app:expr, $condition:expr, $($observer:expr),+ $(,)?) => {
        $( $app.add_observer($observer.run_if($condition)); )+
    };
}
pub(crate) use add_observers_run_if;

use crate::{
    animation::PAnimationPlugin, loading::LoadingPlugin, localization::LocalizationPlugin,
    nameplate::NameplatePlugin, npc_spawner::NpcSpawnerPlugin, vr_controllers::VrControllersPlugin,
};

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        // Decided *before* anything else below — which plugin group even gets added is a
        // one-time choice at build time, long before any `Startup` system (including
        // `networking::load_client_config`, which reads the *rest* of `config.toml` the normal
        // way, via the `AssetServer`) could run. See `is_vr_enabled_presync`'s doc comment for why
        // this can't just reuse that later, `AssetServer`-based path.
        let vr_enabled = networking::is_vr_enabled_presync();

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
            SkeinPlugin::default(),
            PhysicsPlugins::default(),
            bevy_replicon::prelude::RepliconPlugins,
            RepliconQuinnetPlugins,
            SharedReplicationPlugin,
            (
                CubeSpawnerPlugin,
                NpcSpawnerPlugin,
                PlayerCharacterPlugin,
                FpsControllerPlugin,
                GameStatePlugin { vr_enabled },
                NameplatePlugin,
                ui::PrototypeUiPlugin,
                networking::NetworkingPlugin,
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
            .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 100.,
                ..default()
            })
            .register_type::<ColliderConstructor>()
            .add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
    }
}
