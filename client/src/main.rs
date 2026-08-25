use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;

use console::PConsolePlugin;
use cube_spawner::CubeSpawnerPlugin;
use fps_controller::FpsControllerPlugin;
use game_state::{GameState, GameStatePlugin};
use particles::ParticleEffectsPlugin;
use player_character::PlayerCharacterPlugin;

mod animation;
mod combat;
mod console;
mod cube_spawner;
mod fps_controller;
mod game_state;
mod loading;
mod nameplate;
mod npc_spawner;
mod particles;
mod player_character;
mod targeting;
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

use crate::{
    animation::PAnimationPlugin, loading::LoadingPlugin, nameplate::NameplatePlugin,
    npc_spawner::NpcSpawnerPlugin,
};

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins,
            PAnimationPlugin,
            PConsolePlugin,
            LoadingPlugin,
            SeedlingPlugins,
            ParticleEffectsPlugin,
            SkeinPlugin::default(),
            PhysicsPlugins::default(),
            CubeSpawnerPlugin,
            NpcSpawnerPlugin,
            PlayerCharacterPlugin,
            FpsControllerPlugin,
            GameStatePlugin,
            NameplatePlugin,
            ui::PrototypeUiPlugin,
        ))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 100.,
            ..default()
        })
        .register_type::<ColliderConstructor>()
        .add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
    }
}
