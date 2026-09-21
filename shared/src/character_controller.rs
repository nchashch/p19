use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A plugin that implements a basic platformer kinematic character controller using move-and-slide,
/// with support for ground detection and configurable movement settings.
///
/// Driven entirely by [`MovementInput`]/[`JumpInput`] events rather than any input library —
/// anything can trigger them for any entity with [`CharacterController`]: `client`'s
/// `player_character.rs` translates `bevy_enhanced_input` events into them for the player, and an
/// AI system could trigger the same events for an NPC to reuse this exact controller.
///
/// **Currently gutted**: every system/observer body below is a `todo!()` stub — the previous
/// move-and-slide/ground-detection/gravity/damping implementation (originally lifted from an
/// avian3d example, see git history) has been intentionally removed to make way for a rewrite
/// using lightyear's own client-side prediction (`lightyear_inputs_*`/`PredictionPlugin`, neither
/// of which this project uses yet — see `client/src/main.rs`'s note on why `PredictionPlugin` is
/// currently disabled). Every component/event type below is unchanged and still replicated/
/// triggered exactly as before; only the systems that acted on them are stubbed.
pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_movement_input);
        app.add_observer(on_jump_input);
        // Run movement logic in `FixedUpdate` to ensure consistent behavior regardless of frame rate.
        app.add_systems(
            FixedUpdate,
            (
                update_grounded,
                apply_gravity,
                integrate_horizontal_linear_velocity,
                apply_movement_damping,
                move_and_slide,
                apply_forces_to_dynamic_bodies,
            )
                .chain(),
        );
    }
}

/// Request to set `entity`'s desired horizontal movement direction (world-space, XZ plane — only
/// direction matters, magnitude is discarded by `normalize_or_zero` during integration). A zero
/// vector means "stop."
#[derive(EntityEvent)]
pub struct MovementInput {
    pub entity: Entity,
    pub direction: Vec3,
}

/// Request for `entity` to jump — a no-op unless it currently has [`Grounded`].
#[derive(EntityEvent)]
pub struct JumpInput {
    pub entity: Entity,
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to set `DesiredMotion` from the input event.
fn on_movement_input(_input: On<MovementInput>, _controllers: Query<&mut DesiredMotion>) {
    todo!("character controller rewrite: set DesiredMotion from MovementInput")
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to set vertical `LinearVelocity` from `CharacterMovementSettings::jump_impulse`
/// if the entity was `Grounded`.
fn on_jump_input(
    _input: On<JumpInput>,
    _controllers: Query<(
        &CharacterMovementSettings,
        &mut LinearVelocity,
        Has<Grounded>,
    )>,
) {
    todo!("character controller rewrite: apply jump impulse if Grounded")
}

/// A marker component indicating that an entity is using a character controller.
///
/// This also requires the entity to have a `CustomPositionIntegration` component, which is used
/// to prevent Avian from automatically applying the character's velocity to its position,
/// since the character controller will handle movement manually using move-and-slide.
#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
#[require(
    RigidBody::Kinematic,
    CustomPositionIntegration,
    // We don't want to impart speculative collision impulses in this case
    SpeculativeMargin(0.0),
    CharacterCollisions,
    DesiredMotion,
)]
pub struct CharacterController;

/// Marks an entity as a character driven by this controller — the player and NPCs alike.
#[derive(Component, Reflect, Default, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Character;

/// Physics layers for [`CollisionLayers`], so the player capsule and NPC bodies can be told not
/// to physically collide with each other while each still collides normally with level geometry
/// and props (which stay on the implicit `Default` layer, since nothing else in this codebase
/// uses `CollisionLayers` yet).
#[derive(PhysicsLayer, Default, Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
pub enum GameLayer {
    #[default]
    Default,
    Player,
    Npc,
}

/// Moving or standing still — not performing any kind of action (attack, hurt, etc). Removed
/// while a one-off animation/action plays and re-inserted once it finishes; systems that drive
/// locomotion (walk/idle/jump) only act while this is present.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
#[component(storage = "SparseSet")]
pub struct Idle;

/// Component for configuring movement settings for a character controller.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CharacterMovementSettings {
    /// The acceleration used for character movement.
    pub acceleration: Scalar,
    /// The damping coefficient used for slowing down movement.
    pub damping: Scalar,
    /// The strength of a jump.
    pub jump_impulse: Scalar,
    /// The gravitational acceleration used for the character.
    pub gravity: Vector,
    /// The maximum speed that gravity can accelerate the character to.
    /// This prevents the character from accelerating indefinitely while falling.
    pub terminal_velocity: Scalar,
}

impl Default for CharacterMovementSettings {
    fn default() -> Self {
        Self {
            acceleration: 100.0,
            damping: 10.0,
            jump_impulse: 7.0,
            gravity: Vector::new(0.0, -9.81 * 2.0, 0.0),
            terminal_velocity: 50.0,
        }
    }
}

/// Component for configuring ground detection for a character controller.
#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct GroundDetection {
    /// The maximum angle (in radians) where a surface is considered ground/ceiling
    /// relative to the up-direction. Outside of this angle, surfaces are considered walls.
    ///
    /// **Default**: 30 degrees (π / 6 radians)
    pub max_angle: Scalar,
    /// The maximum distance for ground detection.
    pub max_distance: Scalar,
    /// The shape cast collider used for ground detection.
    pub cast_shape: Option<ColliderConstructor>,
}

impl Default for GroundDetection {
    fn default() -> Self {
        Self {
            max_angle: PI / 6.0,
            max_distance: 0.2,
            cast_shape: None,
        }
    }
}

/// A marker component indicating that an entity is on a surface that is considered
/// ground, meaning the steepness is less than [`GroundDetection::max_angle`].
///
/// Characters that are grounded can jump, and do not slide down slopes.
///
/// Replicated (see `shared::replication::SharedReplicationPlugin`) rather than computed
/// client-side — the server's `update_grounded` (run as part of `CharacterControllerPlugin`,
/// server-only) is the single source of truth, same as the rest of physics/movement resolution.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// A component containing information about the current collisions for a character controller.
///
/// This is used to apply forces to dynamic rigid bodies hit by the character.
#[derive(Component, Default, Deref)]
pub struct CharacterCollisions(Vec<CharacterCollision>);

/// Information about a collision between a character controller and another collider.
pub struct CharacterCollision {
    /// The collider that was hit by the character.
    pub collider: Entity,
    /// The point of contact in world space.
    pub point: Vector,
    /// The normal of the contact surface, pointing away from the character.
    pub normal: Dir3,
    /// The velocity of the character at the point of contact.
    pub character_velocity: Vector,
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to shapecast downward from each `GroundDetection` entity and insert/remove
/// [`Grounded`] based on whether the hit surface's angle is within `max_angle` of up.
fn update_grounded(
    _commands: Commands,
    _query: Query<(
        Entity,
        &GroundDetection,
        &GlobalTransform,
        Option<&CollisionLayers>,
    )>,
    _spatial_query: SpatialQuery,
) {
    todo!("character controller rewrite: shapecast-based ground detection")
}

#[derive(Component, Clone, Default, Debug, Serialize, Deserialize)]
pub struct DesiredMotion(pub Vec3);

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to integrate `DesiredMotion * CharacterMovementSettings::acceleration` into
/// horizontal `LinearVelocity`.
fn integrate_horizontal_linear_velocity(
    _time: Res<Time>,
    _controllers: Query<(
        &CharacterMovementSettings,
        &mut LinearVelocity,
        &mut DesiredMotion,
    )>,
) {
    todo!("character controller rewrite: integrate DesiredMotion into horizontal LinearVelocity")
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to integrate gravity into `LinearVelocity` with a terminal-velocity clamp.
fn apply_gravity(
    _time: Res<Time>,
    _controllers: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
) {
    todo!("character controller rewrite: gravity integration with terminal-velocity clamp")
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to damp X/Z `LinearVelocity` (leaving Y/gravity untouched).
fn apply_movement_damping(
    _query: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
    _time: Res<Time>,
) {
    todo!("character controller rewrite: damp horizontal LinearVelocity")
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to perform avian3d `MoveAndSlide`-based movement/collision resolution
/// (slope/ceiling/climb/slip handling), updating `Transform`/`LinearVelocity`/`CharacterCollisions`.
///
/// For simplicity, we assume that the character is not a child entity,
/// and its collider is on the same entity as the `CharacterController`.
fn move_and_slide(
    _query: Query<
        (
            Entity,
            Option<&GroundDetection>,
            Option<&mut CharacterCollisions>,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
            Option<&CollisionLayers>,
        ),
        With<CharacterController>,
    >,
    _move_and_slide: MoveAndSlide,
    _time: Res<Time>,
) {
    todo!("character controller rewrite: move-and-slide collision resolution")
}

/// Gutted pending the lightyear-idiomatic prediction rewrite — see `CharacterControllerPlugin`'s
/// doc comment. Used to apply linear impulses to dynamic rigid bodies the character touched
/// (recorded in `CharacterCollisions` by `move_and_slide`), so characters push dynamic props.
fn apply_forces_to_dynamic_bodies(
    _characters: Query<(&ComputedMass, &CharacterCollisions)>,
    _colliders: Query<&ColliderOf>,
    _rigid_bodies: Query<(&RigidBody, Forces)>,
) {
    todo!("character controller rewrite: push dynamic bodies the character collides with")
}
