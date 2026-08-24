use crate::{cube_spawner::HitPoints, game_state::GameState};
use bevy::color::palettes::css::{DARK_SLATE_GRAY, GREEN};
use bevy::prelude::*;

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_nameplates, track_nameplates));
    }
}

#[derive(Component)]
struct Nameplate {
    target: Entity,
    offset: Vec3,
}

/// Guard marker on the *target* entity, so `spawn_nameplates` only spawns one nameplate per target.
#[derive(Component)]
struct HasNameplate;

#[derive(Component)]
struct HealthFill;

fn spawn_nameplates(
    mut commands: Commands,
    targets: Query<(Entity, &Name), (With<HitPoints>, Without<HasNameplate>)>,
) {
    for (target, name) in targets {
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
        commands.entity(target).insert(HasNameplate);
    }
}

fn track_nameplates(
    mut commands: Commands,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    targets: Query<(&GlobalTransform, &HitPoints)>,
    mut nameplates: Query<(Entity, &Nameplate, &mut Node, &mut Visibility)>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, (With<HealthFill>, Without<Nameplate>)>,
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
        match camera.world_to_viewport(camera_transform, world_pos) {
            Ok(screen_pos) => {
                *visibility = Visibility::Inherited;
                node.left = px(screen_pos.x);
                node.top = px(screen_pos.y);
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
        }
    }
}
