use avian3d::prelude::*;
use bevy::prelude::*;

use crate::character_controller::{
    Character, CharacterController, DesiredMotion, GroundDetection, Grounded, Idle,
};
use crate::client_events::{
    AttackAttempt, Jump, KillAttempt, LoadLevelRequest, Movement, SpawnCubeRequest, SpawnNpcRequest,
};
use crate::combat::{Dead, Gcd, HitPoints};
use crate::cube_spawner::Cube;
use crate::level::LevelRoot;
use crate::npc_spawner::Npc;
use crate::player::{PlayerCharacter, Selectable};
use crate::server_events::{Attack, EntityDied, Kill, LoadLevel, ServerInGame, UnloadLevel};

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
        });

        app.component::<LevelRoot>().replicate();
        app.component::<PlayerCharacter>().replicate();
        app.component::<Character>().replicate();
        app.component::<CharacterController>().replicate();
        app.component::<Name>().replicate();
        app.component::<HitPoints>().replicate();
        app.component::<Gcd>().replicate();
        app.component::<GroundDetection>().replicate();
        app.component::<Grounded>().replicate();
        app.component::<Collider>().replicate();
        app.component::<CollisionLayers>().replicate();
        app.component::<DesiredMotion>().replicate();
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

        app.register_message::<Movement>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<Jump>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<LoadLevelRequest>()
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

        app.register_message::<UnloadLevel>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<LoadLevel>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<ServerInGame>()
            .add_direction(NetworkDirection::ServerToClient);
    }
}
