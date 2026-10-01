use crate::networking::owned_players;
use crate::replay::{RecordedMessage, ReplayRecorder};
use avian3d::{math::TAU, prelude::*};
use bevy::prelude::*;
use lightyear::prelude::*;
use noiz::{
    prelude::*,
    rng::{AnyValueFromBits, NoiseRng},
};
use shared::assets::level::ClientWorldAsset;
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
    controlled: &Query<(Entity, &ControlledBy)>,
    casters: &mut Query<&mut Gcd>,
    tick: Tick,
    spatial_query: &SpatialQuery,
    commands: &mut Commands,
) {
    // Caster resolution (post-M2): `caster` is the *connection* entity the `MessageReceiver`
    // lives on, but `Gcd` lives on that connection's separately-spawned player character
    // (`ControlledBy { owner: <connection> }`) — looking it up on the connection itself
    // silently dropped every spawn request (pre-M2 the connection entity *was* the player).
    // Select the entity with the immutable `contains` first and borrow once after the loop:
    // keeping a `Mut<Gcd>` from a loop iteration alive across later `get_mut` calls is an
    // E0499 under stable rustc (local nightlies' borrow-checker improvements accept it).
    let player = owned_players(caster, *controlled)
        .into_iter()
        .find(|player| casters.contains(*player));
    let Some(player) = player else {
        debug!("spawn_npc from `{caster}` dropped: no player character carrying `Gcd`");
        return;
    };
    let Ok(mut gcd) = casters.get_mut(player) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    gcd.0.reset();

    let translation = request.transform.translation;
    const CAPSULE_RADIUS: f32 = 0.4;
    const CAPSULE_LENGTH: f32 = 1.0;
    let shape = Collider::capsule(CAPSULE_RADIUS, CAPSULE_LENGTH);
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
        .with_child((
            ClientWorldAsset {
                asset_path: "rigs/armature/npc.glb".to_string(),
            },
            Transform::from_translation(Vec3::new(
                0.0,
                -(CAPSULE_LENGTH / 2.0 + CAPSULE_RADIUS),
                0.0,
            )),
        ))
        .id();
    let _ = npc_entity;
}

fn spawn_npc(
    receivers: Query<(Entity, &mut MessageReceiver<SpawnNpcRequest>)>,
    mut commands: Commands,
    controlled: Query<(Entity, &ControlledBy)>,
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
                &controlled,
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
    controlled: &Query<(Entity, &ControlledBy)>,
    casters: &mut Query<&mut Gcd>,
    tick: Tick,
    spatial_query: &SpatialQuery,
    commands: &mut Commands,
) {
    // Caster resolution — see the matching comment in [`apply_spawn_npc`].
    let player = owned_players(caster, *controlled)
        .into_iter()
        .find(|player| casters.contains(*player));
    let Some(player) = player else {
        debug!("spawn_cube from `{caster}` dropped: no player character carrying `Gcd`");
        return;
    };
    let Ok(mut gcd) = casters.get_mut(player) else {
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
    controlled: Query<(Entity, &ControlledBy)>,
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
                &controlled,
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
