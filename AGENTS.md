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
singleplayer runs a local client and server. **`p19`** (from the working title "prototype 19") is
the internal codename only; product branding is deliberately undecided — don't introduce a public
name. Cargo workspace with three members — package names are `p19-*` (Rust crate paths `p19_*`,
binary names `p19-client`/`p19-server`), living in `crates/<short name>/`:

- **`p19-client`** (`crates/client/`) — rendering, UI, input, camera, presentation. Sends intent as
  network messages and renders what the server replicates; never decides outcomes.
- **`p19-server`** (`crates/server/`) — headless authoritative simulation: level loading (real `.glb` +
  Avian colliders, no GPU), player spawning, movement, combat, spawning cubes/NPCs.
- **`p19-shared`** (`crates/shared/`) — what both sides must agree on: replication registration, message
  types, the player bundle, spawn bundles, shared data components, game states.

Reflected type paths (Skein extras in `.glb`/`.gltf`, BRP queries) therefore start with
`p19_shared::`/`p19_client::`/`p19_server::`. Documents written before the 2026-10-02 rename
(old ADRs, playtests, bug reports) still say `shared::…`, `client::…`, `target/release/client`.

Status: pre-release prototype. It runs end to end from a fresh clone with Git LFS (connect →
lobby → pick a level → play: movement, combat, spawning); see "Known gaps" for what's missing.
Steam Deck is the primary / minimum-spec target (see "Platform targets").

Key facts to internalize:

- **Networking is `lightyear` 0.30** over UDP/netcode. `p19_shared::replication::SharedReplicationPlugin`
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
  state; lightyear rollback reconciles the prediction. Remote bodies are interpolated. Prediction
  is a per-client choice (Options → Client-side prediction, `ClientPrediction`, default off) sent
  in `InGameRequest { predict }`; with it off the own character is interpolated too and the
  client runs no KCC.

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
  `Fire<A>` events directly works (that is how `p19_server::replay` injects input) and is the likely
  path for AI. Combat is likewise reachable only from a client `AttackAttempt`/`KillAttempt`.
- **Replay movement rate mismatch**: `server --replay` reproduces sessions deterministically but
  replayed movement covers far less distance than live (~1 vs ~12.7 units over 60 ticks). Leading
  suspect: the injected `Fire<A>` events' `fired_secs`/`elapsed_secs` are hardcoded to `0.0`. See
  `crates/server/src/replay.rs`'s module doc.
- **KCC has no ground friction**: with zero wish velocity, ahoy's `ground_accelerate` leaves
  velocity untouched, so a character coasts indefinitely once input stops.
- **Join-burst input corrections**: when a second client joins, the replication burst can push the
  existing client's inputs one tick late for ~10 ticks (`server_late_input_mismatch` errors).
  Benign and self-healing; deliberately not fixed.
- **VR**: `bevy_xr_utils` 0.6.0 (crates.io) has `suggest_action_bindings` commented out in
  `tracking_utils.rs`, so controller grip poses likely never track (both stay at identity). Not
  re-verified on a headset. VR locomotion mocks the replicated `Movement` action with `ActionMock`
  each frame, which overrides keyboard movement while in VR; look comes from the headset through
  `FpsCamera`.
- **Netcode posture is dev-grade**: TOFU-pinned self-signed TLS for the token endpoint; production
  needs a CA-signed certificate on a real backend.
- **Dead or unused code**: `GameState::Loading`/`Paused` (never entered), `lifecycle/loading.rs`'s
  `clear_effects` (never called), `crates/client/src/assets/level.rs` and `crates/shared/src/server_state.rs`
  (empty; the latter isn't even declared), `PreloadCollection` (never loaded), `Character`
  registered for replication twice, the `vleue_navigator` workspace dependency and the client's
  `bevy_simple_text_input` dependency (nothing uses either). The HUD hotbar is built but not
  spawned (`hud.rs`'s `HOTBAR_ENABLED = false`).
- **Doc/code mismatches to distrust**: `p19_shared::assets::level::Level`'s doc comment claims
  `model`/`skybox` are dependency-tracked handles — the loader just converts them to plain
  `AssetPath`s. `crates/shared/src/server_events.rs` uses stale replicon terminology.
- **Tests**: only a handful of unit tests (`p19_server::networking` token-address fallback,
  `p19_shared::player` name generation). CI builds but runs no gameplay tests.

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
- Steam Deck button glyphs (`input_icons.rs`, the pause menu's controls tips in `modal_menu.rs`)
  and gamepad-first UI navigation (`ui/markup.rs`'s `MenuControls`) are already in place.

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

`p19_shared::player::player()` is independent of all of this.

## Commands

Run from the workspace root (`prototype_19/`).

```sh
cargo check --workspace           # fastest compile check
cargo clippy --workspace
cargo test --workspace            # the few unit tests
cargo build -p p19-client --release
cargo build -p p19-server --release
cargo run -p p19-server --release     # listens on UDP 0.0.0.0:6000 (+ HTTPS token endpoint :6001)
cargo run -p p19-client --release     # connects when the main menu's Connect button is pressed
```

- **Fresh clone**: run `git lfs install` before cloning (or `git lfs install && git lfs pull`
  after): the binary assets are LFS objects, and without LFS they are small pointer files
  (`version https://git-lfs.github.com/spec/v1 …`) the game can't load. Nothing else needs
  provisioning: the server creates `assets/server/network/` and the client its TOFU pin on first
  run. Verified by running both binaries against exactly the git-tracked asset set (playtest
  0039).
- **Layout**: crates in `crates/{client,server,shared}/`; assets outside them in
  `assets/client/` and `assets/server/` (the two asset roots) and `assets/src/` (raw `.blend` files
  and source packs; nothing loads from it). `assets/client` and `assets/server` are tracked: binary
  assets (`.glb`, `.ktx2`, `.png`, `.wav`, fonts, …) through Git LFS (`.gitattributes`, per
  extension — a bare directory pattern matches no file), text assets in plain git.
  `assets/src` is ignored, and so is each root's `network/` (the server's netcode key and
  token-TLS key pair, the client's TOFU pin): machine-local, recreated on first run, never
  committed.
- **Asset root resolution** (`p19_shared::paths::asset_dir`, used for `AssetPlugin::file_path` and
  for every plain-`std::fs` read of asset-root files — config, network identity, TLS pin):
  1. `BEVY_ASSET_ROOT` set → `$BEVY_ASSET_ROOT/assets` (isolated playtest asset sets, overrides);
  2. `<workspace>/assets/<client|server>` if it exists (development checkout; no env vars needed,
     any working directory);
  3. `assets/` beside the executable (deployed builds, e.g. the Steam Deck staging dir).
  Don't rely on Bevy's default `CARGO_MANIFEST_DIR`-relative `assets/`: it would point inside
  `crates/*`. Dev-only locations (agent screenshot staging) use `p19_shared::paths::workspace_root`.
- **Client config**: `assets/client/config.toml` (`server_ip`, default `127.0.0.1`; `vr`, default
  `false`). Missing fields fall back to defaults. There is no in-game UI for these.
- **Steam Deck builds**: always build inside the `steamrt4` toolbox with
  `scripts/steam_deck_toolbox.sh cargo build -p <p19-client|p19-server> --release`. A host build links the
  host's newer glibc and fails on the Deck (`GLIBC_2.4x not found`); since host and toolbox share
  the toolchain, cargo can't tell them apart — recover with `cargo clean -p <crate> --release`
  inside the toolbox. Check with
  `objdump -T target/release/<bin> | grep -oE 'GLIBC_[0-9.]+' | sort -Vu | tail -5` (should top
  out around `GLIBC_2.39`).
- `scripts/build_steam_deck.sh` cleans, builds the client in the toolbox, and stages it with a
  copy of `assets/client/` in **`target/steamdeck/release/`** (wiped and recreated each run; a
  full `cargo clean` deletes it too — never keep hand-placed files there), then checks the GLIBC
  baseline. Ready for SteamOS Devkit Client's Title Upload: `Local Folder` =
  `<repo>/target/steamdeck/release/`, `Start Command` = `./p19-client`.
- `../build.sh` (outside the workspace) builds in a `steamrt-sniper` podman container.
- **CI** (`.github/workflows/ci.yml`): "Build server" (host `ubuntu-latest`, debug) and "Build
  client (steamrt4, shippable)" (release, inside `registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk`,
  with a GLIBC check). Both free runner disk space first.
- **Tracy** is opt-in: `cargo run -p p19-client --release --features tracy`. Only use it with a Tracy
  GUI attached — without one, `tracy-client` buffers every span in memory (RSS grows ~150 MB/s).
  `[profile.release] debug = true` exists for Tracy symbol names. Save captures under
  `target/profiling/` (e.g. `target/profiling/tracy_profile.tracy`) — gitignored via `/target`,
  removed by `cargo clean`, so copy anything worth keeping elsewhere first.

## Dependency layout

- The root `Cargo.toml` centralizes versions in `[workspace.dependencies]`; members use
  `dep.workspace = true`. A member cannot override `default-features` on an inherited dependency,
  so `server`/`shared` declare `bevy` directly with `default-features = false` (headless).
- **`bevy_markup`** 0.3.0 from crates.io (the owner's own crate — HTML/Tera + CSS + Fluent →
  Bevy UI; every client UI surface, see "UI (bevy_markup)"; source:
  `github.com/nchashch/bevy_markup`). It requires bevy `^0.19.1` (`Cargo.lock` pins 0.19.1). Its
  README asks apps to patch `fluent-syntax` to its fork (`nchashch/fluent-rs`,
  `fix/fuzzing-bugs-0.11`: a panic on a broken unicode escape and a stack overflow on deeply
  nested expressions), done in the root `[patch.crates-io]`.
- Other crates needing local patches are pinned git branches on the owner's forks; check the
  fork's source (`~/.cargo/git/checkouts/<crate>-*/`) when behavior differs from upstream docs:
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
- **Fonts are the system's**: `bevy`'s `system_font_discovery` resolves the CSS generics
  (`serif`, `sans-serif`, `monospace`) to installed fonts and lets `parley` fall back to any
  installed font for missing glyphs (`ja-JP`). No font files ship (ADR 0016); a script renders
  only if the player's system has a font for it. Exceptions: the dev console (`chill_bevy_console`
  takes only a path) uses Bevy's built-in FiraMono.
- `chill_bevy_console` is the in-game dev console (backtick).
- Other crates.io deps of note: `avian3d` 0.7, `bevy_skein`, `bevy_enhanced_input` 0.26,
  `bevy_asset_loader`, `bevy_hanabi`, `bevy_vello`, `bevy_seedling`, `audionimbus`,
  `bevy_mod_openxr`/`bevy_mod_xr`/`bevy_xr_utils` 0.6, `rustls` (aws-lc-rs provider), `rcgen`.
- **Reflected `TypePath`s include the crate/module path.** Renaming a crate or moving a module
  that holds Skein-authored types breaks previously exported `.glb`s until they're re-exported.

## Architecture: client (`crates/client/src/`)

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

States live in `p19_shared::game_state`: `GameState` (`AssetLoading` default, `MainMenu`, `Lobby`,
`InGame`; `Loading`/`Paused` are unused), `VRState`, `ModalMenuState` (the in-game pause menu,
overlaying `InGame`), `ServerState` (server-only). `InputDeviceState`
(`KeyboardMouse`/`Gamepad`) tracks the last-used device for UI glyphs.

1. `AssetLoading` → `MainMenu` once `CommonAssets` loads (`LoadingState`).
2. `Startup`: `spawn_client_link` creates the one persistent connection entity (`ClientLink`),
   reused across reconnects. Systems needing "my connection" use `Single<&mut MessageSender<T>>`.
3. Connect button → `events::Connect` → `on_connect_request` fetches a netcode connect token over
   HTTPS (`GET https://<server_ip>:6001/connect_token`), then `poll_token_fetch` connects with
   `Authentication::Token`. The client pins the token endpoint's self-signed certificate
   fingerprint on first use (`assets/client/network/token-tls-fingerprint.txt`; delete to re-trust).
   The netcode private key never reaches the client.
4. `On<Add, Connected>` (`on_connected`) → `Lobby`.
5. Lobby: the level picker sends `LoadLevelRequest`; Play sends `InGameRequest { predict }`
   (from `ClientPrediction`).
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
  controls and camera rig, plus ahoy's `CharacterController` (the client-side KCC) only if the
  character is `Predicted`. Without prediction the character has no `CharacterControllerState`
  (the data frame shows grounded "—"). `ClientPrediction` (resource, reflected, default `false`,
  not persisted) is the Options setting; it applies from the next Play. `decorate_other_players` gives other players a model, and `face_look_direction` turns it to
  their replicated `LookDirection` yaw (+π: the rig's front faces +Z). It also registers
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
  stream. The replicated `Look` action gets no bindings: it gets an enabled `ActionMock`
  (`MockSpan::Manual`) plus lightyear's `InputMarker` (what makes an action send; bindings add it
  automatically, a mock doesn't), and `write_look_input` (`FixedPreUpdate`, before BEI's update)
  writes the `FpsCamera` direction into that mock every tick. Every BEI action already carries a
  disabled default `ActionMock`, so never filter on `Without<ActionMock>`. Mouse and right-stick
  look are client-local `RotateCamera` actions in `PlayerControls` (the stick with a radial
  `DeadZone` 0.15; the mouse action's `Scale` is kept at `MOUSE_LOOK_SENSITIVITY` (0.005 rad/px)
  × `MouseSensitivity` by `apply_mouse_sensitivity` — resource, reflected, default 1, not
  persisted, applies immediately); `rotate_camera` turns `FpsCamera` and is gated with the other gameplay
  observers. Hotkey observers send
  `AttackAttempt`/`KillAttempt` (target = `Selected`) and spawn requests via
  `MessageSender<T>::send::<OrderedReliable>`. `return_to_main_menu` (Escape / gamepad Start /
  pause menu) triggers `Disconnect` and sets `MainMenu`. Registers `TargetingPlugin`.
- **`controls/targeting.rs`** — `raycast_from_center` raycasts against `Selectable` entities each
  frame into `Hovered`/`Selected` (needs a window).
- **`controls/fps_controller.rs`** — `FpsCamera { yaw, pitch }`: the client's source of truth for
  look, in ahoy's `CharacterLook` convention (yaw 0 faces −Z, pitch positive looks up).
  `orient_fps_camera` derives the rig's `Transform` from it. VR (`vr_controllers.rs`) sets it from
  the headset's global rotation each frame.
- **`controls/camera.rs`** — the player camera bundle; in `--mcp` mode, `OffscreenRenderTarget`
  (1280×800) and the headless camera maintenance (see below).
- **`events.rs`** — client-local events (`Connect`, `Disconnect`, `SpawnCube`, `SpawnNpc`,
  animation triggers, `AttackSelected`/`KillSelected`).
- **`lifecycle/networking.rs`** — connection, token fetch, and every `GameState` transition above.
  `ServerAddress` comes from `config.toml`.
- **`lifecycle/lobby.rs`** — on `OnEnter(Lobby)` spawns the lobby UI (`ui/lobby.rs`) and its
  `MenuControls` context.
- **`lifecycle/loading.rs`** — `OnEnter(InGame)` spawns the HUD; `spawn_client_world_assets` loads
  the `.glb` referenced by any replicated `ClientWorldAsset` and inserts `WorldAssetRoot`. This is
  how server-authored world content appears client-side.
- **`ui/`** — every UI surface; see "UI (bevy_markup)" below.
- **`presentation/`** — `animation.rs` (reads `Character`/`Idle` and ahoy's grounded state),
  `mesh_primitive.rs` (polling: turns a replicated `MeshPrimitive` into `Mesh3d` + default
  material), `particles.rs` (`bevy_hanabi`).
- **`dev/console.rs`** — `chill_bevy_console` commands: `fps`, `physics_debug`, `respawn`,
  `despawn_cubes`, `despawn_npcs`, `play_animation`, `load_level`, `controls`, `nameplates`, `hud`,
  `kcc_debug` (dumps the local player's input → movement chain). Its output is localized through
  `ui/localization.rs`'s `localized()` (bevy_fluent `Localization`), not bevy_markup.
- **`dev/tool_api.rs`** — the agent tool API (see below).
- **`assets/collections.rs`** — `CommonAssets`: five world-asset handles plus optional "furniture"
  (`#[asset(key = "…", optional)]` `Option<Handle<T>>` — sounds, skybox, icon atlases; no
  fonts, see "Fonts are the system's"); every consumer degrades when absent. No sounds ship at
  the moment: the manifest lists no `crunch` / `explosion` keys, so combat is silent. A
  manifest entry whose file is missing (as opposed to an omitted key) keeps the client in
  `AssetLoading` forever.
  `CommonAssets::placeholder()` is used by `--no-common-assets`.

### UI (bevy_markup)

Every UI surface is a `bevy_markup` `HtmlUi`: a Tera template + the one stylesheet + Fluent,
built into plain Bevy UI nodes. Design: ADR 0015. Templates and `theme.css` live in
`crates/client/src/ui/html/` and are compiled in (`embedded_asset!`, path
`embedded://p19_client/ui/html/<file>`), so they are versioned with the code and present under
every asset root (`BEVY_ASSET_ROOT` sets, `--no-common-assets`, `--no-render`). Each surface
module registers its own templates; `markup::template(&asset_server, "x.html")` loads one.

- **`ui/markup.rs`** (`MarkupPlugin`) — the shared layer: `BevyMarkupPlugin`, `DefaultStylesheet`
  = `theme.css`, the system's fonts in `FontFamilies` (`register_ui_fonts`, at `Startup`: CSS
  `serif` / `sans-serif` / `monospace`), `ActiveLocale`
  following bevy_fluent's `Locale` (bundle `locales/<id>/main.ftl.yml`; the language picker writes
  `Locale`), and the interaction layer:
  - **One input path.** Primary-button clicks on `data-on-click` elements arrive as bevy_markup
    `ElementSignal` messages (right/middle clicks are `data-on-auxclick`, unused here); `UiConfirm`
    (gamepad South or Enter, `MenuControls`) emits the same message for the focused element.
    `signal.source` says what produced it: `Pointer { pointer, button, position, .. }` (mouse,
    or a VR laser's `PointerId::Custom` from `quad_panel.rs`) or `Activation(input)` (the key or
    gamepad button `on_ui_confirm` saw). Buttons are routed by name with bevy_markup's
    `app.on_html_click("lobby.play", system)` (namespaced names: `main-menu.connect`,
    `pause.resume`, `wrist-game.main-menu`, …; each handler a system taking `In<ElementSignal>`);
    the selector reads the `ElementSignal` messages itself (`selector.toggle`/`selector.pick`).
    Handlers read per-feature `data-*` attributes (`signal.data("selector")`) or a structured
    `data-with` payload (row indices).
  - **Navigation/focus** is bevy_markup's (its `focus` module, browser-style): `data-on-click`
    elements are focusable, the `autofocus` attribute takes the initial focus, focus survives
    rebuilds by element `id`, an `HtmlModal` root (selector popup, pause menu) confines it, an
    `HtmlNoFocus` root (VR wrist panels) never takes it, and `.button:focus-visible { outline }`
    in `theme.css` is the focus ring (directional input shows it, a pointer press hides it and
    moves focus). This module only binds input: `MenuControls` (`markup::menu_controls()`,
    spawned per UI state: main menu, lobby, pause menu) → `HtmlFocus::navigate` (d-pad, arrows,
    left stick; auto-repeat after 0.4 s, then every 0.08 s) and `HtmlFocus::activate` (South /
    Enter). Left/right on a focused slider step it instead of moving focus (`navigate_or_step`).
    A dead-end move fires bevy_markup's `FocusEdge` (the selector pages on it).
  - **Tooltips** are bevy_markup's `data-tooltip="key"` (optional `data-tooltip-args='{…}'`,
    `data-tooltip-placement="above"`): `markup.rs` inserts `HtmlTooltips(tooltip.html)`, and
    hovering shows a `tooltip.html` root
    anchored beside (or above) the element with bevy_markup's `HtmlAnchor` (follows it, stays in
    the viewport, renders on its UI camera, despawned with it).
- **`ui/ui.rs`** — main menu (`main_menu.html`): Connect, Options, Credits, Quit. Options opens
  the options screen (`options.html`): the Language selector (`en-US`/`ru-RU`/`ja-JP`, labels in
  their own script), the Mouse sensitivity slider (continuous 0.1×–3.0×, keyboard/gamepad step
  0.05, values kept at 0.01; `apply_slider_input` writes `MouseSensitivity`), the
  Client-side prediction toggle (`options.prediction`, flips
  `ClientPrediction`; the label follows it every frame via `update_options_screen`) and Back. The same main-menu template (`wrist = true`) is the VR wrist panel:
  Connect, Language (the selector itself), Quit — no Options/Credits, since screen-space modals
  don't show in a headset. Registers `HudPlugin`, `SelectorPlugin`, `MenuScreenPlugin` and
  `CreditsPlugin`.
- **`ui/menu_screen.rs`** — main-menu screens (Options, Credits): `open_menu_screen` spawns the
  screen's root (`HtmlModal`, `.menu-screen-root` z-index 100) and a separate cancel context
  (`MenuScreenControls`: `UiCancel` on Escape / gamepad East, bound only while a screen is open).
  Back (`menu-screen.back`), Escape or East close it and focus the button that opened it; with a
  selector popup open, cancel closes the popup first.
- **`ui/credits.rs`** — the credits screen (`credits.html`): the third-party assets from its
  `CREDITS` table, which mirrors `assets/CREDITS.md` — **add an asset to both**, plus its
  `credits-use-<id>` Fluent key in every locale.
- **`ui/slider.rs`** — the horizontal slider bevy_markup lacks: `<div is="slider"
  data-slider="key" data-on-click="slider.activate">` with `.slider-fill` / `.slider-thumb`
  children whose `width` / `left` percentages come from the template context (keep `data-*`
  constant: a changed dataset respawns the element and would break a drag). Pointer press and
  `Pointer<Drag>` observers on the element report `SliderInput { key, change:
  SliderChange::Set(fraction) }`; left/right `UiNavigate` on the focused slider report
  `Step(±1)`. The app owns value, range and step. Agents drive it with `game/mouse` (`move_to`,
  `button` press, `move_to`, release); in `game/ui` it is the clickable node without text.
- **`ui/selector.rs`** — generic popup (ADR 0001's behavior): a `Selector { key, options }`
  entity per picker; a toggle element (`data-on-click="selector.toggle"`,
  `data-selector="key"`) opens a `selector.html` root (`HtmlModal`) anchored right of it
  (`HtmlAnchor`; closes with the toggle), 5 visible rows
  (ids `slot-0..4`, updated in place when paging, so focus stays on its row) over a paginated
  window, a discrete scrollbar, wheel and edge paging, resume at the last pick (its row is
  `autofocus`); picks arrive as `SelectorPicked { selector, value }` messages.
- **`ui/lobby.rs`** — `lobby.html`: Play (`InGameRequest`), Level selector (options from the
  replicated `Levels`, re-seeded on change; labels are Fluent keys; a pick sends
  `LoadLevelRequest`), Main Menu (`Disconnect` + `MainMenu`).
- **`ui/modal_menu.rs`** — pause menu (`pause_menu.html`, `HtmlModal`, Main Menu above Resume,
  Resume auto-focused, then the mouse sensitivity slider — the shared `ui.mouse_sensitivity`
  component from `components.html`, the same one the options screen uses; its context comes from
  `ui::ui::mouse_sensitivity_context`, rewritten every frame by `update_pause_menu`), the controls tips (`controls_tips.html`, keyboard/mouse vs Steam Deck rows
  by `InputDeviceState`; glyphs are `is="input-icon" data-icon="<name>"`) and the VR in-game
  wrist panel (`wrist_game.html`).
- **`ui/hud.rs`** — crosshair (dot, or the `CrosshairGcdMaterial` ring while the GCD runs;
  `is="crosshair-dot"`/`"crosshair-gcd-ring"`; hotbar cells `is="gcd-overlay"`), data
  frame (Tab/Select via `DataFrameVisible`; its context is written every frame while shown),
  hotbar (`HOTBAR_ENABLED = false`); `HudVisible` (console `hud`) hides them. Reads ahoy's
  `CharacterControllerState::grounded`.
- **`ui/nameplate.rs`** — one screen-space `nameplate.html` root per `HitPoints` entity, kept
  centered over the target's head by bevy_markup's `HtmlWorldAnchor` (hidden off screen, behind
  the camera or over an invisible target; despawned with it); name, health (`style="width: …%"`),
  distance fade (root `style="opacity: …"`, from `HtmlWorldAnchorView::distance`) and the
  `hidden` class (toggle off / faded out → `display: none`) are template values written every
  frame (rounded to 1%);
  hidden by default (`NameplatesVisible`, console `nameplates`; reflected, so BRP
  `world.insert_resources` toggles it too).
- **`ui/npc_ui_quad.rs`** — one `npc_sign.html` root rendered into a shared texture shown on every
  NPC's billboard quad (`NpcUiQuad`/`NpcUiQuadMesh`, used by `gameplay/npc_spawner.rs`).
- **`ui/quad_panel.rs`** — `quad_panel(.., content: impl Bundle)`: an interactive UI root on a
  render-to-texture 3D quad, picked by the desktop crosshair ray or VR lasers (VR wrist panels).
- **`ui/tui_panel.rs`** — terminal-styled demo panel (`tui_panel.html`) top-right of the main menu;
  elapsed seconds and the gauge (`style="width: …%"`) are template values written every frame.
- **`ui/input_icons.rs`** — Kenney keyboard/mouse and Steam Deck glyph atlases
  (`InputIconAtlases::image_node(name)`, `None` when a pack is absent).
- **`ui/localization.rs`** — the `Locale` resource and the console's `Localization`.

bevy_markup rules that bite (the crate's own `AGENTS.md`, in its repository
`github.com/nchashch/bevy_markup` — not in the crates.io package —
documents the full CSS subset and pipeline):

- An `HtmlUi` root's children belong to the pipeline. A `TemplateContext`/locale/template change
  that alters the rendered HTML updates them in place (`HtmlUiBuilt`): elements still in the
  document — matched by unique `id`, else by position — keep their entities, hover/focus and
  app components; only appearing/disappearing ones are spawned/despawned. An identical render does
  nothing, so write contexts unconditionally, even every frame (no app-side diffing); give
  repeated or optional elements stable `id`s. Per-frame visuals are template values in a
  `style="…"` attribute (`width`, `opacity`, …), rounded to what's visible. Style-only changes
  restyle in place (`HtmlUiRestyled`). Never parent other entities under an `HtmlUi` root (the
  menu/lobby `WorldAssetRoot` backgrounds are separate entities). Material uniforms (crosshair
  GCD ring) stay component updates; overlays follow elements (`HtmlAnchor`) or world points
  (`HtmlWorldAnchor`) by bevy_markup.
- App components on built elements (an atlas `ImageNode` for an icon, a `MaterialNode`, a
  marker a per-frame system queries) are declared in the template: `<div is="<name>" data-…>`
  runs the system registered with `app.define_html_element("<name>", system)`
  (`In<ElementConnected>`: entity, UI root, `data-*` dataset) on every spawn of that element,
  before `HtmlUiBuilt`, once per element entity (in-place updates and restyles keep it and what
  it attached; a changed `is`/dataset spawns a new element). No lookups by `id`.
- CSS handles `position`/insets, `z-index`, `border-radius`, `border-color` and
  `pointer-events: none` (inherited; re-enable with `auto`) on elements *and* on each `HtmlUi`
  root: every template is wrapped in `<html class="<surface>-root">`, and `theme.css`'s "Roots"
  section places, sizes, stacks and (un)picks the roots — `z-index` there is Bevy's `ZIndex`
  among roots (pause menu 100/101, selector popup 900, tooltips 1000; the agent cursor's
  `GlobalZIndex::MAX` stays on top). Spawning code sets no placement: overlays beside an
  element use `HtmlAnchor`, over a world point `HtmlWorldAnchor`. CSS leaves undeclared `Node` fields and
  components alone and gives back anything it stops declaring. `overflow` works; combinators don't. No `border-image` (its
  image would never load under `--no-render`, so the UI would never build).
- `button` isn't a container: buttons are `<div class="button" id="…" data-on-click="…">` with a
  `<p data-l10n-id="…">English fallback</p>` label — write them with the shared components in
  `html/components.html` (`{% include "components.html" %}`, then
  `{{ <ui.button id=… signal=… key=… label=… /> }}` / `<ui.selector_toggle …/>`). Give every
  clickable a stable `id`.

### Headless camera mechanics (`--mcp`)

- With no primary window, `retarget_cameras_to_offscreen` points every camera at the offscreen
  texture. The first claimed camera keeps the clear (order 1); later ones get increasing order and
  `ClearColorConfig::None`; the newest camera's 3D view wins.
- Retargeting calls `projection.set_changed()` so `camera_system` recomputes `target_info` (a camera
  whose target was fixed after it was added would otherwise never render).
- Exactly one `IsDefaultUiCamera` must exist: `maintain_default_ui_camera` hands the marker between
  the headless bootstrap UI camera (`HeadlessUiCameraBootstrap`) and the player camera, and
  `keep_ui_camera_drawn_last` keeps the holder drawn last.

## Architecture: shared (`crates/shared/src/`)

- **`replication.rs`** — `SharedReplicationPlugin` and the `OrderedReliable` channel
  (`LookDirection` is registered in `inputs.rs`, next to the input protocol).
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
  before it simulates tick N. Defines the `Look` action (absolute `Vec2(yaw, pitch)`) and the
  replicated `LookDirection` component, and registers `apply_look`, the `Fire<Look>` observer
  both binaries run: it validates the input (`LookDirection::from_input`: finite, yaw wrapped,
  pitch clamped to `MAX_LOOK_PITCH`) and writes `LookDirection` and ahoy's `CharacterLook`
  (skipping `Dead` characters).
- **`player.rs`** — `PlayerCharacter`, `Selectable`, `PlayerCharacterSpawner`; `player(name,
  position)` bundle: capsule `Collider::capsule(0.4, 1.0)`, `RigidBody::Kinematic`, collision
  layers, `HitPoints {100,100}`, `Gcd`, ahoy `CharacterController` + `CharacterLook`, the
  `PlayerInputContext`, `LookDirection`, and unbound action entities (`Movement`, `Jump`,
  `Look`). `generate_player_name` makes unique "Adjective Noun" names
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

## Architecture: server (`crates/server/src/`)

Headless (`MinimalPlugins`): `StatesPlugin`, `LogPlugin`, `.init_asset::<Mesh/Image>()` are added
explicitly, and `crates/server/Cargo.toml` enables `bevy/reflect_auto_register` (world-asset spawning
reflects every component). The GLTF/Skein/world-serialization pipeline needs no GPU.

- **`main.rs`** — `build_app(networking_plugin)` shared by the live server and `--replay`.
  `ScheduleRunnerPlugin::run_loop(1/60 s)` (the default busy loop burns ~2 cores).
  `SingleThreadedExecutor` and one-thread task pools (determinism and VPS density).
  `SkeinPlugin { handle_brp: false }` (`ServerToolsPlugin` owns BRP). `ServerState`
  `Startup → Lobby` once `LevelMetadataAssets` loads.
- **`networking.rs`**
  - `start_endpoint`: binds UDP `0.0.0.0:6000`; loads or creates the netcode key at
    `<server asset root>/network/netcode.key` (`assets/server/network/` in a dev checkout; see
    "Asset root resolution") and the self-signed TLS identity (`token-tls.crt`/`.key`) beside it.
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
    `RemoteId` if the request's `predict` is set (else to nobody: the owner interpolates its
    character), `ControlledBy { owner, lifetime: Persistent }`, `ClientInGame`,
    `ChildOf(InGameRoot)`; moves the connection to the game room.
  - `owned_players(connection)` resolves a connection's characters.
- **`rooms.rs`** — allocates `GameRoom`/`LobbyRoom` and spawns the two roots at `Startup`.
  `HierarchySendPlugin::<ChildOf>` is already added by lightyear; don't add it again.
- **`lobby.rs`** — replicates `Levels` to the lobby room; puts each new connection in the lobby
  room.
- **`level_state.rs`** — `LevelState`: `Idle` → `Loading(path)` → `LevelLoaded(path)`; any
  `LoadLevelRequest` outside `Idle` is rejected and logged.
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
  `game/select_level`, `game/input` (action-level `ActionMock`; bypasses modifiers; `rotate`
  mocks the client-local camera action), `game/gamepad`
  / `game/keyboard` / `game/mouse` (device-level mocks through real bindings), `game/ui`,
  `game/cameras`, `game/screenshot` + `game/screenshot/get`.
- `game/ui` marks a node `clickable` iff it has a bevy_markup `data-on-click` hook
  (`ElementSignals`); its `text` is the node's text block (`Text` + `TextSpan` runs), buttons
  aggregate their subtree; `interaction` comes from bevy_markup's `PseudoState` (`:active` /
  `:hover`). Gamepad South and Enter (`game/gamepad` / `game/keyboard`) both confirm the focused
  element headlessly.
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
  console_closed)`; the *continuous* replicated input path is frozen separately by
  `controls.rs`'s `gate_replicated_input_context`, which deactivates the local player's
  `PlayerInputContext` via BEI's `ContextActivity` while the dev console or pause modal is
  open (without it, BEI's binding readers keep streaming WASD/Space to the server while the
  player types — bug_0007). Lifecycle/spawn observers stay ungated (a gated one-shot trigger is
  lost forever).
- **Look is client-owned input, never accumulated** (ADR 0017). The client sends its absolute
  camera direction as the `Look` action every tick; both sides apply it with
  `p19_shared::inputs::apply_look`. Don't send look deltas or derive look from a transform:
  two accumulators of one delta stream drift apart permanently whenever either misses or alters
  a step (bug_0009, bug_0010), and ahoy's `spin_character_look` rotates `CharacterLook` with a
  spinning floor. Read where a character looks from `LookDirection`, not `CharacterLook`.
- **Inputs** are `#[derive(InputAction)]` markers in `controls/actions.rs`, handled by observers
  (`On<Fire<T>>`, `On<Start<T>>`, `On<Complete<T>>`).
- **`EntityEvent`** supports one target; for two-entity events (`Attack`), the scoped side is
  `entity`, the other a plain field.
- **BRP**: components must be `Reflect` + `#[reflect(Component)]` to be queryable; handle-holding
  components come back as serialization errors (`-23402` = present, `-23403` = absent);
  `world.list_resources` lists reflected resources only.
- Hierarchy walks use `Query<&Children>::iter_descendants` / `Query<&ChildOf>::iter_ancestors`.
