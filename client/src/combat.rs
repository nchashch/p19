//! Client-side combat: input handling and presentation (sound, particles, `Selected` bookkeeping).
//! The actual range check, damage application, and death detection live in `shared::combat` —
//! this module only reacts to the events that logic fires, it doesn't decide anything itself.

use crate::{particles::CubeParticleEffect, targeting::Selected};
use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;
use shared::combat::SharedCombatPlugin;
use shared::events::{Attack, EntityDied, Kill};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SharedCombatPlugin);
        app.add_systems(Update, tick_lifetimes);
        app.add_observer(on_attack);
        app.add_observer(on_kill);
        app.add_observer(on_entity_died);
    }
}

#[derive(Component)]
struct Lifetime(Timer);

/// Reacts to a confirmed hit — sound only. Animation (attacker's "attack"/"hurt" one-offs) reacts
/// to the same `Attack` event independently in `animation.rs`.
fn on_attack(_: On<Attack>, mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(SamplePlayer::new(asset_server.load("explosion.wav")));
}

/// Reacts to a confirmed kill — sound only, mirroring `on_attack`.
fn on_kill(_: On<Kill>, mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(SamplePlayer::new(asset_server.load("explosion.wav")));
}

/// Reacts to an authoritative death — sound, particle effect, and clearing `Selected` if it
/// pointed at whatever just died. The despawn itself already happened in `shared`.
fn on_entity_died(
    died: On<EntityDied>,
    effect: Res<CubeParticleEffect>,
    asset_server: Res<AssetServer>,
    mut selected: ResMut<Selected>,
    mut commands: Commands,
) {
    commands.spawn(SamplePlayer::new(asset_server.load("crunch.wav")));
    commands.spawn((
        ParticleEffect::new(effect.0.clone()),
        died.transform,
        Lifetime(Timer::from_seconds(2.0, TimerMode::Once)),
    ));
    if selected.0 == Some(died.entity) {
        selected.0 = None;
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
