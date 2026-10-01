//! Authoritative NPC-spawning: the spawn-block check and the resulting entity's simulation state
//! (physics body, `Character`/`Idle`, `HitPoints`) live here so the decision "does this NPC exist,
//! where" is made once. Presentation (the renderable model, `Selectable`, despawn-on-menu) stays
//! in `client`, reacting to `NpcSpawned`.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::character_controller::{Character, GameLayer, Idle};
use crate::combat::HitPoints;

// `Reflect` + `#[reflect(Component)]` (matching `Cube`'s derive set): `bevy_remote`'s BRP
// resolves component names through `AppTypeRegistry`, so without reflection the `game/*` QA
// tooling can't see spawned NPCs at all — `world.query` for `shared::npc_spawner::Npc`
// silently matches nothing even on entities that carry it (confirmed live: the same query
// pattern works for `Cube`, which is reflected).
#[derive(Component, Serialize, Deserialize, Default, Clone, Reflect, Debug)]
#[reflect(Component)]
pub struct Npc;

#[derive(Component)]
pub struct NpcSpawner;

pub fn npc(shape: Collider, translation: Vec3) -> impl Bundle {
    (
        Npc,
        Name::new("NPC"),
        Idle,
        Character,
        HitPoints {
            hit_points: 100,
            max_hit_points: 100,
        },
        Transform::from_translation(translation),
        RigidBody::Dynamic,
        shape,
        CollisionLayers::new(
            GameLayer::Npc,
            LayerMask::ALL & !LayerMask::from(GameLayer::Player),
        ),
        LockedAxes::new()
            .lock_rotation_x()
            .lock_rotation_y()
            .lock_rotation_z(),
    )
}
