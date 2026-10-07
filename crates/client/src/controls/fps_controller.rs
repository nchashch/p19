use bevy::prelude::*;
use bevy::transform::TransformSystems;
use p19_shared::inputs::LookDirection;

pub struct FpsControllerPlugin;

impl Plugin for FpsControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            orient_fps_camera.before(TransformSystems::Propagate),
        );
    }
}

/// The local player's view direction — the client's source of truth for look (ADR 0017). Same
/// convention as ahoy's `CharacterLook` and the replicated `LookDirection`: yaw about +Y with 0
/// facing −Z, pitch positive looking up. Mouse/stick turn it (`controls::rotate_camera`), VR
/// sets it from the headset, `controls::write_look_input` sends it to the server every tick, and
/// [`orient_fps_camera`] derives the rig's `Transform` from it.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct FpsCamera {
    pub yaw: f32,
    pub pitch: f32,
}

impl FpsCamera {
    /// Turns by `delta_yaw` (positive = left) and `delta_pitch` (positive = up), with the same
    /// wrapping and clamping the server applies to the result.
    pub fn turn(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.set(self.yaw + delta_yaw, self.pitch + delta_pitch);
    }

    /// Sets an absolute direction, wrapped and clamped like [`LookDirection::from_input`].
    pub fn set(&mut self, yaw: f32, pitch: f32) {
        if let Some(direction) = LookDirection::from_input(Vec2::new(yaw, pitch)) {
            self.yaw = direction.yaw;
            self.pitch = direction.pitch;
        }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }
}

fn orient_fps_camera(mut cameras: Query<(&FpsCamera, &mut Transform), Changed<FpsCamera>>) {
    for (camera, mut transform) in &mut cameras {
        transform.rotation = camera.rotation();
    }
}
