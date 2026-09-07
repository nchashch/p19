use crate::events::RespawnPlayer;
use crate::lifecycle::assets::LevelAssets;
use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy_asset_loader::prelude::{DynamicAssetCollections, StandardDynamicAssetCollection};
use bevy_hanabi::ParticleEffect;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_quinnet::client::QuinnetClient;
use bevy_seedling::sample::SamplePlayer;
use shared::game_state::GameState;
use shared::level::LevelRoot;
use shared::server_events::{LoadLevel, UnloadLevel};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            crate::ui::hud::spawn_in_game_scene,
        );
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);
        app.add_systems(
            Update,
            on_level_assets_loaded.run_if(resource_exists_and_changed::<LevelAssets>),
        );
        app.add_observer(unload_level);
        app.add_observer(load_level);
    }
}

fn on_level_assets_loaded() {
    todo!();
}

/// Fires once the level's `WorldAssetRoot` entity (spawned in `load_level`) has actually finished
/// instantiating — not just once its asset bytes are loaded, which `AssetServer::is_loaded_with_dependencies`
/// would tell you but can be true a moment before the entities themselves exist.
fn on_level_ready(
    _ready: On<WorldInstanceReady>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    next_state.set(GameState::InGame);
    commands.trigger(RespawnPlayer);
}

pub fn initial_respawn(mut commands: Commands) {
    commands.trigger(RespawnPlayer);
}

/// `event.id` is a `.ron` dynamic-asset manifest (e.g. `"levels/Level.assets.ron"`), not a `.glb`
/// path directly — see `assets::LevelAssets`'s doc comment for the two-key-resolution-phases
/// mechanism this relies on. Checks the manifest file itself exists synchronously (same
/// "validate right before committing" shape the old direct-`.glb` version used, just against a
/// different file) before registering it as `GameState::Loading`'s dynamic asset file and
/// transitioning — a bad id fails here instead of entering `Loading` with a `LevelAssets` that
/// will never resolve. The actual `LoadLevelRequest` — which still needs a concrete `.glb#SceneN`
/// path, since that's what the server (and `spawn_level`, below) load — isn't sent here: it has
/// to wait for `LevelAssets.level` to actually finish resolving against this manifest, which
/// `on_level_assets_loaded` reacts to once `GameState::Loading`'s own `LoadingState` finishes.
fn load_level(
    event: On<LoadLevel>,
    asset_server: Res<AssetServer>,
    mut dynamic_assets: ResMut<DynamicAssetCollections<GameState>>,
    mut commands: Commands,
) {
    let asset_path = AssetPath::parse(&event.id);
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        warn!("load_level: no asset source for {:?}", event.id);
        return;
    };
    if let Err(err) = futures_lite::future::block_on(source.reader().read(asset_path.path())) {
        warn!("load_level: {:?} not found ({err})", event.id);
        return;
    }
    dynamic_assets.register_file::<StandardDynamicAssetCollection>(GameState::Loading, &event.id);
    commands.trigger(UnloadLevel {
        next_state: GameState::Loading,
    });
}

fn unload_level(
    event: On<UnloadLevel>,
    mut commands: Commands,
    mut client: ResMut<QuinnetClient>,
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
    client.close_all_connections();
    commands.set_state(event.next_state.clone());
}

fn spawn_level(
    level_root: Single<(Entity, &LevelRoot)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let entity = todo!();
    let handle = todo!();
    commands
        .entity(entity)
        .insert(DespawnOnExit(GameState::InGame))
        .with_children(|parent| {
            parent.spawn(WorldAssetRoot(handle)).observe(on_level_ready);
        });
}
