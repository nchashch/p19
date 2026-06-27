#[derive(Component)]
struct Crosshair;

fn spawn_crosshair(mut commands: Commands) {
    // Fullscreen centered overlay
    commands
        .spawn((
            Crosshair,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .with_children(|parent| {
            // Zero-size pivot at screen center
            parent
                .spawn(Node {
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                })
                .with_children(|pivot| {
                    // Horizontal bar
                    pivot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(16.0),
                            height: Val::Px(2.0),
                            left: Val::Px(-8.0),
                            top: Val::Px(-1.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));
                    // Vertical bar
                    pivot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(2.0),
                            height: Val::Px(16.0),
                            left: Val::Px(-1.0),
                            top: Val::Px(-8.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));
                });
        });
}

fn raycast_from_center(
    player_collider_entity: Query<Entity, (With<FpsPlayerCharacter>, With<Collider>)>,
    spatial_query: SpatialQuery,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    window_query: Query<&Window>,
    mut hovered: ResMut<Hovered>,
    disable_fps_camera_control: Res<DisableFpsCameraControl>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok(player_collider_entity) = player_collider_entity.single() else {
        return;
    };

    // Center of the screen in logical (not physical) pixels.
    let screen_origin = if disable_fps_camera_control.0 {
        window.cursor_position().unwrap_or(Vec2::ZERO)
    } else {
        window.size() / 2.0
    };

    // Screen space -> world ray. Returns Err if the camera has no usable
    // viewport/projection this frame.
    let Ok(ray) = camera.viewport_to_world(camera_transform, screen_origin) else {
        return;
    };

    if let Some(hit) = spatial_query.cast_ray(
        ray.origin,
        ray.direction, // already a Dir3
        f32::MAX,      // max distance
        true,          // treat shapes as solid (hit registers if origin is inside)
        &SpatialQueryFilter::from_excluded_entities([player_collider_entity]),
    ) {
        hovered.0 = Some(hit.entity);
    } else {
        hovered.0 = None;
    }
}

#[derive(Resource)]
struct Hovered(Option<Entity>);

#[derive(Component)]
struct Console;

fn update_console(mut query: Query<&mut Text, With<Console>>, hovered: Res<Hovered>) {
    if !hovered.is_changed() {
        return;
    }
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    text.0 = format!("Entity hovered: {:?}", hovered.0);
}

fn spawn_label(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Px(20.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Entity hovered: "),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Console,
            ));
        });
}

