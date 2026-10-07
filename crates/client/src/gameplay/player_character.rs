use crate::{
    assets::collections::CommonAssets,
    controls::camera::{PlayerCameraPlugin, player_camera},
    controls::controls::{self, PlayerControlsPlugin},
    controls::fps_controller::FpsCamera,
    gameplay::combat::CombatPlugin,
};
use bevy::prelude::*;
use bevy_ahoy::CharacterControllerState;
use bevy_ahoy::CharacterLook;
use bevy_ahoy::prelude::CharacterController as AhoyCharacterController;
use lightyear::prelude::{Controlled, Predicted};
use lightyear::prelude::{AppComponentExt, PredictionBuilderExt};
use p19_shared::cube_spawner::CubeSpawner;
use p19_shared::game_state::GameState;
use p19_shared::npc_spawner::NpcSpawner;
use p19_shared::inputs::LookDirection;
use p19_shared::player::PlayerCharacter;

pub struct PlayerCharacterPlugin;

impl Plugin for PlayerCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((CombatPlugin, PlayerControlsPlugin, PlayerCameraPlugin));
        app.init_resource::<ClientPrediction>()
            .register_type::<ClientPrediction>();

        // Rollback for the KCC's decision state (bug_0004): only the four physics components
        // were rollback-registered, so a server correction rewound `Position`/`Velocity` while
        // ahoy's jump-buffer/coyote stopwatches and grounded-hit data kept their
        // post-correction values → ghost jumps / swallowed buffered jumps. `local_rollback`
        // makes lightyear maintain `PredictionHistory<CharacterControllerState>` on predicted
        // entities (snapshot in `FixedPostUpdate`, restore on rewind) without replicating the
        // state to anyone. Must run after `ClientPlugins` (this plugin is added later in
        // `main.rs`'s tuple): `local_rollback` registers rollback metadata only if
        // `PredictionRegistry` already exists, which `PredictionPlugin` creates.
        //
        // `AccumulatedInput` is deliberately NOT registered: it is cleared and re-filled from
        // the replayed input stream every tick, so it re-derives correctly during rollback —
        // snapshotting it would only store post-consumption zeros.
        app.component::<CharacterControllerState>()
            .local_rollback();

        app.add_systems(
            Update,
            (
                on_player_spawned,
                // `--no-render` skips `bevy_mod_outline` (its plugin needs `RenderApp`), and
                // spawning the rig-model child here goes through outline's world serialisation
                // — which panics on the missing outline resources the moment a second player
                // replicates in. There is nothing to decorate render-less anyway.
                decorate_other_players.run_if(not(resource_exists::<
                    crate::controls::camera::NoRenderMode,
                >)),
                face_look_direction,
            ),
        );
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct PlayerModel;

/// Options → Client-side prediction: whether this client predicts its own character. Sent with
/// the next `InGameRequest` (the server attaches `PredictionTarget` only if set), so a change
/// applies from the next Play. On: movement responds immediately and rollback reconciles it
/// with the server. Off: the character moves only when the server's state arrives, interpolated
/// like every remote body (input-to-motion latency ≈ round trip + interpolation delay).
/// Off by default. Reflected, so BRP's `world.insert_resources` can set it too. Not persisted.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Resource)]
pub struct ClientPrediction(pub bool);

#[derive(Component, Reflect, Default)]
pub struct OtherPlayer;

/// Turns other players' models to their replicated `LookDirection` yaw (their own client's look,
/// as the server last accepted it). Pitch isn't shown: the rig has no head/aim bones driven yet.
fn face_look_direction(
    // `Changed<Children>` covers the model being attached after the last look change.
    players: Query<
        (&LookDirection, &Children),
        (
            With<OtherPlayer>,
            Or<(Changed<LookDirection>, Changed<Children>)>,
        ),
    >,
    mut models: Query<&mut Transform, With<PlayerModel>>,
) {
    for (look, children) in &players {
        let mut models = models.iter_many_mut(children);
        while let Some(mut transform) = models.fetch_next() {
            // The rig's front faces +Z (glTF's forward), a look yaw of 0 faces −Z.
            transform.rotation = Quat::from_rotation_y(look.yaw + core::f32::consts::PI);
        }
    }
}

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
/// `p19_server::networking::spawn_player_for_client`). Not derived by querying `With<PlayerCharacter>`
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
///
/// The client-side KCC goes only on a `Predicted` character (prediction on, see
/// [`ClientPrediction`]): without prediction the server's replicated state is the only motion,
/// and a local sim would fight it. `Predicted` lands in the same receive pass as `Controlled`
/// (the same assumption `interpolated_remotes` makes).
fn on_player_spawned(
    spawned: Query<(Entity, Has<Predicted>), (With<PlayerCharacter>, Added<Controlled>)>,
    mut commands: Commands,
    common_assets: Option<Res<CommonAssets>>,
) {
    let Some(common_assets) = common_assets else {
        return;
    };
    for (entity, predicted) in spawned {
        commands.insert_resource(LocalPlayer(Some(entity)));
        info!(predicted, "local player character spawned");
        if predicted {
            commands.entity(entity).insert(
                // Ahoy's KCC predicts the local player; lightyear's rollback reconciles it with
                // the server. The `#[require]`d support components (`AccumulatedInput`,
                // `CustomPositionIntegration`, …) insert automatically.
                AhoyCharacterController::default(),
            );
        }
        commands
            .entity(entity)
            .insert((
                DespawnOnExit(GameState::InGame),
                // Fed from the FPS camera every frame (`controls::update_character_look`) —
                // ahoy derives movement direction from the look yaw.
                CharacterLook::default(),
            ))
            .with_children(|parent| {
                parent.spawn(controls::player_controls());
                parent
                    // `Visibility::default()` on these two plain transform-anchor entities matters
                    // for the same reason `PlayerCharacter`'s `#[require(Visibility)]` does (see
                    // `p19_shared::player`) — without it, the chain from `Player` down to the camera
                    // (which does have `Visibility`, via `Camera3d`) breaks here instead, producing
                    // the same `bevy_app::hierarchy` B0004 warning one link further down.
                    .spawn((Transform::from_xyz(0., 0.5, 0.), Visibility::default()))
                    .with_children(|parent| {
                        parent
                            .spawn((FpsCamera::default(), Transform::IDENTITY, Visibility::default()))
                            .with_children(|parent| {
                                parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), CubeSpawner));
                                parent.spawn((Transform::from_xyz(0.0, 0.0, -4.0), NpcSpawner));
                                parent.spawn(player_camera(&common_assets));
                            });
                    });
            });
    }
}
