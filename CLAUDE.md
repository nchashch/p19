# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Bevy 0.19 (Rust, edition 2024) 3D game prototype. Single binary crate (`prototype_19`). No workspace, no tests currently exist in the repo.

## Commands

```sh
cargo build          # debug build
cargo build --release
cargo run
cargo check          # fastest way to verify compile errors
cargo clippy
```

There is no test suite (`cargo test` has nothing to run). Builds are slow the first time — `bevy`, `avian3d`, and friends are heavy dependency trees; `target/` is already populated from prior builds so incremental builds are what you'll normally see.

`../build.sh` (one level up, outside this crate) cross-builds a release binary inside a `steamrt-sniper` podman container for Steam Runtime compatibility — only relevant for release packaging, not day-to-day dev.

## Dependency layout — local sibling crates

Several dependencies are **not** pulled from crates.io — they're path-dependencies on sibling checkouts one directory up (`../<crate>`), i.e. forked/vendored versions of these libraries:

- `polyanya` (`../polyanya`) — navmesh pathfinding
- `rerecast` (`../rerecast/crates/rerecast`) — navmesh generation
- `bevy_mod_outline` (`../bevy_mod_outline`) — selection/hover outlines
- `bevy_vello` (`../bevy_vello`) — vector graphics rendering
- `bevy_seedling` (`../bevy_seedling`) — audio
- `audionimbus` (`../audionimbus/audionimbus`) — spatial audio

If something doesn't behave like upstream docs suggest, or a symbol can't be found, check the sibling checkout's source directly rather than assuming crates.io behavior — these may be patched/ahead-of/behind the published versions. `bevy_mod_outline` in particular carries a local fix: `pipeline_key.rs`'s `DerivedPipelineKey::new` used to force `motion_vector_prepass` to `false` for the `Stencil`/`FloodInit` pass types regardless of the view's actual state, which crashed (wgpu bind-group-layout validation error) when outlining a skinned mesh with `TemporalAntiAliasing` enabled — the mesh's shared bind group is built as the motion-vector variant (because the main pass needs it), but those two outline pass types requested the plain variant. Fixed by letting them inherit the view's real flag like the `Volume` pass type already did. Worth upstreaming; if this crate is ever re-vendored from upstream, check whether that fix needs reapplying.

`bevy`, `avian3d` (physics), `bevy_skein`, `bevy_enhanced_input`, `bevy_asset_loader`, and `bevy_hanabi` (particles) are currently pulled from crates.io, but `Cargo.toml` keeps commented-out `path = "../..."` lines for all of these too — if a registry release is broken or lags a needed feature, swapping to the sibling checkout is the established escape hatch. Check `Cargo.toml` before assuming a dependency's behavior matches its published docs.

`chill_bevy_console` (crates.io) provides the in-game dev console (backtick to toggle) — see the console-gating convention below; it shapes how observers are registered throughout the codebase.

`assets/` is gitignored (large binary game assets — models, audio, fonts) despite being present on disk; don't expect `git log`/`git status` to track changes there.

## Architecture

`main.rs` is now just composition — a `Plugin` (`Prototype19`) listing feature plugins, plus the crate-root `add_observers_run_if!` macro (see conventions below). Game flow is driven by a top-level state machine (`game_state::GameState`: `MainMenu -> Loading -> InGame`), and most gameplay/UI entities are scoped to a state via `DespawnOnEnter`/`DespawnOnExit` so they self-clean on transitions instead of needing manual teardown systems.

Key modules:

- **`character_controller.rs`** — generic kinematic character controller built on Avian3D's `MoveAndSlide` primitive (custom, not Avian's built-in character controller). Owns the `FixedUpdate` physics-integration loop only: grounding via shape-casting, gravity, movement damping, and slope climb/slip logic via velocity decomposition against the hit normal. Reusable/data-driven (`CharacterMovementSettings`, `GroundDetection` are components). It does **not** own input response — turning player input into `DesiredMotion`/jump impulses is `player_character.rs`'s job (`on_jump`, `on_movement`, `on_movement_stop`).
- **`player_character.rs`** — the player entity: spawns it with the character controller, sets up all `bevy_enhanced_input` actions/bindings (WASD/gamepad movement, jump, mouse-look, respawn, spawn cube/NPC, main menu) in one `Actions::<PlayerCharacter>::spawn` block, and reacts to that input via observers. Composes `TargetingPlugin`/`CombatPlugin`/`CharacterControllerPlugin` rather than implementing targeting/combat itself. Also owns cursor-lock state (`unlock_cursor`, `lock_cursor_for_rotation`/`unlock_cursor_after_rotation` tied to holding RMB) and skybox/cubemap loading.
- **`targeting.rs`** — the hover/select hub: `Hovered`/`Selected` resources, `raycast_from_center` (cursor/center-screen raycast against `Selectable` entities), `select`/`deselect` observers, and the `bevy_mod_outline` integration that colors outlines based on hover/selection state.
- **`combat.rs`** — attack/damage (`attack`, `DAMAGE`/`ATTACK_RANGE` consts) and destruction (`despawn_cube`, `despawn_zero_hp`, both funneling through a shared `kill_entity` helper that plays the crunch sound + particle effect + despawn + clears `Selected` if needed).
- **`fps_controller.rs`** — first-person camera look (pitch/yaw from mouse/stick input), gated by the `DisableFpsCameraControl` resource. Also drives the visible `PlayerModel`'s yaw (not pitch, to avoid tilting the body) to track the camera.
- **`cube_spawner.rs`** — spawns physics-driven "Cube" props with hit points and outline components; the game's stand-in for destructible targets.
- **`npc_spawner.rs`** — spawns `Npc` entities: a `RigidBody::Dynamic` capsule with `LockedAxes` locking all rotation (prevents toppling) and the shared `rig.glb` model as a child. Selectable/outlinable/attackable like cubes.
- **`animation.rs`** — two unrelated things in one file:
  - A **working** single-clip system: loads `rig.glb`'s `Gltf` container once at `Startup`, builds one shared `AnimationGraph` plus a name→node `HashMap` (`Animations` resource) from `gltf.named_animations`, and a global observer (`bind_animation_player`, reacts to any `WorldInstanceReady`) plays the clip named by the `ANIMATION_NAME` const on loop via `AnimationTransitions::play(...).repeat()`. This fires for *any* entity whose `rig.glb` scene finishes loading — currently both NPCs and the player's own `PlayerModel`, since they share the same rig.
  - **Dead** multi-rig scaffolding below it (`DragonAction`/`BirdAction`/`HumanoidAction`/`QuadrupedAction`/`Rigs`, `load_rigs`/`load_meshes`/`locomotion`, all `todo!()`) — speculative infrastructure for multiple distinct rig types with named per-type actions, not wired to anything. Don't confuse it with the working system above it, and don't assume it's reachable.
- **`loading.rs`** — level (`Level.glb`) loading and the `Loading -> InGame` transition (`wait_for_level` polls `is_loaded_with_dependencies`).
- **`console.rs`** — wraps `chill_bevy_console::ChillConsole` plus `avian3d::PhysicsDebugPlugin` and Bevy's `FpsOverlayPlugin`, both added permanently but disabled by default and toggled via console commands (`fps`, `physics_debug`) that flip runtime config resources — `FpsOverlayConfig.enabled`/`.frame_time_graph_config.enabled` (the frame-time graph has its own independent `enabled` flag, separate from the text overlay's), and `GizmoConfigStore::config_mut::<PhysicsGizmos>().0.enabled` — rather than adding/removing the plugins themselves (plugins can't be toggled after `App::run()`). Also registers `respawn`, `despawn_cubes`, `despawn_npcs` commands.
- **`ui.rs`** — UI built with Bevy's `bsn!`/`Scene`/`SceneList` macros (scene-graph-as-data DSL), not manual widget spawning. Menu, HUD panels (controls list, player stats, selected-target frame) are declared as composable `Scene` functions (`panel`, `button`, etc.) and spawned per-`GameState` via `SceneList`.
- **`particles.rs`** — `bevy_hanabi` particle effect setup (used for destruction VFX).

## Notable conventions

- Interaction/selection targeting uses a raycast against `Selectable`-tagged entities each frame (`targeting::raycast_from_center`), writing into the `Hovered`/`Selected` resources that gameplay (`combat.rs`), UI (`ui.rs`), and the outline system all read from — this is the hub to touch when changing targeting behavior.
- Input is entirely `bevy_enhanced_input`: actions are marker structs (`#[derive(InputAction)]`), defined near whichever module owns their handler (e.g. `Select`/`Deselect` in `targeting.rs`, `Attack`/`DespawnCube` in `combat.rs`) but all bound together in `player_character.rs`'s single `Actions::<PlayerCharacter>::spawn` block, and handled via observers (`On<Fire<T>>`, `On<Start<T>>`, `On<Complete<T>>`) rather than polling `ButtonInput` directly.
- Cross-cutting gameplay events (`RespawnPlayer`, `SpawnCube`, `SpawnNpc`, etc.) are Bevy observers/triggers (`commands.trigger(...)`), not plain systems reading queues.
- **Console-gating**: gameplay observers that shouldn't fire while the dev console is open are registered through the crate-root `add_observers_run_if!` macro (`main.rs`, `pub(crate) use`d so every module can call it) with `chill_bevy_console::console_closed` as the condition — see its use in `player_character.rs`, `targeting.rs`, `combat.rs`. The important exception: observers that are lifecycle/cleanup rather than moment-to-moment input (`respawn_player`, `unlock_cursor_after_rotation`, `on_movement_stop`) are registered as plain, ungated `app.add_observer(...)`. This isn't arbitrary — gating `respawn_player` behind `console_closed` was a real bug once: opening the console in the main menu and then transitioning to `InGame` silently dropped the one-shot spawn trigger forever (observer triggers aren't queued/retried), leaving no player and no camera. Lifecycle/spawn handlers must stay ungated; only the player-facing "please do X now" input actions should be gated.
