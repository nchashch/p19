use crate::{GameState, fps_controller::FpsCamera};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_mod_outline::{AsyncWorldInheritOutline, OutlinePlugin, OutlineVolume};
use rand::distr::{Distribution, Uniform};

pub struct CubeSpawnerPlugin;

impl Plugin for CubeSpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(OutlinePlugin::JUMP_FLOOD);
        app.add_observer(spawn_cube);
        app.add_systems(Update, add_outline_component);
    }
}

#[derive(Event)]
pub struct SpawnCube;

pub fn spawn_cube(
    _event: On<SpawnCube>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cube_spawner: Query<&GlobalTransform, With<CubeSpawner>>,
    fps_camera: Query<&FpsCamera>,
) {
    let between = Uniform::try_from(-10.0..10.0).unwrap();
    let mut rng = rand::rng();
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    for transform in cube_spawner {
        let angular_velocity = Vec3::new(
            between.sample(&mut rng),
            between.sample(&mut rng),
            between.sample(&mut rng),
        );
        let linear_velocity = Vec3::Z.rotate_x(fps_camera.pitch).rotate_y(fps_camera.yaw) * 100.;
        // commands.spawn(SamplePlayer::new(asset_server.load("explosion.wav")));
        commands.spawn((
            Cube,
            HitPoints {
                hit_points: 100,
                max_hit_points: 100,
            },
            Selectable,
            NoOutline,
            transform.compute_transform(),
            AngularVelocity(angular_velocity),
            LinearVelocity(linear_velocity),
            RigidBody::Dynamic,
            Collider::cuboid(2.0, 2.0, 2.0),
            WorldAssetRoot(asset_server.load("Cube.glb#Scene0")),
            DespawnOnEnter(GameState::MainMenu),
        ));

        commands.spawn((
            Npc,
            transform.compute_transform(),
            WorldAssetRoot(asset_server.load("rig.glb#Scene0")),
            DespawnOnEnter(GameState::MainMenu),
        ));
    }
}

#[derive(Component)]
pub struct Npc;

#[derive(Component)]
pub struct Selectable;

#[derive(Component)]
pub struct NoOutline;

fn add_outline_component(mut commands: Commands, query: Query<Entity, With<NoOutline>>) {
    for entity in query {
        commands
            .entity(entity)
            .insert((
                OutlineVolume {
                    visible: false,
                    width: 4.0,
                    colour: Color::srgb(1.0, 1.0, 1.0),
                },
                AsyncWorldInheritOutline::default(),
            ))
            .remove::<NoOutline>();
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
