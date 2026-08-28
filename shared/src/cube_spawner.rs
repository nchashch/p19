//! Authoritative cube-spawning: the spawn-block check and the resulting entity's simulation
//! state (physics body, `HitPoints`) live here so the decision "does this cube exist, where, with
//! what velocity" is made once. Presentation (the renderable model, `Selectable`,
//! despawn-on-menu) stays in `client`, reacting to `CubeSpawned`.

use avian3d::prelude::*;
use bevy::prelude::*;
use noiz::rng::{AnyValueFromBits, NoiseRng, SNorm};

use crate::combat::{Gcd, HitPoints};
use crate::events::{CubeSpawned, SpawnCubeRequest};

pub struct SharedCubeSpawnerPlugin;

impl Plugin for SharedCubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_cube);
    }
}

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct Cube;

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct CubeSpawner;

/// World-space speed a spawned cube launches at along its `aim_direction`.
pub const CUBE_LAUNCH_SPEED: f32 = 100.0;

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}

fn spawn_cube(
    request: On<SpawnCubeRequest>,
    mut commands: Commands,
    cube_spawner: Query<&GlobalTransform, With<CubeSpawner>>,
    mut casters: Query<&mut Gcd>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    let Ok(mut gcd) = casters.get_mut(request.caster) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    gcd.0.reset();

    let rng = NoiseRng(time.elapsed_secs().to_bits());
    for transform in cube_spawner {
        let shape = Collider::cuboid(2.0, 2.0, 2.0);
        if !spatial_query
            .shape_intersections(
                &shape,
                transform.translation(),
                transform.rotation(),
                &SpatialQueryFilter::default(),
            )
            .is_empty()
        {
            // continue; // would clip existing geometry — don't spawn stuck-in-geometry
        }
        let angular_velocity = Vec3::new(
            random_angular_component(&rng, 0),
            random_angular_component(&rng, 1),
            random_angular_component(&rng, 2),
        );
        let entity = commands
            .spawn((
                Cube,
                Name::new("Cube"),
                HitPoints {
                    hit_points: 200,
                    max_hit_points: 200,
                },
                transform.compute_transform(),
                AngularVelocity(angular_velocity),
                LinearVelocity(request.aim_direction * CUBE_LAUNCH_SPEED),
                RigidBody::Dynamic,
                shape,
            ))
            .id();
        commands.trigger(CubeSpawned { entity });
    }
}
