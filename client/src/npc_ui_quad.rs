//! Demonstrates rendering `bevy_ui` onto a texture and displaying that texture on a 3D quad — the
//! same technique as Bevy's own `examples/ui/render_ui_to_texture.rs` (a second `Camera2d`
//! targeting an off-screen `Image` instead of the window, with a UI root pointed at it via
//! `UiTargetCamera`), applied here to a small `Rectangle` mesh parented onto each NPC
//! (`npc_spawner::decorate_npcs`) instead of a spinning cube. The quad is a child with a fixed
//! local *translation* (it sits above the NPC's head) but `billboard_npc_ui_quads` overrides its
//! local *rotation* every frame so it always faces the player's camera, unlike `rig.glb`, which
//! rotates rigidly with the NPC — a flat panel of text/a button reads far worse edge-on or facing
//! away than a `rig.glb`-style mesh does, which is fine from any angle.
//!
//! One shared render target/camera/UI root for every NPC, not one per instance — the UI content
//! here is static (an "NPC" label and a "Kill" button), so there's nothing per-NPC to render
//! differently. A version that showed each NPC's own name/HP would need its own `Image`/
//! `Camera2d`/UI root per entity instead of sharing `NpcUiQuad`.
//!
//! The "Kill" button is real `bevy_ui` content (`widgets::button`, with its normal hover/press
//! observers), driven by a virtual pointer rather than the real mouse — same technique as the
//! Bevy example's own `drive_diegetic_pointer`, but using the crosshair's screen-center ray
//! (`targeting::screen_center_ray`, the same one `targeting::raycast_from_center` uses for world
//! selection) instead of the window cursor position, and `bevy_picking`'s `MeshRayCast` instead of
//! Avian's `SpatialQuery` — Avian's `RayHitData` has no UV coordinate on the hit surface, which is
//! exactly what's needed to turn "the ray hit this quad here" into "the pointer is at this pixel
//! on the render texture."

use bevy::{
    asset::{RenderAssetUsages, uuid::Uuid},
    camera::RenderTarget,
    color::palettes::css::{DARK_SLATE_GRAY, WHITE_SMOKE},
    picking::{
        PickingSystems,
        pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    text::FontSourceTemplate,
};
use bevy_replicon::prelude::ClientTriggerExt;
use shared::client_events::KillAttempt;

use crate::targeting::screen_center_ray;
use crate::widgets::{Activate, SERIF_FONT, button};

pub struct NpcUiQuadPlugin;

impl Plugin for NpcUiQuadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcUiQuadTarget>();
        app.add_systems(Startup, setup_npc_ui_quad);
        app.add_systems(Update, billboard_npc_ui_quads);
        // Same schedule/set the Bevy example this is based on uses for its own virtual pointer —
        // `PickingSystems::Input` is where real pointer backends (mouse, touch) also turn raw
        // input into `PointerInput` events, so this needs to run alongside them, before hit-testing
        // consumes whatever `PointerInput`s exist for this frame.
        app.add_systems(
            First,
            drive_npc_ui_quad_pointer.in_set(PickingSystems::Input),
        );
    }
}

/// The quad mesh + material every NPC's sign uses — see `npc_spawner::decorate_npcs`. Also holds
/// the texture camera's entity, so `drive_npc_ui_quad_pointer` can look up its `RenderTarget`
/// without a separate query/marker just for that one entity.
#[derive(Resource, Clone)]
pub struct NpcUiQuad {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    texture_camera: Entity,
}

/// Marks the quad mesh entity itself (not the NPC it's parented to) so `drive_npc_ui_quad_pointer`
/// can filter `MeshRayCast` down to just these — casting against every mesh in the level every
/// frame would be needlessly expensive and could hit unrelated geometry.
#[derive(Component, Clone, Default)]
pub struct NpcUiQuadMesh;

/// Which NPC (if any) the crosshair is currently over a UI quad for — read by the "Kill" button's
/// own `Activate` observer to know which entity to dispatch `KillAttempt` against, since the
/// button entity itself is shared across every NPC's quad (see the module doc comment) and has no
/// way to know on its own which quad it was clicked through.
#[derive(Resource, Default)]
struct NpcUiQuadTarget(Option<Entity>);

const TEXTURE_SIZE: u32 = 256;
const QUAD_WIDTH: f32 = 0.8;
const QUAD_HEIGHT: f32 = 0.4;

fn setup_npc_ui_quad(
    mut commands: Commands,
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

    commands
        .spawn_scene(bsn! {
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(12),
            }
            BackgroundColor(DARK_SLATE_GRAY)
            Children [
                (
                    Text("NPC")
                    TextColor(WHITE_SMOKE)
                    TextFont {
                        font: FontSourceTemplate::Handle(SERIF_FONT),
                        font_size: px(64),
                    }
                ),
                (
                    button(px(140), px(50), "hud-npc-kill")
                    on(on_kill_button)
                ),
            ]
        })
        // `UiTargetCamera` doesn't implement `FromTemplate`, so it can't be constructed through
        // bsn!'s tuple-call component syntax the way e.g. `BackgroundColor` above can — inserted
        // directly instead, same effect.
        .insert(UiTargetCamera(texture_camera));

    // The pointer this whole module drives — see `drive_npc_ui_quad_pointer`. A stable id is
    // needed since (per `PointerId`'s own docs) pointers can be spawned/despawned independently of
    // any one entity; `Uuid::from_u128` with an arbitrary fixed constant is the same pattern the
    // Bevy example this is based on uses for its own virtual pointer.
    commands.spawn(NPC_UI_QUAD_POINTER_ID);

    let mesh = meshes.add(Rectangle::new(QUAD_WIDTH, QUAD_HEIGHT));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle),
        unlit: true,
        ..default()
    });

    commands.insert_resource(NpcUiQuad {
        mesh,
        material,
        texture_camera,
    });
}

fn on_kill_button(_event: On<Activate>, target: Res<NpcUiQuadTarget>, mut commands: Commands) {
    if let Some(entity) = target.0 {
        commands.client_trigger(KillAttempt { entity });
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

const NPC_UI_QUAD_POINTER_ID: PointerId =
    PointerId::Custom(Uuid::from_u128(0x4e5043_5549_5051_4144_000000000000));

/// Off-canvas sentinel position for when the crosshair isn't over any NPC's quad this frame —
/// moving the virtual pointer here (rather than simply not sending a `Move` event) is what makes
/// the "Kill" button's hover state actually clear once you look away from it; `bevy_picking`
/// re-hit-tests off each pointer's last known location every frame, not off whether a fresh event
/// arrived, so a pointer left sitting on the button's last position would read as still-hovered
/// forever.
const OFF_CANVAS: Vec2 = Vec2::new(-1.0, -1.0);

/// Feeds the crosshair ray into `bevy_ui`'s normal picking pipeline as a synthetic pointer, so
/// `hud.rs`'s reticle can hover/click real `bevy_ui` content (the "Kill" button) the same way a
/// real cursor would — see the module doc comment for why `MeshRayCast` instead of `SpatialQuery`.
fn drive_npc_ui_quad_pointer(
    mut last_position: Local<Vec2>,
    mut ray_cast: MeshRayCast,
    camera_query: Query<(&Camera, &GlobalTransform), With<IsDefaultUiCamera>>,
    window_query: Query<&Window>,
    npc_ui_quad: Option<Res<NpcUiQuad>>,
    quads: Query<(), With<NpcUiQuadMesh>>,
    parents: Query<&ChildOf>,
    render_targets: Query<&RenderTarget>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut target: ResMut<NpcUiQuadTarget>,
    mut pointer_inputs: MessageWriter<PointerInput>,
) {
    let Some(npc_ui_quad) = npc_ui_quad else {
        return; // not ready yet (only true for the first frame or so after Startup)
    };
    let Ok(render_target) = render_targets.get(npc_ui_quad.texture_camera) else {
        return;
    };
    let Some(normalized_target) = render_target.normalize(None) else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(ray) = screen_center_ray(camera, camera_transform, window) else {
        return;
    };

    let settings = MeshRayCastSettings {
        visibility: RayCastVisibility::VisibleInView,
        filter: &|entity| quads.contains(entity),
        early_exit_test: &|_| false,
    };

    let hit = ray_cast
        .cast_ray(ray, &settings)
        .first()
        .and_then(|(quad_entity, hit)| Some((*quad_entity, hit.uv?)));

    let (position, hit_npc) = match hit {
        Some((quad_entity, uv)) => (
            uv * TEXTURE_SIZE as f32,
            parents.get(quad_entity).ok().map(ChildOf::parent),
        ),
        None => (OFF_CANVAS, None),
    };
    target.0 = hit_npc;

    if position != *last_position {
        pointer_inputs.write(PointerInput::new(
            NPC_UI_QUAD_POINTER_ID,
            Location {
                target: normalized_target.clone(),
                position,
            },
            PointerAction::Move {
                delta: position - *last_position,
            },
        ));
        *last_position = position;
    }

    // Only meaningful while actually hovering a quad — off-canvas, nothing is there to press.
    if hit_npc.is_some() {
        if mouse_buttons.just_pressed(MouseButton::Left) {
            pointer_inputs.write(PointerInput::new(
                NPC_UI_QUAD_POINTER_ID,
                Location {
                    target: normalized_target.clone(),
                    position,
                },
                PointerAction::Press(PointerButton::Primary),
            ));
        }
        if mouse_buttons.just_released(MouseButton::Left) {
            pointer_inputs.write(PointerInput::new(
                NPC_UI_QUAD_POINTER_ID,
                Location {
                    target: normalized_target,
                    position,
                },
                PointerAction::Release(PointerButton::Primary),
            ));
        }
    }
}
