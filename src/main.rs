use avian3d::prelude::*;
use bevy::{light::DirectionalLightShadowMap, prelude::*};
use bevy_skein::SkeinPlugin;
use game_state::{GameState, GameStatePlugin};

use cube_spawner::CubeSpawnerPlugin;

mod cube_spawner;
mod game_state;
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
            GameStatePlugin,
            ui::PrototypeUiPlugin,
            // PhysicsDebugPlugin::default(),
        ));
        app.insert_resource(DirectionalLightShadowMap { size: 4096 });
        app.register_type::<ColliderConstructor>();
        app.add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
        app.add_systems(OnEnter(GameState::InGame), ui::in_game_scene.spawn());
    }
}

// TODO: Add support for ktx2 in .glb files.
/*

Currently there is an error:

ERROR bevy_asset::server: Failed to load asset 'Cube_opt.glb' with asset loader 'bevy_gltf::loader::GltfLoader': invalid glTF file: invalid glTF: textures[0].source: Missing data; extensionsRequired[0] = "KHR_texture_basisu": Unsupported extension;

*/
