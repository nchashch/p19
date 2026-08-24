use crate::{
    add_observers_run_if,
    cube_spawner::{Cube, HitPoints},
    particles::CubeParticleEffect,
    player_character::PlayerCharacter,
    targeting::Selected,
};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_seedling::prelude::*;
use chill_bevy_console::console_closed;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick_lifetimes, despawn_zero_hp));
        add_observers_run_if!(app, console_closed, attack, despawn_cube);
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Attack;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct DespawnCube;

pub const DAMAGE: i32 = 7;
pub const ATTACK_RANGE: f32 = 10.0;

#[derive(Component)]
struct Lifetime(Timer);

fn attack(
    _: On<Start<Attack>>,
    selected: ResMut<Selected>,
    player: Query<&Transform, With<PlayerCharacter>>,
    mut target: Query<(&mut HitPoints, &Transform)>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Some(entity) = selected.0 else {
        return;
    };
    let Ok((mut target_hit_points, target_transform)) = target.get_mut(entity) else {
        return;
    };
    if player_transform
        .translation
        .distance(target_transform.translation)
        <= ATTACK_RANGE
    {
        target_hit_points.hit_points -= DAMAGE;
    }
}

fn despawn_zero_hp(
    mut selected: ResMut<Selected>,
    query: Query<(Entity, &HitPoints, &Transform)>,
    effect: Res<CubeParticleEffect>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, hit_points, transform) in query {
        if hit_points.hit_points <= 0 {
            kill_entity(
                &mut commands,
                &effect,
                &asset_server,
                &mut selected,
                entity,
                *transform,
            );
        }
    }
}

fn despawn_cube(
    _: On<Start<DespawnCube>>,
    mut selected: ResMut<Selected>,
    cube: Query<(Entity, &Transform), With<Cube>>,
    effect: Res<CubeParticleEffect>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok((entity, transform)) = cube.get(entity) {
            kill_entity(
                &mut commands,
                &effect,
                &asset_server,
                &mut selected,
                entity,
                *transform,
            );
        }
    }
}

/// Plays the destruction sound and particle effect, despawns `entity`, and clears
/// `selected` if it pointed at the entity being destroyed.
fn kill_entity(
    commands: &mut Commands,
    effect: &CubeParticleEffect,
    asset_server: &AssetServer,
    selected: &mut Selected,
    entity: Entity,
    transform: Transform,
) {
    commands.spawn(SamplePlayer::new(asset_server.load("crunch.wav")));
    commands.entity(entity).despawn();
    commands.spawn((
        ParticleEffect::new(effect.0.clone()),
        transform,
        Lifetime(Timer::from_seconds(2.0, TimerMode::Once)),
    ));
    if selected.0 == Some(entity) {
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
