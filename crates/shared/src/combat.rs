use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Component, Clone, Default, Reflect, Debug, Serialize, Deserialize)]
#[reflect(Component)]
pub struct HitPoints {
    pub hit_points: i32,
    pub max_hit_points: i32,
}

pub const DAMAGE: i32 = 49;
pub const ATTACK_RANGE: f32 = 10.0;

/// Global cooldown shared by every ability (`Attack`, `Kill`, ...) — using any one of them starts
/// it, and none of them can fire again until it finishes.
#[derive(Component, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct Gcd(pub Timer);

pub const GCD_DURATION: f32 = 0.5;

impl Default for Gcd {
    fn default() -> Self {
        let mut timer = Timer::new(Duration::from_secs_f32(GCD_DURATION), TimerMode::Once);
        timer.finish(); // start ready — a freshly spawned character shouldn't wait out a GCD
        Self(timer)
    }
}

pub const DEAD_DURATION: f32 = 1.0;

#[derive(Component, Serialize, Deserialize)]
pub struct Dead(pub Timer);

impl Default for Dead {
    fn default() -> Self {
        let timer = Timer::new(Duration::from_secs_f32(DEAD_DURATION), TimerMode::Once);
        Self(timer)
    }
}
