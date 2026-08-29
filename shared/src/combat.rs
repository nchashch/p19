//! Authoritative combat simulation — the part of combat that must produce identical results on
//! client and server: range checks, damage application, and death detection. Presentation
//! (sounds, particles, UI selection state, input bindings) stays in `client`, reacting to the
//! events fired here rather than deciding anything itself.

use std::time::Duration;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::client_events::{AttackAttempt, KillAttempt};
use crate::server_events::{Attack, EntityDied, Kill};

pub struct SharedCombatPlugin;

impl Plugin for SharedCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(resolve_attack);
        app.add_observer(resolve_kill);
        app.add_systems(Update, (despawn_zero_hp, tick_gcd));
    }
}

#[derive(Component, Clone, Default, Reflect, Debug, Serialize, Deserialize)]
#[reflect(Component)]
pub struct HitPoints {
    pub hit_points: i32,
    pub max_hit_points: i32,
}

pub const DAMAGE: i32 = 49;
pub const ATTACK_RANGE: f32 = 10.0;

fn resolve_attack(
    attempt: On<AttackAttempt>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut commands: Commands,
) {
    let Ok(mut gcd) = casters.get_mut(attempt.attacker) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    let Ok(attacker_transform) = positions.get(attempt.attacker) else {
        return;
    };
    let Ok(target_transform) = positions.get(attempt.entity) else {
        return;
    };
    let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
        return;
    };
    if attacker_transform
        .translation
        .distance(target_transform.translation)
        <= ATTACK_RANGE
    {
        gcd.0.reset();
        hit_points.hit_points -= DAMAGE;
        commands.trigger(Attack {
            entity: attempt.entity,
            attacker: attempt.attacker,
        });
    }
}

fn resolve_kill(
    attempt: On<KillAttempt>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut commands: Commands,
) {
    let Ok(mut gcd) = casters.get_mut(attempt.killer) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    let Ok(killer_transform) = positions.get(attempt.killer) else {
        return;
    };
    let Ok(target_transform) = positions.get(attempt.entity) else {
        return;
    };
    let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
        return;
    };
    if killer_transform
        .translation
        .distance(target_transform.translation)
        <= ATTACK_RANGE
    {
        gcd.0.reset();
        hit_points.hit_points = 0;
        commands.trigger(Kill {
            entity: attempt.entity,
            killer: attempt.killer,
        });
    }
}

fn despawn_zero_hp(query: Query<(Entity, &HitPoints, &Transform)>, mut commands: Commands) {
    for (entity, hit_points, transform) in query {
        if hit_points.hit_points <= 0 {
            commands.trigger(EntityDied {
                entity,
                transform: *transform,
            });
            commands.entity(entity).despawn();
        }
    }
}

/// Global cooldown shared by every ability (`Attack`, `Kill`, ...) — using any one of them starts
/// it, and none of them can fire again until it finishes.
#[derive(Component, Serialize, Deserialize)]
pub struct Gcd(pub Timer);

pub const GCD_DURATION: f32 = 0.5;

impl Default for Gcd {
    fn default() -> Self {
        let mut timer = Timer::new(Duration::from_secs_f32(GCD_DURATION), TimerMode::Once);
        timer.finish(); // start ready — a freshly spawned character shouldn't wait out a GCD
        Self(timer)
    }
}

fn tick_gcd(time: Res<Time>, mut query: Query<&mut Gcd>) {
    for mut gcd in &mut query {
        gcd.0.tick(time.delta());
    }
}
