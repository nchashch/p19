use avian3d::{math::TAU, prelude::*};
use bevy::prelude::*;
use lightyear::prelude::*;
use noiz::{
    prelude::*,
    rng::{AnyValueFromBits, NoiseRng},
};
use shared::{
    client_events::SpawnCubeRequest, cube_spawner::cube, npc_spawner::npc, player::Selectable,
};
use shared::{client_events::SpawnNpcRequest, combat::Gcd};

pub struct ServerSpawnPlugin;

impl Plugin for ServerSpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_npc, spawn_cube));
    }
}

fn spawn_npc(
    receivers: Query<(Entity, &mut MessageReceiver<SpawnNpcRequest>)>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    for (entity, mut receiver) in receivers {
        for request in receiver.receive() {
            let Ok(mut gcd) = casters.get_mut(entity) else {
                continue;
            };
            if !gcd.0.is_finished() {
                continue; // still on global cooldown
            }
            gcd.0.reset();

            let translation = request.transform.translation;
            let shape = Collider::capsule(0.4, 1.0);
            if !spatial_query
                .shape_intersections(
                    &shape,
                    translation,
                    Quat::IDENTITY,
                    &SpatialQueryFilter::default(),
                )
                .is_empty()
            {
                continue; // would clip existing geometry — don't spawn stuck-in-geometry
            }
            const SEED: u32 = 112;
            let rng = NoiseRng(SEED); // seed: u32 — anything, e.g. an entity index
            let bits = rng.rand_u32(time.elapsed_secs().to_bits()); // input: u32, or UVec2/3/4, IVec2/3/4 — a "coordinate"
            let normalized: f32 = UNorm.any_value(bits); // UNorm maps bits -> f32 in (0, 1)
            let random_angle = normalized * TAU; // 0..2π

            let entity = commands
                .spawn((
                    npc(shape, translation),
                    Replicate::to_clients(NetworkTarget::All),
                    Selectable,
                ))
                .id();
        }
    }
}

fn spawn_cube(
    receivers: Query<(Entity, &mut MessageReceiver<SpawnCubeRequest>)>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    for (caster, mut receiver) in receivers {
        for request in receiver.receive() {
            let Ok(mut gcd) = casters.get_mut(caster) else {
                continue;
            };
            if !gcd.0.is_finished() {
                continue; // still on global cooldown
            }
            gcd.0.reset();

            let rng = NoiseRng(time.elapsed_secs().to_bits());
            let shape = Collider::cuboid(2.0, 2.0, 2.0);
            if !spatial_query
                .shape_intersections(
                    &shape,
                    request.transform.translation,
                    request.transform.rotation,
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
                    cube(
                        request.transform,
                        angular_velocity,
                        request.aim_direction,
                        shape,
                    ),
                    Replicate::to_clients(NetworkTarget::All),
                    Selectable,
                ))
                .id();
        }
    }
}

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}
