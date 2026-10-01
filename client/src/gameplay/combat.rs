//! Client-side combat: input handling and presentation (sound, particles, `Selected` bookkeeping).
//! The actual range check, damage application, and death detection live in `shared::combat` —
//! this module only reacts to the events that logic fires, it doesn't decide anything itself.

use bevy::prelude::*;
use bevy_ahoy::prelude::CharacterController as AhoyCharacterController;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;
use shared::{
    combat::Dead,
    server_events::{Attack, EntityDied, Kill},
};

use crate::{
    assets::collections::CommonAssets, controls::targeting::Selected,
    presentation::particles::CubeParticleEffect,
};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_lifetimes);
        app.add_observer(on_attack);
        app.add_observer(on_kill);
        app.add_observer(on_entity_died);
        app.add_systems(Update, hide_dead);
    }
}

#[derive(Component)]
struct Lifetime(Timer);

/// Reacts to a confirmed hit — sound only. Animation (attacker's "attack"/"hurt" one-offs) reacts
/// to the same `Attack` event independently in `animation.rs`.
fn on_attack(_: On<Attack>, mut commands: Commands, common_assets: Res<CommonAssets>) {
    // `None` = playtest-assets/`--no-common-assets` mode omitted the sample — no sound.
    if let Some(explosion) = &common_assets.explosion {
        commands.spawn(SamplePlayer::new(explosion.clone()));
    }
}

/// Reacts to a confirmed kill — sound only, mirroring `on_attack`.
fn on_kill(_: On<Kill>, mut commands: Commands, common_assets: Res<CommonAssets>) {
    // `None` = playtest-assets/`--no-common-assets` mode omitted the sample — no sound.
    if let Some(explosion) = &common_assets.explosion {
        commands.spawn(SamplePlayer::new(explosion.clone()));
    }
}

/// Reacts to an authoritative death — sound, particle effect, and clearing `Selected` if it
/// pointed at whatever just died. The despawn itself is a separate, server-driven replication
/// event that arrives independently of this one — see `EntityDied`'s doc comment for why this
/// reacts to that explicit signal rather than inferring "died" from `HitPoints` disappearing.
fn on_entity_died(
    died: On<EntityDied>,
    effect: Res<CubeParticleEffect>,
    common_assets: Res<CommonAssets>,
    mut selected: ResMut<Selected>,
    mut commands: Commands,
) {
    // `None` = playtest-assets/`--no-common-assets` mode omitted the sample — no sound.
    if let Some(crunch) = &common_assets.crunch {
        commands.spawn(SamplePlayer::new(crunch.clone()));
    }
    commands.spawn((
        ParticleEffect::new(effect.0.clone()),
        Transform::from_translation(died.position),
        Lifetime(Timer::from_seconds(2.0, TimerMode::Once)),
    ));
    if selected.0 == Some(died.entity) {
        selected.0 = None;
    }
}

/// Hides dead entities and stops their owner-side local simulation. Idempotent by the
/// `Visibility != Hidden` guard, not a spam-every-frame re-queue: these entities despawn ~1 s
/// later server-side, and a queued command racing the replicated despawn panics on apply
/// ("Entity despawned" — hit live when a *remote* player's corpse despawned on this client).
/// Once hidden, nothing is queued again for the corpse's remaining life.
fn hide_dead(dead: Query<(Entity, &Visibility), With<Dead>>, mut commands: Commands) {
    for (entity, visibility) in &dead {
        if *visibility == Visibility::Hidden {
            continue;
        }
        commands.entity(entity).insert(Visibility::Hidden);
        // Stop the owner's *local* prediction from moving their own corpse: the server
        // already starves a dead player's KCC (`kill_zero_hp` removes `RigidBody`), but this
        // client's ahoy KCC keeps running on the predicted entity, so held movement inputs
        // would rubber-band the corpse against the frozen server position until despawn.
        // Removing the controller stops the local simulation; the `InputMarker` stream keeps
        // flowing harmlessly (nothing consumes it — the server look accumulator gates on
        // `Without<Dead>` too).
        commands.entity(entity).remove::<AhoyCharacterController>();
    }
}

fn tick_lifetimes(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut lifetime) in &mut query {
        if lifetime.0.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
