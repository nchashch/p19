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
mod loading;
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
    animation::PAnimationPlugin, loading::LoadingPlugin, nameplate::NameplatePlugin,
    npc_spawner::NpcSpawnerPlugin, vr_controllers::VrControllersPlugin,
};

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            add_xr_plugins(DefaultPlugins.build()),
            bevy::feathers::FeathersPlugins,
            PAnimationPlugin,
            PConsolePlugin,
            LoadingPlugin,
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
                GameStatePlugin,
                NameplatePlugin,
                ui::PrototypeUiPlugin,
                networking::NetworkingPlugin,
            ),
        ))
        .insert_resource(OxrSessionConfig {
            blend_mode_preference: vec![EnvironmentBlendMode::OPAQUE],
            ..default()
        })
        .add_plugins((
            bevy_mod_xr::hand_debug_gizmos::HandGizmosPlugin,
            VrControllersPlugin,
        ))
        .insert_resource(UiTheme(create_dark_theme()))
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
