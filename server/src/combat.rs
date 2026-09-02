use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use shared::{
    client_events::{AttackAttempt, KillAttempt},
    combat::{ATTACK_RANGE, DAMAGE, Dead, Gcd, HitPoints},
    player::Selectable,
    server_events::{Attack, EntityDied, Kill},
};

pub struct ServerCombatPlugin;

impl Plugin for ServerCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(resolve_attack);
        app.add_observer(resolve_kill);
        app.add_systems(Update, (kill_zero_hp, despawn_dead, tick_gcd, tick_dead));
    }
}

// TODO: Handle player death properly, currently it is broken. A player can't move but can still
// look around and attack / kill other entities while dead, before being despawned.
fn kill_zero_hp(
    query: Query<(Entity, &HitPoints, &Transform), Without<Dead>>,
    mut commands: Commands,
) {
    for (entity, hit_points, transform) in query {
        if hit_points.hit_points <= 0 {
            // Sent before the despawn below, not after — client presentation (`client::combat`'s
            // `on_entity_died`) needs this as the unambiguous "actually died in combat" signal,
            // distinct from `HitPoints` merely being removed for some other reason (returning to
            // the main menu despawns every `InGame`-scoped entity at once via `DespawnOnExit`).
            commands.server_trigger(ToClients {
                targets: SendTargets::All,
                message: EntityDied {
                    entity,
                    position: transform.translation,
                },
            });
            commands
                .entity(entity)
                .insert(Dead::default())
                .remove::<Selectable>()
                .remove::<RigidBody>()
                .remove::<Collider>();
        }
    }
}

fn despawn_dead(query: Query<(Entity, &Dead)>, mut commands: Commands) {
    for (entity, dead) in query {
        if dead.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn tick_dead(time: Res<Time>, mut query: Query<&mut Dead>) {
    for mut dead in &mut query {
        dead.0.tick(time.delta());
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
