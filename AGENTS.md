# AGENTS.md

Guidance for AI coding agents working in this repository. This file describes the **current state**
of the code only. History — what was tried, what broke, how it was fixed — lives in
`docs/agents/adr/` (decisions), `docs/agents/bug_reports/` (defects) and `docs/agents/playtests/`
(agent playtest runs). When the code changes, update this file to match; do not append changelog
entries here.

## Rules

- **`docs/humans/` is human-only. Agents must never create, edit, move, rename or delete anything
  in `docs/humans/`.** Read it if useful; if it looks wrong or stale, tell the user instead of
  changing it.
- Agent-facing documentation lives under `docs/agents/` and is Markdown (no typst).
- Read the relevant skill in `docs/agents/skills/` **before** doing its task:
  - `playtest.md` — driving the game headlessly through the agent tool API (launch recipe,
    `game/*` methods, input mocking, screenshots, failure modes) and writing playtest reports in
    `docs/agents/playtests/`.
  - `bugreport.md` — filing numbered bug reports in `docs/agents/bug_reports/` (one bug per file,
    reproduce first, never delete, `Fixed in <commit>` status). File defects you discover there.
  - `adr.md` — writing Architecture Decision Records in `docs/agents/adr/` (from 0014 onward).
- Consult the latest playtest report before writing a new one.
- When a fix changes behavior described here, update this file in the same change.

## What this is

A Bevy 0.19 (Rust, edition 2024) 3D multiplayer game prototype, built "always multiplayer": even
singleplayer runs a local client and server. Cargo workspace with three members:

- **`client`** — rendering, UI, input, camera, presentation. Sends intent as network messages and
  renders what the server replicates; never decides outcomes.
- **`server`** — headless authoritative simulation: level loading (real `.glb` + Avian colliders,
  no GPU), player spawning, movement, combat, spawning cubes/NPCs.
- **`shared`** — what both sides must agree on: replication registration, message types, the
  player bundle, spawn bundles, shared data components, game states.

Status: pre-release, not playable end to end. Steam Deck is the primary / minimum-spec target (see
"Platform targets").

Key facts to internalize:

- **Networking is `lightyear` 0.30** over UDP/netcode. `shared::replication::SharedReplicationPlugin`
  is the single place both binaries register replicated components (`app.component::<T>().replicate()`)
  and messages (`app.register_message::<T>().add_direction(...)`, plus `.add_map_entities()` for any
  message carrying an `Entity`). All messages use the one `OrderedReliable` channel.
- **The player character is a separate entity from the client's connection entity.** The server
  spawns it as a `ChildOf` child of `InGameRoot`, owned via `ControlledBy { owner: <connection>, .. }`.
  Server-side, find a connection's player with `networking::owned_players`; client-side, the local
  player is the entity with lightyear's `Controlled` (stored in the `LocalPlayer` resource).
- **Room-based interest management**: two persistent rooms (`GameRoom`, `LobbyRoom`) and two
  persistent roots (`InGameRoot`, `LobbyRoot`). A client receives only entities in a room its
  connection is a member of.
- **Movement is server-authoritative and client-predicted** with `bevy_ahoy`'s kinematic character
  controller running on both binaries over lightyear-replicated `bevy_enhanced_input` (BEI) action
  state; lightyear rollback reconciles the prediction. Remote bodies are interpolated.

## Known gaps

Current, confirmed gaps. Don't assume these work.

- **Disconnected players are never cleaned up.** Player characters are spawned with
  `ControlledBy { lifetime: Lifetime::Persistent }`, so they persist after disconnect. A new client
  then receives the leftover character's `ClientInGame`, enters `GameState::InGame`, but never gets
  `Controlled`/`LocalPlayer` (no `position` in `game/state`). Only a server restart recovers.
- **All players spawn at the same point.** A joiner whose capsule overlaps an idle player is
  stacked on top of them (~2.73 high, `grounded: true`). Needs per-player spawn offsets.
- **No level switching.** `LevelState` rejects any `LoadLevelRequest` once a level is
  `Loading`/`LevelLoaded`; the server never unloads a level. Restart the server to change levels.
- **`levels/minimal.level.ron` is the only level** and renders black: its content has no lights.
- **No NPC/AI input path.** `ActionMock` does not drive actions on this server (lightyear's
  `get_action_state` writes `ActionState` directly from the replicated buffer). Triggering
  `Fire<A>` events directly works (that is how `server::replay` injects input) and is the likely
  path for AI. Combat is likewise reachable only from a client `AttackAttempt`/`KillAttempt`.
- **Replay movement rate mismatch**: `server --replay` reproduces sessions deterministically but
  replayed movement covers far less distance than live (~1 vs ~12.7 units over 60 ticks). Leading
  suspect: the injected `Fire<A>` events' `fired_secs`/`elapsed_secs` are hardcoded to `0.0`. See
  `server/src/replay.rs`'s module doc.
- **KCC has no ground friction**: with zero wish velocity, ahoy's `ground_accelerate` leaves
  velocity untouched, so a character coasts indefinitely once input stops.
- **Join-burst input corrections**: when a second client joins, the replication burst can push the
  existing client's inputs one tick late for ~10 ticks (`server_late_input_mismatch` errors).
  Benign and self-healing; deliberately not fixed.
- **VR**: `bevy_xr_utils` 0.6.0 (crates.io) has `suggest_action_bindings` commented out in
  `tracking_utils.rs`, so controller grip poses likely never track (both stay at identity). Not
  re-verified on a headset. VR locomotion mocks the replicated actions with `ActionMock` each frame,
  which overrides keyboard input while in VR.
- **Headless (`--mcp`) cannot activate `FeathersButton`s with literal Enter**:
  `bevy_input_focus::dispatch_focused_input` requires a `PrimaryWindow`, which `--mcp` never
  creates. Mouse clicks (`game/mouse`) and gamepad South work headlessly; windowed clients are
  unaffected.
- **Netcode posture is dev-grade**: TOFU-pinned self-signed TLS for the token endpoint; production
  needs a CA-signed certificate on a real backend.
- **Dead or unused code**: `GameState::Loading`/`Paused` (never entered), `lifecycle/loading.rs`'s
  `clear_effects` (never called), `client/src/assets/level.rs` and `shared/src/server_state.rs`
  (empty; the latter isn't even declared), `PreloadCollection` (never loaded), `client/src/ui/framework.rs`
  (unwired 9-slice button demo), `Character` registered for replication twice, the
  `vleue_navigator` workspace dependency (no member uses it). The main menu's Options rows and
  Credits button are stubs that log "not implemented yet".
- **Doc/code mismatches to distrust**: `shared::assets::level::Level`'s doc comment claims
  `model`/`skybox` are dependency-tracked handles — the loader just converts them to plain
  `AssetPath`s. A doc comment in `server/src/main.rs` claims the server has no `assets/` of its own
  (it does: `server/assets/`). `shared/src/server_events.rs` uses stale replicon terminology.
- **Tests**: only a handful of unit tests (`server::networking` token-address fallback,
  `shared::player` name generation). CI builds but runs no gameplay tests.

## Platform targets

Steam Deck is the min-spec target: ~4 GB VRAM. Higher-end PCs get fidelity (AA, shadow/texture
resolution, draw distance), never different content — what exists in the world must be identical
across hardware, since this is multiplayer.

- Assets use KTX2 textures and BC6H skyboxes (`scripts/hdri_to_skybox.py`, `glbpack.sh`), 3–4×
  smaller on disk and in VRAM.
- Intended scaling: a texture-quality setting resolves the same `#[asset(key = "...")]` keys to
  different source directories or mip-stripped variants.
- `PreloadCollection` is intended for proximity-based loading under the VRAM budget but is unused.
  There is no `PreloadBeacon` component (it appears only in a `TODO` comment).
- Steam Deck button glyphs (`input_icons.rs`, `hud.rs`'s `controls_tips`) and gamepad-first UI
  navigation (`ui.rs`'s `MenuControls`) are already in place.

## Character authoring pipeline (design, mostly unimplemented)

Goal: one data-driven spawning system for players, NPCs and AI enemies, built from independent
axes:

- **Rig** (`assets/rigs/*.glb`): bones, animations, Skein markers (a `RigRoot` component on the
  armature root; future attachment points, hit colliders, camera pivot). The visible mesh is meant
  to be a separate **skin** `.glb`; sharing the armature's pose needs the skin's
  `SkinnedMesh.joints` re-targeted to the instantiated bones by name.
- **Controller** (`assets::controller::Controller`, `.controller.ron`): movement-feel data.
  `RonAssetPlugin::<Controller>` is registered; nothing applies it.
- **Stats** (`assets::stats::Stats`, `.stats.ron`): RPG numbers meant to replace the global
  `DAMAGE`/`ATTACK_RANGE` constants for players and NPCs alike. Not registered as an asset.
- **Abilities**: no code yet; intended as a shared data-driven grammar extending `Gcd`.
- **`Character`** (`.character.ron`): aggregates `rig`/`controller`/`stats` as plain asset paths
  (no manifest indirection). Not registered; nothing resolves it.

`shared::player::player()` is independent of all of this.

## Commands

Run from the workspace root (`prototype_19/`).

```sh
cargo check --workspace           # fastest compile check
cargo clippy --workspace
cargo test --workspace            # the few unit tests
cargo build -p client --release
cargo build -p server --release
cargo run -p server --release     # listens on UDP 0.0.0.0:6000 (+ HTTPS token endpoint :6001)
cargo run -p client --release     # connects when the main menu's Connect button is pressed
```

- **Assets**: `client/assets/` and `server/assets/` are separate asset roots (Bevy resolves assets
  relative to each crate's `CARGO_MANIFEST_DIR`, so `client/assets/` must stay inside `client/`).
  `assets_src/` holds raw `.blend` files and source packs; nothing loads from it. Assets are not
  tracked in git.
- **Client config**: `client/assets/config.toml` (`server_ip`, default `127.0.0.1`; `vr`, default
  `false`). Missing fields fall back to defaults. There is no in-game UI for these.
- **Steam Deck builds**: always build inside the `steamrt4` toolbox with
  `scripts/steam_deck_toolbox.sh cargo build -p <client|server> --release`. A host build links the
  host's newer glibc and fails on the Deck (`GLIBC_2.4x not found`); since host and toolbox share
  the toolchain, cargo can't tell them apart — recover with `cargo clean -p <crate> --release`
  inside the toolbox. Check with
  `objdump -T target/release/<bin> | grep -oE 'GLIBC_[0-9.]+' | sort -Vu | tail -5` (should top
  out around `GLIBC_2.39`).
- `scripts/deploy_steam_deck.sh` cleans, builds the client in the toolbox, stages it with
  `client/assets/` in `steam_deck_staging/`, and checks the GLIBC baseline — ready for SteamOS
  Devkit Client's Title Upload (`Start Command` = `./client`).
- `../build.sh` (outside the workspace) builds in a `steamrt-sniper` podman container.
- **CI** (`.github/workflows/ci.yml`): "Build server" (host `ubuntu-latest`, debug) and "Build
  client (steamrt4, shippable)" (release, inside `registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk`,
  with a GLIBC check). Both free runner disk space first.
- **Tracy** is opt-in: `cargo run -p client --release --features tracy`. Only use it with a Tracy
  GUI attached — without one, `tracy-client` buffers every span in memory (RSS grows ~150 MB/s).
  `[profile.release] debug = true` exists for Tracy symbol names.

## Dependency layout

- The root `Cargo.toml` centralizes versions in `[workspace.dependencies]`; members use
  `dep.workspace = true`. A member cannot override `default-features` on an inherited dependency,
  so `server`/`shared` declare `bevy` directly with `default-features = false` (headless).
- **No sibling path dependencies.** Crates needing local patches are pinned git branches on the
  owner's forks; check the fork's source (`~/.cargo/git/checkouts/<crate>-*/`) when behavior
  differs from upstream docs:
  - `bevy_tui_texture` (`fix/fonts`): drops upstream's `.ttf` `AssetLoader`, which crashes
    `bevy_asset_loader`'s dynamic assets with a `TypeId` panic. Consequence: only
    `TuiFontSource::Ready` (embedded bytes) works; `TuiFontSource::Asset` doesn't.
  - `bevy_mod_outline` (`fix/skinned-motion-outline`): stencil/flood-init passes inherit the
    view's `motion_vector_prepass` flag (otherwise skinned meshes + TAA crash in wgpu validation).
  - `gltf` (`feat/khr_texture_basisu`, via `[patch.crates-io]`): adds `KHR_texture_basisu`.
- **lightyear** `0.30` features `avian3d`, `udp`, `netcode`, `input_bei`, `debug`, plus
  `lightyear_avian3d` and `lightyear_inputs_bei` (feature `client`/`server` per binary). The
  `input_bei` feature on the `lightyear` meta-crate is required — without it the client panics at
  connect with "Resource does not exist: LastConfirmedInput". `debug` enables structured
  `lightyear_debug::*` tracing; the server writes it as JSONL when `LIGHTYEAR_DEBUG_FILE` is set
  (its `LogPlugin` raises `lightyear_debug=trace`).
- **bevy_ahoy** 0.2 with `default-features = false` (avoids `avian_pickup`). The plugin group is
  `AhoyPlugins` (not `AhoyPlugin`), registered in both binaries. `InputPlugin` adds
  `EnhancedInputPlugin` itself, so `controls.rs` guards its own add with `is_plugin_added`.
- **bevy_common_assets** (`ron`, `toml`) is a generic data-file loader, separate from
  `bevy_asset_loader`'s collections. `Level` uses its own `LevelAssetLoader`, not
  `RonAssetPlugin`.
- `bevy`'s `system_font_discovery` lets `parley` fall back to a host CJK font (`ja-JP` locale).
- `chill_bevy_console` is the in-game dev console (backtick).
- Other crates.io deps of note: `avian3d` 0.7, `bevy_skein`, `bevy_enhanced_input` 0.26,
  `bevy_asset_loader`, `bevy_hanabi`, `bevy_vello`, `bevy_seedling`, `audionimbus`,
  `bevy_mod_openxr`/`bevy_mod_xr`/`bevy_xr_utils` 0.6, `rustls` (aws-lc-rs provider), `rcgen`.
- **Reflected `TypePath`s include the crate/module path.** Renaming a crate or moving a module
  that holds Skein-authored types breaks previously exported `.glb`s until they're re-exported.

## Architecture: client (`client/src/`)

`main.rs` is pure composition (the `Prototype19` plugin) plus the `add_observers_run_if!` macro.
Modules are grouped into `controls/`, `dev/`, `gameplay/`, `lifecycle/`, `presentation/`, `ui/`,
`assets/`.

### Startup decisions in `main.rs`

- **VR vs desktop** is chosen before the app is built, from `config.toml`'s `vr` (read with
  `std::fs`). Switching needs a restart.
- **Headless modes** are detected pre-sync from the CLI (`config::is_mcp_mode_presync`, etc.):
  `--mcp`, `--no-render` (implies `--mcp` + `--no-common-assets`), `--headless-render`,
  `--no-common-assets`. See "Agent tool API and headless modes".
- **Client physics simulation is disabled**: `IntegratorPlugin`, `SolverPlugin`, `CcdPlugin`,
  `XpbdSolverPlugin` are removed (not `Time::<Physics>::pause()`, which would also stop spatial
  queries). Keep `SolverBodyPlugin`/`IslandPlugin`/`IslandSleepingPlugin` (removing
  `SolverBodyPlugin` crashes the collider tree). `PhysicsSchedule` ambiguity detection is relaxed
  to `Warn`, and `Gravity` is `init_resource`'d manually.
- **Prediction is enabled**: lightyear's `PredictionPlugin` runs as part of `ClientPlugins`.
- Both binaries use `LightyearAvianPlugin { replication_mode: AvianReplicationMode::Position {
  sync_to_transform: true } }` — required because ahoy's KCC writes `Transform`.

### Game flow

States live in `shared::game_state`: `GameState` (`AssetLoading` default, `MainMenu`, `Lobby`,
`InGame`; `Loading`/`Paused` are unused), `VRState`, `ModalMenuState` (the in-game pause menu,
overlaying `InGame`), `ServerState` (server-only). `InputDeviceState`
(`KeyboardMouse`/`Gamepad`) tracks the last-used device for UI glyphs.

1. `AssetLoading` → `MainMenu` once `CommonAssets` loads (`LoadingState`).
2. `Startup`: `spawn_client_link` creates the one persistent connection entity (`ClientLink`),
   reused across reconnects. Systems needing "my connection" use `Single<&mut MessageSender<T>>`.
3. Connect button → `events::Connect` → `on_connect_request` fetches a netcode connect token over
   HTTPS (`GET https://<server_ip>:6001/connect_token`), then `poll_token_fetch` connects with
   `Authentication::Token`. The client pins the token endpoint's self-signed certificate
   fingerprint on first use (`client/assets/network/token-tls-fingerprint.txt`; delete to re-trust).
   The netcode private key never reaches the client.
4. `On<Add, Connected>` (`on_connected`) → `Lobby`.
5. Lobby: the level picker sends `LoadLevelRequest`; Play sends `InGameRequest`.
6. `On<Add, ClientInGame>` with `Controlled` (`on_in_game`) → `InGame`; `On<Remove, Controlled>`
   (`on_out_of_game`) → `Lobby`.
7. `On<Add, Disconnected>` (`on_disconnected`) → `MainMenu`, except during `AssetLoading`
   (`Disconnected` is present before any connection attempt).
8. Disconnecting (`events::Disconnect` → `on_disconnect_request`) triggers **both** lightyear's
   `Disconnect` and `Unlink`; `Disconnect` alone leaves a stale `Linked` marker that panics on the
   next connect.

Gameplay/UI entities are scoped with `DespawnOnEnter`/`DespawnOnExit` rather than teardown
systems.

### Modules

- **`gameplay/player_character.rs`** — `on_player_spawned` is a **polling** `Update` system
  (`Added<Controlled>` on `PlayerCharacter`) that sets `LocalPlayer` and adds the local player's
  controls and camera rig. `decorate_other_players` gives other players a model. It also registers
  `CharacterControllerState` for local rollback (`app.component::<CharacterControllerState>().local_rollback()`,
  which needs `PredictionRegistry` from `ClientPlugins` to exist first). `AccumulatedInput` is
  deliberately not registered.
- **`gameplay/interpolated_remotes.rs`** — polling system that inserts lightyear's `Interpolated`
  on every replicated body that isn't `Predicted` and isn't `RigidBody::Static`.
- **`gameplay/combat.rs`** — client combat presentation; `hide_dead` (polling) removes
  `AhoyCharacterController` from dead entities so the owner's corpse stops simulating.
- **`gameplay/npc_spawner.rs`, `cube_spawner.rs`** — request spawns from the hotkeys and decorate
  replicated NPCs/cubes; `apply_model_offset` turns a replicated `ModelOffset(Vec3)` into
  `Transform::from_translation(offset)` on entities lacking a `Transform`.
- **`controls/actions.rs`** — every `#[derive(InputAction)]` marker, all in one file.
- **`controls/controls.rs`** — `bind_replicated_ahoy_actions`: a **polling** system that binds real
  inputs to the server-spawned replicated action entities once this client has `Controlled` on
  them (self-terminating via `Without<Bindings>`); inserting `Bindings` starts the replicated input
  stream. The right-stick look binding has a radial `DeadZone` (0.15). Hotkey observers send
  `AttackAttempt`/`KillAttempt` (target = `Selected`) and spawn requests via
  `MessageSender<T>::send::<OrderedReliable>`. `return_to_main_menu` (Escape / gamepad Start /
  pause menu) triggers `Disconnect` and sets `MainMenu`. Registers `TargetingPlugin`.
- **`controls/targeting.rs`** — `raycast_from_center` raycasts against `Selectable` entities each
  frame into `Hovered`/`Selected` (needs a window).
- **`controls/camera.rs`** — FPS camera driven by the same `RotateCamera` action the server
  consumes; in `--mcp` mode, `OffscreenRenderTarget` (1280×800) and the headless camera
  maintenance (see below).
- **`events.rs`** — client-local events (`Connect`, `Disconnect`, `Play`, `SpawnCube`, `SpawnNpc`,
  animation triggers, `AttackSelected`/`KillSelected`).
- **`lifecycle/networking.rs`** — connection, token fetch, and every `GameState` transition above.
  `ServerAddress` comes from `config.toml`.
- **`lifecycle/lobby.rs`** — spawns the lobby UI and its `MenuControls` context; registers the
  level-picker systems. Its `on_play` observer sends `LoadLevelRequest` for a hardcoded
  `levels/spawn.level.ron` on the `Play` event — that level no longer exists, and this path may be
  dead (the lobby's Play button sends `InGameRequest` directly).
- **`lifecycle/loading.rs`** — `OnEnter(InGame)` spawns the HUD; `spawn_client_world_assets` loads
  the `.glb` referenced by any replicated `ClientWorldAsset` and inserts `WorldAssetRoot`. This is
  how server-authored world content appears client-side.
- **`ui/ui.rs`** — main menu on `bevy::feathers`: Connect, Options (stub selector), Credits (stub),
  Quit, Language (`en-US`/`ru-RU`/`ja-JP`). Gamepad confirm (`on_ui_confirm`) synthesizes the real
  `bevy::ui_widgets::Activate`; literal Enter on old `widgets::button()` surfaces goes through
  `on_ui_confirm_enter`'s `LegacyActivate` bridge (`FeathersButton` handles Enter natively —
  synthesizing a second activation double-fires).
- **`ui/lobby.rs`** — Play (`InGameRequest`), level picker (`selector.rs` over the replicated
  `Levels`, names localized via Fluent), Main Menu. Feathers buttons must listen for
  `bevy::ui_widgets::Activate`, not the legacy `ui::widgets` one.
- **`ui/selector.rs`** — generic paginated popup (5 rows) firing `UiSelected { entity }`; used by
  the level, language and options pickers.
- **`ui/hud.rs`** — data frame, hotbar, crosshair, control tips; reads ahoy's
  `CharacterControllerState::grounded`.
- **`ui/tui_panel.rs`** — ratatui panel on the main menu; skipped unless the font exists at build
  time (`client/build.rs` emits the `has_tui_font` cfg).
- **`ui/` others** — `input_icons.rs` (Kenney/Steam Deck glyphs), `widgets.rs` (hand-rolled
  button/panel used by HUD and pause modal), `modal_menu.rs`, `nameplate.rs`, `npc_ui_quad.rs`,
  `quad_panel.rs`, `localization.rs` (Fluent), `framework.rs` (unused demo).
- **`presentation/`** — `animation.rs` (reads `Character`/`Idle` and ahoy's grounded state),
  `mesh_primitive.rs` (polling: turns a replicated `MeshPrimitive` into `Mesh3d` + default
  material), `particles.rs` (`bevy_hanabi`).
- **`dev/console.rs`** — `chill_bevy_console` commands: `fps`, `physics_debug`, `respawn`,
  `despawn_cubes`, `despawn_npcs`, `play_animation`, `load_level`, `controls`, `nameplates`, `hud`,
  `kcc_debug` (dumps the local player's input → movement chain).
- **`dev/tool_api.rs`** — the agent tool API (see below).
- **`assets/collections.rs`** — `CommonAssets`: five world-asset handles plus optional "furniture"
  (`#[asset(key = "…", optional)]` `Option<Handle<T>>` — fonts, sounds, skybox, icon atlases);
  every consumer degrades when absent. `CommonAssets::placeholder()` is used by
  `--no-common-assets`. `override_default_font` patches Bevy's default font;
  `override_feathers_button_font` re-inserts `InheritableFont` on feathers buttons (mutating in
  place doesn't propagate).

### Headless camera mechanics (`--mcp`)

- With no primary window, `retarget_cameras_to_offscreen` points every camera at the offscreen
  texture. The first claimed camera keeps the clear (order 1); later ones get increasing order and
  `ClearColorConfig::None`; the newest camera's 3D view wins.
- Retargeting calls `projection.set_changed()` so `camera_system` recomputes `target_info` (a camera
  whose target was fixed after it was added would otherwise never render).
- Exactly one `IsDefaultUiCamera` must exist: `maintain_default_ui_camera` hands the marker between
  the headless bootstrap UI camera (`HeadlessUiCameraBootstrap`) and the player camera, and
  `keep_ui_camera_drawn_last` keeps the holder drawn last.

## Architecture: shared (`shared/src/`)

- **`replication.rs`** — `SharedReplicationPlugin` and the `OrderedReliable` channel.
  - Replicated components: `ClientWorldAsset`, `InGameRoot`, `Levels`, `ClientInGame`,
    `ClientInLobby`, `PlayerCharacter`, `Character`, `Name`, `HitPoints`, `Gcd`, `Collider`,
    `RigidBody`, `LockedAxes`, `ModelOffset`, `CollisionLayers`, `Selectable`, `Npc`, `Idle`,
    `Cube`, `Dead`.
  - `RigidBody` must be replicated: without it client colliders get no `ColliderOf`, ahoy's
    collision query sees no level geometry, and characters fall through the world.
  - `Transform` is **not** replicated: bodies sync via `Position`/`Rotation`; non-body children
    use `ModelOffset`.
  - Messages: `AttackAttempt`/`KillAttempt` (client→server, entity-mapped),
    `SpawnCubeRequest`/`SpawnNpcRequest`/`LoadLevelRequest`/`InGameRequest`/`LobbyRequest`/
    `ObserveRequest`/`ClientDespawn` (client→server), `Attack`/`Kill`/`EntityDied`
    (server→client, entity-mapped). Movement input is not a message.
- **`inputs.rs`** — `SharedInputsPlugin`: lightyear BEI input replication for `PlayerInputContext`,
  with a 2-tick minimum input delay (`balanced()` preset) so tick-N input reaches the server
  before it simulates tick N.
- **`player.rs`** — `PlayerCharacter`, `Selectable`, `PlayerCharacterSpawner`; `player(name,
  position)` bundle: capsule `Collider::capsule(0.4, 1.0)`, `RigidBody::Kinematic`, collision
  layers, `HitPoints {100,100}`, `Gcd`, ahoy `CharacterController` + `CharacterLook`, the
  `PlayerInputContext`, and unbound action entities (`Movement`, `Jump`, two `RotateCamera`s
  marked `MouseLook`/`StickLook`). `generate_player_name` makes unique "Adjective Noun" names
  (deterministic seed, `#2`/`#3` suffixes).
- **`character_controller.rs`** — `Character`/`Idle` (animation markers) and `GameLayer`.
  Grounded state comes from ahoy's `CharacterControllerState::grounded`; there is no marker.
- **`combat.rs`** — data only: `HitPoints`, `Gcd(Timer)` (starts finished), `Dead(Timer)`,
  `DAMAGE = 49`, `ATTACK_RANGE = 10.0`, `GCD_DURATION = 0.5`, `DEAD_DURATION = 1.0`.
- **`client_events.rs`** / **`server_events.rs`** — message types. Payload entities are only ever
  targets (`AttackAttempt.entity`); actors are resolved server-side. `EntityDied` carries the
  position because the entity may already be gone client-side.
- **`level.rs`** — `InGameRoot`/`LobbyRoot` (field-less markers, `#[require(Transform,
  Visibility)]`); `Levels` (replicated list of `(AssetPath, Level)`).
- **`cube_spawner.rs`** / **`npc_spawner.rs`** — `cube(...)`/`npc(...)` bundles, `Cube`/`Npc`
  (both `Reflect` — BRP can only query reflected components), `ModelOffset`.
- **`mesh_primitive.rs`** — `MeshPrimitive`: Skein-authorable primitive parameters, so a Blender
  empty with no mesh data becomes rendered geometry (client-side only, no collider).
- **`assets/level.rs`** — `Level { name, model, skybox }` (`name` is a Fluent key) with a custom
  `LevelAssetLoader` (`.level.ron`); `LevelMetadataAssets` (folder collection of `levels/`);
  `ClientWorldAsset { asset_path }` (server-side marker whose `.glb` the client loads); `Skybox`
  and `ClientReplicate` (defined, not wired up).
- **`game_state.rs`** — the shared state enums.

## Architecture: server (`server/src/`)

Headless (`MinimalPlugins`): `StatesPlugin`, `LogPlugin`, `.init_asset::<Mesh/Image>()` are added
explicitly, and `server/Cargo.toml` enables `bevy/reflect_auto_register` (world-asset spawning
reflects every component). The GLTF/Skein/world-serialization pipeline needs no GPU.

- **`main.rs`** — `build_app(networking_plugin)` shared by the live server and `--replay`.
  `ScheduleRunnerPlugin::run_loop(1/60 s)` (the default busy loop burns ~2 cores).
  `SingleThreadedExecutor` and one-thread task pools (determinism and VPS density).
  `SkeinPlugin { handle_brp: false }` (`ServerToolsPlugin` owns BRP). `ServerState`
  `Startup → Lobby` once `LevelMetadataAssets` loads.
- **`networking.rs`**
  - `start_endpoint`: binds UDP `0.0.0.0:6000`; loads or creates the netcode key at
    `<asset root>/assets/network/netcode.key` (root = `BEVY_ASSET_ROOT`, else runtime
    `CARGO_MANIFEST_DIR`, else the executable's directory) and the self-signed TLS identity
    (`token-tls.crt`/`.key`) beside it.
  - HTTPS token endpoint on :6001 (`GET /connect_token`): fresh token per request, 30 s expiry; its
    server address is the address the client used to reach the endpoint (`Host` header, falling
    back to the default-route IP, then the peer IP).
  - `NetcodeConfig`: `server_addr_check: true` with `additional_expected_addresses` =
    `127.0.0.1:6000` and the default-route IP `:6000` (the wildcard bind can never match a token's
    concrete address on its own).
  - `load_level_request`: guarded by `LevelState`; parents the level's `WorldAssetRoot` under
    `InGameRoot`. `on_level_ready` (`WorldInstanceReady`) → `ServerState::InGame`,
    `LevelState::LevelLoaded`.
  - `in_game_request` (only while `ServerState::InGame`; otherwise silently dropped): spawns
    `player(...)` with `Replicate::to_clients(All)`, `PredictionTarget` scoped to the owner's
    `RemoteId`, `ControlledBy { owner, lifetime: Persistent }`, `ClientInGame`,
    `ChildOf(InGameRoot)`; moves the connection to the game room.
  - `owned_players(connection)` resolves a connection's characters.
- **`rooms.rs`** — allocates `GameRoom`/`LobbyRoom` and spawns the two roots at `Startup`.
  `HierarchySendPlugin::<ChildOf>` is already added by lightyear; don't add it again.
- **`lobby.rs`** — replicates `Levels` to the lobby room; puts each new connection in the lobby
  room.
- **`level_state.rs`** — `LevelState`: `Idle` → `Loading(path)` → `LevelLoaded(path)`; any
  `LoadLevelRequest` outside `Idle` is rejected and logged.
- **`input.rs`** — `accumulate_look` (`Without<Dead>`) turns the replicated rotate actions into
  `CharacterLook`.
- **`combat.rs`** — `apply_attack`/`apply_kill` resolve the sender's **living** player character
  (dead players can't attack), check `Gcd` and range, apply damage, and broadcast `Attack`/`Kill`
  with the player character as attacker. `kill_zero_hp` broadcasts `EntityDied`, inserts `Dead`,
  removes `Selectable`/`RigidBody`/`Collider` (stopping movement); `despawn_dead` despawns when the
  timer ends. `Gcd`/`Dead` tick in `FixedUpdate` by `TickDuration`.
- **`spawn.rs`** — `apply_spawn_npc`/`apply_spawn_cube`: resolve the sender's player `Gcd`, spawn
  standalone entities with `Replicate::to_clients(All)` and **`Rooms::single(game_room)`** (without
  it they'd replicate to lobby clients too). Seeds are `tick ^ connection bits` (deterministic).
  NPCs get an overlap check and a model child (`ClientWorldAsset` + `ModelOffset`); the cube
  overlap check is deliberately disabled (cube stacking is a mechanic).
- **`tools.rs`** — server BRP on :15701 and MCP on :15711 (`--brp-port`/`--mcp-port`), always
  compiled in; one method, `server/state` (app state, clients, players' transform/HP/owner).
- **`replay.rs`** — `SERVER_REPLAY_RECORD=<file>` records client→server gameplay messages and
  per-tick resolved actions (JSONL, keyed by `PeerId`); `server --replay <file>` re-simulates
  deterministically via `TimeUpdateStrategy::FixedTimesteps(1)` (frozen while loading). Input is
  injected by triggering `Fire<A>` directly. Replay still includes `NetworkingPlugin` (a resource
  it inserts is required) and binds :6000 — don't run it next to a live server.

## Agent tool API and headless modes

Full playbook: `docs/agents/skills/playtest.md`. Design: ADRs 0009–0012.

- Behind the `dev-tools` cargo feature (works in dev and release). BRP on `127.0.0.1:15702`, MCP on
  15710 (`--brp-port`/`--mcp-port` for fleets; a non-default BRP port also isolates screenshot
  storage). Bevy 0.19 builtin BRP methods are `world.*`.
- Custom methods: `game/state`, `game/client_info`, `game/trigger`, `game/select`, `game/levels`,
  `game/select_level`, `game/input` (action-level `ActionMock`; bypasses modifiers), `game/gamepad`
  / `game/keyboard` / `game/mouse` (device-level mocks through real bindings), `game/ui`,
  `game/cameras`, `game/screenshot` + `game/screenshot/get`.
- Implementation gotchas: BEI's gamepad-button reader uses `Gamepad`'s `analog` field, not
  `digital`; mouse motion/scroll must be written as `MouseMotion`/`MouseWheel` events (the
  accumulated resources are overwritten every frame); `game/select {"nearest": true}` excludes the
  local player by entity (a `Without<LocalPlayer>` filter on a resource excludes nothing).
- Modes:
  - `--mcp`: no winit window; `ScheduleRunnerPlugin` at 60 Hz; all cameras render into the
    1280×800 offscreen target; screenshots go to `docs/agents/playtests/dist/screenshots/`;
    agent-cursor overlay. Incompatible with VR (`--mcp` wins).
  - `--no-render`: `--mcp` without render plugins (no GPU needed; ~0.7 core per client). UI layout,
    picking and `game/ui` still work (`shim_camera_computed`); screenshots return an error.
    `bevy_mod_outline`/`bevy_hanabi` are skipped, and `SyncWorldPlugin` is added explicitly (render
    sync hooks need `PendingSyncEntity`; without it a despawn panics the replication receiver).
  - `--headless-render`: observer at 2 fps (logic catches up to 60 Hz); joins with
    `game/trigger observe` (no player); `game/screenshot {"camera": id}` renders a chosen camera.
  - `--no-common-assets`: skips the `CommonAssets` manifest entirely (placeholder collection);
    world content still loads by path through `ClientWorldAsset`.
- Isolated plaintext playtest assets live in `docs/agents/playtests/playtest_assets/playtest_NNNN/` (point
  `BEVY_ASSET_ROOT` at them).

## Conventions

- **Server never trusts client-supplied identity.** The actor is the connection entity that owns
  the `MessageReceiver<T>` (`Query<(Entity, &mut MessageReceiver<T>)>`); payload entities are only
  targets. Resolve the connection's player via `owned_players`.
- **Entity-carrying messages** need `#[entities]` on the field, `#[derive(MapEntities)]`, and
  `.add_map_entities()` at registration; mapping only works for replicated entities.
- **Rooms**: an entity replicates to a client only if both carry a `Rooms` sharing a room id; an
  entity with no `Rooms` bypasses filtering and goes to everyone. `Rooms::single(id)` replaces
  membership. Content and the watching client's connection both need room membership. Children
  of a room-tagged root inherit its room.
- **BEI**: each physical input is consumed by the first action reading it per tick — never bind
  one input to two actions. Ahoy's input observers write the *context* entity's
  `AccumulatedInput`, and its KCC runs on the entity with `CharacterController`; they must be the
  same entity (`PlayerInputContext` lives on the player). BEI API notes: `Bindings::spawn` takes
  exactly one `SpawnableList` (modifiers are sibling components); `ActionOf::get` needs
  `bevy::ecs::relationship::Relationship` in scope; `SpawnWith` is Bevy's.
- **Determinism (server)**: no wall-clock time in simulation — seed RNG from the tick, tick timers
  in `FixedUpdate` by `TickDuration`. Replay depends on it.
- **Observers vs polling**: component `On<Add>`/`On<Remove>` observers drive state transitions,
  but reactions to *replicated* components that must work on late join/reconnect use polling
  `Update` systems (`on_player_spawned`, `hide_dead`, `bind_replicated_ahoy_actions`,
  `interpolated_remotes`), because the initial replication sync doesn't reliably fire per-component
  `Add` observers. Don't convert either way without testing fresh connect, reconnect and late join.
- **Console gating**: player-input observers are registered with `add_observers_run_if!(...,
  console_closed)`; lifecycle/spawn observers stay ungated (a gated one-shot trigger is lost
  forever).
- **Inputs** are `#[derive(InputAction)]` markers in `controls/actions.rs`, handled by observers
  (`On<Fire<T>>`, `On<Start<T>>`, `On<Complete<T>>`).
- **`EntityEvent`** supports one target; for two-entity events (`Attack`), the scoped side is
  `entity`, the other a plain field.
- **BRP**: components must be `Reflect` + `#[reflect(Component)]` to be queryable; handle-holding
  components come back as serialization errors (`-23402` = present, `-23403` = absent);
  `world.list_resources` lists reflected resources only.
- Hierarchy walks use `Query<&Children>::iter_descendants` / `Query<&ChildOf>::iter_ancestors`.
