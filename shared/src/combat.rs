//! Authoritative combat simulation — the part of combat that must produce identical results on
//! client and server: range checks, damage application, and death detection. Presentation
//! (sounds, particles, UI selection state, input bindings) stays in `client`, reacting to the
//! events fired here rather than deciding anything itself.

use bevy::prelude::*;

pub struct SharedCombatPlugin;

impl Plugin for SharedCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(resolve_attack);
        app.add_systems(Update, despawn_zero_hp);
    }
}

#[derive(Component, Clone, Default, Reflect, Debug)]
#[reflect(Component)]
pub struct HitPoints {
    pub hit_points: i32,
    pub max_hit_points: i32,
}

pub const DAMAGE: i32 = 49;
pub const ATTACK_RANGE: f32 = 10.0;

/// A request to attack `entity` (the target), from `attacker` — not yet confirmed to land.
/// Fired by client input today; a future server would fire it from a received network message
/// instead.
#[derive(EntityEvent)]
pub struct AttackAttempt {
    pub entity: Entity,
    pub attacker: Entity,
}

/// Fired once an `AttackAttempt` is confirmed in range and damage has been applied — this is the
/// fact client-side presentation (animation, sound) reacts to, not `AttackAttempt` itself.
#[derive(EntityEvent)]
pub struct Attack {
    pub entity: Entity,
    pub attacker: Entity,
}

/// Fired when an entity's `HitPoints` drop to zero or below, right before it's despawned —
/// carries its last `Transform` since client-side presentation (particles) needs a spawn
/// position after the entity itself is already gone.
#[derive(EntityEvent)]
pub struct EntityDied {
    pub entity: Entity,
    pub transform: Transform,
}

fn resolve_attack(
    attempt: On<AttackAttempt>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut commands: Commands,
) {
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
        hit_points.hit_points -= DAMAGE;
        commands.trigger(Attack {
            entity: attempt.entity,
            attacker: attempt.attacker,
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
