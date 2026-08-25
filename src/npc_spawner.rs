use std::f32::consts::TAU;

use avian3d::prelude::*;
use bevy::prelude::*;
use noiz::{
    prelude::*,
    rng::{AnyValueFromBits, NoiseRng},
};

use crate::{
    cube_spawner::HitPoints,
    game_state::GameState,
    player_character::{Character, Idle},
    targeting::Selectable,
};

pub struct NpcSpawnerPlugin;

impl Plugin for NpcSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_npc);
    }
}

#[derive(Event)]
pub struct SpawnNpc;

#[derive(Component)]
pub struct NpcSpawner;

#[derive(Component)]
pub struct Npc;

pub fn spawn_npc(
    _event: On<SpawnNpc>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    npc_spawner: Query<&GlobalTransform, With<NpcSpawner>>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
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
        let bits = rng.rand_u32((time.elapsed_secs() * 1000_000.0) as u32); // input: u32, or UVec2/3/4, IVec2/3/4 — a "coordinate"
        let normalized: f32 = UNorm.any_value(bits); // UNorm maps bits -> f32 in (0, 1)
        let random_angle = normalized * TAU; // 0..2π
        commands
            .spawn((
                (
                    Npc,
                    Name::new("NPC"),
                    Idle,
                    Character,
                    HitPoints {
                        hit_points: 100,
                        max_hit_points: 100,
                    },
                ),
                InheritedVisibility::default(),
                Transform::from_translation(translation),
                DespawnOnEnter(GameState::MainMenu),
                RigidBody::Dynamic,
                shape,
                LockedAxes::new()
                    .lock_rotation_x()
                    .lock_rotation_y()
                    .lock_rotation_z(),
                Selectable,
            ))
            .with_child((
                WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
                Transform::from_translation(Vec3::new(0.0, -0.9, 0.0))
                    .with_rotation(Quat::from_rotation_y(random_angle)),
            ));
    }
}
