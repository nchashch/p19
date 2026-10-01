use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use lightyear::core::tick::TickDuration;
use lightyear::prelude::*;
use crate::replay::{RecordedMessage, ReplayRecorder};
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
            (kill_zero_hp, despawn_dead, resolve_attack, resolve_kill),
        );
        // `tick_gcd`/`tick_dead` used to tick by `Res<Time>::delta()` in this same `Update`
        // tuple — real (virtual/wall-clock) elapsed time, subject to OS scheduling jitter, not
        // the fixed simulation step. Two identical replay runs of the same recorded input log
        // would see these timers cross their threshold on different ticks purely from real-time
        // noise. Moved to `FixedUpdate` (runs exactly once per simulation tick, however many
        // times that happens to be per real `App::update()` call — 0, 1, or more under
        // real-time jitter/catch-up, unlike `Update` which always runs exactly once) and ticked
        // by the fixed `TickDuration` instead, so both timers advance in lockstep with
        // `lightyear`'s own tick counter (`LocalTimeline`, incremented once per `FixedUpdate`
        // run) rather than with the real clock. `Update`'s combat-resolution systems above still
        // read the fully-updated `Gcd`/`Dead` state correctly regardless: `FixedUpdate` runs
        // before `Update` in Bevy's main schedule order, so any catch-up ticks for this frame
        // have already applied by the time `resolve_attack`/`resolve_kill` check
        // `gcd.0.is_finished()`.
        app.add_systems(FixedUpdate, (tick_gcd, tick_dead));
    }
}

// TODO: Handle player death properly, currently it is broken. A player can't move (RigidBody
// removal below starves ahoy's KCC) or look around (`server::input::accumulate_look` gates on
// `Without<Dead>`) while dead, but dead *attackers* still need gating in
// `apply_attack`/`apply_kill` — unreachable until the caster-resolution fix below lands, since
// combat currently no-ops for everyone. Also: killing a player currently panics the victim's
// client (lightyear `sync_last_confirmed_checkpoint` needs `Res<ServerMutateTicks>`, which
// doesn't exist client-side — see the AGENTS.md dead-player gap entry; ablation-confirmed
// pre-existing, exposed by this death path becoming exercisable).
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

fn tick_dead(tick_duration: Res<TickDuration>, mut query: Query<&mut Dead>) {
    for mut dead in &mut query {
        dead.0.tick(tick_duration.0);
    }
}

fn tick_gcd(tick_duration: Res<TickDuration>, mut query: Query<&mut Gcd>) {
    for mut gcd in &mut query {
        gcd.0.tick(tick_duration.0);
    }
}

/// The per-message resolution logic, extracted out of [`resolve_kill`] so `server::replay`'s
/// replay driver can call the exact same code path against a recorded [`KillAttempt`] instead
/// of a live [`MessageReceiver`]-drained one.
pub(crate) fn apply_kill(
    killer: Entity,
    attempt: &KillAttempt,
    positions: &Query<&Transform>,
    targets: &mut Query<&mut HitPoints>,
    casters: &mut Query<&mut Gcd>,
    sender: &mut ServerMultiMessageSender,
    server: &Server,
) -> Result {
    let Ok(mut gcd) = casters.get_mut(killer) else {
        // Continue here skips only the inner loop iteration, which is what we want.
        //
        // Since there could be a situation where there are two attack attempts coming from
        // the same client -- one of them invalid and one valid.
        return Ok(());
    };
    if !gcd.0.is_finished() {
        return Ok(());
    }
    let Ok(killer_transform) = positions.get(killer) else {
        return Ok(());
    };
    let Ok(target_transform) = positions.get(attempt.entity) else {
        return Ok(());
    };
    let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
        return Ok(());
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
            server,
            &NetworkTarget::All,
        )?;
    }
    Ok(())
}

fn resolve_kill(
    receivers: Query<(Entity, &mut MessageReceiver<KillAttempt>)>,
    positions: Query<&Transform>,
    mut targets: Query<&mut HitPoints>,
    mut casters: Query<&mut Gcd>,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) -> Result {
    for (killer, mut receiver) in receivers {
        for attempt in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    killer,
                    RecordedMessage::Kill(attempt.clone()),
                );
            }
            apply_kill(
                killer,
                &attempt,
                &positions,
                &mut targets,
                &mut casters,
                &mut sender,
                &server,
            )?;
        }
    }
    Ok(())
}

/// See [`apply_kill`]'s doc comment — same reasoning, for [`AttackAttempt`].
pub(crate) fn apply_attack(
    attacker: Entity,
    attempt: &AttackAttempt,
    positions: &Query<&Transform>,
    targets: &mut Query<&mut HitPoints>,
    casters: &mut Query<&mut Gcd>,
    sender: &mut ServerMultiMessageSender,
    server: &Server,
) -> Result {
    let Ok(mut gcd) = casters.get_mut(attacker) else {
        // Continue here skips only the inner loop iteration, which is what we want.
        //
        // Since there could be a situation where there are two attack attempts coming from
        // the same client -- one of them invalid and one valid.
        return Ok(());
    };
    if !gcd.0.is_finished() {
        return Ok(());
    }
    let Ok(attacker_transform) = positions.get(attacker) else {
        return Ok(());
    };
    let Ok(target_transform) = positions.get(attempt.entity) else {
        return Ok(());
    };
    let Ok(mut hit_points) = targets.get_mut(attempt.entity) else {
        return Ok(());
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
            server,
            &NetworkTarget::All,
        )?;
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
    remote_ids: Query<&RemoteId>,
    timeline: Res<LocalTimeline>,
    mut recorder: Option<ResMut<ReplayRecorder>>,
) -> Result {
    for (attacker, mut receiver) in receivers {
        for attempt in receiver.receive() {
            if let Some(recorder) = recorder.as_deref_mut() {
                recorder.record_message(
                    timeline.tick(),
                    &remote_ids,
                    attacker,
                    RecordedMessage::Attack(attempt.clone()),
                );
            }
            apply_attack(
                attacker,
                &attempt,
                &positions,
                &mut targets,
                &mut casters,
                &mut sender,
                &server,
            )?;
        }
    }
    Ok(())
}

