use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use chill_bevy_console::console_closed;
use std::f32::consts::PI;

use crate::game_state::GameState;

pub struct FpsControllerPlugin;

impl Plugin for FpsControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_crosshair);
        app.insert_resource(DisableFpsCameraControl(false));
        app.add_observer(apply_fps_camera_rotation.run_if(console_closed));
    }
}

#[derive(Component)]
pub struct FpsCamera {
    pub direction: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub sensitivity: f32,
}

impl FpsCamera {
    pub fn new() -> Self {
        Self {
            sensitivity: 0.001,
            pitch: 0.0,
            yaw: -PI,
            direction: Vec3::Z,
        }
    }
}

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct FpsCameraRotation;

fn apply_fps_camera_rotation(
    rotation: On<Fire<FpsCameraRotation>>,
    mut fps_camera: Query<(&mut FpsCamera, &mut Transform)>,
    disable_fps_camera_control: Res<DisableFpsCameraControl>,
) {
    if disable_fps_camera_control.0 {
        return;
    }
    if let Ok((mut fps_camera, mut camera_transform)) = fps_camera.single_mut() {
        let delta_pitch = rotation.value.y * fps_camera.sensitivity;
        let delta_yaw = -rotation.value.x * fps_camera.sensitivity;
        fps_camera.pitch =
            (fps_camera.pitch + delta_pitch).clamp(-PI / 2. + 0.0001, PI / 2. - 0.0001);
        fps_camera.yaw = fps_camera.yaw + delta_yaw;
        let d = Vec3::Z.rotate_x(fps_camera.pitch);
        let d = d.rotate_y(fps_camera.yaw);
        let d = d.normalize_or_zero();
        fps_camera.direction = d;
        camera_transform.look_at(fps_camera.direction, Vec3::Y);
    }
}

#[derive(Resource)]
pub struct DisableFpsCameraControl(pub bool);

#[derive(Component)]
pub struct Crosshair;

fn spawn_crosshair(_commands: Commands) {}
