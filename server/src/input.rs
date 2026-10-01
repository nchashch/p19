//! Server-side input handling for the ahoy KCC stack (M2).
//!
//! The client's look is camera-driven (`FpsCamera` + `update_character_look`), but a headless
//! server has no camera — so the server's `CharacterLook` (which ahoy's `run_kcc` derives the
//! movement orientation from) is **accumulated from the replicated ahoy `RotateCamera`
//! action**: the owning client's action state streams over `BEIStateSequence`, BEI applies the
//! mocked state server-side, and this observer accumulates the per-tick deltas into the
//! character's look.

use bevy::prelude::*;
use bevy_ahoy::CharacterLook;
use bevy_ahoy::input::RotateCamera;
use lightyear_inputs_bei::prelude::Fire;

pub struct ServerInputPlugin;

impl Plugin for ServerInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(accumulate_look);
    }
}

fn accumulate_look(
    rotate: On<Fire<RotateCamera>>,
    // Corpses don't look: `kill_zero_hp` already stops a dead player's *movement* (RigidBody
    // removal starves ahoy's KCC), and this gate closes the remaining "can still look around
    // while dead" agency. Combat gating for dead *attackers* lands with the combat
    // caster-resolution fix (`resolve_attack`/`resolve_kill` currently no-op for everyone).
    mut looks: Query<&mut CharacterLook, Without<shared::combat::Dead>>,
) {
    let Ok(mut look) = looks.get_mut(rotate.context) else {
        return;
    };
    // Mirrors the client camera's accumulation signs EXACTLY (`rotate_camera` in
    // `controls.rs` — the same events this observer and the client's camera both consume, so
    // the two sides' looks agree by construction): yaw takes -x, pitch takes +y. The
    // binding-level `Scale`/`DeltaScale` modifiers (chosen in `controls.rs`'s binding
    // system) already put the value into radians-per-tick, so no unit conversion happens
    // here — unlike ahoy's own `rotate_camera`, which expects degrees.
    look.yaw -= rotate.value.x;
    look.pitch = (look.pitch + rotate.value.y).clamp(
        -core::f32::consts::FRAC_PI_2 + 0.0001,
        core::f32::consts::FRAC_PI_2 - 0.0001,
    );
}
