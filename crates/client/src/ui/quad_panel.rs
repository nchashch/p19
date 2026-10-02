//! A reusable "UI-on-a-quad" building block — the same render-to-texture technique
//! `npc_ui_quad.rs` uses (a `Camera2d` targeting an off-screen `Image`, with a UI root pointed at
//! it via `UiTargetCamera`, displayed on a `Rectangle` mesh), factored out so any caller can spawn
//! a *fully interactive* instance of it wherever they like — `quad_panel()` returns a bundle for
//! the visual 3D quad, which the caller positions/parents (above an NPC's head, floating in space,
//! attached to a VR controller for a wrist-mounted panel, ...).
//!
//! Unlike `npc_ui_quad.rs`, which deliberately shares *one* render target across every NPC (many
//! instances, identical static content, so real per-instance interactivity wasn't worth a
//! render-target-per-NPC), each `quad_panel()` call gets its own `Image`/`Camera2d`/UI root — the
//! expected use here is a handful of independent panels with genuinely different, genuinely
//! interactive content each, not dozens of copies of the same thing.
//!
//! Maps exactly three ray sources into real `bevy_ui` picking (`PointerInput` events — hover,
//! press, release, the works) against every `QuadPanel` in the world at once: the desktop
//! screen-center crosshair (`targeting::screen_center_ray`, left mouse button), and each VR
//! controller's laser (`vr_controllers`'s tracked grip pose, that hand's trigger) — see
//! `drive_quad_panel_pointer_desktop`/`drive_quad_panel_pointer_vr`.

use bevy::{
    asset::{RenderAssetUsages, uuid::Uuid},
    camera::{NormalizedRenderTarget, RenderTarget},
    picking::{
        PickingSystems,
        pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use bevy_mod_openxr::openxr_session_running;
use bevy_xr_utils::{
    actions::XRUtilsActionState,
    tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip},
};

use crate::controls::targeting::screen_center_ray;
use crate::controls::vr_controllers::{LeftTriggerAction, RightTriggerAction, analog_press_edges};

pub struct QuadPanelPlugin;

impl Plugin for QuadPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_quad_panel_pointers);
        // Same schedule/set real pointer backends (mouse, touch) use to turn raw input into
        // `PointerInput` events — this needs to run alongside them, before hit-testing consumes
        // whatever `PointerInput`s exist for this frame.
        app.add_systems(
            First,
            (
                drive_quad_panel_pointer_desktop,
                drive_quad_panel_pointer_vr.run_if(openxr_session_running),
            )
                .in_set(PickingSystems::Input),
        );
    }
}

/// Marks a quad-panel instance's mesh entity, and holds what the pointer-driving systems need to
/// resolve a `MeshRayCast` hit on it into a `bevy_ui` pointer position: which camera renders its
/// content, and the render texture's pixel size (a hit's UV, in `0.0..1.0`, needs scaling into
/// that to become a `Location::position`).
#[derive(Component)]
pub struct QuadPanel {
    texture_camera: Entity,
    texture_size: Vec2,
}

/// Builds one quad-panel instance: an off-screen `Camera2d`/`Image` render target, `scene` spawned
/// as its UI content, and a `Rectangle` mesh (`width` x `height`, in world units) displaying that
/// texture — returned as a bundle for the caller to spawn wherever it belongs (as a child of an
/// NPC, a VR controller, or anywhere else). `texture_width`/`texture_height` are the render
/// texture's resolution, which is also the coordinate space `scene`'s `Node`s lay out in (a `px`
/// size in `scene` maps directly to that many texture pixels), independent of the mesh's physical
/// `width`/`height` in the 3D world.
pub fn quad_panel(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    width: f32,
    height: f32,
    texture_width: u32,
    texture_height: u32,
    scene: impl Scene,
) -> impl Bundle {
    let size = Extent3d {
        width: texture_width,
        height: texture_height,
        ..default()
    };
    // Same recipe `npc_ui_quad.rs` uses: a blank target image the UI camera below renders into
    // instead of the window, which then gets read back as an ordinary texture by the quad's
    // `StandardMaterial`.
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image_handle = images.add(image);

    let texture_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -1, // render before the main pass camera, same as npc_ui_quad.rs
                ..default()
            },
            RenderTarget::Image(image_handle.clone().into()),
        ))
        .id();

    // `UiTargetCamera` doesn't implement `FromTemplate`, so it can't be constructed through
    // bsn!'s tuple-call component syntax inside `scene` itself — inserted directly instead.
    commands
        .spawn_scene(scene)
        .insert(UiTargetCamera(texture_camera));

    let mesh = meshes.add(Rectangle::new(width, height));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle),
        unlit: true,
        // `StandardMaterial`'s default (`Some(Face::Back)`) hides this quad completely from
        // whichever side its `+Z` normal isn't facing — fine for something whose orientation is
        // known and fixed (a nameplate always billboarded toward the camera), but every caller of
        // this reusable module can't be assumed to have that guarantee (e.g. a VR
        // controller/wrist-attached panel, where the exact facing depends on an OpenXR grip-pose
        // orientation this project has no documented convention for — see `vr_controllers`'s
        // module doc comment). Rendering both sides means a panel is never invisible outright just
        // because it's mounted backward; at worst its (mirrored) text reads wrong from the back,
        // a far easier problem to notice and fix than "nothing shows up at all."
        cull_mode: None,
        ..default()
    });

    (
        QuadPanel {
            texture_camera,
            texture_size: Vec2::new(texture_width as f32, texture_height as f32),
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Visibility::default(),
    )
}

// Arbitrary, mutually distinct constants — see `PointerId::Custom`'s own docs for why a stable id
// is needed (pointers can be spawned/despawned independently of any one entity).
const DESKTOP_POINTER_ID: PointerId =
    PointerId::Custom(Uuid::from_u128(0x1111_1111_1111_1111_1111_1111_1111_1111));
const VR_LEFT_POINTER_ID: PointerId =
    PointerId::Custom(Uuid::from_u128(0x2222_2222_2222_2222_2222_2222_2222_2222));
const VR_RIGHT_POINTER_ID: PointerId =
    PointerId::Custom(Uuid::from_u128(0x3333_3333_3333_3333_3333_3333_3333_3333));

/// Spawns an entity for each of the three pointer ids above. This is the step that was actually
/// missing before: `bevy_picking`'s own `PointerInput::receive` only updates `PointerLocation`/
/// `PointerPress` on entities that *already* carry a matching `PointerId` component (it matches by
/// iterating existing `Query<(&PointerId, &mut PointerLocation, &mut PointerPress)>` — see that
/// function's source), and `update_pointer_map`/the `ui_picking` backend likewise only ever query
/// existing `PointerId` entities. Sending `PointerInput` messages for an id with no such entity is
/// not "implicit registration" (a wrong assumption an earlier version of this file made in a
/// comment here) — those messages are simply read and dropped, matched against nothing, and hover/
/// click silently never happens. `PointerId` requires (`#[require(...)]`) `PointerLocation`/
/// `PointerPress`/`PointerInteraction`, so spawning just the bare id is enough — same as
/// `npc_ui_quad.rs`'s original (working) `commands.spawn(NPC_UI_QUAD_POINTER_ID)`, and how
/// `bevy_picking`'s own touch backend spawns a pointer entity the first time a given touch starts.
fn spawn_quad_panel_pointers(mut commands: Commands) {
    commands.spawn(DESKTOP_POINTER_ID);
    commands.spawn(VR_LEFT_POINTER_ID);
    commands.spawn(VR_RIGHT_POINTER_ID);
}

/// Off-canvas sentinel position for when a ray isn't over any `QuadPanel` this frame — moving the
/// virtual pointer here (rather than simply not sending a `Move` event) is what makes hover state
/// actually clear once the ray looks away; `bevy_picking` re-hit-tests off each pointer's last
/// known location every frame, not off whether a fresh event arrived, so a pointer left sitting on
/// a widget's last position would read as still-hovered forever.
const OFF_CANVAS: Vec2 = Vec2::new(-1.0, -1.0);

/// Per-ray-source state `drive_quad_panel_pointer` needs across frames — each caller
/// (`drive_quad_panel_pointer_desktop`/`_vr`) owns its own as a `Local`.
#[derive(Default)]
struct QuadPointerState {
    /// Which panel's render target this pointer is currently (or was last) interacting with.
    /// Needed so a "ray moved off every panel" frame still has *some* target to send the
    /// `OFF_CANVAS`-clearing `Move` to — cleared (`None`) once that clearing move has been sent,
    /// so it isn't repeated every frame the ray stays off every panel.
    target: Option<NormalizedRenderTarget>,
    position: Vec2,
}

/// Casts `ray` against every `QuadPanel` and feeds a synthetic pointer (`pointer_id`) into
/// `bevy_ui`'s normal picking pipeline, so real `bevy_ui` content on any of them can be
/// hovered/clicked the same way a real cursor would — `bevy_picking`'s `MeshRayCast` rather than
/// Avian's `SpatialQuery`, since Avian's `RayHitData` has no UV coordinate on the hit surface,
/// exactly what's needed to turn "the ray hit this panel here" into "the pointer is at this pixel
/// on its render texture." Shared by the desktop crosshair and each VR controller's laser.
fn drive_quad_panel_pointer(
    pointer_id: PointerId,
    ray: Ray3d,
    pressed: bool,
    released: bool,
    ray_cast: &mut MeshRayCast,
    quads: &Query<&QuadPanel>,
    render_targets: &Query<&RenderTarget>,
    state: &mut QuadPointerState,
    pointer_inputs: &mut MessageWriter<PointerInput>,
) {
    let settings = MeshRayCastSettings {
        visibility: RayCastVisibility::VisibleInView,
        filter: &|entity| quads.contains(entity),
        early_exit_test: &|_| false,
    };

    let hit = ray_cast
        .cast_ray(ray, &settings)
        .first()
        .and_then(|(entity, hit)| {
            let panel = quads.get(*entity).ok()?;
            let target = render_targets
                .get(panel.texture_camera)
                .ok()?
                .normalize(None)?;
            Some((target, hit.uv? * panel.texture_size))
        });

    let (target, position) = match hit {
        Some((target, position)) => {
            state.target = Some(target.clone());
            (target, position)
        }
        None => {
            let Some(target) = state.target.take() else {
                return; // this pointer has never hit a panel — nothing to clear
            };
            (target, OFF_CANVAS)
        }
    };

    if position != state.position {
        pointer_inputs.write(PointerInput::new(
            pointer_id,
            Location {
                target: target.clone(),
                position,
            },
            PointerAction::Move {
                delta: position - state.position,
            },
        ));
        state.position = position;
    }

    // Only meaningful while actually hovering a panel — off-canvas, nothing is there to press.
    if position != OFF_CANVAS {
        if pressed {
            pointer_inputs.write(PointerInput::new(
                pointer_id,
                Location {
                    target: target.clone(),
                    position,
                },
                PointerAction::Press(PointerButton::Primary),
            ));
        }
        if released {
            pointer_inputs.write(PointerInput::new(
                pointer_id,
                Location { target, position },
                PointerAction::Release(PointerButton::Primary),
            ));
        }
    }
}

/// Drives `DESKTOP_POINTER_ID` from the screen-center crosshair ray + left mouse button.
fn drive_quad_panel_pointer_desktop(
    mut state: Local<QuadPointerState>,
    mut ray_cast: MeshRayCast,
    camera_query: Query<(&Camera, &GlobalTransform), With<IsDefaultUiCamera>>,
    window_query: Query<&Window>,
    quads: Query<&QuadPanel>,
    render_targets: Query<&RenderTarget>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut pointer_inputs: MessageWriter<PointerInput>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(ray) = screen_center_ray(camera, camera_transform, window) else {
        return;
    };

    drive_quad_panel_pointer(
        DESKTOP_POINTER_ID,
        ray,
        mouse_buttons.just_pressed(MouseButton::Left),
        mouse_buttons.just_released(MouseButton::Left),
        &mut ray_cast,
        &quads,
        &render_targets,
        &mut state,
        &mut pointer_inputs,
    );
}

/// Drives `VR_LEFT_POINTER_ID`/`VR_RIGHT_POINTER_ID` from each controller's laser ray (same -Z
/// grip-forward convention as `vr_controllers::controller_ray_hit` — see its doc comment) and
/// trigger. Each hand's press/release-edge state (`left_trigger_held`/`right_trigger_held`) is
/// this system's own, independent of `vr_controllers::update_vr_pointers`'s identically-named
/// locals — the same physical trigger press is meant to drive *both* world-object select and
/// whatever `QuadPanel`s the ray happens to hit, and each needs its own "was it already held".
fn drive_quad_panel_pointer_vr(
    mut left_state: Local<QuadPointerState>,
    mut right_state: Local<QuadPointerState>,
    mut left_trigger_held: Local<bool>,
    mut right_trigger_held: Local<bool>,
    mut ray_cast: MeshRayCast,
    quads: Query<&QuadPanel>,
    render_targets: Query<&RenderTarget>,
    left_grip: Single<&GlobalTransform, With<XrTrackedLeftGrip>>,
    right_grip: Single<&GlobalTransform, With<XrTrackedRightGrip>>,
    left_trigger: Single<&XRUtilsActionState, With<LeftTriggerAction>>,
    right_trigger: Single<&XRUtilsActionState, With<RightTriggerAction>>,
    mut pointer_inputs: MessageWriter<PointerInput>,
) {
    let (left_pressed, left_released) = analog_press_edges(&left_trigger, &mut left_trigger_held);
    let left_ray = Ray3d::new(left_grip.translation(), left_grip.forward());
    drive_quad_panel_pointer(
        VR_LEFT_POINTER_ID,
        left_ray,
        left_pressed,
        left_released,
        &mut ray_cast,
        &quads,
        &render_targets,
        &mut left_state,
        &mut pointer_inputs,
    );

    let (right_pressed, right_released) =
        analog_press_edges(&right_trigger, &mut right_trigger_held);
    let right_ray = Ray3d::new(right_grip.translation(), right_grip.forward());
    drive_quad_panel_pointer(
        VR_RIGHT_POINTER_ID,
        right_ray,
        right_pressed,
        right_released,
        &mut ray_cast,
        &quads,
        &render_targets,
        &mut right_state,
        &mut pointer_inputs,
    );
}
