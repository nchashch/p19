use avian3d::prelude::*;
use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};
use bevy::feathers::{dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_replicon_quinnet::RepliconQuinnetPlugins;
use bevy_seedling::prelude::*;
use bevy_skein::SkeinPlugin;
use shared::replication::SharedReplicationPlugin;

use console::PConsolePlugin;
use cube_spawner::CubeSpawnerPlugin;
use fps_controller::FpsControllerPlugin;
use game_state::{GameState, GameStatePlugin};
use particles::ParticleEffectsPlugin;
use player_character::PlayerCharacterPlugin;

use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use openxr::EnvironmentBlendMode;

mod actions;
mod animation;
mod camera;
mod combat;
mod console;
mod controls;
mod cube_spawner;
mod events;
mod fps_controller;
mod game_state;
mod hud;
mod input_icons;
mod loading;
mod localization;
mod nameplate;
mod networking;
mod npc_spawner;
mod npc_ui_quad;
mod particles;
mod player_character;
mod targeting;
mod ui;
mod vr_controllers;
mod widgets;

/// Registers each `$observer` with `$app`, gated behind `$condition` (e.g.
/// `chill_bevy_console::console_closed`, to suppress gameplay observers while the
/// dev console is open).
macro_rules! add_observers_run_if {
    ($app:expr, $condition:expr, $($observer:expr),+ $(,)?) => {
        $( $app.add_observer($observer.run_if($condition)); )+
    };
}
pub(crate) use add_observers_run_if;

use crate::{
    animation::PAnimationPlugin, loading::LoadingPlugin, localization::LocalizationPlugin,
    nameplate::NameplatePlugin, npc_spawner::NpcSpawnerPlugin, npc_ui_quad::NpcUiQuadPlugin,
    vr_controllers::VrControllersPlugin,
};

fn main() {
    App::new().add_plugins(Prototype19).run();
}

struct Prototype19;

impl Plugin for Prototype19 {
    fn build(&self, app: &mut App) {
        // Decided *before* anything else below — which plugin group even gets added is a
        // one-time choice at build time, long before any `Startup` system (including
        // `networking::load_client_config`, which reads the *rest* of `config.toml` the normal
        // way, via the `AssetServer`) could run. See `is_vr_enabled_presync`'s doc comment for why
        // this can't just reuse that later, `AssetServer`-based path.
        let vr_enabled = networking::is_vr_enabled_presync();

        if vr_enabled {
            app.add_plugins(add_xr_plugins(
                DefaultPlugins.build().disable::<PipelinedRenderingPlugin>(),
            ));
        } else {
            app.add_plugins(DefaultPlugins);
        }

        app.add_plugins((
            bevy::feathers::FeathersPlugins,
            PAnimationPlugin,
            PConsolePlugin,
            LoadingPlugin,
            LocalizationPlugin,
            SeedlingPlugins,
            ParticleEffectsPlugin,
            SkeinPlugin::default(),
            // The server is authoritative for physics — the client only needs colliders and
            // spatial queries (`SpatialQuery` raycasts/shapecasts, e.g. `targeting.rs`'s hover
            // raycast and `character_controller.rs`'s grounding shape-casts), not to actually
            // simulate anything itself. Disabling just the solver-related plugins (as opposed to
            // e.g. `Time::<Physics>::pause()`, which would also stop broad/narrow-phase from
            // running and leave spatial queries stale against moving colliders) keeps collision
            // detection and `Position`/`Rotation` <-> `Transform` sync running every frame, while
            // nothing is left to apply forces or resolve contacts locally. One visible
            // consequence: `RigidBody::Dynamic` cubes/NPCs no longer predict their own motion
            // between replication ticks — they only move when a new server `Transform` arrives —
            // unlike `PlayerCharacter` movement, which stays smooth since it's driven directly by
            // `character_controller.rs`'s kinematic move-and-slide, not the solver.
            //
            // Deliberately *not* also disabling `SolverBodyPlugin`/`IslandPlugin`/
            // `IslandSleepingPlugin`, despite the "solver" naming — confirmed by testing, not
            // theory: disabling `SolverBodyPlugin` crashed `update_moved_collider_aabbs` with an
            // index-out-of-bounds panic, because the collider tree that spatial queries depend on
            // indexes into a `SolverBody`-tracked slot for every awake dynamic/kinematic body —
            // it's shared per-body bookkeeping the broad-phase relies on, not solving itself.
            // `IntegratorPlugin`/`SolverPlugin`/`CcdPlugin` are the actual force/contact/sweep
            // resolution steps, and disabling only those was enough to stop local motion
            // prediction without touching that bookkeeping. Also not disabling
            // `JointPlugin`/`JointGraphPlugin<_>` — nothing in this project ever spawns a joint,
            // so those stay registered but inert (no joint entities for them to act on) rather
            // than needing their own disable calls.
            PhysicsPlugins::default()
                .build()
                .disable::<IntegratorPlugin>()
                .disable::<SolverPlugin>()
                .disable::<CcdPlugin>()
                // `XpbdSolverPlugin` (from the `xpbd_joints` feature, on by default) has its own
                // joint-motor warm-start systems that unconditionally read the `SolverConfig`
                // resource `SolverPlugin` normally provides — confirmed by testing: without also
                // disabling this, startup panicked with "Resource does not exist: SolverConfig"
                // even with zero joints ever spawned. Safe to disable outright for the same reason
                // `JointPlugin`/`JointGraphPlugin<_>` are left inert rather than needing their own
                // exception: nothing in this project ever spawns a joint.
                .disable::<XpbdSolverPlugin>(),
            bevy_replicon::prelude::RepliconPlugins,
            RepliconQuinnetPlugins,
            SharedReplicationPlugin,
            (
                CubeSpawnerPlugin,
                NpcSpawnerPlugin,
                NpcUiQuadPlugin,
                PlayerCharacterPlugin,
                FpsControllerPlugin,
                GameStatePlugin { vr_enabled },
                NameplatePlugin,
                ui::PrototypeUiPlugin,
                networking::NetworkingPlugin,
            ),
        ));

        // `PhysicsSchedulePlugin` (added above, inside `PhysicsPlugins`) configures
        // `PhysicsSchedule` with `ambiguity_detection: LogLevel::Error` — appropriate when the
        // full solver stack is present, since Avian's own plugins rely on each other's system
        // sets to establish a total order. With several of those solver plugins disabled above,
        // that ordering chain has gaps, and Bevy now reports ~50 systems as ambiguous relative to
        // each other (panicking at schedule-build time rather than just warning, because of the
        // `Error` level). Every ambiguity in that list is between joint-related systems (this
        // project never spawns a joint), collider-hierarchy systems (no compound/child colliders
        // here — colliders live directly on the root rigid-body entity), or `trigger_collision_events`
        // (nothing here reads Avian's `CollisionStarted`/`CollisionEnded` — `character_controller.rs`
        // tracks its own `CharacterCollisions` from shape-casts instead) — none of which this
        // project's systems actually race on. Relaxing back to `Warn` (Bevy's own schedule
        // default) accepts that, instead of manually chaining ~50 system pairs by hand.
        app.edit_schedule(PhysicsSchedule, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Warn,
                ..default()
            });
        });

        // `IntegratorPlugin` (disabled above) is normally what initializes the `Gravity`
        // resource — confirmed by testing: without this, `IslandSleepingPlugin`'s
        // `resource_changed::<Gravity>` run condition (checking whether to re-evaluate sleeping
        // thresholds) panicked at startup with "Resource does not exist: Gravity", even though
        // nothing here actually reads `Gravity` for real integration anymore. `Gravity` is just
        // inert data (a `Vector` newtype with a `Default`), not tied to any system of its own, so
        // this is a safe, minimal stand-in rather than re-enabling `IntegratorPlugin` itself.
        app.init_resource::<Gravity>();

        // `OxrSessionConfig`/`HandGizmosPlugin`/`VrControllersPlugin` are all meaningless (and, for
        // `VrControllersPlugin`, actively broken — its `Startup`/`On<PlayerSpawned>` systems use
        // `Single<Entity, With<XrTrackingRoot>>`, an entity that only exists once the XR plugins
        // above actually ran, so a bare `Single` query on it panics under plain `DefaultPlugins`)
        // without an active XR session, so they're only added when `vr_enabled` is true, not just
        // toggled off via a `run_if` inside them.
        if vr_enabled {
            app.insert_resource(OxrSessionConfig {
                blend_mode_preference: vec![EnvironmentBlendMode::OPAQUE],
                ..default()
            })
            .add_plugins((
                bevy_mod_xr::hand_debug_gizmos::HandGizmosPlugin,
                VrControllersPlugin,
            ));
        }

        app.insert_resource(UiTheme(create_dark_theme()))
            .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 100.,
                ..default()
            })
            .register_type::<ColliderConstructor>()
            .add_systems(OnEnter(GameState::MainMenu), ui::main_menu_scene.spawn());
    }
}
