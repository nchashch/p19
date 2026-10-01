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

/// Adjective vocabulary for [`generate_player_name`]'s first word.
pub const NAME_ADJECTIVES: &[&str] = &[
    "Amber", "Bold", "Brisk", "Calm", "Clever", "Crimson", "Daring", "Eager", "Fierce",
    "Gentle", "Gilded", "Grim", "Humble", "Ivory", "Jolly", "Keen", "Lucky", "Mellow",
    "Nimble", "Noble", "Onyx", "Pale", "Quiet", "Rapid", "Rusty", "Sable", "Shrewd",
    "Silent", "Swift", "Tender", "Tidy", "Umber", "Valiant", "Vast", "Wary", "Wild",
];

/// Noun vocabulary for [`generate_player_name`]'s second word.
pub const NAME_NOUNS: &[&str] = &[
    "Ash", "Badger", "Birch", "Blade", "Bloom", "Boar", "Bramble", "Brook", "Comet",
    "Crag", "Crow", "Dart", "Ember", "Falcon", "Fjord", "Flint", "Frost", "Gale",
    "Heron", "Hollow", "Kite", "Lantern", "Maple", "Marrow", "Moss", "Otter", "Pine",
    "Raven", "Reed", "Shale", "Sparrow", "Stone", "Thistle", "Thorn", "Viper", "Willow",
];

/// Generates a unique, human-readable player name: two vocabulary words (seeded from
/// [`NoiseRng`] — tick-XOR-connection-bits at the call site, the same replay-determinism
/// convention as `spawn.rs`'s RNG), with a `#N` suffix appended on collision with any name in
/// `taken` (the caller passes every existing player's `Name`): `"Brisk Falcon"`, then
/// `"Brisk Falcon #2"`, `"Brisk Falcon #3"`, …
pub fn generate_player_name(seed: u32, taken: &std::collections::HashSet<String>) -> String {
    use noiz::prelude::Noise;
    use noiz::rng::AnyValueFromBits;
    let rng = noiz::rng::NoiseRng(seed);
    let adjectives = NAME_ADJECTIVES;
    let nouns = NAME_NOUNS;
    let base = format!(
        "{} {}",
        adjectives[rng.rand_u32(0) as usize % adjectives.len()],
        nouns[rng.rand_u32(1) as usize % nouns.len()],
    );
    if !taken.contains(&base) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base} #{suffix}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(base: &str, count: u32) -> std::collections::HashSet<String> {
        (0..count)
            .map(|i| {
                if i == 0 {
                    base.to_string()
                } else {
                    format!("{base} #{}", i + 1)
                }
            })
            .collect()
    }

    #[test]
    fn generated_name_has_two_words() {
        let name = generate_player_name(7, &Default::default());
        assert_eq!(name.split_whitespace().count(), 2);
    }

    #[test]
    fn colliding_base_gets_suffix_two() {
        let taken = names(&generate_player_name(7, &Default::default()), 1);
        let name = generate_player_name(7, &taken);
        assert!(name.ends_with(" #2"));
    }

    #[test]
    fn suffix_increments_past_existing_suffixes() {
        let base = generate_player_name(7, &Default::default());
        let taken = names(&base, 3); // base, #2, #3 all taken
        let name = generate_player_name(7, &taken);
        assert_eq!(name, format!("{base} #4"));
    }

    #[test]
    fn different_seeds_tend_to_differ() {
        let a = generate_player_name(1, &Default::default());
        let b = generate_player_name(2, &Default::default());
        assert_ne!(a, b);
    }
}

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
