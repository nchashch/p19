//! Authoritative cube-spawning: the spawn-block check and the resulting entity's simulation
//! state (physics body, `HitPoints`) live here so the decision "does this cube exist, where, with
//! what velocity" is made once. Presentation (the renderable model, `Selectable`,
//! despawn-on-menu) stays in `client`, reacting to `CubeSpawned`.

use avian3d::prelude::*;
use bevy::prelude::*;
use noiz::rng::{AnyValueFromBits, NoiseRng, SNorm};

use crate::combat::HitPoints;

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

/// A request to spawn a cube at each `CubeSpawner`, launched along `aim_direction` — not yet
/// confirmed (a spawner currently overlapping geometry may reject it). Fired by client input
/// today; a future server would fire it from a received network message instead. `aim_direction`
/// is supplied by the caller since `shared` has no camera/look-direction concept of its own.
#[derive(Event)]
pub struct SpawnCubeRequest {
    pub aim_direction: Vec3,
}

/// Fired once a cube has actually been spawned — the fact client-side presentation (model,
/// selectability, despawn-on-menu) reacts to.
#[derive(EntityEvent)]
pub struct CubeSpawned {
    pub entity: Entity,
}

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}

fn spawn_cube(
    request: On<SpawnCubeRequest>,
    mut commands: Commands,
    cube_spawner: Query<&GlobalTransform, With<CubeSpawner>>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
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
