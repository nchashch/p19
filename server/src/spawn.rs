use avian3d::{math::TAU, prelude::*};
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use noiz::{
    prelude::*,
    rng::{AnyValueFromBits, NoiseRng},
};
use shared::{
    client_events::SpawnCubeRequest,
    cube_spawner::{CubeSpawner, cube},
    npc_spawner::npc,
    player::Selectable,
    server_events::CubeSpawned,
};
use shared::{
    client_events::SpawnNpcRequest, combat::Gcd, npc_spawner::NpcSpawner, server_events::NpcSpawned,
};

pub struct ServerSpawnPlugin;

impl Plugin for ServerSpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_npc);
        app.add_observer(spawn_cube);
    }
}

fn spawn_npc(
    request: On<FromClient<SpawnNpcRequest>>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    info!("server spawning npc");
    let ClientId::Client(entity) = request.client_id else {
        return;
    };
    info!("npc spawned by {:?}", entity);
    let Ok(mut gcd) = casters.get_mut(entity) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
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
        return; // would clip existing geometry — don't spawn stuck-in-geometry
    }
    const SEED: u32 = 112;
    let rng = NoiseRng(SEED); // seed: u32 — anything, e.g. an entity index
    let bits = rng.rand_u32(time.elapsed_secs().to_bits()); // input: u32, or UVec2/3/4, IVec2/3/4 — a "coordinate"
    let normalized: f32 = UNorm.any_value(bits); // UNorm maps bits -> f32 in (0, 1)
    let random_angle = normalized * TAU; // 0..2π

    let entity = commands
        .spawn((npc(shape, translation), Replicated, Selectable))
        .id();
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: NpcSpawned {
            entity,
            facing_yaw: random_angle,
        },
    });
}

fn spawn_cube(
    request: On<FromClient<SpawnCubeRequest>>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    let ClientId::Client(caster) = request.client_id else {
        return;
    };
    let Ok(mut gcd) = casters.get_mut(caster) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
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
            Replicated,
            Selectable,
        ))
        .id();
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: CubeSpawned { entity },
    });
}

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}
