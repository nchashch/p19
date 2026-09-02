//! Client-side combat: input handling and presentation (sound, particles, `Selected` bookkeeping).
//! The actual range check, damage application, and death detection live in `shared::combat` —
//! this module only reacts to the events that logic fires, it doesn't decide anything itself.

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;
use shared::{
    combat::Dead,
    server_events::{Attack, EntityDied, Kill},
};

use crate::{particles::CubeParticleEffect, targeting::Selected};

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
fn on_attack(_: On<Attack>, mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(SamplePlayer::new(asset_server.load("audio/explosion.wav")));
}

/// Reacts to a confirmed kill — sound only, mirroring `on_attack`.
fn on_kill(_: On<Kill>, mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("kill triggered");
    commands.spawn(SamplePlayer::new(asset_server.load("audio/explosion.wav")));
}

/// Reacts to an authoritative death — sound, particle effect, and clearing `Selected` if it
/// pointed at whatever just died. The despawn itself is a separate, server-driven replication
/// event that arrives independently of this one — see `EntityDied`'s doc comment for why this
/// reacts to that explicit signal rather than inferring "died" from `HitPoints` disappearing.
fn on_entity_died(
    died: On<EntityDied>,
    effect: Res<CubeParticleEffect>,
    asset_server: Res<AssetServer>,
    mut selected: ResMut<Selected>,
    mut commands: Commands,
) {
    info!("position: {:?}", died.position);
    commands.spawn(SamplePlayer::new(asset_server.load("audio/crunch.wav")));
    commands.spawn((
        ParticleEffect::new(effect.0.clone()),
        Transform::from_translation(died.position),
        Lifetime(Timer::from_seconds(2.0, TimerMode::Once)),
    ));
    if selected.0 == Some(died.entity) {
        selected.0 = None;
    }
}

fn hide_dead(query: Query<Entity, With<Dead>>, mut commands: Commands) {
    for dead in query {
        commands.entity(dead).insert(Visibility::Hidden);
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
