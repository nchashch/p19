use crate::{game_state::GameState, player_character::RespawnPlayer};
use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Loading),
            (crate::ui::in_game_scene.spawn(), spawn_level),
        );
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);
        app.add_observer(load_level);
    }
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

#[derive(Event)]
pub struct LoadLevel {
    pub id: String,
}

/// The level asset id requested by the most recent `LoadLevel`, consumed by `spawn_level` once
/// we've actually entered `GameState::Loading`.
#[derive(Resource)]
struct PendingLevel(String);

fn load_level(
    event: On<LoadLevel>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // `event.id` includes the GLTF scene label (e.g. "levels/Level.glb#Scene0") — strip it via
    // `AssetPath` before checking, since the label isn't part of the file on disk. Check the
    // reader directly rather than going through `AssetServer::load`, so a bad id fails
    // synchronously here instead of transitioning into `Loading` with nothing that will ever
    // fire `WorldInstanceReady` to get back out of it.
    let asset_path = AssetPath::parse(&event.id);
    let Ok(source) = asset_server.get_source(asset_path.source()) else {
        warn!("load_level: no asset source for {:?}", event.id);
        return;
    };
    if let Err(err) = futures_lite::future::block_on(source.reader().read(asset_path.path())) {
        warn!("load_level: {:?} not found ({err})", event.id);
        return;
    }

    commands.insert_resource(PendingLevel(event.id.clone()));
    next_state.set(GameState::Loading);
}

/// Spawns the level's `WorldAssetRoot` entity. Deliberately deferred to `OnEnter(GameState::Loading)`
/// rather than done straight from `load_level`: `load_level` typically runs *while still in
/// `GameState::InGame`* (e.g. reloading a level via the console), and `OnExit(GameState::InGame)`'s
/// `DespawnOnExit` sweep doesn't run until the next `StateTransition` pass — after `load_level`
/// returns. Spawning the new entity immediately in `load_level` meant it existed in time to get
/// caught by that same sweep (it's tagged `DespawnOnExit(GameState::InGame)` too, for later reloads),
/// killing it — and its `WorldInstanceReady` observer — before the scene ever finished loading,
/// which left the game stuck in `Loading` forever. Waiting for `OnEnter(Loading)` means the sweep
/// has already completed by the time this runs.
fn spawn_level(
    pending: Res<PendingLevel>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    // let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset("Level.glb#Scene0"));
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset(pending.0.clone()));
    commands
        .spawn((WorldAssetRoot(handle), DespawnOnExit(GameState::InGame)))
        .observe(on_level_ready);
}
