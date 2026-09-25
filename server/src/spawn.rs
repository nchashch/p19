use avian3d::{math::TAU, prelude::*};
use bevy::prelude::*;
use lightyear::prelude::*;
use crate::replay::{RecordedMessage, ReplayRecorder};
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

/// The per-message resolution logic, extracted out of [`spawn_npc`] so `server::replay`'s
/// replay driver can call the exact same code path against a recorded [`SpawnNpcRequest`]
/// instead of a live [`MessageReceiver`]-drained one.
pub(crate) fn apply_spawn_npc(
    caster: Entity,
    request: &SpawnNpcRequest,
    casters: &mut Query<&mut Gcd>,
    tick: Tick,
    spatial_query: &SpatialQuery,
    commands: &mut Commands,
) {
    let Ok(mut gcd) = casters.get_mut(caster) else {
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
    // Tick + caster index, NOT wall-clock time (`Time::elapsed_secs()` used to be here): a
    // deterministic-replay requirement — re-simulating the identical recorded message at the
    // identical tick must draw the identical "random" angle, which a wall-clock read can never
    // guarantee (see AGENTS.md's replay-determinism notes). XORing in the caster's index keeps
    // two simultaneous spawns within the same tick from drawing the same value.
    let bits = rng.rand_u32(tick.0 ^ caster.to_bits() as u32);
    let normalized: f32 = UNorm.any_value(bits); // UNorm maps bits -> f32 in (0, 1)
    let random_angle = normalized * TAU; // 0..2π
    let _ = random_angle;

    let npc_entity = commands
        .spawn((
            npc(shape, translation),
            Replicate::to_clients(NetworkTarget::All),
            Selectable,
        ))
        .id();
    let _ = npc_entity;
}

fn spawn_npc(
    receivers: Query<(Entity, &mut MessageReceiver<SpawnNpcRequest>)>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    timeline: Res<LocalTimeline>,
    spatial_query: SpatialQuery,
    remote_ids: Query<&RemoteId>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) {
    for (caster, mut receiver) in receivers {
        for request in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    caster,
                    RecordedMessage::SpawnNpc(request.clone()),
                );
            }
            apply_spawn_npc(
                caster,
                &request,
                &mut casters,
                timeline.tick(),
                &spatial_query,
                &mut commands,
            );
        }
    }
}

/// See [`apply_spawn_npc`]'s doc comment — same reasoning, for [`SpawnCubeRequest`].
pub(crate) fn apply_spawn_cube(
    caster: Entity,
    request: &SpawnCubeRequest,
    casters: &mut Query<&mut Gcd>,
    tick: Tick,
    spatial_query: &SpatialQuery,
    commands: &mut Commands,
) {
    let Ok(mut gcd) = casters.get_mut(caster) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    gcd.0.reset();

    // Tick + caster index, not wall-clock time — see the matching comment in
    // `apply_spawn_npc` above; same replay-determinism requirement.
    let rng = NoiseRng(tick.0 ^ caster.to_bits() as u32);
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
    let cube_entity = commands
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
    let _ = cube_entity;
}

fn spawn_cube(
    receivers: Query<(Entity, &mut MessageReceiver<SpawnCubeRequest>)>,
    mut commands: Commands,
    mut casters: Query<&mut Gcd>,
    timeline: Res<LocalTimeline>,
    spatial_query: SpatialQuery,
    remote_ids: Query<&RemoteId>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) {
    for (caster, mut receiver) in receivers {
        for request in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    caster,
                    RecordedMessage::SpawnCube(request.clone()),
                );
            }
            apply_spawn_cube(
                caster,
                &request,
                &mut casters,
                timeline.tick(),
                &spatial_query,
                &mut commands,
            );
        }
    }
}

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}
