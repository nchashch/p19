use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;

use crate::character_controller::{
    Character, CharacterController, DesiredMotion, GroundDetection, Idle,
};
use crate::client_events::{
    AttackAttempt, Jump, KillAttempt, LoadLevelRequest, Movement, SpawnCubeRequest, SpawnNpcRequest,
};
use crate::combat::{Gcd, HitPoints};
use crate::cube_spawner::Cube;
use crate::level::LevelRoot;
use crate::npc_spawner::Npc;
use crate::player::{PlayerCharacter, Selectable};
use crate::server_events::{Attack, CubeSpawned, Kill, LoadLevel, NpcSpawned, PlayerSpawned};

pub struct SharedReplicationPlugin;

impl Plugin for SharedReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<LevelRoot>();
        app.replicate::<Transform>();
        app.replicate::<PlayerCharacter>();
        app.replicate::<Character>();
        app.replicate::<CharacterController>();
        app.replicate::<Name>();
        app.replicate::<HitPoints>();
        app.replicate::<Gcd>();
        app.replicate::<GroundDetection>();
        app.replicate::<Collider>();
        app.replicate::<DesiredMotion>();
        app.replicate::<RigidBody>();
        app.replicate::<LinearVelocity>();
        app.replicate::<AngularVelocity>();
        app.replicate::<Selectable>();
        app.replicate::<Npc>();
        app.replicate::<Idle>();
        app.replicate::<Character>();
        app.replicate::<LockedAxes>();
        app.replicate::<LockedAxes>();
        app.replicate::<Cube>();

        // `_mapped_` variants: every event here carries at least one `Entity` field, and those
        // ids are only meaningful once remapped from the sender's world to the receiver's —
        // see the `#[entities]` attributes on each event type.
        app.add_mapped_client_event::<AttackAttempt>(Channel::Ordered);
        app.add_mapped_client_event::<KillAttempt>(Channel::Ordered);

        app.add_client_event::<SpawnCubeRequest>(Channel::Ordered);
        app.add_client_event::<SpawnNpcRequest>(Channel::Ordered);
        app.add_client_event::<LoadLevelRequest>(Channel::Ordered);
        app.add_client_event::<Movement>(Channel::Ordered);
        app.add_client_event::<Jump>(Channel::Ordered);

        app.add_mapped_server_event::<Attack>(Channel::Ordered);
        app.add_mapped_server_event::<Kill>(Channel::Ordered);
        app.add_mapped_server_event::<CubeSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<NpcSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<LoadLevel>(Channel::Ordered);
        app.add_mapped_server_event::<PlayerSpawned>(Channel::Ordered);
    }
}
