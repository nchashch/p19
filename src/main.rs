use avian3d::prelude::*;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::prelude::*;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;

use cube_spawner::CubeSpawnerPlugin;
use game_state::{GameState, GameStatePlugin};
use player_character::PlayerCharacterPlugin;

use crate::{fps_controller::FpsControllerPlugin, particles::ParticleEffectsPlugin};

mod character_controller;
mod cube_spawner;
mod fps_camera;
mod fps_controller;
mod game_state;
mod particles;
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
            SeedlingPlugins,
            ParticleEffectsPlugin,
            SkeinPlugin::default(),
            PhysicsPlugins::default(),
            CubeSpawnerPlugin,
            PlayerCharacterPlugin,
            FpsControllerPlugin,
            GameStatePlugin,
            ui::PrototypeUiPlugin,
            // PhysicsDebugPlugin::default(),
            FpsOverlayPlugin::default(),
        ));
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 10.,
            ..default()
        });
        app.register_type::<ColliderConstructor>();
        app.add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
        app.add_systems(
            OnEnter(GameState::Loading),
            (ui::in_game_scene.spawn(), load_level),
        );
        app.add_systems(Update, wait_for_level.run_if(in_state(GameState::Loading)));
    }
}

#[derive(Resource)]
pub struct LevelScene(Handle<WorldAsset>);

fn wait_for_level(
    asset_server: Res<AssetServer>,
    level: Res<LevelScene>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // is_loaded_with_dependencies is the one you want for scenes/glTF —
    // it waits on the whole dependency graph, not just the root asset.
    if asset_server.is_loaded_with_dependencies(&level.0) {
        next_state.set(GameState::InGame);
    }
}

fn load_level(asset_server: Res<AssetServer>, mut commands: Commands) {
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset("Level.glb#Scene0"));
    commands.insert_resource(LevelScene(handle.clone()));
    commands.spawn((
        WorldAssetRoot(handle),
        DespawnOnExit::<GameState>(GameState::InGame),
    ));
}
