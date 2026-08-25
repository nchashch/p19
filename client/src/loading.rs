use bevy::prelude::*;

use crate::game_state::GameState;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Loading),
            (crate::ui::in_game_scene.spawn(), load_level),
        )
        .add_systems(Update, wait_for_level.run_if(in_state(GameState::Loading)));
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
