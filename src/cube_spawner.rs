use crate::{GameState, fps_controller::FpsCamera, targeting::Selectable};
use avian3d::prelude::*;
use bevy::prelude::*;
use noiz::rng::{AnyValueFromBits, NoiseRng, SNorm};

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_cube);
    }
}

#[derive(Event)]
pub struct SpawnCube;

/// Samples an `SNorm` value (f32 in (-1, 1)) from `rng` for the given `input`, scaled to (-10, 10).
fn random_angular_component(rng: &NoiseRng, input: u32) -> f32 {
    let normalized: f32 = SNorm.any_value(rng.rand_u32(input));
    normalized * 10.0
}

pub fn spawn_cube(
    _event: On<SpawnCube>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cube_spawner: Query<&GlobalTransform, With<CubeSpawner>>,
    fps_camera: Query<&FpsCamera>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    let rng = NoiseRng((time.elapsed_secs() * 1_000_000.0) as u32);
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
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
            continue; // would clip existing geometry — don't spawn stuck-in-geometry
        }
        let angular_velocity = Vec3::new(
            random_angular_component(&rng, 0),
            random_angular_component(&rng, 1),
            random_angular_component(&rng, 2),
        );
        let linear_velocity = Vec3::Z.rotate_x(fps_camera.pitch).rotate_y(fps_camera.yaw) * 100.;
        commands.spawn((
            Cube,
            Name::new("Cube"),
            HitPoints {
                hit_points: 200,
                max_hit_points: 200,
            },
            Selectable,
            transform.compute_transform(),
            AngularVelocity(angular_velocity),
            LinearVelocity(linear_velocity),
            RigidBody::Dynamic,
            shape,
            WorldAssetRoot(asset_server.load("Cube.glb#Scene0")),
            DespawnOnEnter(GameState::MainMenu),
        ));
    }
}

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct Cube;

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct CubeSpawner;

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct HitPoints {
    pub hit_points: i32,
    pub max_hit_points: i32,
}
