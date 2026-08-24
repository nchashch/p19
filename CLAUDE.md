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

If something doesn't behave like upstream docs suggest, or a symbol can't be found, check the sibling checkout's source directly rather than assuming crates.io behavior — these may be patched/ahead-of/behind the published versions.

`bevy`, `avian3d` (physics), `bevy_skein`, `bevy_enhanced_input`, `bevy_asset_loader`, and `bevy_hanabi` (particles) are currently pulled from crates.io, but `Cargo.toml` keeps commented-out `path = "../..."` lines for all of these too — if a registry release is broken or lags a needed feature, swapping to the sibling checkout is the established escape hatch. Check `Cargo.toml` before assuming a dependency's behavior matches its published docs.

`assets/` is gitignored (large binary game assets — models, audio, fonts) despite being present on disk; don't expect `git log`/`git status` to track changes there.

## Architecture

The app is a `Plugin` (`Prototype19` in `main.rs`) composing feature plugins. Game flow is driven by a top-level state machine (`game_state::GameState`: `MainMenu -> Loading -> InGame`), and most gameplay/UI entities are scoped to a state via `DespawnOnEnter`/`DespawnOnExit` so they self-clean on transitions instead of needing manual teardown systems.

Key modules:

- **`character_controller.rs`** — generic kinematic character controller built on Avian3D's `MoveAndSlide` primitive (custom, not Avian's built-in character controller). Handles grounding via shape-casting, gravity, movement damping, and slope climb/slip logic via velocity decomposition against the hit normal. This is reusable/data-driven (`CharacterMovementSettings`, `GroundDetection` are components), not player-specific — `player_character.rs` configures it for the player entity.
- **`player_character.rs`** — the player entity: spawns it with the character controller, sets up all `bevy_enhanced_input` actions/bindings (WASD/gamepad movement, jump, mouse-look, select/deselect, attack, spawn/despawn cube, respawn, main menu), and owns interaction state as resources (`Hovered`, `Selected`) populated by a center-screen/cursor raycast against `Selectable` entities. Combat (`attack`) and cube lifecycle (spawn/despawn/despawn-on-zero-hp) also live here.
- **`fps_controller.rs`** — first-person camera look (pitch/yaw from mouse/stick input), gated by the `DisableFpsCameraControl` resource (toggled while right-mouse camera-rotate is or isn't held, and when UI/menu should own the cursor).
- **`cube_spawner.rs`** — spawns physics-driven "Cube" props with hit points and outline components (via `bevy_mod_outline`); this is currently the game's stand-in for enemies/targets.
- **`particles.rs`** — `bevy_hanabi` particle effect setup (used for cube-destruction VFX).
- **`ui.rs`** — UI built with Bevy's `bsn!`/`Scene`/`SceneList` macros (scene-graph-as-data DSL), not manual widget spawning. Menu, HUD panels (controls list, console/debug, player stats, selected-target frame) are declared as composable `Scene` functions (`panel`, `button`, etc.) and spawned per-`GameState` via `SceneList`.
- **`fps_camera.rs`** — currently a near-empty stub (duplicate `FpsCamera` marker unrelated to the real one in `fps_controller.rs`); not the file to look in for camera logic.
- **`main.rs`** — plugin wiring, level (`Level.glb`) and player-rig (`rig.glb`) glTF loading/animation-graph setup. Several functions here (`load_rigs`, `load_meshes`, `locomotion`, and the `DragonAction`/`BirdAction`/`HumanoidAction`/`QuadrupedAction`/`Rigs` types) are unimplemented (`todo!()`) scaffolding for a not-yet-built multi-rig animation system — don't assume they're wired up to anything yet.

## Notable conventions

- Interaction/selection targeting uses a raycast against `Selectable`-tagged entities each frame (`raycast_from_center`), writing into the `Hovered`/`Selected` resources that both gameplay (`attack`, `despawn_cube`) and UI (`ui.rs`) read from — this is the hub to touch when changing targeting behavior.
- Input is entirely `bevy_enhanced_input`: actions are marker structs (`#[derive(InputAction)]`), bound to keys/gamepad/mouse in `player_character.rs`'s `Actions::<PlayerCharacter>::spawn` block, and handled via observers (`On<Fire<T>>`, `On<Start<T>>`, `On<Complete<T>>`) rather than polling `ButtonInput` directly.
- Cross-cutting gameplay events (`RespawnPlayer`, `SpawnCube`, etc.) are Bevy observers/triggers (`commands.trigger(...)`), not plain systems reading queues.
