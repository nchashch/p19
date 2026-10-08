use avian3d::prelude::*;
use bevy::asset::io::AssetSourceId;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::platform::collections::HashSet;
use bevy::{dev_tools::fps_overlay::FpsOverlayConfig, prelude::*};
use chill_bevy_console::{ChillConsole, CommandArgs, ConsoleAppExt, ConsoleCommand, ConsoleConfig};
use futures_lite::StreamExt;
use std::path::Path;

use crate::controls::fps_controller::FpsCamera;
use crate::controls::targeting::Selected;
use crate::events::PlayAnimationLooping;
use crate::events::RespawnPlayer;
use crate::gameplay::player_character::LocalPlayer;
use crate::presentation::animation::Animations;
use crate::ui::hud::HudVisible;
use crate::ui::nameplate::NameplatesVisible;
use bevy::time::Stopwatch;
use bevy_ahoy::input::{AccumulatedInput, Jump as AhoyJump, Movement as AhoyMovement};
use bevy_ahoy::prelude::CharacterController as AhoyCharacterController;
use bevy_ahoy::{CharacterControllerState, CharacterLook};
use bevy_enhanced_input::prelude::{Action, ActionMock, Actions, Bindings, TriggerState};
use p19_shared::cube_spawner::Cube;
use p19_shared::inputs::{Look, PlayerInputContext};
use p19_shared::npc_spawner::Npc;

/// The console's output is deliberately plain English — the one surface not localized through
/// bevy_markup's `data-l10n-id`.
pub struct PConsolePlugin;

impl Plugin for PConsolePlugin {
    fn build(&self, app: &mut App) {
        // The FPS overlay is an on-screen visual whose plugin's `setup` system needs
        // render-side `Assets<ShaderBuffer>` — absent in `--no-render` mode, where the plugin
        // (and the startup toggle that writes its config) is skipped entirely. (The `fps`
        // console command is likewise a no-op-with-error there.)
        if !crate::config::is_no_render_presync() {
            // The system's monospace font (no font files ship with the game).
            app.add_plugins(FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    text_config: TextFont {
                        font: FontSource::Monospace,
                        ..FpsOverlayConfig::default().text_config
                    },
                    ..default()
                },
            });
            app.add_systems(Startup, disable_fps_overlay);
        }
        app.add_plugins((
            ChillConsole {
                // No font file: `font_path: None` uses Bevy's built-in default font (FiraMono,
                // compiled into Bevy; this crate's own `embedded-font` feature is off). The
                // console only takes a path or a handle, not a system family.
                config: ConsoleConfig {
                    font_path: None,
                    ..default()
                },
                ..default()
            },
            PhysicsDebugPlugin::default(),
        ))
        .add_systems(Startup, disable_physics_debug)
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
        ))
        .add_console_command(ConsoleCommand::new(
            "kcc_debug",
            "kcc_debug - dump the local player's ahoy KCC wiring state (M1 migration diagnostics)",
            kcc_debug_cmd,
        ));
    }
}

fn controls_cmd(In(_args): CommandArgs) -> String {
    "[WASD] move\n[RMB] rotate camera\n[LMB] select\n[ESC] deselect\n[E] spawn cube\n[R] spawn NPC\n[T] kill selected\n[F] attack selected\n[Esc] menu\n[Tab] stats"
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

fn load_level_cmd(In(args): CommandArgs, asset_server: Res<AssetServer>) -> String {
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
        return format!("please provide level asset id, available levels:\n{levels_list}");
    };
    if levels.contains(&format!("\"collections/{}\"", id)) {
        // TODO: Load level here.
        todo!();
        format!("loading level \"{id}\"")
    } else {
        format!("no such level, available levels:\n{levels_list}")
    }
}

fn play_animation_cmd(
    In(args): CommandArgs,
    animations: Res<Animations>,
    selected: Res<Selected>,
    mut commands: Commands,
) -> String {
    let Some(selected) = selected.0 else {
        return "nothing selected".to_string();
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

fn nameplates_cmd(
    In(_args): CommandArgs,
    mut nameplates_visible: ResMut<NameplatesVisible>,
) -> String {
    nameplates_visible.0 = !nameplates_visible.0;
    "nameplates toggled".to_string()
}

fn hud_cmd(In(_args): CommandArgs, mut hud_visible: ResMut<HudVisible>) -> String {
    hud_visible.0 = !hud_visible.0;
    "HUD toggled".to_string()
}

/// Dumps every link of the ahoy KCC input→movement chain on the local player, so a
/// "doesn't move at all" report pinpoints which link breaks:
///
/// input binding → BEI action fires (`Action<AhoyMovement>`'s live value) → ahoy's observer
/// writes `AccumulatedInput` → `run_kcc` matches (needs `RigidBody`/`LinearVelocity`/`Position`/
/// `Rotation`/`RigidBodyColliders` — see ahoy's `Ctx` query) → `Position` changes →
/// `position_to_transform` syncs `Transform` (diff printed!) → camera/visuals follow.
///
/// Interpretation hints:
/// - `Action<AhoyMovement>` value stays zero while pressing W → BEI binding/context problem.
/// - `AccumulatedInput.last_movement` is usually `None` in a polled dump even while moving
///   (ahoy clears it after every fixed loop) — judge input delivery by `LinearVelocity`
///   instead.
/// - `LinearVelocity` nonzero / `Position` changing while `Transform` stays put → the
///   `position_to_transform` sync isn't covering this entity.
/// - `CharacterControllerState.grounded: false` while standing → collider/ground-detection
///   problem (ahoy warns if the body has more than one collider).
fn kcc_debug_cmd(
    In(_args): CommandArgs,
    local_player: Res<LocalPlayer>,
    player_q: Query<(
        Entity,
        Has<AhoyCharacterController>,
        Has<PlayerInputContext>,
        Has<RigidBody>,
        Has<RigidBodyColliders>,
        Option<&LinearVelocity>,
        Option<&AccumulatedInput>,
        Option<&CharacterLook>,
        Option<&CharacterControllerState>,
        Option<&Position>,
        Option<&Rotation>,
        Option<&Transform>,
        Option<&GlobalTransform>,
        Option<&Actions<PlayerInputContext>>,
    )>,
    action_kinds: Query<(
        Entity,
        Option<(&Action<AhoyMovement>, &TriggerState, Has<Bindings>)>,
        Option<(&Action<AhoyJump>, &TriggerState, Has<Bindings>)>,
        Option<(&Action<Look>, &TriggerState, &ActionMock)>,
    )>,
    camera: Query<(&GlobalTransform, &FpsCamera)>,
) -> String {
    let Some(player) = local_player.0 else {
        return "no local player (not connected / not in game)".to_string();
    };
    let Ok((
        entity,
        has_kcc,
        has_context,
        has_rigid_body,
        has_colliders,
        linvel,
        accumulated,
        look,
        state,
        position,
        rotation,
        transform,
        global_transform,
        actions,
    )) = player_q.get(player)
    else {
        return format!("local player entity {player} failed the KCC debug query (despawned?)");
    };
    let mut out = format!("kcc debug for local player {entity}:\n");
    out += &format!(
        "  components: kcc={has_kcc} context={has_context} rigid_body={has_rigid_body} \
         linear_velocity={} rigid_body_colliders={has_colliders}\n",
        linvel.is_some(),
    );
    if let Some(velocity) = linvel {
        out += &format!("  LinearVelocity: {:?}\n", velocity.0);
    }
    if let Some(look) = look {
        out += &format!(
            "  CharacterLook: yaw={:.3} pitch={:.3}\n",
            look.yaw, look.pitch
        );
    } else {
        out += "  CharacterLook: MISSING\n";
    }
    if let Some(input) = accumulated {
        out += &format!(
            "  AccumulatedInput: last_movement={:?} jumped={:?}\n",
            input.last_movement,
            input.jumped.as_ref().map(Stopwatch::elapsed),
        );
    } else {
        out += "  AccumulatedInput: MISSING\n";
    }
    if let Some(state) = state {
        out += &format!(
            "  KCC state: grounded={:?} crouching={}\n",
            state.grounded.is_some(),
            state.crouching
        );
    }
    if let (Some(position), Some(rotation)) = (position, rotation) {
        out += &format!("  physics: pos={:?} rot={:?}\n", position.0, rotation.0);
    }
    if let (Some(transform), Some(global_transform)) = (transform, global_transform) {
        out += &format!(
            "  transform: local={:?} global={:?}\n",
            transform.translation,
            global_transform.translation(),
        );
    }
    if let Some(actions) = actions {
        out += &format!(
            "  action entities (PlayerInputContext): {}\n",
            actions.iter().count()
        );
        for action_entity in actions.iter() {
            let Ok((_, movement, jump, look)) = action_kinds.get(action_entity) else {
                continue;
            };
            if let Some((movement, trigger, bound)) = movement {
                out += &format!(
                    "    {action_entity}: AhoyMovement value={:?} trigger={trigger:?} bound={bound}\n",
                    **movement
                );
            } else if let Some((jump, trigger, bound)) = jump {
                out += &format!(
                    "    {action_entity}: AhoyJump value={:?} trigger={trigger:?} bound={bound}\n",
                    **jump
                );
            } else if let Some((look, trigger, mock)) = look {
                out += &format!(
                    "    {action_entity}: Look value={:?} trigger={trigger:?} mocked={}\n",
                    **look, mock.enabled
                );
            } else {
                out += &format!("    {action_entity}: (no ahoy action — unexpected)\n");
            }
        }
    } else {
        out += "  action entities: MISSING (no Actions<PlayerInputContext> on the player)\n";
    }
    if let Ok((camera_transform, camera)) = camera.single() {
        out += &format!(
            "  fps camera: yaw={:.3} pitch={:.3} global translation {:?}\n",
            camera.yaw,
            camera.pitch,
            camera_transform.translation()
        );
    } else {
        out += "  fps camera: NOT FOUND (look feeding is broken)\n";
    }
    info!("{out}");
    out
}

/// Truncates a `Debug` dump so long output stays readable in the console. (Unused while the
/// action dump reads `Action`'s `Deref` value + `TriggerState` directly — kept for future
/// debug commands; clippy flags it until then.)
#[allow(dead_code)]
fn debug_truncated<T: std::fmt::Debug>(value: T) -> String {
    let dump = format!("{value:?}");
    let flat = dump.replace(['\n', '\r'], " ");
    if flat.chars().count() <= 160 {
        flat
    } else {
        let cut: String = flat.chars().take(160).collect();
        format!("{cut}…")
    }
}

fn disable_fps_overlay(mut fps_overlay_config: ResMut<FpsOverlayConfig>) {
    fps_overlay_config.enabled = false;
    fps_overlay_config.frame_time_graph_config.enabled = false;
}

fn disable_physics_debug(mut gizmo_config: ResMut<GizmoConfigStore>) {
    gizmo_config.config_mut::<PhysicsGizmos>().0.enabled = false;
}
