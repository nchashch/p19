use crate::assets::LevelAssets;
use crate::networking::{PendingLevelId, ServerAddress, connect_to_server};
use crate::{events::RespawnPlayer, game_state::GameState};
use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy_asset_loader::prelude::{DynamicAssetCollections, StandardDynamicAssetCollection};
use bevy_quinnet::client::QuinnetClient;
use bevy_replicon::prelude::{ClientTriggerExt, RepliconChannels};
use shared::client_events::LoadLevelRequest;
use shared::level::LevelRoot;
use std::net::IpAddr;

use crate::events::LoadLevel;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), crate::hud::spawn_in_game_scene);
        app.add_systems(OnEnter(GameState::InGame), initial_respawn);
        app.add_systems(
            Update,
            on_level_assets_loaded.run_if(resource_exists_and_changed::<LevelAssets>),
        );
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
    mut next_state: ResMut<NextState<GameState>>,
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
    next_state.set(GameState::Loading);
}

/// Reacts once `GameState::Loading`'s own `LoadingState` (see `main.rs`) has resolved
/// `LevelAssets.level` against whichever manifest `load_level` just registered — i.e. once we
/// finally know the level's real `.glb#SceneN` path, not just the `.ron` manifest that names it.
/// `resource_exists_and_changed` (rather than e.g. `OnEnter` of some new state) is what detects
/// this: `main.rs` deliberately doesn't give this `LoadingState` a `continue_to_state`, since
/// resolving the manifest is just the first half of "loading a level" — `GameState::Loading`
/// itself shouldn't end until the server round trip and the client's own local scene
/// (`spawn_level`, `on_level_ready`) are done too.
///
/// **Must be `resource_exists_and_changed`, not `resource_added`** — confirmed the hard way (a
/// real bug: a second `Play` press after returning to the main menu silently never sent a
/// `LoadLevelRequest`, freezing the client in `GameState::Loading` forever).
/// `bevy_asset_loader`'s `check_loading_collection` re-`insert_resource`s `LevelAssets` on every
/// reload, but it's never *removed* in between — and `World::insert_resource` on an
/// already-present resource goes through `Column::replace`/`SparseSet::insert`, which only bumps
/// the *changed* tick, not *added* (that's stamped once, by `.initialize()`, only on the true
/// absent-to-present transition — confirmed directly against `bevy_ecs`'s `bundle/info.rs` and
/// `storage/table/column.rs`). So `resource_added::<LevelAssets>` is true exactly once, ever, for
/// the whole app's lifetime — every subsequent reload updates the resource's content just fine,
/// but never fires an observer gated on `resource_added`.
///
/// **And it can't be plain `resource_changed`** either, on its own or chained as a second
/// `.run_if(...)` alongside `resource_exists` — `resource_changed::<T>` takes a hard `Res<T>`
/// param, which fails validation (panics, by default) before `LevelAssets` exists at all the very
/// first time. Chaining `.run_if(resource_exists::<T>).run_if(resource_changed::<T>)` doesn't
/// protect against that either: each `.run_if(...)` is validated independently, so the second
/// condition's hard `Res<T>` param still gets checked (and still panics) regardless of what the
/// first one returned — there's no short-circuiting across separate `run_if` calls the way `&&`
/// would short-circuit. `resource_exists_and_changed::<T>` avoids the whole problem by taking
/// `Option<Res<T>>` internally and doing both checks in one condition.
///
/// Same "already connected vs. need to connect first" branch `load_level` used to do directly —
/// unchanged from before this was split into two steps, just now working off the level's actual
/// resolved id instead of the caller-supplied one.
fn on_level_assets_loaded(
    level_assets: Res<LevelAssets>,
    asset_server: Res<AssetServer>,
    channels: Res<RepliconChannels>,
    server_address: Res<ServerAddress>,
    mut client: ResMut<QuinnetClient>,
    mut pending_level_id: ResMut<PendingLevelId>,
    mut commands: Commands,
) {
    let Some(id) = asset_server
        .get_path(level_assets.level.id())
        .map(AssetPath::into_owned)
    else {
        warn!("on_level_assets_loaded: couldn't resolve LevelAssets.level's asset path");
        return;
    };
    if client.is_connected() {
        commands.client_trigger(LoadLevelRequest { id });
    } else {
        // Same "validate right before committing" shape as `load_level`'s own checks — a bad
        // address fails here, synchronously, rather than staying in `GameState::Loading` with a
        // connection attempt that's never going to succeed.
        let Ok(addr) = server_address.0.trim().parse::<IpAddr>() else {
            warn!(
                "on_level_assets_loaded: {:?} is not a valid IP address",
                server_address.0
            );
            return;
        };
        pending_level_id.0 = Some(id);
        connect_to_server(&channels, &mut client, addr);
    }
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
