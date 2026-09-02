use crate::networking::{PendingLevelId, connect_to_server};
use crate::{events::RespawnPlayer, game_state::GameState};
use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy_quinnet::client::QuinnetClient;
use bevy_replicon::prelude::{ClientTriggerExt, RepliconChannels};
use shared::client_events::LoadLevelRequest;
use shared::level::LevelRoot;

use crate::events::LoadLevel;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            crate::hud::in_game_scene.spawn(),
        );
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);
        app.add_observer(load_level);
        app.add_observer(spawn_level);
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

fn load_level(
    event: On<LoadLevel>,
    asset_server: Res<AssetServer>,
    channels: Res<RepliconChannels>,
    mut client: ResMut<QuinnetClient>,
    mut pending_level_id: ResMut<PendingLevelId>,
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
    let id = asset_path.into_owned();
    // The client only ever connects here, on `Play` — not eagerly at `Startup` — so the
    // connection may still need to be opened, in which case the actual `LoadLevelRequest` has to
    // wait for `ClientState::Connected` (see `networking.rs`'s `on_connected`) rather than being
    // sent immediately. If already connected (e.g. a second `Play` press), send it right away —
    // waiting on `OnEnter(ClientState::Connected)` again would hang forever, since that
    // transition already happened and won't refire.
    if client.is_connected() {
        commands.client_trigger(LoadLevelRequest { id });
    } else {
        pending_level_id.0 = Some(id);
        connect_to_server(&channels, &mut client);
    }
    next_state.set(GameState::Loading);
}

fn spawn_level(
    _add: On<Add, LevelRoot>,
    level_root: Single<(Entity, &LevelRoot)>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let (entity, level_root) = *level_root;
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset(&level_root.id));
    commands
        .entity(entity)
        .insert(DespawnOnExit(GameState::InGame))
        .with_children(|parent| {
            parent.spawn(WorldAssetRoot(handle)).observe(on_level_ready);
        });
}
