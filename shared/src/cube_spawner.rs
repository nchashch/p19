//! Authoritative cube-spawning: the spawn-block check and the resulting entity's simulation
//! state (physics body, `HitPoints`) live here so the decision "does this cube exist, where, with
//! what velocity" is made once. Presentation (the renderable model, `Selectable`,
//! despawn-on-menu) stays in `client`, reacting to `CubeSpawned`.

use crate::assets::level::ClientWorldAsset;
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::combat::HitPoints;

#[derive(Component, Clone, Default, Reflect, Debug, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Cube;

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct CubeSpawner;

pub fn cube(
    transform: Transform,
    angular_velocity: Vec3,
    aim_direction: Vec3,
    shape: Collider,
) -> impl Bundle {
    (
        Cube,
        ClientWorldAsset {
            asset_path: "rigs/prop/cube.glb".to_string(),
        },
        Name::new("Cube"),
        HitPoints {
            hit_points: 200,
            max_hit_points: 200,
        },
        transform,
        AngularVelocity(angular_velocity),
        LinearVelocity(aim_direction * CUBE_LAUNCH_SPEED),
        RigidBody::Dynamic,
        shape,
    )
}

/// World-space speed a spawned cube launches at along its `aim_direction`.
pub const CUBE_LAUNCH_SPEED: f32 = 100.0;
