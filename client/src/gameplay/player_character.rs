use crate::{
    assets::collections::CommonAssets,
    controls::camera::{PlayerCameraPlugin, player_camera},
    controls::controls::{self, PlayerControlsPlugin},
    controls::fps_controller::FpsCamera,
    gameplay::combat::CombatPlugin,
};
use bevy::prelude::*;
use lightyear::prelude::Controlled;
use shared::cube_spawner::CubeSpawner;
use shared::game_state::GameState;
use shared::npc_spawner::NpcSpawner;
use shared::player::PlayerCharacter;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((CombatPlugin, PlayerControlsPlugin, PlayerCameraPlugin));

        app.add_systems(Update, (on_player_spawned, decorate_other_players));
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
    // Unlike `on_player_spawned` (an observer on a server event that can't fire before a level
    // is actually loaded), this is a plain, unconditional `Update` system — with no `Single`/state
    // gate of its own to lean on, it starts running from app startup, before `GameState::
    // AssetLoading` finishes and inserts `CommonAssets`. Confirmed by testing: a bare `Res<
    // CommonAssets>` here panicked ("Resource does not exist") on every startup, not just a
    // hypothetical race.
    common_assets: Option<Res<CommonAssets>>,
    mut commands: Commands,
) {
    let Some(local_player) = local_player.0 else {
        return;
    };
    let Some(common_assets) = common_assets else {
        return;
    };
    for entity in players {
        if entity == local_player {
            continue;
        }
        commands.entity(entity).insert(OtherPlayer).with_child((
            WorldAssetRoot(common_assets.rig_world.clone()),
            Transform::from_translation(Vec3::new(0.0, -0.9, 0.0)),
            PlayerModel,
        ));
    }
}

/// The player's own `PlayerCharacter` entity, identified via `lightyear::prelude::Controlled` —
/// the client automatically gets this marker on its local copy of whichever entity the server
/// tagged `ControlledBy { owner: <that client's connection entity> }` (see
/// `server::networking::spawn_player_for_client`). Not derived by querying `With<PlayerCharacter>`
/// alone, since once other players are connected there can be several such entities replicated in
/// and nothing about them locally distinguishes "mine" from "someone else's" beyond `Controlled`.
#[derive(Resource, Deref, Clone, Copy)]
pub struct LocalPlayer(pub Option<Entity>);

/// A plain polling `Update` system, not an `On<Add, Controlled>` observer — mirrors
/// `combat.rs`'s `hide_dead` for the same reason (see its doc comment): a client that joins after
/// the level/player characters already exist gets them via replication's initial full-state sync,
/// which doesn't reliably fire per-component `Add` observers the way a live single-component
/// insert during an ongoing session does. `Added<Controlled>` (a query filter, backed by the
/// component's own change-detection tick) doesn't have that problem.
///
/// `common_assets` is `Option<Res<_>>`, not a bare `Res<_>`, for the same reason
/// `decorate_other_players` right below needs it: with no `Single`/state gate of its own, this
/// now starts running from app startup — before `GameState::AssetLoading` finishes and inserts
/// `CommonAssets` — same as that system's own doc comment already explains.
fn on_player_spawned(
    spawned: Query<Entity, (With<PlayerCharacter>, Added<Controlled>)>,
    mut commands: Commands,
    common_assets: Option<Res<CommonAssets>>,
) {
    let Some(common_assets) = common_assets else {
        return;
    };
    for entity in spawned {
        commands.insert_resource(LocalPlayer(Some(entity)));
        commands
            .entity(entity)
            .insert(DespawnOnExit(GameState::InGame))
            .with_children(|parent| {
                parent.spawn(controls::player_controls());
                parent
                    // `Visibility::default()` on these two plain transform-anchor entities matters
                    // for the same reason `PlayerCharacter`'s `#[require(Visibility)]` does (see
                    // `shared::player`) — without it, the chain from `Player` down to the camera
                    // (which does have `Visibility`, via `Camera3d`) breaks here instead, producing
                    // the same `bevy_app::hierarchy` B0004 warning one link further down.
                    .spawn((Transform::from_xyz(0., 0.5, 0.), Visibility::default()))
                    .with_children(|parent| {
                        parent
                            .spawn((FpsCamera::new(), Transform::IDENTITY, Visibility::default()))
                            .with_children(|parent| {
                                parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                                parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), NpcSpawner));
                                parent.spawn(player_camera(&common_assets));
                            });
                    });
            });
    }
}
