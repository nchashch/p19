use crate::{
    camera::{Cubemap, PlayerCameraPlugin, player_camera},
    combat::CombatPlugin,
    controls::{self, PlayerControlsPlugin},
    fps_controller::FpsCamera,
    game_state::GameState,
};
use bevy::prelude::*;
use shared::npc_spawner::NpcSpawner;
use shared::{character_controller::CharacterControllerPlugin, player::player};
use shared::{
    cube_spawner::CubeSpawner,
    player::{PlayerCharacter, PlayerCharacterSpawner},
};

use crate::events::RespawnPlayer;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerName>();

        app.add_plugins((
            CharacterControllerPlugin,
            CombatPlugin,
            PlayerControlsPlugin,
            PlayerCameraPlugin,
        ));
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);

        app.add_observer(decorate_player);
    }
}

/// The name entered in the main menu's text field, used for the player's `Name` component.
/// Defaults to "Player" so a fresh app (or skipping the field) behaves as before.
#[derive(Resource)]
pub struct PlayerName(pub String);

impl Default for PlayerName {
    fn default() -> Self {
        Self("Player".to_string())
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerModel;

pub fn initial_respawn(mut commands: Commands) {
    commands.trigger(RespawnPlayer);
}

pub fn decorate_player(
    _: On<Add, PlayerCharacter>,
    mut commands: Commands,
    player_character: Query<Entity, With<PlayerCharacter>>,
    asset_server: Res<AssetServer>,
    mut cubemap: ResMut<Cubemap>,
) {
    let Ok(player_character) = player_character.single() else {
        return;
    };
    commands
        .entity(player_character)
        .insert(DespawnOnExit(GameState::InGame))
        .with_children(|parent| {
            parent.spawn(controls::player_controls());
            parent
                .spawn((Transform::from_xyz(0., 0.5, 0.),))
                .with_children(|parent| {
                    parent
                        .spawn((FpsCamera::new(), Transform::IDENTITY))
                        .with_children(|parent| {
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                            parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), NpcSpawner));
                            parent.spawn(player_camera(&asset_server, &mut cubemap));
                        });
                });
        });
}
