use avian3d::prelude::*;
use bevy::asset::io::AssetSourceId;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::platform::collections::HashSet;
use bevy::{dev_tools::fps_overlay::FpsOverlayConfig, prelude::*};
use bevy_fluent::prelude::Localization;
use chill_bevy_console::{ChillConsole, CommandArgs, ConsoleAppExt, ConsoleCommand, ConsoleConfig};
use fluent::FluentArgs;
use futures_lite::StreamExt;
use std::path::Path;

use crate::controls::targeting::Selected;
use crate::events::PlayAnimationLooping;
use crate::events::RespawnPlayer;
use crate::presentation::animation::Animations;
use crate::ui::hud::HudVisible;
use crate::ui::localization::localized;
use crate::ui::nameplate::NameplatesVisible;
use shared::cube_spawner::Cube;
use shared::npc_spawner::Npc;

/// The command name strings passed to `ConsoleCommand::new` (and its `help` usage line, a
/// `&'static str` baked in at `build()` time, before `Localization` even exists as a resource) are
/// deliberately left hardcoded — only what each command prints at runtime (well after localization
/// has had time to load) is localized, via `localized_output` below.
pub struct PConsolePlugin;

impl Plugin for PConsolePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ChillConsole {
                // `font_path` resolves via a normal `asset_server.load(path)` (see
                // `chill_bevy_console`'s `ConsoleAssets::from_world`), same as any other font
                // reference in this codebase — `None` (the default) falls back to the crate's
                // embedded Ubuntu Mono / Bevy's built-in default font instead.
                config: ConsoleConfig {
                    font_path: Some("fonts/mono/IBMPlexMono-Regular.ttf".to_string()),
                    ..default()
                },
                ..default()
            },
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
            "load_level",
            "load_level <id> - load level with asset id string",
            load_level_cmd,
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
        ))
        .add_console_command(ConsoleCommand::new(
            "nameplates",
            "nameplates - toggle nameplate visibility",
            nameplates_cmd,
        ))
        .add_console_command(ConsoleCommand::new(
            "hud",
            "hud - toggle HUD visibility",
            hud_cmd,
        ));
    }
}

/// Falls back to plain English rather than a `.ftl` key when `Localization` genuinely isn't loaded
/// yet — the only realistic way to hit this is opening the console and running a command within
/// the first fraction of a second of startup, before the (tiny) locale folder finishes loading.
fn localized_output(
    localization: &Option<Res<Localization>>,
    key: &'static str,
    args: &FluentArgs,
) -> String {
    match localization {
        Some(localization) => localized(localization, key, args),
        None => "loading localization...".to_string(),
    }
}

fn controls_cmd(In(_args): CommandArgs, localization: Option<Res<Localization>>) -> String {
    localized_output(&localization, "console-controls", &FluentArgs::new())
}

fn respawn_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut commands: Commands,
) -> String {
    commands.trigger(RespawnPlayer);
    localized_output(&localization, "console-respawned", &FluentArgs::new())
}

fn despawn_cubes_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut commands: Commands,
    cubes: Query<Entity, With<Cube>>,
    mut selected: ResMut<Selected>,
) -> String {
    selected.0 = None;
    for cube in cubes {
        commands.entity(cube).despawn();
    }
    localized_output(&localization, "console-cubes-despawned", &FluentArgs::new())
}

fn despawn_npcs_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut commands: Commands,
    npcs: Query<Entity, With<Npc>>,
    mut selected: ResMut<Selected>,
) -> String {
    selected.0 = None;
    for npc in npcs {
        commands.entity(npc).despawn();
    }
    localized_output(&localization, "console-npcs-despawned", &FluentArgs::new())
}

fn load_level_cmd(
    In(args): CommandArgs,
    localization: Option<Res<Localization>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) -> String {
    let mut levels = HashSet::new();
    let source = asset_server.get_source(AssetSourceId::Default).unwrap();
    let mut stream =
        futures_lite::future::block_on(source.reader().read_directory(Path::new("collections")))
            .unwrap();
    while let Some(path) = futures_lite::future::block_on(stream.next()) {
        // Only list the `.ron` dynamic-asset manifests (see `assets::LevelAssets`), not the
        // `.glb` files they point at — those aren't valid ids to pass to `load_level` any more.
        if path.extension().is_some_and(|ext| ext == "ron") {
            levels.insert(format!("{path:?}"));
        }
    }
    let mut levels_list = "".to_string();
    for level in &levels {
        levels_list += &format!("{level}\n");
    }
    let Some(id) = args.get(0) else {
        let mut args = FluentArgs::new();
        args.set("levels", levels_list);
        return localized_output(&localization, "console-load-level-missing-id", &args);
    };
    if levels.contains(&format!("\"collections/{}\"", id)) {
        // TODO: Load level here.
        todo!();
        let mut args = FluentArgs::new();
        args.set("id", id);
        localized_output(&localization, "console-load-level-loading", &args)
    } else {
        let mut args = FluentArgs::new();
        args.set("levels", levels_list);
        localized_output(&localization, "console-load-level-not-found", &args)
    }
}

fn play_animation_cmd(
    In(args): CommandArgs,
    localization: Option<Res<Localization>>,
    animations: Res<Animations>,
    selected: Res<Selected>,
    mut commands: Commands,
) -> String {
    let Some(selected) = selected.0 else {
        return localized_output(
            &localization,
            "console-nothing-selected",
            &FluentArgs::new(),
        );
    };
    let Some(clip_name) = args.get(0) else {
        let mut clip_names = "".to_string();
        for clip_name in animations.nodes.keys() {
            clip_names += &format!(" {clip_name}\n");
        }
        let mut args = FluentArgs::new();
        args.set("clips", clip_names);
        return localized_output(&localization, "console-available-clips", &args);
    };
    if !animations.nodes.contains_key(clip_name) {
        let mut clip_names = "".to_string();
        for clip_name in animations.nodes.keys() {
            clip_names += &format!(" {clip_name}\n");
        }
        let mut args = FluentArgs::new();
        args.set("clip", clip_name);
        args.set("clips", clip_names);
        return localized_output(&localization, "console-clip-not-found", &args);
    }
    commands.trigger(PlayAnimationLooping {
        entity: selected,
        name: clip_name.to_string(),
    });
    let mut args = FluentArgs::new();
    args.set("clip", clip_name);
    localized_output(&localization, "console-playing-clip", &args)
}

fn fps_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut fps_overlay_config: ResMut<FpsOverlayConfig>,
) -> String {
    fps_overlay_config.enabled = !fps_overlay_config.enabled;
    fps_overlay_config.frame_time_graph_config.enabled =
        !fps_overlay_config.frame_time_graph_config.enabled;
    localized_output(&localization, "console-fps-toggled", &FluentArgs::new())
}

fn physics_debug_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) -> String {
    gizmo_config.config_mut::<PhysicsGizmos>().0.enabled =
        !gizmo_config.config_mut::<PhysicsGizmos>().0.enabled;
    localized_output(
        &localization,
        "console-physics-debug-toggled",
        &FluentArgs::new(),
    )
}

fn nameplates_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut nameplates_visible: ResMut<NameplatesVisible>,
) -> String {
    nameplates_visible.0 = !nameplates_visible.0;
    localized_output(
        &localization,
        "console-nameplates-toggled",
        &FluentArgs::new(),
    )
}

fn hud_cmd(
    In(_args): CommandArgs,
    localization: Option<Res<Localization>>,
    mut hud_visible: ResMut<HudVisible>,
) -> String {
    hud_visible.0 = !hud_visible.0;
    localized_output(&localization, "console-hud-toggled", &FluentArgs::new())
}

fn disable_fps_overlay(mut fps_overlay_config: ResMut<FpsOverlayConfig>) {
    fps_overlay_config.enabled = false;
    fps_overlay_config.frame_time_graph_config.enabled = false;
}

fn disable_physics_debug(mut gizmo_config: ResMut<GizmoConfigStore>) {
    gizmo_config.config_mut::<PhysicsGizmos>().0.enabled = false;
}
