use crate::game_state::GameState;
use shared::combat::HitPoints;
use bevy::color::palettes::css::{DARK_SLATE_GRAY, GREEN};
use bevy::prelude::*;

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, track_nameplates);
        app.add_observer(spawn_nameplates);
    }
}

#[derive(Component)]
struct Nameplate {
    target: Entity,
    offset: Vec3,
}

#[derive(Component)]
struct HealthFill;

/// Distance from the camera at which the nameplate starts fading, and the distance at which
/// it's fully transparent (and hidden).
const FADE_START_DISTANCE: f32 = 15.0;
const FADE_END_DISTANCE: f32 = 30.0;

fn spawn_nameplates(add: On<Add, HitPoints>, mut commands: Commands, names: Query<&Name>) {
    let target = add.entity;
    // HitPoints and Name are always spawned together in the same bundle across the codebase,
    // but fall back gracefully rather than panicking if that's ever not the case.
    let Ok(name) = names.get(target) else {
        return;
    };
    commands
        .spawn((
            Nameplate {
                target,
                offset: Vec3::new(0.0, 1.5, 0.0),
            },
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnEnter(GameState::MainMenu),
        ))
        .with_children(|parent| {
            parent.spawn((Text::new(name.as_str()), Pickable::IGNORE));
            parent
                .spawn((
                    Node {
                        width: px(60),
                        height: px(6),
                        ..default()
                    },
                    BackgroundColor(DARK_SLATE_GRAY.into()),
                    Pickable::IGNORE,
                ))
                .with_children(|bar| {
                    bar.spawn((
                        HealthFill,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(GREEN.into()),
                        Pickable::IGNORE,
                    ));
                });
        });
}

fn track_nameplates(
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    targets: Query<(&GlobalTransform, &HitPoints)>,
    mut nameplates: Query<(Entity, &Nameplate, &mut Node, &mut Visibility)>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, (With<HealthFill>, Without<Nameplate>)>,
    mut text_colors: Query<&mut TextColor>,
    mut backgrounds: Query<&mut BackgroundColor>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (nameplate_entity, nameplate, mut node, mut visibility) in &mut nameplates {
        let Ok((target_transform, hit_points)) = targets.get(nameplate.target) else {
            // Target despawned (killed, or otherwise removed) — clean up after it.
            commands.entity(nameplate_entity).despawn();
            continue;
        };

        let world_pos = target_transform.translation() + nameplate.offset;
        let distance = camera_transform.translation().distance(world_pos);
        let alpha = 1.0
            - ((distance - FADE_START_DISTANCE) / (FADE_END_DISTANCE - FADE_START_DISTANCE))
                .clamp(0.0, 1.0);

        match camera.world_to_viewport(camera_transform, world_pos) {
            Ok(screen_pos) => {
                node.left = px(screen_pos.x);
                node.top = px(screen_pos.y);
                *visibility = if alpha <= 0.0 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
            Err(_) => {
                *visibility = Visibility::Hidden;
            }
        }

        let health_percent =
            (hit_points.hit_points as f32 / hit_points.max_hit_points as f32 * 100.0).max(0.0);
        for descendant in children.iter_descendants(nameplate_entity) {
            if let Ok(mut fill_node) = fills.get_mut(descendant) {
                fill_node.width = percent(health_percent);
            }
            if let Ok(mut text_color) = text_colors.get_mut(descendant) {
                text_color.0 = text_color.0.with_alpha(alpha);
            }
            if let Ok(mut background) = backgrounds.get_mut(descendant) {
                background.0 = background.0.with_alpha(alpha);
            }
        }
    }
}
