//! A Skein-authorable component that carries no baked mesh data at all — just the parameters of
//! a real Bevy mesh primitive (`Cuboid`, `Sphere`, `Capsule3d`, ...). Author it in Blender on
//! any object (a bare Empty works fine, it needs no mesh data of its own) via Skein's normal
//! "add component" panel, matching its own presets/known-types list against whatever this app's
//! `AppTypeRegistry` exposes over BRP (`docs/adr/0009-agent-tool-api-via-brp.md`) — the same
//! mechanism this project already uses for `shared::level::InGameRoot`/`assets::character::RigRoot`
//! and every other Skein-authored marker.
//!
//! Both `client` and `server` register [`SharedMeshPrimitivePlugin`], so the type is known to
//! either binary's `AppTypeRegistry` regardless of which one a given `.glb` is loaded by — this
//! is not a replicated component, there is no network hop involved (see `AGENTS.md`'s "Server"
//! section for how the two binaries each load level content independently). Only `client`
//! reacts to it (`client::presentation::mesh_primitive` turns it into a real
//! `Mesh3d`/`MeshMaterial3d` for rendering, purely visual — no collider is generated from this
//! component). Neither side needs a single baked vertex in the glTF file to do that — the whole
//! point.

use bevy::math::primitives::{
    Capsule3d, Cone, ConicalFrustum, Cuboid, Cylinder, Plane3d, Sphere, Tetrahedron, Torus,
    Triangle3d,
};
use bevy::mesh::{Mesh, Meshable};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Registers [`MeshPrimitive`]/[`MeshPrimitiveShape`] for reflection so Skein can discover them
/// (over BRP, from a running dev-tools-enabled `client` — see ADR 0009) and so `bevy_skein`'s
/// glTF-extras importer can deserialize an authored value back into a real component on either
/// binary. No systems of its own — `client::presentation::mesh_primitive` registers the actual
/// spawn reaction on top of this (see the module doc comment).
pub struct SharedMeshPrimitivePlugin;

impl Plugin for SharedMeshPrimitivePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshPrimitiveShape>();
    }
}

/// Authored in Blender via Skein, on any object — an Empty with zero mesh data is the whole
/// point (see the module doc comment). Wraps a [`MeshPrimitiveShape`]; a bare newtype rather than
/// flattening the enum directly onto the component so Skein's panel shows one clean "MeshPrimitive"
/// entry with a shape picker, matching how every other Skein-authored data component in this
/// project shapes its public API.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MeshPrimitive(pub MeshPrimitiveShape);

/// One variant per solid/flat Bevy 3D mesh primitive — every type under
/// `bevy::math::primitives::dim3` that implements `Meshable` **and** produces genuine solid or
/// flat geometry. `Segment3d`/`Polyline3d` are deliberately excluded despite also implementing
/// `Meshable`: they mesh into zero-width line geometry, not something meaningful to spawn as a
/// game prop.
///
/// Fields are plain, Skein-editable primitives (`f32`/`Vec3`/`Vec2`) rather than the real
/// `bevy_math` types' own constrained wrapper types (`Dir3`, etc.) where they'd otherwise show
/// up — `Plane3d::new` itself already takes a raw `Vec3` and normalizes it internally, so a
/// Blender-authored value (accidentally non-unit-length, even literally zero) can never fail to
/// deserialize into a valid shape; it just silently degrades to whatever `Dir3`'s own fallback
/// does, matching this project's general "authoring mistake should degrade, not panic or reject
/// the whole file" posture (see `shared::assets::level::Level`'s own doc comment for the same
/// reasoning applied to a different asset).
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MeshPrimitiveShape {
    Cuboid {
        size: Vec3,
    },
    Sphere {
        radius: f32,
    },
    Capsule {
        radius: f32,
        length: f32,
    },
    Cylinder {
        radius: f32,
        height: f32,
    },
    Cone {
        radius: f32,
        height: f32,
    },
    ConicalFrustum {
        radius_top: f32,
        radius_bottom: f32,
        height: f32,
    },
    Torus {
        inner_radius: f32,
        outer_radius: f32,
    },
    Triangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
    },
    Tetrahedron {
        a: Vec3,
        b: Vec3,
        c: Vec3,
        d: Vec3,
    },
    Plane {
        normal: Vec3,
        half_size: Vec2,
    },
}

impl MeshPrimitiveShape {
    /// Builds the real `Mesh` asset this shape describes, via Bevy's own `Meshable` trait
    /// (`shape.mesh()` returns a shape-specific `MeshBuilder`, e.g. `SphereMeshBuilder`, which
    /// `.build()` turns into the final `Mesh` — the exact mechanism `Assets<Mesh>::add` expects).
    /// `client`'s spawn system calls this to get something to hand to `Assets<Mesh>`.
    pub fn to_mesh(&self) -> Mesh {
        match *self {
            Self::Cuboid { size } => Cuboid::from_size(size).mesh().build(),
            Self::Sphere { radius } => Sphere::new(radius).mesh().build(),
            Self::Capsule { radius, length } => Capsule3d::new(radius, length).mesh().build(),
            Self::Cylinder { radius, height } => Cylinder::new(radius, height).mesh().build(),
            Self::Cone { radius, height } => Cone::new(radius, height).mesh().build(),
            Self::ConicalFrustum {
                radius_top,
                radius_bottom,
                height,
            } => ConicalFrustum {
                radius_top,
                radius_bottom,
                height,
            }
            .mesh()
            .build(),
            Self::Torus {
                inner_radius,
                outer_radius,
            } => Torus::new(inner_radius, outer_radius).mesh().build(),
            Self::Triangle { a, b, c } => Triangle3d::new(a, b, c).mesh().build(),
            Self::Tetrahedron { a, b, c, d } => Tetrahedron::new(a, b, c, d).mesh().build(),
            Self::Plane { normal, half_size } => Plane3d::new(normal, half_size).mesh().build(),
        }
    }
}
