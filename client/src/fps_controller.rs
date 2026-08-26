use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use chill_bevy_console::console_closed;
use std::f32::consts::PI;

use crate::player_character::PlayerModel;

pub struct FpsControllerPlugin;

impl Plugin for FpsControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(apply_fps_camera_rotation.run_if(console_closed));
        app.add_systems(Update, rotate_player_model);
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

fn rotate_player_model(
    fps_camera: Query<&FpsCamera>,
    mut player_model: Query<&mut Transform, With<PlayerModel>>,
) {
    let Ok(fps_camera) = fps_camera.single() else {
        return;
    };
    let Ok(mut player_model) = player_model.single_mut() else {
        return;
    };
    player_model.rotation = Quat::from_rotation_y(fps_camera.yaw);
}

fn apply_fps_camera_rotation(
    rotation: On<Fire<FpsCameraRotation>>,
    mut fps_camera: Query<(&mut FpsCamera, &mut Transform)>,
) {
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

