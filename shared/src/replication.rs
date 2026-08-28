//! Minimal replication scaffold: just enough to prove a `client` and `server`
//! binary can exchange server-authoritative state over `bevy_replicon_quinnet`.
//!
//! `DemoPosition` is the only replicated payload for now. Real gameplay state
//! (player position, NPCs, cubes, ...) is not wired into replication yet — see
//! `server::networking`/`client::networking` for what actually drives this.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

pub struct SharedReplicationPlugin;

impl Plugin for SharedReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.replicate::<DemoPosition>();
        app.replicate::<Transform>();
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
