use avian3d::prelude::*;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::{dev_tools::fps_overlay::FpsOverlayConfig, prelude::*};
use chill_bevy_console::{ChillConsole, CommandArgs, ConsoleAppExt, ConsoleCommand};

use crate::animation::{Animations, PlayAnimationLooping};
use crate::{player_character::RespawnPlayer, targeting::Selected};
use shared::cube_spawner::Cube;
use shared::npc_spawner::Npc;

pub struct PConsolePlugin;

impl Plugin for PConsolePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ChillConsole::default(),
            PhysicsDebugPlugin::default(),
            FpsOverlayPlugin::default(),
        ))
        .add_systems(Startup, (disable_fps_overlay, disable_physics_debug))
        .add_console_command(ConsoleCommand::new(
            "controls",
            "controls - get control scheme",
            controls_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "respawn",
            "respawn - respawn player",
            respawn_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "despawn_cubes",
            "despawn_cubes - despawn all cubes",
            despawn_cubes_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "despawn_npcs",
            "despawn_npcs - despawn all npcs",
            despawn_npcs_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "play_animation",
            "play_animation <clip_name> - play animation on selected entity",
            play_animation_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "fps",
            "fps - toggle fps overlay",
            fps_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "physics_debug",
            "physics_debug - toggle physics debug gizmos",
            physics_debug_cmd,
        ));
    }
}

fn controls_cmd(In(_args): CommandArgs) -> String {
    r#"
    [WASD] move
    [RMB] rotate camera
    [LMB] select
    [ESC] deselect
    [E] spawn cube
    [R] spawn NPC
    [T] kill selected
    [F] attack selected
    [F1] main menu
    "#
    .to_string()
}

fn respawn_cmd(In(_args): CommandArgs, mut commands: Commands) -> String {
    commands.trigger(RespawnPlayer);
    "respawned".to_string()
}

fn despawn_cubes_cmd(
    In(_args): CommandArgs,
    mut commands: Commands,
    cubes: Query<Entity, With<Cube>>,
    mut selected: ResMut<Selected>,
) -> String {
    selected.0 = None;
    for cube in cubes {
        commands.entity(cube).despawn();
    }
    "cubes despawned".to_string()
}

fn despawn_npcs_cmd(
    In(_args): CommandArgs,
    mut commands: Commands,
    npcs: Query<Entity, With<Npc>>,
    mut selected: ResMut<Selected>,
) -> String {
    selected.0 = None;
    for npc in npcs {
        commands.entity(npc).despawn();
    }
    "npcs despawned".to_string()
}

fn play_animation_cmd(
    In(args): CommandArgs,
    animations: Res<Animations>,
    selected: Res<Selected>,
    mut commands: Commands,
) -> String {
    let Some(selected) = selected.0 else {
        return format!("nothing selected");
    };
    let Some(clip_name) = args.get(0) else {
        let mut clip_names = "".to_string();
        for clip_name in animations.nodes.keys() {
            clip_names += &format!(" {clip_name}\n");
        }
        return format!("available clips:\n{clip_names}");
    };
    if !animations.nodes.contains_key(clip_name) {
        let mut clip_names = "".to_string();
        for clip_name in animations.nodes.keys() {
            clip_names += &format!(" {clip_name}\n");
        }
        return format!("clip {clip_name} doesn't exist, available clips:\n{clip_names}");
    }
    commands.trigger(PlayAnimationLooping {
        entity: selected,
        name: clip_name.to_string(),
    });
    format!("playing clip \"{clip_name}\"")
}

fn fps_cmd(In(_args): CommandArgs, mut fps_overlay_config: ResMut<FpsOverlayConfig>) -> String {
    fps_overlay_config.enabled = !fps_overlay_config.enabled;
    fps_overlay_config.frame_time_graph_config.enabled =
        !fps_overlay_config.frame_time_graph_config.enabled;
    "fps overlay toggled".to_string()
}

fn physics_debug_cmd(In(_args): CommandArgs, mut gizmo_config: ResMut<GizmoConfigStore>) -> String {
    gizmo_config.config_mut::<PhysicsGizmos>().0.enabled =
        !gizmo_config.config_mut::<PhysicsGizmos>().0.enabled;
    "physics debug gizmos toggled".to_string()
}

fn disable_fps_overlay(mut fps_overlay_config: ResMut<FpsOverlayConfig>) {
    fps_overlay_config.enabled = false;
    fps_overlay_config.frame_time_graph_config.enabled = false;
}

fn disable_physics_debug(mut gizmo_config: ResMut<GizmoConfigStore>) {
    gizmo_config.config_mut::<PhysicsGizmos>().0.enabled = false;
}
