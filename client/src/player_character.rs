use crate::{
    camera::{Cubemap, PlayerCameraPlugin, player_camera},
    combat::CombatPlugin,
    controls::{self, PlayerControlsPlugin},
    fps_controller::FpsCamera,
    game_state::GameState,
};
use bevy::prelude::*;
use shared::npc_spawner::NpcSpawner;
use shared::{character_controller::CharacterControllerPlugin, player::PlayerCharacter};
use shared::{cube_spawner::CubeSpawner, server_events::PlayerSpawned};

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerName>();

        app.add_plugins((
            // CharacterControllerPlugin,
            CombatPlugin,
            PlayerControlsPlugin,
            PlayerCameraPlugin,
        ));

        app.add_observer(on_player_spawned);
        app.add_systems(Update, decorate_other_players);
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

#[derive(Component, Reflect, Default)]
pub struct OtherPlayer;

pub fn decorate_other_players(
    players: Query<Entity, (With<PlayerCharacter>, Without<OtherPlayer>)>,
    local_player: Res<LocalPlayer>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    for entity in players {
        if entity == local_player {
            continue;
        }
        commands.entity(entity).insert(OtherPlayer).with_child((
            WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
            Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            PlayerModel,
        ));
    }
}

/// The player's own `PlayerCharacter` entity, as told to us by the server via `PlayerSpawned` —
/// not derived by querying `With<PlayerCharacter>`, since once other players are connected there
/// can be several such entities replicated in and nothing about them locally distinguishes
/// "mine" from "someone else's."
#[derive(Resource, Deref, Clone, Copy)]
pub struct LocalPlayer(pub Option<Entity>);

pub fn on_player_spawned(
    spawned: On<PlayerSpawned>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cubemap: ResMut<Cubemap>,
) {
    commands.insert_resource(LocalPlayer(Some(spawned.entity)));
    commands
        .entity(spawned.entity)
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
