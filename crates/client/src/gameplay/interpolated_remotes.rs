//! Marks replicated moving bodies this client doesn't predict as [`Interpolated`], so remote
//! players/cubes/NPCs render at smoothly-interpolated `Position`/`Rotation` instead of snapping
//! to whatever the last received server tick said.
//!
//! Everything else was already in place: `lightyear`'s default `interpolation` feature means
//! `SharedPlugins` registers `InterpolationMarkerPlugin` + `InterpolationPlugin`, and
//! `LightyearAvianPlugin`'s `AvianReplicationMode::Position` registers velocity-aware Hermite
//! rules for the `(Position, Rotation, LinearVelocity, AngularVelocity)` bundle (plus
//! per-component linear fallbacks). What was missing was purely the marker: without it, remote
//! entities updated at raw arrival cadence (perceptible jitter for anything moving).
//!
//! Filter, and why:
//! - `Without<Predicted>` — the local player's entity is predicted + rollback-driven; delayed
//!   interpolation writing `Position` on it would fight lightyear's rollback. Remote entities
//!   have no `PredictionTarget` scoped to this client, so no `Predicted` marker arrives.
//! - `RigidBody != Static` — level geometry is `RigidBody::Static` and never moves; marking it
//!   would buy `ConfirmedHistory` components and per-frame sampling for the entire level for
//!   zero visual gain. Players (Kinematic) and cubes/NPCs (Dynamic) are the things that move.
//!
//! Known footgun accepted (see `lightyear_avian3d`'s plugin docs): `RigidBody` *is* replicated
//! onto these entities — deliberately, so Avian attaches their colliders client-side and the
//! local KCC collides against them. The docs warn against `RigidBody` on `Interpolated`
//! entities when the client runs a full physics sim; this client's integrator/solver are
//! disabled (server is authoritative), so the only consumer of those bodies is broadphase +
//! spatial queries, which is exactly what should follow the interpolated (visible) position.

use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::prelude::*;

pub struct InterpolatedRemotesPlugin;

impl Plugin for InterpolatedRemotesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, interpolate_remote_bodies);
    }
}

/// Polling `Update` system, not an observer, for the same replication-arrival reason as
/// `player_character.rs`'s decoration systems: a replicated entity's components (and lightyear's
/// client-side `Predicted` marker) all land during the same `PreUpdate` receive pass, and the
/// initial full-state sync on a late join doesn't reliably fire per-component observers. The
/// `Without<Interpolated>` filter makes this self-terminating — each entity is visited once.
fn interpolate_remote_bodies(
    bodies: Query<(Entity, &RigidBody), (Without<Interpolated>, Without<Predicted>)>,
    mut commands: Commands,
) {
    for (entity, rigid_body) in &bodies {
        if *rigid_body == RigidBody::Static {
            continue;
        }
        commands.entity(entity).insert(Interpolated);
    }
}
