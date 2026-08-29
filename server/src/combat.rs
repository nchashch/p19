use bevy::prelude::*;
use bevy_replicon::prelude::*;
use shared::{
    client_events::{AttackAttempt, KillAttempt},
    combat::{ATTACK_RANGE, DAMAGE, Gcd, HitPoints},
    server_events::{Attack, Kill},
};

pub struct ServerCombatPlugin;

impl Plugin for ServerCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(resolve_attack);
        app.add_observer(resolve_kill);
        app.add_systems(Update, (despawn_zero_hp, tick_gcd));
    }
}

fn despawn_zero_hp(query: Query<(Entity, &HitPoints, &Transform)>, mut commands: Commands) {
    for (entity, hit_points, _transform) in query {
        if hit_points.hit_points <= 0 {
            commands.entity(entity).despawn();
        }
    }
}

fn tick_gcd(time: Res<Time>, mut query: Query<&mut Gcd>) {
    for mut gcd in &mut query {
        gcd.0.tick(time.delta());
    }
}

fn resolve_kill(
    attempt: On<FromClient<KillAttempt>>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut commands: Commands,
) {
    let ClientId::Client(killer) = attempt.client_id else {
        return;
    };
    let Ok(mut gcd) = casters.get_mut(killer) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    let Ok(killer_transform) = positions.get(killer) else {
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
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: Kill { killer },
        });
    }
}

fn resolve_attack(
    attempt: On<FromClient<AttackAttempt>>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut commands: Commands,
) {
    let ClientId::Client(attacker) = attempt.client_id else {
        return;
    };
    let Ok(mut gcd) = casters.get_mut(attacker) else {
        return;
    };
    if !gcd.0.is_finished() {
        return; // still on global cooldown
    }
    let Ok(attacker_transform) = positions.get(attacker) else {
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
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: Attack {
                entity: attempt.entity,
                attacker,
            },
        });
    }
}
