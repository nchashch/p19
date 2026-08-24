use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use bevy::{dev_tools::fps_overlay::FpsOverlayPlugin, platform::collections::HashMap};
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;
use chill_bevy_console::{
    ChillConsole, CommandArgs, ConsoleAppExt, ConsoleCommand, console_closed,
};
use cube_spawner::CubeSpawnerPlugin;
use fps_controller::FpsControllerPlugin;
use game_state::{GameState, GameStatePlugin};
use particles::ParticleEffectsPlugin;
use player_character::PlayerCharacterPlugin;

mod character_controller;
mod cube_spawner;
mod fps_camera;
mod fps_controller;
mod game_state;
mod particles;
mod player_character;
mod ui;

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins,
            ChillConsole::default(),
            SeedlingPlugins,
            ParticleEffectsPlugin,
            SkeinPlugin::default(),
            PhysicsPlugins::default(),
            CubeSpawnerPlugin,
            PlayerCharacterPlugin,
            FpsControllerPlugin,
            GameStatePlugin,
            ui::PrototypeUiPlugin,
            // PhysicsDebugPlugin::default(),
            // FpsOverlayPlugin::default(),
        ))
        .add_console_command(ConsoleCommand::new(
            "say",
            "say <text> - echo text",
            say_cmd,
        ));
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 10.,
            ..default()
        });
        app.register_type::<ColliderConstructor>();
        app.add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
        app.add_systems(
            OnEnter(GameState::Loading),
            (ui::in_game_scene.spawn(), load_level),
        );
        app.add_systems(OnEnter(GameState::InGame), spawn_character);
        app.add_systems(Update, wait_for_level.run_if(in_state(GameState::Loading)));
    }
}

fn say_cmd(In(args): CommandArgs) -> String {
    args.join(" ")
}

#[derive(Resource)]
pub struct LevelScene(Handle<WorldAsset>);

fn wait_for_level(
    asset_server: Res<AssetServer>,
    level: Res<LevelScene>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // is_loaded_with_dependencies is the one you want for scenes/glTF —
    // it waits on the whole dependency graph, not just the root asset.
    if asset_server.is_loaded_with_dependencies(&level.0) {
        next_state.set(GameState::InGame);
    }
}

fn load_level(asset_server: Res<AssetServer>, mut commands: Commands) {
    let handle = asset_server.load(GltfAssetLabel::Scene(0).from_asset("Level.glb#Scene0"));
    commands.insert_resource(LevelScene(handle.clone()));
    commands.spawn((
        WorldAssetRoot(handle),
        DespawnOnExit::<GameState>(GameState::InGame),
    ));
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

#[derive(Component)]
struct AnimationRoot(Entity);

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
