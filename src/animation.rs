use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::game_state::GameState;

pub struct PAnimationPlugin;

impl Plugin for PAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_character);
    }
}

const MODEL: &str = "rig.glb";

#[derive(Resource)]
struct Animations {
    graph: Handle<AnimationGraph>,
    nodes: HashMap<String, AnimationNodeIndex>,
}

pub fn spawn_character(mut commands: Commands, asset_server: Res<AssetServer>) {
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

    commands
        .spawn((WorldAssetRoot(gltf.default_scene.clone().unwrap()),))
        .observe(bind_animation_player);

    *done = true;
}

fn bind_animation_player(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    animations: Res<Animations>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
) {
    for child in children.iter_descendants(ready.entity) {
        if players.contains(child) {
            commands.entity(child).insert((
                AnimationGraphHandle(animations.graph.clone()),
                AnimationTransitions::new(),
            ));
            commands.entity(ready.entity).insert(AnimationRoot(child));
        }
    }
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
