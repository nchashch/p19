use avian3d::math::{Scalar, Vector};
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;
use shared::character_controller::GameLayer;

#[derive(Asset, TypePath, Deserialize)]
pub struct Controller {
    collider: ColliderConstructor,

    acceleration: Scalar,
    damping: Scalar,
    jump_impulse: Scalar,
    gravity: Vector,
    terminal_velocity: Scalar,

    ground_detection_collider: ColliderConstructor,
    ground_detection_max_angle: Scalar,
    ground_detection_max_distance: Scalar,

    game_layer: GameLayer,
}
