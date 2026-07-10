use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use std::f32::consts::PI;

pub struct ThirdPersonControllerPlugin;

impl Plugin for ThirdPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(apply_third_person_camera_rotation);
    }
}

#[derive(Component)]
pub struct ThirdPersonCamera {
    pub direction: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub sensitivity: f32,
    pub zoom: f32,
}

impl ThirdPersonCamera {
    pub fn new() -> Self {
        Self {
            sensitivity: 0.001,
            pitch: 0.0,
            yaw: -PI,
            direction: Vec3::Z,
            zoom: 10.0,
        }
    }
}

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct ThirdPersonCameraRotation;

fn apply_third_person_camera_rotation(
    rotation: On<Fire<ThirdPersonCameraRotation>>,
    mut third_person_camera: Query<(&mut ThirdPersonCamera, &mut Transform), With<Camera3d>>,
) {
    if let Ok((mut third_person_camera, mut camera_transform)) = third_person_camera.single_mut() {
        let delta_pitch = rotation.value.y * third_person_camera.sensitivity;
        let delta_yaw = -rotation.value.x * third_person_camera.sensitivity;
        third_person_camera.pitch =
            (third_person_camera.pitch + delta_pitch).clamp(-PI / 2. + 0.0001, PI / 2. - 0.0001);
        third_person_camera.yaw = third_person_camera.yaw + delta_yaw;
        let d = Vec3::Z.rotate_x(third_person_camera.pitch);
        let d = d.rotate_y(third_person_camera.yaw);
        let d = d.normalize_or_zero();
        third_person_camera.direction = d;
        camera_transform.look_at(third_person_camera.direction, Vec3::Y);
    }
}
