use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::assets::level::ClientWorldAsset;
use crate::character_controller::{Character, Idle};
use crate::client_events::{
    AttackAttempt, ClientDespawn, InGameRequest, KillAttempt, LoadLevelRequest, LobbyRequest,
    ObserveRequest, SpawnCubeRequest, SpawnNpcRequest,
};
use crate::combat::{Dead, Gcd, HitPoints};
use crate::cube_spawner::Cube;
use crate::level::{InGameRoot, Levels};
use crate::npc_spawner::Npc;
use crate::player::{PlayerCharacter, Selectable};
use crate::server_events::{Attack, EntityDied, Kill};

use lightyear::prelude::*;

/// The one channel every message in this project sends on — ordered and reliable, mirroring the
/// single `Channel::Ordered` every `bevy_replicon`-era registration used uniformly before this
/// migration. A zero-sized marker type — `Channel` has a blanket impl for every `Send + Sync +
/// 'static` type, so no explicit `impl Channel for OrderedReliable` is needed (or allowed).
/// Registered once below via `AppChannelExt::add_channel`.
pub struct OrderedReliable;

pub struct SharedReplicationPlugin;

impl Plugin for SharedReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.add_channel::<OrderedReliable>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.component::<ClientWorldAsset>().replicate();
        app.component::<InGameRoot>().replicate();
        app.component::<Levels>().replicate();
        app.component::<ClientInGame>().replicate();
        app.component::<ClientInLobby>().replicate();

        app.component::<PlayerCharacter>().replicate();
        app.component::<Character>().replicate();
        app.component::<Name>().replicate();
        app.component::<HitPoints>().replicate();
        app.component::<Gcd>().replicate();
        app.component::<Collider>().replicate();
        // Required for ahoy's KCC to see replicated colliders client-side: ahoy's collision
        // query (`ColliderComponents`) requires `Position`/`Rotation`/`ColliderOf` on collider
        // entities, and none of those exist client-side without a `RigidBody` — `Position`/
        // `Rotation` only replicate *filtered* on `With<RigidBody>` (see lightyear_avian's
        // `register_position_mode_protocol`), and a replicated `Collider` without a body is a
        // loose collider avian never attaches (no `ColliderOf`). With `RigidBody` replicated,
        // level geometry (e.g. `minimal_level.glb`'s `RigidBody::Static` floor) attaches
        // client-side and the KCC collides with it. Also gives remote cubes/NPCs their local
        // `RigidBody::Dynamic`, matching client main.rs's "full Avian simulation runs
        // client-side" intent.
        app.component::<RigidBody>().replicate();
        app.component::<CollisionLayers>().replicate();
        app.component::<Selectable>().replicate();
        app.component::<Npc>().replicate();
        app.component::<Idle>().replicate();
        app.component::<Character>().replicate();
        app.component::<Cube>().replicate();
        app.component::<Dead>().replicate();

        app.register_message::<AttackAttempt>()
            .add_direction(NetworkDirection::ClientToServer)
            .add_map_entities();
        app.register_message::<KillAttempt>()
            .add_direction(NetworkDirection::ClientToServer)
            .add_map_entities();

        app.register_message::<SpawnCubeRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<SpawnNpcRequest>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<LoadLevelRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<InGameRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ObserveRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ClientDespawn>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<LobbyRequest>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<Attack>()
            .add_direction(NetworkDirection::ServerToClient)
            .add_map_entities();
        app.register_message::<Kill>()
            .add_direction(NetworkDirection::ServerToClient)
            .add_map_entities();
        app.register_message::<EntityDied>()
            .add_direction(NetworkDirection::ServerToClient)
            .add_map_entities();
    }
}

#[derive(Component, Serialize, Deserialize)]
pub struct ClientInGame;

#[derive(Component, Serialize, Deserialize)]
pub struct ClientInLobby;
