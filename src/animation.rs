use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use std::time::Duration;

pub struct PAnimationPlugin;

impl Plugin for PAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_rig_gltf);
        app.add_systems(Update, build_graph_when_loaded);
        app.add_observer(bind_animation_player);
        app.add_observer(on_play_animation);
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

#[derive(Event)]
pub struct PlayAnimation {
    pub root: Entity,
    pub name: String,
}

fn on_play_animation(
    event: On<PlayAnimation>,
    roots: Query<&AnimationRoot>,
    animations: Res<Animations>,
    mut targets: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Ok(AnimationRoot(entity)) = roots.get(event.root) else {
        return; // rig hasn't finished loading yet
    };
    let Some(&node) = animations.nodes.get(&event.name) else {
        return;
    };
    let Ok((mut player, mut transitions)) = targets.get_mut(*entity) else {
        return;
    };
    transitions
        .play(&mut player, node, Duration::from_millis(250))
        .repeat();
}

enum DragonAction {
    Idle,
    Walk,
    Run,
    Jump,
    Strike,
    Fly,
    Land,
}

enum BirdAction {
    Idle,
    Walk,
    Run,
    Jump,
    Strike,
    Fly,
    Land,
}

enum HumanoidAction {
    Idle,
    Walk,
    Run,
    Jump,
    Strike,
}

enum QuadrupedAction {
    Idle,
    Walk,
    Run,
    Jump,
    Strike,
    Bite,
}

#[derive(Resource)]
struct Rigs {
    humanoid_graph: Handle<AnimationGraph>,
    humanoid_nodes: HashMap<HumanoidAction, AnimationNodeIndex>,

    quadruped_graph: Handle<AnimationGraph>,
    quadruped_nodes: HashMap<QuadrupedAction, AnimationNodeIndex>,
}

fn load_rigs() {
    // load all rigs
    todo!();
}

fn load_meshes() {
    todo!();
}

fn locomotion() {
    todo!();
}

// Armature + Animations
// Mesh variants authored against Armature
