//! Turns a Skein-authored `p19_shared::mesh_primitive::MeshPrimitive` into real, renderable geometry
//! — see that module's own doc comment for the full authoring story (an Empty in Blender, no
//! baked mesh data at all, just the primitive's parameters). Purely visual (`Mesh3d`/
//! `MeshMaterial3d`, no collider) — `server` registers the same component's reflection too (see
//! `p19_shared::mesh_primitive::SharedMeshPrimitivePlugin`) but doesn't react to it.

use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use p19_shared::mesh_primitive::MeshPrimitive;

pub struct ClientMeshPrimitivePlugin;

impl Plugin for ClientMeshPrimitivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_mesh_primitives);
    }
}

/// One shared default material for every `MeshPrimitive` that doesn't otherwise carry its own
/// (there's no material-authoring story on this component yet — see the module doc comment;
/// pairing `MeshPrimitive` with a separately Skein-authored material component is the natural
/// follow-up once one exists) — a `Local` cache so repeated spawns don't allocate a fresh
/// `StandardMaterial` asset per entity for what's visually the same flat gray.
#[derive(Default)]
struct DefaultPrimitiveMaterial(Option<Handle<StandardMaterial>>);

/// A polling `Update` system, not an `Added<MeshPrimitive>` observer — same reasoning as
/// `gameplay::player_character::on_player_spawned`'s (see its own doc comment): replication and
/// Skein/glTF instantiation don't reliably fire per-component `Add` observers on arrival the way
/// a live mid-session insert does, so this polls instead. `Without<Mesh3d>` makes it
/// self-terminating: once handled, the entity carries `Mesh3d` and stops matching.
fn spawn_mesh_primitives(
    unhandled: Query<(Entity, &MeshPrimitive), Without<Mesh3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut default_material: Local<DefaultPrimitiveMaterial>,
    mut commands: Commands,
) {
    for (entity, primitive) in &unhandled {
        let mesh = meshes.add(primitive.0.to_mesh());
        let material = default_material
            .0
            .get_or_insert_with(|| materials.add(StandardMaterial::default()))
            .clone();
        commands
            .entity(entity)
            .insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}
