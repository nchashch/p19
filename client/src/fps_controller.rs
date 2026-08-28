use bevy::prelude::*;
use std::f32::consts::PI;

use crate::player_character::PlayerModel;

pub struct FpsControllerPlugin;

impl Plugin for FpsControllerPlugin {
    fn build(&self, app: &mut App) {
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
