use avian3d::prelude::*;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::prelude::*;
use bevy_skein::SkeinPlugin;

use cube_spawner::CubeSpawnerPlugin;
use game_state::{GameState, GameStatePlugin};
use player_character::PlayerCharacterPlugin;

mod character_controller;
mod cube_spawner;
mod fps_camera;
mod game_state;
mod player_character;
mod ui;

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins,
            SkeinPlugin::default(),
            PhysicsPlugins::default(),
            CubeSpawnerPlugin,
            PlayerCharacterPlugin,
            GameStatePlugin,
            ui::PrototypeUiPlugin,
            PhysicsDebugPlugin::default(),
            FpsOverlayPlugin::default(),
        ));
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 10.,
            ..default()
        });
        app.register_type::<ColliderConstructor>();
        app.add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
        app.add_systems(OnEnter(GameState::InGame), ui::in_game_scene.spawn());
    }
}
