use avian3d::prelude::LinearVelocity;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use std::time::Duration;

use crate::player_character::{Character, Idle};
use shared::character_controller::Grounded;
use shared::combat::Attack;

pub struct PAnimationPlugin;

impl Plugin for PAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_rig_gltf);
        app.add_systems(
            Update,
            (build_graph_when_loaded, animation_finished, locomotion),
        );
        app.add_observer(bind_animation_player);
        app.add_observer(on_play_animation_looping);
        app.add_observer(on_play_animation_once);
        app.add_observer(on_attack);
        app.add_observer(on_animation_finished);
    }
}

const MODEL: &str = "rig.glb";
// const ANIMATION_NAME: &str = "metarigAction";
const ANIMATION_NAME: &str = "idle";

#[derive(Resource)]
pub struct Animations {
    pub graph: Handle<AnimationGraph>,
    pub nodes: HashMap<String, AnimationNodeIndex>,
}

fn load_rig_gltf(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ModelHandle(asset_server.load(MODEL)));
}

#[derive(Resource)]
struct ModelHandle(Handle<Gltf>);

#[derive(Component)]
struct AnimationRoot(Entity);

fn build_graph_when_loaded(
    mut commands: Commands,
    model: Res<ModelHandle>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(gltf) = gltfs.get(&model.0) else {
        return;
    };

    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let nodes = gltf
        .named_animations
        .iter()
        .map(|(name, clip)| (name.to_string(), graph.add_clip(clip.clone(), 1.0, root)))
        .collect();

    commands.insert_resource(Animations {
        graph: graphs.add(graph),
        nodes,
    });

    *done = true;
}

fn bind_animation_player(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    animations: Res<Animations>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for child in children.iter_descendants(ready.entity) {
        let Ok(mut player) = players.get_mut(child) else {
            continue;
        };
        let node = animations.nodes.get(ANIMATION_NAME).copied().or_else(|| {
            warn!(
                "animation {ANIMATION_NAME:?} not found; available: {:?}",
                animations.nodes.keys().collect::<Vec<_>>()
            );
            animations.nodes.values().next().copied()
        });
        let Some(node) = node else { continue };

        let mut transitions = AnimationTransitions::new();
        transitions.play(&mut player, node, Duration::ZERO).repeat();
        commands
            .entity(child)
            .insert((AnimationGraphHandle(animations.graph.clone()), transitions));
        commands.entity(ready.entity).insert(AnimationRoot(child));
    }
}

#[derive(EntityEvent)]
pub struct PlayAnimationLooping {
    pub entity: Entity,
    pub name: String,
}

#[derive(EntityEvent)]
pub struct PlayAnimationOnce {
    pub entity: Entity,
    pub name: String,
}

fn on_play_animation_looping(
    event: On<PlayAnimationLooping>,
    children: Query<&Children>,
    roots: Query<&AnimationRoot>,
    animations: Res<Animations>,
    mut targets: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for root in std::iter::once(event.entity).chain(children.iter_descendants(event.entity)) {
        let Some(&node) = animations.nodes.get(&event.name) else {
            return;
        };
        let Ok(AnimationRoot(entity)) = roots.get(root) else {
            continue; // rig hasn't finished loading yet
        };
        let Ok((mut player, mut transitions)) = targets.get_mut(*entity) else {
            return;
        };
        if transitions.get_main_animation() == Some(node) {
            continue; // already playing this one — don't restart it
        }
        transitions
            .play(&mut player, node, Duration::from_millis(250))
            .repeat();
    }
}

fn on_play_animation_once(
    event: On<PlayAnimationOnce>,
    children: Query<&Children>,
    roots: Query<&AnimationRoot>,
    animations: Res<Animations>,
    mut targets: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for root in std::iter::once(event.entity).chain(children.iter_descendants(event.entity)) {
        let Some(&node) = animations.nodes.get(&event.name) else {
            return;
        };
        let Ok(AnimationRoot(entity)) = roots.get(root) else {
            continue; // rig hasn't finished loading yet
        };
        let Ok((mut player, mut transitions)) = targets.get_mut(*entity) else {
            return;
        };
        transitions.play(&mut player, node, Duration::from_millis(250));
    }
}

#[derive(EntityEvent)]
struct AnimationFinished {
    entity: Entity,
}

/// Fires `AnimationFinished` when a one-off (non-looping) animation completes, with `entity` set
/// to the topmost ancestor of the `AnimationPlayer` — e.g. the `PlayerCharacter` entity, not the
/// `PlayerModel` child the player's own AnimationPlayer actually lives on.
///
/// `is_finished()` is what distinguishes a one-off from a looping animation here: `PlayAnimationOnce`
/// leaves the default `RepeatAnimation::Never`, so it can finish; `PlayAnimationLooping` calls
/// `.repeat()` (`RepeatAnimation::Forever`), which never finishes, so this never fires for it.
/// `just_completed()` on top of that makes it fire exactly once, on the tick it actually finishes.
fn animation_finished(
    players: Query<(Entity, &AnimationPlayer)>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    for (player_entity, player) in &players {
        for (_, active) in player.playing_animations() {
            if active.is_finished() && active.just_completed() {
                let root = parents.root_ancestor::<ChildOf>(player_entity);
                commands.trigger(AnimationFinished { entity: root });
            }
        }
    }
}

fn locomotion(
    characters: Query<(Entity, &LinearVelocity), With<Character>>,
    grounded: Query<(), With<Grounded>>,
    idle: Query<(), With<Idle>>,
    mut commands: Commands,
) {
    for (entity, v) in characters {
        let grounded = grounded.contains(entity);
        let idle = idle.contains(entity);
        let moving = v.xz().length() > 0.4;
        if idle {
            if grounded {
                if moving {
                    commands.trigger(PlayAnimationLooping {
                        entity,
                        name: "walk".to_string(),
                    });
                } else {
                    commands.trigger(PlayAnimationLooping {
                        entity,
                        name: "idle".to_string(),
                    });
                }
            } else {
                commands.trigger(PlayAnimationLooping {
                    entity,
                    name: "jump".to_string(),
                });
            }
        }
    }
}

fn on_attack(attack: On<Attack>, mut commands: Commands) {
    commands.entity(attack.entity).remove::<Idle>();
    commands.trigger(PlayAnimationOnce {
        entity: attack.attacker,
        name: "attack".into(),
    });
    commands.trigger(PlayAnimationOnce {
        entity: attack.entity,
        name: "hurt".into(),
    });
}

fn on_animation_finished(finished: On<AnimationFinished>, mut commands: Commands) {
    commands.entity(finished.entity).insert(Idle);
}
