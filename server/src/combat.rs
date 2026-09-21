use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use lightyear::prelude::*;
use shared::{
    client_events::{AttackAttempt, KillAttempt},
    combat::{Dead, Gcd, HitPoints, ATTACK_RANGE, DAMAGE},
    player::Selectable,
    replication::OrderedReliable,
    server_events::{Attack, EntityDied, Kill},
};

pub struct ServerCombatPlugin;

impl Plugin for ServerCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                kill_zero_hp,
                despawn_dead,
                tick_gcd,
                tick_dead,
                resolve_attack,
                resolve_kill,
            ),
        );
    }
}

// TODO: Handle player death properly, currently it is broken. A player can't move but can still
// look around and attack / kill other entities while dead, before being despawned.
fn kill_zero_hp(
    query: Query<(Entity, &HitPoints, &Transform), Without<Dead>>,
    mut commands: Commands,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for (entity, hit_points, transform) in query {
        if hit_points.hit_points <= 0 {
            // Sent before the despawn below, not after — client presentation (`client::combat`'s
            // `on_entity_died`) needs this as the unambiguous "actually died in combat" signal,
            // distinct from `HitPoints` merely being removed for some other reason (returning to
            // the main menu despawns every `InGame`-scoped entity at once via `DespawnOnExit`).
            sender.send::<EntityDied, OrderedReliable>(
                &EntityDied {
                    entity,
                    position: transform.translation,
                },
                &server,
                &NetworkTarget::All,
            )?;
            commands
                .entity(entity)
                .insert(Dead::default())
                .remove::<Selectable>()
                .remove::<RigidBody>()
                .remove::<Collider>();
        }
    }
    Ok(())
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
    receivers: Query<(Entity, &mut MessageReceiver<KillAttempt>)>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for (killer, mut receiver) in receivers {
        for attempt in receiver.receive() {
            let Ok(mut gcd) = casters.get_mut(killer) else {
                // Continue here skips only the inner loop iteration, which is what we want.
                //
                // Since there could be a situation where there are two attack attempts coming from
                // the same client -- one of them invalid and one valid.
                continue;
            };
            if !gcd.0.is_finished() {
                continue;
            }
            let Ok(killer_transform) = positions.get(killer) else {
                continue;
            };
            let Ok(target_transform) = positions.get(attempt.entity) else {
                continue;
            };
            let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
                continue;
            };
            if killer_transform
                .translation
                .distance(target_transform.translation)
                <= ATTACK_RANGE
            {
                gcd.0.reset();
                hit_points.hit_points = 0;
                sender.send::<Kill, OrderedReliable>(
                    &Kill {
                        entity: attempt.entity,
                        killer,
                    },
                    &server,
                    &NetworkTarget::All,
                )?;
            }
        }
    }
    Ok(())
}

fn resolve_attack(
    receivers: Query<(Entity, &mut MessageReceiver<AttackAttempt>)>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for (attacker, mut receiver) in receivers {
        for attempt in receiver.receive() {
            let Ok(mut gcd) = casters.get_mut(attacker) else {
                // Continue here skips only the inner loop iteration, which is what we want.
                //
                // Since there could be a situation where there are two attack attempts coming from
                // the same client -- one of them invalid and one valid.
                continue;
            };
            if !gcd.0.is_finished() {
                continue;
            }
            let Ok(attacker_transform) = positions.get(attacker) else {
                continue;
            };
            let Ok(target_transform) = positions.get(attempt.entity) else {
                continue;
            };
            let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
                continue;
            };
            if attacker_transform
                .translation
                .distance(target_transform.translation)
                <= ATTACK_RANGE
            {
                gcd.0.reset();
                hit_points.hit_points -= DAMAGE;
                sender.send::<Attack, OrderedReliable>(
                    &Attack {
                        entity: attempt.entity,
                        attacker,
                    },
                    &server,
                    &NetworkTarget::All,
                )?;
            }
        }
    }
    Ok(())
}
