use bevy::prelude::*;
use bevy_mod_openxr::openxr_session_running;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_replicon::prelude::ClientTriggerExt;
use bevy_xr_utils::actions::{
    ActionType, ActiveSet, XRUtilsAction, XRUtilsActionSet, XRUtilsActionState,
    XRUtilsActionSystems, XRUtilsBinding,
};
use bevy_xr_utils::tracking_utils::{
    TrackingUtilitiesPlugin, XrTrackedLeftGrip, XrTrackedRightGrip,
};
use shared::server_events::PlayerSpawned;

const SNAP_TURN_ANGLE: f32 = 15f32.to_radians();
const SNAP_TURN_THRESHOLD: f32 = 0.6;
const STICK_DEAD_ZONE_LOWER: f32 = 0.15;
const STICK_DEAD_ZONE_UPPER: f32 = 1.0;
/// Vertical offset from the player capsule's origin (its geometric center, per
/// `shared::player::player`'s `Collider::capsule(0.4, 1.0)`) down to the floor the capsule
/// stands on — mirrors the same `-0.9` used for the visible `rig.glb` model in
/// `player_character.rs`'s `decorate_other_players`. Without this, the OpenXR playspace's floor
/// would sit at the capsule's mid-height, and the HMD's real tracked height above it would put
/// the viewer's eyes a good meter too high.
const RIG_FLOOR_OFFSET: f32 = -0.9;

/// Debug visualization for VR controller poses: a colored cube tracking each hand's grip
/// pose, driven by `bevy_xr_utils`'s `TrackingUtilitiesPlugin` (which owns the actual OpenXR
/// action/binding setup, hardcoded to the touch_controller interaction profile). Also owns VR
/// locomotion: the playspace (`XrTrackingRoot`) is parented under a `VrPlayspaceRig` that itself
/// rides along with the local player's capsule, left-stick drives capsule movement relative to
/// the rig's yaw, and right-stick snap-turns that yaw by `SNAP_TURN_ANGLE` per push.
pub struct VrControllersPlugin;

impl Plugin for VrControllersPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            TrackingUtilitiesPlugin,
            bevy_xr_utils::actions::XRUtilsActionsPlugin,
        ))
        .add_observer(on_player_spawned)
        .add_systems(Startup, spawn_controller_cubes)
        .add_systems(
            Startup,
            create_locomotion_actions.before(XRUtilsActionSystems::CreateEvents),
        )
        .add_systems(
            Update,
            vr_locomotion
                .run_if(openxr_session_running)
                .run_if(chill_bevy_console::console_closed),
        );
    }
}

/// The client-only entity the OpenXR playspace (`XrTrackingRoot`) is reparented under once the
/// local player spawns — a child of the player capsule, not the capsule itself, so snap-turning
/// the playspace's facing doesn't fight with the capsule's own server-authoritative `Transform`.
#[derive(Component)]
pub struct VrPlayspaceRig {
    yaw: f32,
}

#[derive(Component)]
struct LeftStickAction;

#[derive(Component)]
struct RightStickAction;

fn on_player_spawned(
    spawned: On<PlayerSpawned>,
    xr_root: Single<Entity, With<XrTrackingRoot>>,
    mut commands: Commands,
) {
    let rig = commands
        .spawn((
            VrPlayspaceRig { yaw: 0.0 },
            Transform::from_xyz(0.0, RIG_FLOOR_OFFSET, 0.0),
            Visibility::default(),
            ChildOf(spawned.entity),
        ))
        .id();
    commands.entity(*xr_root).insert(ChildOf(rig));
}

fn spawn_controller_cubes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    xr_root: Single<Entity, With<XrTrackingRoot>>,
) {
    let mesh = meshes.add(Cuboid::new(0.08, 0.08, 0.12));
    let left = materials.add(Color::srgb(0.2, 0.4, 1.0));
    let right = materials.add(Color::srgb(1.0, 0.3, 0.2));

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(left),
        Transform::default(),
        Visibility::default(),
        XrTrackedLeftGrip,
        ChildOf(*xr_root),
    ));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(right),
        Transform::default(),
        Visibility::default(),
        XrTrackedRightGrip,
        ChildOf(*xr_root),
    ));
}

/// Creates the two thumbstick actions (via `bevy_xr_utils`'s generic action API) that
/// `vr_locomotion` reads every frame. Ordered before `XRUtilsActionSystems::CreateEvents` so the
/// actions/bindings exist as entities before that system turns them into real OpenXR objects.
fn create_locomotion_actions(mut commands: Commands) {
    let set = commands
        .spawn((
            XRUtilsActionSet {
                name: "locomotion".into(),
                pretty_name: "Locomotion".into(),
                priority: u32::MIN,
            },
            ActiveSet,
        ))
        .id();

    let left_stick = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_stick".into(),
                localized_name: "Left Thumbstick".into(),
                action_type: ActionType::Vector,
            },
            LeftStickAction,
        ))
        .id();
    let left_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/left/input/thumbstick".into(),
        })
        .id();
    commands.entity(left_stick).add_child(left_binding);
    commands.entity(set).add_child(left_stick);

    let right_stick = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_stick".into(),
                localized_name: "Right Thumbstick".into(),
                action_type: ActionType::Vector,
            },
            RightStickAction,
        ))
        .id();
    let right_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/right/input/thumbstick".into(),
        })
        .id();
    commands.entity(right_stick).add_child(right_binding);
    commands.entity(set).add_child(right_stick);
}

/// Below `lower` snaps to zero; above `upper` clamps to length 1; rescaled linearly between —
/// same radial dead-zone shape `controls.rs` applies to the flat-mode gamepad stick via
/// `bevy_enhanced_input`'s `DeadZone`, reimplemented here since raw OpenXR action state doesn't
/// go through that input pipeline at all.
fn radial_dead_zone(value: Vec2, lower: f32, upper: f32) -> Vec2 {
    let len = value.length();
    if len <= lower {
        return Vec2::ZERO;
    }
    let rescaled = ((len - lower) / (upper - lower)).min(1.0);
    value.normalize() * rescaled
}

/// Left stick moves the player capsule relative to the VR rig's yaw (which right-stick
/// snap-turns); right stick snap-turns that yaw by `SNAP_TURN_ANGLE` per push, edge-triggered
/// (`snap_turn_active`) so holding the stick past the threshold doesn't spin continuously.
/// Movement is only re-sent when it changes (`last_sent_direction`) rather than every frame —
/// otherwise, once the stick returns to neutral, this would spam a zero `Movement` every tick and
/// immediately cancel any simultaneous keyboard-driven movement (VR headset + keyboard together
/// is an explicit target input combo for this project).
fn vr_locomotion(
    left_stick: Single<&XRUtilsActionState, With<LeftStickAction>>,
    right_stick: Single<&XRUtilsActionState, With<RightStickAction>>,
    rig: Single<(&mut Transform, &mut VrPlayspaceRig)>,
    mut commands: Commands,
    mut last_sent_direction: Local<Option<Vec3>>,
    mut snap_turn_active: Local<bool>,
) {
    let (mut rig_transform, mut rig) = rig.into_inner();

    if let XRUtilsActionState::Vector(state) = *left_stick {
        let stick = radial_dead_zone(
            Vec2::from(state.current_state),
            STICK_DEAD_ZONE_LOWER,
            STICK_DEAD_ZONE_UPPER,
        );
        let rotated = Rot2::radians(rig.yaw) * stick;
        let direction = Vec3::new(-rotated.x, 0.0, rotated.y);
        if *last_sent_direction != Some(direction) {
            commands.client_trigger(shared::client_events::Movement { direction });
            *last_sent_direction = Some(direction);
        }
    }

    if let XRUtilsActionState::Vector(state) = *right_stick {
        let x = state.current_state[0];
        if x.abs() > SNAP_TURN_THRESHOLD {
            if !*snap_turn_active {
                rig.yaw -= x.signum() * SNAP_TURN_ANGLE;
                rig_transform.rotation = Quat::from_rotation_y(rig.yaw);
            }
            *snap_turn_active = true;
        } else {
            *snap_turn_active = false;
        }
    }
}
