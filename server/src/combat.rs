use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use lightyear::core::tick::TickDuration;
use lightyear::prelude::*;
use crate::networking::owned_players;
use crate::replay::{RecordedMessage, ReplayRecorder};
use p19_shared::{
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

// Dead players have no agency: they can't move (the `RigidBody` removal below starves ahoy's
// KCC), look around (`p19_server::input::accumulate_look` gates on `Without<Dead>`), or attack
// (`apply_attack`/`apply_kill` only resolve a living caster). The death-path client panic found
// in playtest 0015 is fixed: `--no-render` now adds `SyncWorldPlugin` — see the AGENTS.md
// dead-player entry.
fn kill_zero_hp(
    query: Query<(Entity, &HitPoints, &Transform), Without<Dead>>,
    mut commands: Commands,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) -> Result {
    for (entity, hit_points, transform) in query {
        if hit_points.hit_points <= 0 {
            // Sent before the despawn below, not after — client presentation (`p19_client::gameplay::combat`'s
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

/// The per-message resolution logic, extracted out of [`resolve_kill`] so `p19_server::replay`'s
/// replay driver can call the exact same code path against a recorded [`KillAttempt`] instead
/// of a live [`MessageReceiver`]-drained one.
pub(crate) fn apply_kill(
    killer: Entity,
    attempt: &KillAttempt,
    controlled: &Query<(Entity, &ControlledBy)>,
    dead: &Query<(), With<Dead>>,
    positions: &Query<&Transform>,
    targets: &mut Query<&mut HitPoints>,
    casters: &mut Query<&mut Gcd>,
    sender: &mut ServerMultiMessageSender,
    server: &Server,
) -> Result {
    // Caster resolution + dead-attacker gate — see the matching comment in [`apply_attack`].
    let player = owned_players(killer, *controlled)
        .into_iter()
        .find(|player| casters.contains(*player) && !dead.contains(*player));
    let Some(player) = player else {
        debug!("kill from `{killer}` dropped: no living player character");
        return Ok(());
    };
    let Ok(mut gcd) = casters.get_mut(player) else {
        return Ok(());
    };
    if !gcd.0.is_finished() {
        return Ok(());
    }
    let Ok(killer_transform) = positions.get(player) else {
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
                killer: player,
            },
            server,
            &NetworkTarget::All,
        )?;
    }
    Ok(())
}

fn resolve_kill(
    receivers: Query<(Entity, &mut MessageReceiver<KillAttempt>)>,
    controlled: Query<(Entity, &ControlledBy)>,
    dead: Query<(), With<Dead>>,
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
                &controlled,
                &dead,
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
    controlled: &Query<(Entity, &ControlledBy)>,
    dead: &Query<(), With<Dead>>,
    positions: &Query<&Transform>,
    targets: &mut Query<&mut HitPoints>,
    casters: &mut Query<&mut Gcd>,
    sender: &mut ServerMultiMessageSender,
    server: &Server,
) -> Result {
    // Caster resolution (post-M2): `attacker` is the *connection* entity the `MessageReceiver`
    // lives on, but its `Gcd` and `Transform` live on the separately-spawned player character
    // (`ControlledBy { owner: <connection> }`) — looking them up on the connection itself
    // silently dropped every attack request (see the matching comment in `spawn.rs`).
    // Excluding dead players here *is* the dead-attacker gate: a corpse keeps its `Gcd`, so
    // without this check a dead player could keep attacking.
    let player = owned_players(attacker, *controlled)
        .into_iter()
        .find(|player| casters.contains(*player) && !dead.contains(*player));
    let Some(player) = player else {
        debug!("attack from `{attacker}` dropped: no living player character");
        return Ok(());
    };
    let Ok(mut gcd) = casters.get_mut(player) else {
        return Ok(());
    };
    if !gcd.0.is_finished() {
        return Ok(());
    }
    let Ok(attacker_transform) = positions.get(player) else {
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
                attacker: player,
            },
            server,
            &NetworkTarget::All,
        )?;
    }
    Ok(())
}

fn resolve_attack(
    receivers: Query<(Entity, &mut MessageReceiver<AttackAttempt>)>,
    controlled: Query<(Entity, &ControlledBy)>,
    dead: Query<(), With<Dead>>,
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
                &controlled,
                &dead,
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

