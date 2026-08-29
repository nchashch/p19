//! Minimal replication scaffold: just enough to prove a `client` and `server`
//! binary can exchange server-authoritative state over `bevy_replicon_quinnet`.
//!
//! `DemoPosition` is the only replicated payload for now. Real gameplay state
//! (player position, NPCs, cubes, ...) is not wired into replication yet — see
//! `server::networking`/`client::networking` for what actually drives this.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::client_events::{
    AttackAttempt, KillAttempt, LoadLevelRequest, SpawnCubeRequest, SpawnNpcRequest,
};
use crate::level::LevelRoot;
use crate::server_events::{Attack, CubeSpawned, EntityDied, Kill, LoadLevel, NpcSpawned};

pub struct SharedReplicationPlugin;

impl Plugin for SharedReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<DemoPosition>();
        app.replicate::<LevelRoot>();
        // `LevelRoot` must stay at `Transform::IDENTITY` (see `shared::level`), but children
        // (`WorldAssetRoot` and everything the GLTF scene spawns under it) read their own
        // `Transform` relative to their parent's — without `Transform` itself replicated here,
        // the client's copy of `LevelRoot` has no `Transform` at all, so Bevy's transform
        // propagation never recognizes it as a valid hierarchy root and nothing under it gets a
        // correct `GlobalTransform` (breaking both rendering and Avian's collider placement).
        app.replicate::<Transform>();

        // `_mapped_` variants: every event here carries at least one `Entity` field, and those
        // ids are only meaningful once remapped from the sender's world to the receiver's —
        // see the `#[entities]` attributes on each event type.
        app.add_mapped_client_event::<AttackAttempt>(Channel::Ordered);
        app.add_mapped_client_event::<KillAttempt>(Channel::Ordered);
        app.add_mapped_client_event::<SpawnCubeRequest>(Channel::Ordered);
        app.add_mapped_client_event::<SpawnNpcRequest>(Channel::Ordered);
        app.add_mapped_client_event::<LoadLevelRequest>(Channel::Ordered);

        app.add_mapped_server_event::<Attack>(Channel::Ordered);
        app.add_mapped_server_event::<Kill>(Channel::Ordered);
        app.add_mapped_server_event::<EntityDied>(Channel::Ordered);
        app.add_mapped_server_event::<CubeSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<NpcSpawned>(Channel::Ordered);
        app.add_mapped_server_event::<LoadLevel>(Channel::Ordered);
    }
}

/// Server-authoritative position for the scaffold's demo entity.
///
/// Plain `f32` fields rather than `Transform`/`Vec3` on purpose: those only implement
/// `Serialize`/`Deserialize` when bevy's `serialize` feature is enabled, which isn't part of
/// this workspace's `bevy` feature set.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Default)]
pub struct DemoPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
