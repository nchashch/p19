//! 9-slice UI button demo.
//!
//! Draws a 64x64 panel texture at runtime (16px border), then renders a button
//! with `NodeImageMode::Sliced`:
//! - corners are never scaled
//! - frame border edges *tile* (`SliceScaleMode::Tile`)
//! - the interior tiles too, so the button can be any size
//!
//! Run with: `cargo run --example sliced_button`

use bevy::asset::RenderAssetUsages;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::Pressed;
use bevy::ui_widgets::Activate;

/// Pixel size of one slice (corner) in the source texture.
const SLICE: u32 = 16;
/// Full source texture is 3 slices wide/tall.
const TEX: u32 = SLICE * 3;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, button_tint)
        .run();
}

fn setup(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    commands.spawn(Camera2d);

    let texture = images.add(panel_texture());

    commands
        .spawn((
            Button,
            Node {
                width: Val::Px(420.0),
                height: Val::Px(110.0),
                // Shrink/grow these to see corners stay fixed while edges/center tile
                position_type: PositionType::Absolute,
                left: Val::Percent(30.0),
                top: Val::Percent(40.0),
                ..default()
            },
            ImageNode {
                image: texture,
                image_mode: NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect::all(SLICE as f32),
                    sides_scale_mode: SliceScaleMode::Tile { stretch_value: 1.0 },
                    center_scale_mode: SliceScaleMode::Tile { stretch_value: 1.0 },
                    max_corner_scale: 1.0,
                }),
                ..default()
            },
        ))
        .observe(|_trigger: On<Activate>| {
            info!("button activated");
        })
        .with_child((
            Text::new("9-SLICE BUTTON"),
            Node {
                margin: UiRect::all(Val::Auto),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.88, 0.7)),
        ));
}

/// Tint the button on hover / press.
fn button_tint(mut buttons: Query<(&mut ImageNode, Option<&Hovered>, Has<Pressed>)>) {
    for (mut image, hovered, pressed) in &mut buttons {
        image.color = if pressed {
            Color::srgb(0.6, 0.6, 0.6)
        } else if hovered.is_some_and(|Hovered(h)| *h) {
            Color::srgb(1.15, 1.15, 1.1)
        } else {
            Color::WHITE
        };
    }
}

/// Procedurally draws the 3x3-slice source texture:
/// rounded frame corners, ticked border bands (visible tiling), flat interior.
fn panel_texture() -> Image {
    let mut pixels = vec![0u8; (TEX * TEX * 4) as usize];

    const FRAME_LIGHT: [u8; 4] = [235, 190, 105, 255];
    const FRAME_DARK: [u8; 4] = [110, 72, 36, 255];
    const PANEL: [u8; 4] = [58, 44, 34, 255];

    let mut px = |x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * TEX + x) * 4) as usize;
        pixels[i..i + 4].copy_from_slice(&c);
    };

    for y in 0..TEX {
        for x in 0..TEX {
            let in_left = x < SLICE;
            let in_right = x >= TEX - SLICE;
            let in_top = y < SLICE;
            let in_bottom = y >= TEX - SLICE;

            // Distance to the panel's inner corner (for corner rounding).
            let cx = if in_left { SLICE - 1 } else { TEX - SLICE };
            let cy = if in_top { SLICE - 1 } else { TEX - SLICE };
            let dx = x.abs_diff(cx) as f32;
            let dy = y.abs_diff(cy) as f32;

            let is_frame = (in_left || in_right) || (in_top || in_bottom);
            if is_frame {
                // Outer 4px band is light, next 4px dark, rest blends into panel.
                let band = x.min(y).min((TEX - 1 - x) as u32).min((TEX - 1 - y) as u32);
                let tick = (x % 16 == 0) || (y % 16 == 0);
                let color = match band {
                    0..=3 if tick => [255, 220, 150, 255],
                    0..=3 => FRAME_LIGHT,
                    4..=7 => FRAME_DARK,
                    _ => PANEL,
                };
                // Round the outer corners: clear pixels outside the inner radius.
                if (in_left || in_right) && (in_top || in_bottom) {
                    if dx * dx + dy * dy > 17.0 * 17.0 {
                        continue; // stays transparent
                    }
                }
                px(x, y, color);
            } else {
                px(x, y, PANEL);
            }
        }
    }

    Image::new(
        Extent3d {
            width: TEX,
            height: TEX,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn handle_ui_event() {}

struct MenuEvent {
    action: Entity,  // What action needs to be triggered.
    context: Entity, // Context in which this event was triggered.
}

struct MenuStack {
    stack: Vec<Entity>,
}

// Resolutions:
//
// 3840x2160
// 2560x1440
// 1920x1080
// 1280x800

// confirm menu
// column of buttons
// row of buttons
// grid of buttons
// column of slots
// row of slots
// grid of slots
