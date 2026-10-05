//! Demonstrates rendering UI onto a texture and displaying that texture on a 3D quad — the
//! same technique as Bevy's own `examples/ui/render_ui_to_texture.rs` (a second `Camera2d`
//! targeting an off-screen `Image` instead of the window, with a UI root pointed at it via
//! `UiTargetCamera`), applied here to a small `Rectangle` mesh parented onto each NPC
//! (`npc_spawner::decorate_npcs`) instead of a spinning cube. The quad is a child with a fixed
//! local *translation* (it sits above the NPC's head) but `billboard_npc_ui_quads` overrides its
//! local *rotation* every frame so it always faces the player's camera, unlike `rig.glb`, which
//! rotates rigidly with the NPC — a flat panel of text reads far worse edge-on or facing away than
//! a `rig.glb`-style mesh does, which is fine from any angle.
//!
//! One shared render target/camera/UI root for every NPC, not one per instance — the UI content
//! here is static (just an "NPC" label), so there's nothing per-NPC to render differently. This
//! used to also carry a real, clickable "Kill" button (driven by a virtual pointer fed into
//! `bevy_ui`'s picking pipeline) — removed. That button was a single shared entity rendered once
//! into this one shared texture, so its own hover/press state was inherently global: hovering it
//! through *any* one NPC's quad visually highlighted *all* of them at once, since every quad
//! displays the exact same rendered image. A real per-NPC button would need its own `Image`/
//! `Camera2d`/UI root per entity instead of sharing `NpcUiQuad` (same as a version showing each
//! NPC's own name/HP would). Given that cost, clicking the nameplate now just selects the NPC
//! instead (`update_npc_ui_quad_target`) — the existing hotbar's Kill ability already covers what
//! the button did, once something is selected.
//!
//! Hover *feedback* for "which NPC's nameplate is under the pointer" still needs to be per-quad,
//! though, and — unlike a real UI element's state — that's easy to give it even while sharing one
//! texture: see `update_npc_ui_quad_hover_material`.

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::RenderTarget,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use bevy_mod_openxr::openxr_session_running;
use bevy_xr_utils::{
    actions::XRUtilsActionState,
    tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip},
};

use crate::controls::targeting::{SELECT_RANGE, Selected, screen_center_ray};
use crate::controls::vr_controllers::{LeftTriggerAction, RightTriggerAction, analog_just_pressed};
use crate::ui::markup;
use p19_shared::game_state::GameState;

pub struct NpcUiQuadPlugin;

impl Plugin for NpcUiQuadPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "html/npc_sign.html");
        app.init_resource::<NpcUiQuadTarget>();
        // Not `Startup`: the first `MainMenu` comes after `GameState::AssetLoading`, so the UI
        // font is already registered when the sign first builds, and always before an NPC could
        // possibly spawn (`InGame`-only). `MainMenu` isn't a once-ever state
        // (`return_to_main_menu` re-enters it), so the `run_if` guard keeps this a one-time setup
        // instead of spawning a second render-target camera/sign on every trip back.
        app.add_systems(
            OnEnter(GameState::MainMenu),
            setup_npc_ui_quad.run_if(not(resource_exists::<NpcUiQuad>)),
        );
        app.add_systems(
            Update,
            (
                billboard_npc_ui_quads,
                // `NpcUiQuad` no longer exists during the brief `AssetLoading` window before the
                // first `OnEnter(GameState::MainMenu)` runs `setup_npc_ui_quad` above — guarded,
                // unlike the other systems here, since this is the only one that reads it
                // directly (the rest only touch `NpcUiQuadMesh`/`NpcUiQuadTarget`).
                update_npc_ui_quad_hover_material.run_if(resource_exists::<NpcUiQuad>),
                update_npc_ui_quad_target,
                update_npc_ui_quad_target_vr.run_if(openxr_session_running),
            ),
        );
    }
}

/// The quad mesh + material every NPC's sign uses — see `npc_spawner::decorate_npcs`.
#[derive(Resource, Clone)]
pub struct NpcUiQuad {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    /// Swapped onto whichever quad a pointer currently targets — see
    /// `update_npc_ui_quad_hover_material`.
    material_hovered: Handle<StandardMaterial>,
}

/// Marks the quad mesh entity itself (not the NPC it's parented to) so the target-tracking systems
/// can filter `MeshRayCast` down to just these — casting against every mesh in the level every
/// frame would be needlessly expensive and could hit unrelated geometry.
#[derive(Component, Clone, Default)]
pub struct NpcUiQuadMesh;

/// Which pointer source a `NpcUiQuadTarget` entry came from — an internal stand-in for
/// `bevy_picking::pointer::PointerId`, which this module no longer needs (nothing here feeds
/// `bevy_ui`'s picking pipeline any more — see the module doc comment). Just needs to distinguish
/// concurrent independent rays: the desktop crosshair and each VR controller's laser.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum PointerSource {
    Desktop,
    VrLeft,
    VrRight,
}

/// Which NPC (if any) each pointer source currently has its ray over a UI quad for — read by
/// `update_npc_ui_quad_hover_material` for the tint, and by `update_npc_ui_quad_target`/`_vr`
/// themselves to decide what a press selects. Keyed by `PointerSource` rather than a single
/// `Option<Entity>` since more than one ray can be live at once (desktop crosshair + two VR
/// lasers) and each should track its own target independently.
#[derive(Resource, Default)]
struct NpcUiQuadTarget(bevy::platform::collections::HashMap<PointerSource, Entity>);

const TEXTURE_SIZE: u32 = 256;
const QUAD_WIDTH: f32 = 0.8;
const QUAD_HEIGHT: f32 = 0.4;

fn setup_npc_ui_quad(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = Extent3d {
        width: TEXTURE_SIZE,
        height: TEXTURE_SIZE,
        ..default()
    };
    // Same recipe as the Bevy example this is based on: a blank target image that the UI camera
    // below renders into instead of the window, which then gets read back as an ordinary texture
    // by the quad's `StandardMaterial`.
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
                order: -1, // render before the main pass camera, same reasoning as the Bevy example
                ..default()
            },
            RenderTarget::Image(image_handle.clone().into()),
        ))
        .id();

    // The sign (`html/npc_sign.html`) fills the whole texture.
    commands.spawn((
        markup::template(&asset_server, "npc_sign.html"),
        UiTargetCamera(texture_camera),
    ));

    let mesh = meshes.add(Rectangle::new(QUAD_WIDTH, QUAD_HEIGHT));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle.clone()),
        unlit: true,
        ..default()
    });
    // A second material for whichever quad a pointer currently targets — see
    // `update_npc_ui_quad_hover_material`. Same texture, just a brighter tint multiplied over it.
    let material_hovered = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle),
        base_color: Color::srgb(1.3, 1.3, 0.9),
        unlit: true,
        ..default()
    });

    commands.insert_resource(NpcUiQuad {
        mesh,
        material,
        material_hovered,
    });
}

/// Tints whichever NPC's quad a pointer currently targets. This has to be driven separately from
/// each quad's shared render texture (rather than, say, a button's own hover-driven
/// `BackgroundColor`) precisely because that texture is shared across every NPC — see the module
/// doc comment. Each quad entity has its own `MeshMaterial3d` component, though (see
/// `npc_spawner::decorate_npcs`), even though every one of them starts out pointing at the same
/// handle — swapping just the targeted quad's handle to `NpcUiQuad::material_hovered` doesn't
/// touch any other quad's.
fn update_npc_ui_quad_hover_material(
    npc_ui_quad: Res<NpcUiQuad>,
    target: Res<NpcUiQuadTarget>,
    parents: Query<&ChildOf>,
    mut quads: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), With<NpcUiQuadMesh>>,
) {
    for (quad_entity, mut material) in &mut quads {
        let hovered = parents
            .get(quad_entity)
            .ok()
            .is_some_and(|child_of| target.0.values().any(|&npc| npc == child_of.parent()));
        let handle = if hovered {
            &npc_ui_quad.material_hovered
        } else {
            &npc_ui_quad.material
        };
        if material.0 != *handle {
            material.0 = handle.clone();
        }
    }
}

/// Keeps every NPC's UI quad facing the player's camera, overriding whatever rotation it'd
/// otherwise inherit from its parent NPC. Runs in `Update`, not `PostUpdate` before
/// `TransformSystems::Propagate` — same convention as `nameplate.rs`'s `track_nameplates` — so it
/// reads the NPC's `GlobalTransform` from the end of *last* frame's propagation, one frame behind
/// the camera's actual latest position; imperceptible for anything not spinning or moving fast.
fn billboard_npc_ui_quads(
    camera_query: Query<&GlobalTransform, With<IsDefaultUiCamera>>,
    parent_transforms: Query<&GlobalTransform, Without<NpcUiQuadMesh>>,
    mut quads: Query<(&mut Transform, &ChildOf), With<NpcUiQuadMesh>>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };
    let camera_position = camera_transform.translation();

    for (mut transform, child_of) in &mut quads {
        let Ok(parent_transform) = parent_transforms.get(child_of.parent()) else {
            continue;
        };
        let world_position = parent_transform.transform_point(transform.translation);
        let to_camera = camera_position - world_position;
        if to_camera.length_squared() < f32::EPSILON {
            continue; // camera exactly at the quad's position - nothing sensible to face
        }
        // `RectangleMeshBuilder` gives this mesh a `+Z` normal (see its `Meshable` impl), and
        // `Transform::forward()` (what `looking_to` aims at `direction`) is `-Z` by convention —
        // so aim `-to_camera` to get local `+Z` (the visible face) pointing at the camera instead.
        let world_rotation = Transform::default()
            .looking_to(-to_camera, Vec3::Y)
            .rotation;
        // Convert the desired *world* rotation into this entity's *local* rotation, since it's
        // parented to the NPC and would otherwise have the NPC's own rotation applied on top of
        // whatever gets set here.
        transform.rotation = parent_transform.rotation().inverse() * world_rotation;
    }
}

/// Casts `ray` against NPC UI quads, resolves the hit to its parent NPC, and records that as
/// `source`'s current target (or clears it, on a miss) — shared by the desktop crosshair
/// (`update_npc_ui_quad_target`) and each VR controller's laser (`update_npc_ui_quad_target_vr`).
/// On `pressed`, selects that NPC exactly like `controls::select`/`vr_controllers::try_select` do
/// for a world `Selectable` hit (same `SELECT_RANGE` check) — clicking the nameplate is meant to
/// be equivalent to clicking the NPC's own capsule, not a separate mechanic.
fn update_quad_target(
    source: PointerSource,
    ray: Ray3d,
    pressed: bool,
    ray_cast: &mut MeshRayCast,
    quads: &Query<(), With<NpcUiQuadMesh>>,
    parents: &Query<&ChildOf>,
    target: &mut NpcUiQuadTarget,
    selected: &mut Selected,
) {
    let settings = MeshRayCastSettings {
        visibility: RayCastVisibility::VisibleInView,
        filter: &|entity| quads.contains(entity),
        early_exit_test: &|_| false,
    };

    let hit = ray_cast
        .cast_ray(ray, &settings)
        .first()
        .map(|(entity, hit)| (*entity, hit.distance));

    let hit_npc = hit.and_then(|(quad_entity, distance)| {
        Some((parents.get(quad_entity).ok()?.parent(), distance))
    });

    match hit_npc {
        Some((npc, _)) => {
            target.0.insert(source, npc);
        }
        None => {
            target.0.remove(&source);
        }
    }

    if pressed
        && let Some((npc, distance)) = hit_npc
        && distance < SELECT_RANGE
    {
        selected.0 = Some(npc);
    }
}

/// Drives the desktop crosshair's `PointerSource::Desktop` entry from the screen-center ray + left
/// mouse button.
fn update_npc_ui_quad_target(
    mut ray_cast: MeshRayCast,
    camera_query: Query<(&Camera, &GlobalTransform), With<IsDefaultUiCamera>>,
    window_query: Query<&Window>,
    quads: Query<(), With<NpcUiQuadMesh>>,
    parents: Query<&ChildOf>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut target: ResMut<NpcUiQuadTarget>,
    mut selected: ResMut<Selected>,
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

    update_quad_target(
        PointerSource::Desktop,
        ray,
        mouse_buttons.just_pressed(MouseButton::Left),
        &mut ray_cast,
        &quads,
        &parents,
        &mut target,
        &mut selected,
    );
}

/// Drives `PointerSource::VrLeft`/`VrRight` from each controller's laser ray (same -Z
/// grip-forward convention as `vr_controllers::controller_ray_hit` — see its doc comment) and
/// trigger. Each hand's press-edge state (`left_trigger_held`/`right_trigger_held`) is this
/// system's own, independent of `vr_controllers::update_vr_pointers`'s identically-named locals —
/// the same physical trigger press is meant to drive *both* world-object select and this quad's
/// nameplate-click select, whichever the ray actually hits, and each needs to track "was it
/// already held" for its own purposes.
fn update_npc_ui_quad_target_vr(
    mut left_trigger_held: Local<bool>,
    mut right_trigger_held: Local<bool>,
    mut ray_cast: MeshRayCast,
    quads: Query<(), With<NpcUiQuadMesh>>,
    parents: Query<&ChildOf>,
    left_grip: Single<&GlobalTransform, With<XrTrackedLeftGrip>>,
    right_grip: Single<&GlobalTransform, With<XrTrackedRightGrip>>,
    left_trigger: Single<&XRUtilsActionState, With<LeftTriggerAction>>,
    right_trigger: Single<&XRUtilsActionState, With<RightTriggerAction>>,
    mut target: ResMut<NpcUiQuadTarget>,
    mut selected: ResMut<Selected>,
) {
    let left_pressed = analog_just_pressed(&left_trigger, &mut left_trigger_held);
    let left_ray = Ray3d::new(left_grip.translation(), left_grip.forward());
    update_quad_target(
        PointerSource::VrLeft,
        left_ray,
        left_pressed,
        &mut ray_cast,
        &quads,
        &parents,
        &mut target,
        &mut selected,
    );

    let right_pressed = analog_just_pressed(&right_trigger, &mut right_trigger_held);
    let right_ray = Ray3d::new(right_grip.translation(), right_grip.forward());
    update_quad_target(
        PointerSource::VrRight,
        right_ray,
        right_pressed,
        &mut ray_cast,
        &quads,
        &parents,
        &mut target,
        &mut selected,
    );
}
