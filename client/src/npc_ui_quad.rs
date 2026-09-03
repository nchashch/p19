//! Demonstrates rendering `bevy_ui` onto a texture and displaying that texture on a rigid 3D quad
//! — the same technique as Bevy's own `examples/ui/render_ui_to_texture.rs` (a second `Camera2d`
//! targeting an off-screen `Image` instead of the window, with a UI root pointed at it via
//! `UiTargetCamera`), applied here to a small `Rectangle` mesh parented onto each NPC
//! (`npc_spawner::decorate_npcs`) instead of a spinning cube. The quad is a plain child with a
//! fixed local `Transform` — no billboarding system counter-rotates it toward the camera, so it
//! turns with the NPC exactly like any other attached mesh (e.g. `rig.glb`).
//!
//! One shared render target/camera/UI root for every NPC, not one per instance — the UI content
//! here is static, so there's nothing per-NPC to render differently. A version that showed each
//! NPC's own name/HP would need its own `Image`/`Camera2d`/UI root per entity instead of sharing
//! `NpcUiQuad`.

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    color::palettes::css::{DARK_SLATE_GRAY, WHITE_SMOKE},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};

use crate::widgets::SERIF_FONT;

pub struct NpcUiQuadPlugin;

impl Plugin for NpcUiQuadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_npc_ui_quad);
    }
}

/// The quad mesh + material every NPC's sign uses — see `npc_spawner::decorate_npcs`.
#[derive(Resource, Clone)]
pub struct NpcUiQuad {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

const TEXTURE_SIZE: u32 = 256;
const QUAD_WIDTH: f32 = 0.8;
const QUAD_HEIGHT: f32 = 0.4;

fn setup_npc_ui_quad(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
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
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(DARK_SLATE_GRAY.into()),
            UiTargetCamera(texture_camera),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("NPC"),
                TextColor(WHITE_SMOKE.into()),
                TextFont {
                    font: FontSource::Handle(asset_server.load(SERIF_FONT)),
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
            ));
        });

    let mesh = meshes.add(Rectangle::new(QUAD_WIDTH, QUAD_HEIGHT));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle),
        unlit: true,
        ..default()
    });

    commands.insert_resource(NpcUiQuad { mesh, material });
}
