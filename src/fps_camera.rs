use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct FpsCamera;

pub fn fps_camera() -> impl Bundle {
    ()
}
