use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_ahoy::input::{Jump, Movement, RotateCamera};
use bevy_ahoy::prelude::CharacterController as AhoyCharacterController;
use bevy_ahoy::CharacterLook;
use bevy::ecs::spawn::SpawnWith;
use bevy_enhanced_input::prelude::{Action, Actions, ActionSpawner};
use serde::{Deserialize, Serialize};

use crate::character_controller::{Character, GameLayer, Idle};
use crate::combat::{Gcd, HitPoints};
use crate::inputs::{MouseLook, PlayerInputContext, StickLook};

#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PlayerCharacterSpawner;

/// `#[require(Visibility)]` for the same reason `shared::level::InGameRoot` needs it: the client's
/// mirror of this entity only ever gets whatever's explicitly `.replicate::<T>()`-registered, and
/// `Visibility`/`InheritedVisibility`/`ViewVisibility` are deliberately never replicated (meant to
/// be computed locally, the same way `GlobalTransform` is computed locally from `Transform`).
/// Without this, the client's replicated `PlayerCharacter` had no `Visibility` at all, which
/// produced the same `bevy_app::hierarchy` B0004 warning `InGameRoot` used to (a child with
/// `InheritedVisibility` — e.g. the `rig.glb` model `player_character.rs::decorate_other_players`
/// attaches to other players — parented under an entity that itself has none).
#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
#[require(Visibility)]
pub struct PlayerCharacter;

#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Selectable;

pub fn player(player_name: String, position: Vec3) -> impl Bundle {
    (
        (
            PlayerCharacter,
            Selectable,
            Character,
            Idle,
            Name::new(player_name),
            HitPoints {
                hit_points: 100,
                max_hit_points: 100,
            },
            Gcd::default(),
        ),
        Collider::capsule(0.4, 1.0),
        CollisionLayers::new(
            GameLayer::Player,
            LayerMask::ALL & !LayerMask::from(GameLayer::Npc) & !LayerMask::from(GameLayer::Player),
        ),
        RigidBody::Kinematic,
        Transform::from_translation(position),
        // Ahoy's KCC + its replicated input context — the M2 server-authoritative movement
        // stack. The KCC runs on BOTH binaries over the same `AccumulatedInput` stream (fed
        // by replicated BEI action state — see `shared::inputs`), the server's being
        // authoritative; the client's local copy is the prediction that M3/M4 reconcile.
        // The action entities spawn here (server-side) **without bindings** — input arrives
        // via `BEIStateSequence` replication, and the owning client adds local-only
        // `Bindings` to the replicated action entities (see `controls.rs`'s binding
        // observers). `Actions::<C>::spawn`'s action entities ride the player's replication
        // (`ActionOf<C>` hierarchy sender — registered by `InputPlugin`).
        //
        // Aliased `AhoyCharacterController` — the gutted shared `CharacterController` above
        // is still in this bundle until M4 deletes it.
        AhoyCharacterController::default(),
        // Server-side look is accumulated from the replicated ahoy `RotateCamera` action (see
        // `server::input`); the client overwrites its own `CharacterLook` from its camera
        // every frame.
        CharacterLook::default(),
        PlayerInputContext,
        Actions::<PlayerInputContext>::spawn(SpawnWith(|context: &mut ActionSpawner<_>| {
            context.spawn(Action::<Movement>::new());
            context.spawn(Action::<Jump>::new());
            // Look input — TWO action entities (mouse + stick), because each device needs
            // different action-level scaling (radians/pixel vs radians/second×dt) and BEI
            // applies action-level modifiers to all of an action's bindings. The owning
            // client binds each per its marker (see `controls.rs`'s binding observers); the
            // server-side accumulator treats both identically (see `server::input`).
            context.spawn((Action::<RotateCamera>::new(), MouseLook));
            context.spawn((Action::<RotateCamera>::new(), StickLook));
        })),
    )
}
