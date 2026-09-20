use bevy::prelude::*;
use bevy_hanabi::ParticleEffect;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_seedling::sample::SamplePlayer;
use shared::assets::level::ClientWorldAsset;
use shared::game_state::GameState;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            crate::ui::hud::spawn_in_game_scene,
        );
        app.add_systems(
            Update,
            spawn_client_world_assets.run_if(in_state(GameState::InGame)),
        );
    }
}

fn spawn_client_world_assets(
    client_world_assets: Query<(Entity, &ClientWorldAsset), Without<WorldAssetRoot>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, client_world_asset) in client_world_assets {
        let world_asset: Handle<WorldAsset> = asset_server.load(&client_world_asset.asset_path);
        commands.entity(entity).insert(WorldAssetRoot(world_asset));
    }
}

fn clear_effects(
    mut commands: Commands,
    particle_effects: Query<Entity, With<ParticleEffect>>,
    sample_players: Query<Entity, With<SamplePlayer>>,
    xr_root: Query<Entity, With<XrTrackingRoot>>,
) {
    for particle_effect in particle_effects {
        commands.entity(particle_effect).despawn();
    }
    for sample_player in sample_players {
        commands.entity(sample_player).despawn();
    }
    // `XrTrackingRoot` gets reparented onto the player's own `VrPlayspaceRig` once a player
    // spawns (see `vr_controllers::on_player_spawned`) — which otherwise means it (and everything
    // the VR session actually depends on: the tracked grip cubes, the lasers, any wrist-attached
    // `quad_panel`) gets despawned right along with the player character when
    // `DespawnOnExit(GameState::InGame)` fires below. `XrTrackingRoot` is `bevy_mod_xr`'s own core
    // playspace anchor, not something game logic should ever destroy — doing so froze the VR view
    // entirely while the desktop window kept working fine (confirmed by testing: the two
    // rendering paths are otherwise independent). Detaching it here, before the state transition
    // despawns the player (and so `VrPlayspaceRig`), keeps it alive to be re-parented onto the
    // *next* player's own rig once one spawns again — `on_player_spawned` already does that
    // unconditionally, regardless of whatever this entity's previous parent was.
    for xr_root in &xr_root {
        commands.entity(xr_root).remove::<ChildOf>();
    }
}
