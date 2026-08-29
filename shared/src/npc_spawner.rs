//! Authoritative NPC-spawning: the spawn-block check and the resulting entity's simulation state
//! (physics body, `Character`/`Idle`, `HitPoints`) live here so the decision "does this NPC exist,
//! where" is made once. Presentation (the renderable model, `Selectable`, despawn-on-menu) stays
//! in `client`, reacting to `NpcSpawned`.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::character_controller::{Character, Idle};
use crate::combat::HitPoints;

#[derive(Component, Serialize, Deserialize, Default, Clone)]
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
        LockedAxes::new()
            .lock_rotation_x()
            .lock_rotation_y()
            .lock_rotation_z(),
    )
}
