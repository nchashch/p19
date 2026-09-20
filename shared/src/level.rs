use bevy::{asset::AssetPath, prelude::*};
use serde::{Deserialize, Serialize};

use crate::assets::level::Level;

/// Required components matter here specifically because of replication: the client's mirror of
/// this entity only ever gets whatever's explicitly `.replicate::<T>()`-registered (`LevelRoot`
/// and `Transform` today) — `Visibility`/`InheritedVisibility`/`ViewVisibility` are never
/// replicated (they're meant to be computed locally from `Visibility`, the same way
/// `GlobalTransform` is computed locally from `Transform`), so without requiring them here the
/// client's copy of this entity ends up with none of them at all. Its children (the loaded level
/// scene) *do* have `InheritedVisibility` from their own required components, which produced a
/// real `bevy_app::hierarchy` B0004 warning — a parent missing `InheritedVisibility` while a
/// child has it — and left the whole subtree's visibility propagation inconsistent.
#[derive(Component, Default, Serialize, Deserialize)]
#[require(Transform, Visibility)]
pub struct LevelRoot;

#[derive(Component, Default, Serialize, Deserialize)]
pub struct Levels {
    levels: Vec<(AssetPath<'static>, Level)>,
}

impl Levels {
    pub fn new(levels: Vec<(AssetPath<'static>, Level)>) -> Self {
        Self { levels }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(AssetPath<'static>, Level)> {
        self.levels.iter()
    }
}
