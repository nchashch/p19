//! Authoritative NPC-spawning: the spawn-block check and the resulting entity's simulation state
//! (physics body, `Character`/`Idle`, `HitPoints`) live here so the decision "does this NPC exist,
//! where" is made once. Presentation (the renderable model, `Selectable`, despawn-on-menu) stays
//! in `client`, reacting to `NpcSpawned`.

use std::f32::consts::TAU;

use avian3d::prelude::*;
use bevy::prelude::*;
use noiz::{
    prelude::*,
    rng::{AnyValueFromBits, NoiseRng},
};

use crate::character_controller::{Character, Idle};
use crate::client_events::SpawnNpcRequest;
use crate::combat::{Gcd, HitPoints};
use crate::server_events::NpcSpawned;

pub struct SharedNpcSpawnerPlugin;

impl Plugin for SharedNpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_npc);
    }
}

#[derive(Component)]
pub struct Npc;

#[derive(Component)]
pub struct NpcSpawner;

fn spawn_npc(
    request: On<SpawnNpcRequest>,
    mut commands: Commands,
    npc_spawner: Query<&GlobalTransform, With<NpcSpawner>>,
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

    for transform in npc_spawner {
        let translation = transform.translation();
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
            ))
            .id();
        commands.trigger(NpcSpawned {
            entity,
            facing_yaw: random_angle,
        });
    }
}
