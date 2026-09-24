use bevy::prelude::*;
use bevy_hanabi::ParticleEffect;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_seedling::sample::SamplePlayer;
use lightyear::prelude::client::Remote;
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
            spawn_client_world_assets.run_if(in_state(GameState::InGame)).run_if(
                // `--no-render`: world visuals reference image/mesh assets whose loaders live
                // with the (disabled) render-side asset machinery — and there is nothing to
                // show them on anyway. Everything else about InGame (netcode, player spawn,
                // movement, `game/ui`, input) works unchanged.
                not(resource_exists::<crate::controls::camera::NoRenderMode>),
            ),
        );
    }
}

#[derive(Component)]
struct ClientWorldAssetSpawned;

fn spawn_client_world_assets(
    // `With<Remote>` matters: this system materializes *server-authored* world content only.
    // Without it, any client-locally instantiated entity that carries a Skein-baked
    // `ClientWorldAsset` (glb extras are applied to scene-instantiated entities too) also
    // matches — and if the referenced asset itself has one baked in, each instantiation
    // spawns another match pointing back at the same file: a self-feeding spawn loop
    // (confirmed by testing: entity count growing ~1.5x-per-cycle while loading
    // `rigs/visuals.glb`, which had `ClientWorldAsset` baked into its own node extras).
    // Replicated (server-origin) entities carry lightyear/replicon's `Remote` marker; scene
    // instantiations don't. (`Remote` lives at `lightyear::prelude::client::Remote` in 0.30.)
    client_world_assets: Query<
        (Entity, &ClientWorldAsset),
        (Without<ClientWorldAssetSpawned>, With<Remote>),
    >,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, client_world_asset) in client_world_assets {
        info!(
            "entity {entity} has client world asset {:?}",
            client_world_asset
        );
        let world_asset: Handle<WorldAsset> = asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(client_world_asset.asset_path.clone()));
        commands
            .entity(entity)
            .insert((WorldAssetRoot(world_asset), ClientWorldAssetSpawned));
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
