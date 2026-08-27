//! Client-side combat: input handling and presentation (sound, particles, `Selected` bookkeeping).
//! The actual range check, damage application, and death detection live in `shared::combat` —
//! this module only reacts to the events that logic fires, it doesn't decide anything itself.

use crate::{
    add_observers_run_if, particles::CubeParticleEffect, player_character::PlayerCharacter,
    targeting::Selected,
};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;
use chill_bevy_console::console_closed;
use shared::combat::{Attack, AttackAttempt, EntityDied, Kill, KillAttempt, SharedCombatPlugin};

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SharedCombatPlugin);
        app.add_systems(Update, tick_lifetimes);
        app.add_observer(on_attack);
        app.add_observer(on_kill);
        app.add_observer(on_entity_died);
        add_observers_run_if!(app, console_closed, attack, kill);
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct AttackAction;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct KillAction;

#[derive(Component)]
struct Lifetime(Timer);

/// Input handling only — decides *who* the player wants to attack, not whether it lands.
/// `shared::combat::resolve_attack` does the range check and applies damage.
fn attack(
    _: On<Start<AttackAction>>,
    selected: Res<Selected>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(attacker) = player.single() else {
        return;
    };
    let Some(entity) = selected.0 else {
        return;
    };
    commands.trigger(AttackAttempt { entity, attacker });
}

/// Input handling only — decides *who* the player wants to kill, not whether it lands.
/// `shared::combat::resolve_kill` does the range check and zeroes `HitPoints`. Structurally
/// identical to `attack`, just targeting `KillAttempt` instead of `AttackAttempt`.
fn kill(
    _: On<Start<KillAction>>,
    selected: Res<Selected>,
    player: Query<Entity, With<PlayerCharacter>>,
    mut commands: Commands,
) {
    let Ok(killer) = player.single() else {
        return;
    };
    let Some(entity) = selected.0 else {
        return;
    };
    commands.trigger(KillAttempt { entity, killer });
}

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
