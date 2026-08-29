use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use bevy_replicon::prelude::*;

use crate::character_controller::{Character, CharacterController, DesiredMotion, GroundDetection};
use crate::client_events::{
    AttackAttempt, Jump, KillAttempt, LoadLevelRequest, Movement, SpawnCubeRequest, SpawnNpcRequest,
};
use crate::combat::{Gcd, HitPoints};
use crate::level::LevelRoot;
use crate::player::PlayerCharacter;
use crate::server_events::{Attack, CubeSpawned, EntityDied, Kill, LoadLevel, NpcSpawned};

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

        // `_mapped_` variants: every event here carries at least one `Entity` field, and those
        // ids are only meaningful once remapped from the sender's world to the receiver's —
        // see the `#[entities]` attributes on each event type.
        app.add_mapped_client_event::<AttackAttempt>(Channel::Ordered);
        app.add_mapped_client_event::<KillAttempt>(Channel::Ordered);
        app.add_mapped_client_event::<SpawnCubeRequest>(Channel::Ordered);
        app.add_mapped_client_event::<SpawnNpcRequest>(Channel::Ordered);
        app.add_mapped_client_event::<LoadLevelRequest>(Channel::Ordered);
        app.add_mapped_client_event::<Movement>(Channel::Ordered);
        app.add_mapped_client_event::<Jump>(Channel::Ordered);

        app.add_mapped_server_event::<Attack>(Channel::Ordered);
        app.add_mapped_server_event::<Kill>(Channel::Ordered);
        app.add_mapped_server_event::<EntityDied>(Channel::Ordered);
        app.add_mapped_server_event::<CubeSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<NpcSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<LoadLevel>(Channel::Ordered);
    }
}
