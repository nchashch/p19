use avian3d::prelude::*;
use bevy::color::palettes::css::{AQUA, WHITE, YELLOW};
use bevy::prelude::*;
use bevy_mod_openxr::openxr_session_running;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_replicon::prelude::ClientTriggerExt;
use bevy_xr_utils::actions::{
    ActionType, ActiveSet, XRUtilsAction, XRUtilsActionSet, XRUtilsActionState,
    XRUtilsActionSystems, XRUtilsBinding,
};
use bevy_xr_utils::tracking_utils::{
    TrackingUtilitiesPlugin, XrTrackedLeftGrip, XrTrackedRightGrip, XrTrackedView,
};
use shared::player::Selectable;
use shared::server_events::PlayerSpawned;

use crate::game_state::ModalMenuState;
use crate::player_character::LocalPlayer;
use crate::targeting::{Hovered, SELECT_RANGE, Selected};

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
        .add_systems(Startup, (spawn_controller_cubes, spawn_head_tracker))
        .add_systems(
            Startup,
            (
                create_locomotion_actions.before(XRUtilsActionSystems::CreateEvents),
                create_select_actions.before(XRUtilsActionSystems::CreateEvents),
                create_recenter_action.before(XRUtilsActionSystems::CreateEvents),
            ),
        )
        .add_systems(
            Update,
            (
                vr_locomotion
                    .run_if(openxr_session_running)
                    .run_if(chill_bevy_console::console_closed),
                update_vr_pointers
                    .run_if(openxr_session_running)
                    .run_if(chill_bevy_console::console_closed)
                    .run_if(in_state(ModalMenuState::Closed)),
                recenter_playspace
                    .run_if(openxr_session_running)
                    .run_if(chill_bevy_console::console_closed),
                draw_vr_rig_gizmos.run_if(openxr_session_running),
            ),
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

/// Deselect — left thumbstick *click*, not the vector `LeftStickAction` already read for movement
/// above. Mirrors `controls.rs`'s desktop gamepad mapping (`Deselect` on
/// `GamepadButton::LeftThumb`).
#[derive(Component)]
struct LeftStickClickAction;

/// Select — both triggers, one per hand, "so it works the way VR apps usually work" (either
/// controller can interact with whatever it's pointing at, not just the dominant one). Also read
/// by `npc_ui_quad`'s VR pointer driver for the same physical press to double as a UI click when
/// aimed at an NPC's nameplate quad instead of a world `Selectable` — see
/// `create_select_actions`'s doc comment for why triggers (not a stick click, like deselect) use a
/// `Float` action here.
#[derive(Component)]
pub(crate) struct LeftTriggerAction;

#[derive(Component)]
pub(crate) struct RightTriggerAction;

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

/// Marks the laser child spawned under `XrTrackedLeftGrip`/`XrTrackedRightGrip` — see
/// `update_vr_pointers`, which resizes it every frame to reach whatever it's pointing at.
#[derive(Component)]
struct VrLaserLeft;

#[derive(Component)]
struct VrLaserRight;

/// Width of the laser beam, in meters.
const VR_LASER_WIDTH: f32 = 0.004;
/// How far a controller's laser draws when it doesn't hit anything — independent of
/// `targeting::SELECT_RANGE`, which only gates whether a hit can actually be *selected*, the same
/// distinction `targeting::raycast_from_center`/`controls::select` already draw for the desktop
/// crosshair (the ray/hover has no range limit, only the click does). Purely a visual fallback —
/// see `controller_ray_hit`'s doc comment for why the raycast itself must *not* also be capped to
/// this length.
const VR_LASER_MAX_LENGTH: f32 = 10.0;

fn spawn_controller_cubes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    xr_root: Single<Entity, With<XrTrackingRoot>>,
) {
    let mesh = meshes.add(Cuboid::new(0.08, 0.08, 0.12));
    let left = materials.add(Color::srgb(0.2, 0.4, 1.0));
    let right = materials.add(Color::srgb(1.0, 0.3, 0.2));

    // A unit cuboid, resized per-frame (`update_vr_pointers`) into a thin beam reaching whatever
    // it's pointing at — cheaper than rebuilding the mesh itself every frame, just a `Transform`.
    let laser_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let laser_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.15, 0.15),
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(left),
            Transform::default(),
            Visibility::default(),
            XrTrackedLeftGrip,
            ChildOf(*xr_root),
        ))
        .with_child((
            VrLaserLeft,
            Mesh3d(laser_mesh.clone()),
            MeshMaterial3d(laser_material.clone()),
            Transform::default(),
            Visibility::default(),
        ));
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(right),
            Transform::default(),
            Visibility::default(),
            XrTrackedRightGrip,
            ChildOf(*xr_root),
        ))
        .with_child((
            VrLaserRight,
            Mesh3d(laser_mesh),
            MeshMaterial3d(laser_material),
            Transform::default(),
            Visibility::default(),
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

/// Creates the three actions `update_vr_pointers` (and `npc_ui_quad`'s VR pointer driver) read for
/// select/deselect — deliberately different input types per hand *purpose*, matching
/// `controls.rs`'s desktop gamepad mapping as closely as OpenXR's touch_controller profile allows:
/// deselect is a `Bool` action on left `.../input/thumbstick/click` (a real bool subpath — no
/// thresholding needed, `XRUtilsActionState::Bool`'s `changed_since_last_sync` gives a real press
/// edge directly); select is a `Float` action on `.../input/trigger/value` for *both* hands (the
/// profile has no `.../input/trigger/click` bool subpath, only the analog value and a capacitive
/// `.../input/trigger/touch`, so `update_vr_pointers` thresholds it into a press edge itself) — see
/// `LeftTriggerAction`'s doc comment for why select is symmetric across both hands while deselect
/// isn't. Same ordering requirement as `create_locomotion_actions` — before
/// `XRUtilsActionSystems::CreateEvents`.
fn create_select_actions(mut commands: Commands) {
    let set = commands
        .spawn((
            XRUtilsActionSet {
                name: "select".into(),
                pretty_name: "Select".into(),
                priority: u32::MIN,
            },
            ActiveSet,
        ))
        .id();

    let left_stick_click = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_stick_click".into(),
                localized_name: "Left Thumbstick Click".into(),
                action_type: ActionType::Bool,
            },
            LeftStickClickAction,
        ))
        .id();
    let left_stick_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/left/input/thumbstick/click".into(),
        })
        .id();
    commands
        .entity(left_stick_click)
        .add_child(left_stick_binding);
    commands.entity(set).add_child(left_stick_click);

    let left_trigger = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_trigger".into(),
                localized_name: "Left Trigger".into(),
                action_type: ActionType::Float,
            },
            LeftTriggerAction,
        ))
        .id();
    let left_trigger_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/left/input/trigger/value".into(),
        })
        .id();
    commands
        .entity(left_trigger)
        .add_child(left_trigger_binding);
    commands.entity(set).add_child(left_trigger);

    let right_trigger = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_trigger".into(),
                localized_name: "Right Trigger".into(),
                action_type: ActionType::Float,
            },
            RightTriggerAction,
        ))
        .id();
    let right_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/right/input/trigger/value".into(),
        })
        .id();
    commands.entity(right_trigger).add_child(right_binding);
    commands.entity(set).add_child(right_trigger);
}

/// Grip button on the left controller — for now, only bound to `recenter_playspace`.
#[derive(Component)]
struct LeftGripAction;

/// Creates the left grip action `recenter_playspace` reads. `Float`, not `Bool`, for the same
/// reason `RightTriggerAction` is (see `create_select_actions`'s doc comment) — squeeze has no
/// discrete click subpath on the touch_controller profile either, only `.../input/squeeze/value`.
fn create_recenter_action(mut commands: Commands) {
    let set = commands
        .spawn((
            XRUtilsActionSet {
                name: "recenter".into(),
                pretty_name: "Recenter".into(),
                priority: u32::MIN,
            },
            ActiveSet,
        ))
        .id();

    let left_grip = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_grip".into(),
                localized_name: "Left Grip".into(),
                action_type: ActionType::Float,
            },
            LeftGripAction,
        ))
        .id();
    let left_binding = commands
        .spawn(XRUtilsBinding {
            profile: "/interaction_profiles/oculus/touch_controller".into(),
            binding: "/user/hand/left/input/squeeze/value".into(),
        })
        .id();
    commands.entity(left_grip).add_child(left_binding);
    commands.entity(set).add_child(left_grip);
}

/// Spawns the entity `bevy_xr_utils`'s `TrackingUtilitiesPlugin` writes the HMD's tracked pose
/// into every frame (`update_view`, internal to that crate) — nothing here needs it to render, so
/// unlike `spawn_controller_cubes`'s cubes it's a bare, unparented `Transform` used purely as a
/// value to read in `recenter_playspace`. Without spawning *something* with `XrTrackedView`, that
/// internal system has no entity to write to and the head pose is never tracked anywhere at all.
fn spawn_head_tracker(mut commands: Commands) {
    commands.spawn((Transform::default(), XrTrackedView));
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
///
/// The snap-turn pivots around the player's current head position (`head`, compensating
/// `rig_transform.translation` alongside its rotation), not around `VrPlayspaceRig`'s own fixed
/// origin. Rotating the rig in place (translation untouched) is geometrically centered on the
/// capsule — the rig's origin *is* the capsule's own position, see `on_player_spawned` — but
/// nobody stands exactly at their tracked playspace's calibrated center at all times, and the
/// *visible* effect of a turn is governed by how far the player's real head currently is from
/// whatever point the rig rotates around, not by that point's relationship to the capsule. Left
/// uncompensated, every snap-turn swings the view through an arc sized by that real-world offset,
/// which reads as pivoting around some other, arbitrary point rather than turning in place.
fn vr_locomotion(
    left_stick: Single<&XRUtilsActionState, With<LeftStickAction>>,
    right_stick: Single<&XRUtilsActionState, With<RightStickAction>>,
    head: Single<&Transform, With<XrTrackedView>>,
    rig: Single<(&mut Transform, &mut VrPlayspaceRig), Without<XrTrackedView>>,
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
                let old_rotation = rig_transform.rotation;
                rig.yaw -= x.signum() * SNAP_TURN_ANGLE;
                let new_rotation = Quat::from_rotation_y(rig.yaw);
                // Keep the head's capsule-local (and so world) position fixed across the turn —
                // only the facing direction changes. Same trick `recenter_playspace` uses to solve
                // for a rig translation, just preserving the current position instead of snapping
                // to a canonical one.
                rig_transform.translation +=
                    old_rotation * head.translation - new_rotation * head.translation;
                rig_transform.rotation = new_rotation;
            }
            *snap_turn_active = true;
        } else {
            *snap_turn_active = false;
        }
    }
}

/// Casts a ray from `grip_transform`'s position along its local -Z, out to `f32::MAX` — same as
/// `targeting::raycast_from_center` — rather than capping the search itself at
/// `VR_LASER_MAX_LENGTH`. That cap is a *visual* fallback for the no-hit case only (see
/// `update_vr_pointers`); capping the search distance too was a real bug: with
/// `VR_LASER_MAX_LENGTH` (10m) well under `targeting::SELECT_RANGE` (50m), anything past 10m —
/// including a `Selectable` still well within select range — could never register as hit at all,
/// so the "hovered but out of range" dark outline (for something beyond `SELECT_RANGE`) never had
/// a chance to show either; the ray simply never reached that far to find out.
///
/// -Z is a guess, not a real OpenXR "aim" pose — the vendored `bevy_xr_utils` fork this project
/// uses only tracks grip pose (see this module's own doc comment), so the laser points along
/// however the controller model's grip is oriented rather than its actual aim point. Flip to
/// `grip_transform.back()` here if it visually points backward on your controller.
fn controller_ray_hit(
    spatial_query: &SpatialQuery,
    grip_transform: &GlobalTransform,
    filter: &SpatialQueryFilter,
) -> Option<(Entity, f32)> {
    let hit = spatial_query.cast_ray(
        grip_transform.translation(),
        grip_transform.forward(),
        f32::MAX,
        true, // treat shapes as solid (hit registers if origin is inside)
        filter,
    );
    hit.map(|hit| (hit.entity, hit.distance))
}

/// Rising-edge check for the left thumbstick-click `Bool` action — a real OpenXR bool action
/// reports its own edge directly via `changed_since_last_sync`, unlike a `Float` action (see
/// `analog_just_pressed`), so no manual held-state tracking is needed here.
fn stick_click_just_pressed(state: &XRUtilsActionState) -> bool {
    matches!(state, XRUtilsActionState::Bool(state) if state.current_state && state.changed_since_last_sync)
}

/// Value above this counts as "pressed" for any `Float` action bound to an analog input with no
/// discrete click subpath on the touch_controller profile (trigger, squeeze) — see
/// `create_select_actions`/`create_recenter_action`.
const ANALOG_PRESS_THRESHOLD: f32 = 0.5;

/// Press-edge detector for a `Float` action (trigger, squeeze) — `was_pressed` is the caller's own
/// `Local<bool>`, since (unlike a `Bool` action's `changed_since_last_sync`, see
/// `stick_click_just_pressed`) a float action reports no edge of its own to read. `pub(crate)`
/// (along with `ANALOG_PRESS_THRESHOLD`) so `npc_ui_quad`'s VR pointer driver can turn the exact
/// same trigger reads into `PointerAction::Press`/`Release` — it needs both edges, not just the
/// rising one `analog_just_pressed` gives.
pub(crate) fn analog_press_edges(
    state: &XRUtilsActionState,
    was_pressed: &mut bool,
) -> (bool, bool) {
    let XRUtilsActionState::Float(state) = state else {
        return (false, false);
    };
    let is_pressed = state.current_state > ANALOG_PRESS_THRESHOLD;
    let just_pressed = is_pressed && !*was_pressed;
    let just_released = !is_pressed && *was_pressed;
    *was_pressed = is_pressed;
    (just_pressed, just_released)
}

pub(crate) fn analog_just_pressed(state: &XRUtilsActionState, was_pressed: &mut bool) -> bool {
    analog_press_edges(state, was_pressed).0
}

/// Mirrors `controls::select`'s body exactly (`Selectable` + in-range check) — the VR-input
/// equivalent of that desktop click handler, just bound to the right trigger instead of a
/// gamepad/mouse button, and resolving its own hand's current raycast hit instead of reading the
/// shared `Hovered` resource (which, with two independent controllers, might reflect whichever
/// hand's ray was processed last this frame rather than the hand whose trigger was actually
/// pulled).
fn try_select(
    hit: Option<(Entity, f32)>,
    selectables: &Query<(), With<Selectable>>,
    selected: &mut Selected,
) {
    if let Some((entity, distance)) = hit
        && selectables.contains(entity)
        && distance < SELECT_RANGE
    {
        selected.0 = Some(entity);
    }
}

/// Per-controller laser + hover/select — the VR equivalent of `targeting::raycast_from_center`
/// (hover) and `controls::select` (click), combined into one system since both hands need the
/// same per-frame raycast. Updates `targeting::Hovered` from whichever hand's ray currently hits a
/// `Selectable` (last one processed wins if both do — a shared, single-entity `Hovered` resource
/// was built for one screen-center ray, not two independent controllers; acceptable for outline
/// feedback, which is why `try_select` resolves its own hand's hit independently rather than
/// trusting this).
fn update_vr_pointers(
    spatial_query: SpatialQuery,
    local_player: Res<LocalPlayer>,
    selectables: Query<(), With<Selectable>>,
    mut hovered: ResMut<Hovered>,
    mut selected: ResMut<Selected>,
    left_grip: Single<&GlobalTransform, With<XrTrackedLeftGrip>>,
    right_grip: Single<&GlobalTransform, With<XrTrackedRightGrip>>,
    mut left_laser: Single<&mut Transform, (With<VrLaserLeft>, Without<VrLaserRight>)>,
    mut right_laser: Single<&mut Transform, (With<VrLaserRight>, Without<VrLaserLeft>)>,
    left_stick_click: Single<&XRUtilsActionState, With<LeftStickClickAction>>,
    left_trigger: Single<&XRUtilsActionState, With<LeftTriggerAction>>,
    right_trigger: Single<&XRUtilsActionState, With<RightTriggerAction>>,
    mut left_trigger_held: Local<bool>,
    mut right_trigger_held: Local<bool>,
) {
    let filter = SpatialQueryFilter::from_excluded_entities(match local_player.0 {
        Some(entity) => vec![entity],
        None => vec![],
    });

    let left_hit = controller_ray_hit(&spatial_query, &left_grip, &filter);
    let left_length = left_hit.map_or(VR_LASER_MAX_LENGTH, |(_, distance)| distance);
    left_laser.scale = Vec3::new(VR_LASER_WIDTH, VR_LASER_WIDTH, left_length);
    left_laser.translation = Vec3::new(0.0, 0.0, -left_length / 2.0);

    let right_hit = controller_ray_hit(&spatial_query, &right_grip, &filter);
    let right_length = right_hit.map_or(VR_LASER_MAX_LENGTH, |(_, distance)| distance);
    right_laser.scale = Vec3::new(VR_LASER_WIDTH, VR_LASER_WIDTH, right_length);
    right_laser.translation = Vec3::new(0.0, 0.0, -right_length / 2.0);

    hovered.0 = [left_hit, right_hit]
        .into_iter()
        .flatten()
        .find(|(entity, _)| selectables.contains(*entity));

    // Deselect mirrors `controls.rs`'s desktop gamepad mapping (`Deselect` on
    // `GamepadButton::LeftThumb`) — no Selectable/range check to mirror, `controls::deselect` just
    // clears unconditionally. Select is symmetric across both triggers instead of one gamepad
    // button (see `LeftTriggerAction`'s doc comment for why) — each hand selects whatever its own
    // ray is currently hitting.
    if stick_click_just_pressed(&left_stick_click) {
        selected.0 = None;
    }
    if analog_just_pressed(&left_trigger, &mut left_trigger_held) {
        try_select(left_hit, &selectables, &mut selected);
    }
    if analog_just_pressed(&right_trigger, &mut right_trigger_held) {
        try_select(right_hit, &selectables, &mut selected);
    }
}

/// Local offset of the desktop `FpsCamera` anchor relative to the player capsule — see
/// `player_character.rs`'s `on_player_spawned` (`Transform::from_xyz(0., 0.5, 0.)`, then the
/// `FpsCamera` entity itself at `Transform::IDENTITY`). `recenter_playspace` aligns the HMD's
/// *current* tracked position to this same point.
const FPS_CAMERA_LOCAL_OFFSET: Vec3 = Vec3::new(0.0, 0.5, 0.0);

/// Recenters the playspace on left-grip press: shifts `VrPlayspaceRig`'s *position* (not its yaw
/// — this only realigns where the tracked head currently is, not which way you're facing) so the
/// headset's current real-world position maps to `FPS_CAMERA_LOCAL_OFFSET` relative to the player
/// capsule — the same point the desktop `FpsCamera` sits at.
///
/// `VrPlayspaceRig` is parented directly under the player capsule (see `on_player_spawned`), so
/// its own `Transform` is already capsule-local — no need to separately fetch the capsule's
/// `GlobalTransform` to solve this. `head`'s `Transform` (from `XrTrackedView`, see
/// `spawn_head_tracker`) is in that same local frame already, for the same reason
/// `controller_ray_hit`'s grip transforms are (both are tracked relative to the same OpenXR
/// reference space `XrTrackingRoot`/its children use, and `XrTrackingRoot` itself carries no extra
/// offset of its own under the rig). So solving
/// `rig.translation + rig.rotation * head.translation == FPS_CAMERA_LOCAL_OFFSET` for
/// `rig.translation` is just local-space vector algebra, no world-space transform needed.
fn recenter_playspace(
    left_grip: Single<&XRUtilsActionState, With<LeftGripAction>>,
    mut grip_held: Local<bool>,
    head: Single<&Transform, With<XrTrackedView>>,
    mut rig: Single<&mut Transform, (With<VrPlayspaceRig>, Without<XrTrackedView>)>,
) {
    if !analog_just_pressed(&left_grip, &mut grip_held) {
        return;
    }
    rig.translation = FPS_CAMERA_LOCAL_OFFSET - rig.rotation * head.translation;
}

/// VR rig debug visualization, drawn into Avian's own `PhysicsGizmos` group rather than a group of
/// its own — `Gizmos<PhysicsGizmos>` only actually draws while that group is enabled, so this
/// piggybacks on the console's existing `physics_debug` toggle (`console.rs`) instead of needing a
/// separate command. Draws: `VrPlayspaceRig`'s pivot (yellow sphere + axes — this is the point
/// `vr_locomotion`'s snap-turn rotates around before its head-position compensation, and where
/// `recenter_playspace` moves to align the HMD with the desktop `FpsCamera` point), the HMD's
/// actual tracked world pose (aqua sphere + axes), and a line between them (visualizing how far
/// off-pivot the player's real head currently is — the whole reason that compensation exists).
///
/// The HMD's world pose isn't read directly off `XrTrackedView`'s own `GlobalTransform` — that
/// entity is deliberately unparented (see `spawn_head_tracker`), so its `GlobalTransform` would
/// just equal its local, reference-space-relative `Transform`, not its real position in the game
/// world. Composing it onto `XrTrackingRoot`'s actual (correctly-propagated) `GlobalTransform`
/// gives the real answer instead.
fn draw_vr_rig_gizmos(
    mut gizmos: Gizmos<PhysicsGizmos>,
    rig: Single<&GlobalTransform, With<VrPlayspaceRig>>,
    xr_root: Single<&GlobalTransform, With<XrTrackingRoot>>,
    head: Single<&Transform, With<XrTrackedView>>,
) {
    let head_world = xr_root.mul_transform(**head);

    gizmos.axes(**rig, 0.3);
    gizmos.sphere(rig.translation(), 0.06, YELLOW);

    gizmos.axes(head_world, 0.15);
    gizmos.sphere(head_world.translation(), 0.04, AQUA);

    gizmos.line(rig.translation(), head_world.translation(), WHITE);
}
