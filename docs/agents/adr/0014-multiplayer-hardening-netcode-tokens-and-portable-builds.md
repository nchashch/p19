# 14. Multiplayer hardening, per-client netcode tokens over TLS, and portable CI builds

| Field | Content |
|---|---|
| ADR | 0014 |
| Title | Multiplayer hardening, per-client netcode tokens over TLS, and portable CI builds |
| Date | 2026-10-02 07:27 +0400 |
| Author | Claude Opus 5.5 (Anthropic), via omp |
| Commit | `e1f4638` "Fix stale docs" + uncommitted: this ADR's trimmed Consequences and updated header |
| Span | `d2de40d..d333a09` (the commit after ADR 0013's last edit through `HEAD`; 28 commits, 2026-09-26 → 2026-10-02) |
| Status | Accepted |
| Supersedes | ADR 0013's §1 "LAN whitelist" decision (`server_addr_check: false`), reversed here; the rest of 0013 stands |
| Related | ADR 0003, 0004, 0008, 0013; bug_0001–bug_0006; playtests 0013–0020 |

This is the first ADR written to `docs/agents/skills/adr.md`.

## Context

At ADR 0013 the server was deterministic and replayable, but the multiplayer game itself had
several confirmed breakages documented in `AGENTS.md`:

- Every spawn, attack and kill request was silently dropped. The M2 split moved the player
  onto a separate entity (ADR 0004), but the server still looked up `Gcd`/`Transform` on the
  connection entity.
- Remote players snapped to each update as it arrived instead of moving smoothly.
- Spawned cubes/NPCs replicated to lobby clients.
- Killing a player crashed the victim's `--no-render` client.
- The netcode private key was a hardcoded all-zero constant compiled into the client.
- The workspace only built on the owner's machine. It had path dependencies on sibling
  checkouts (`../polyanya`, `../bevy_mod_outline`, …), no CI, and Steam Deck builds depended
  on a manual toolbox step.

The first real Steam Deck LAN tests during this span then exposed two further connection
bugs (bug_0005, bug_0006).

## Decision

### 1. Portable dependencies and CI (`c4cd561`, `9d36146`, `7195f65`–`012b7c8`, `ec1690e`)

- **Path dependencies replaced** (`c4cd561`):
  - Plain crates.io releases where upstream is sufficient: `bevy_vello 0.14`,
    `bevy_seedling 0.8`, `audionimbus 0.16` with `auto-install`.
  - Pinned git branches on the owner's forks where a local patch is still needed:
    `bevy_tui_texture` (`fix/fonts`), `bevy_mod_outline` (`fix/skinned-motion-outline`, moved
    from crates.io in `4c66e40`), `gltf` (`feat/khr_texture_basisu`).
  - `polyanya`/`rerecast` were dropped for `vleue_navigator 0.16`.
- **GitHub Actions CI** (`.github/workflows/ci.yml`):
  - A host job on `ubuntu-latest` that builds the server (debug).
  - A shippable client build inside the official
    `registry.gitlab.steamos.cloud/steamrt/steamrt4/sdk` container, so CI produces the same
    glibc baseline as `scripts/steam_deck_toolbox.sh`.
  - `012b7c8` reduced Steam builds to the steamrt4 client only.
  - Several follow-up commits worked around runner disk exhaustion by freeing the
    preinstalled SDKs and mounting the VM root into the container.
- **Missing optional fonts no longer break the build** (`6a6e187`): the TUI panel is skipped
  when its font is absent. `client/build.rs` emits a `has_tui_font` cfg instead of failing
  `include_bytes!`, so asset-less checkouts (such as CI) still compile.

### 2. Server-authoritative caster resolution and dead-player agency (`7e90a1c`, `6d2fb0a`, `9377b41`)

- `apply_spawn_npc`/`apply_spawn_cube` (`7e90a1c`) and `apply_attack`/`apply_kill`
  (`6d2fb0a`) now resolve the sender's player character via `networking::owned_players`
  (`ControlledBy { owner: connection }`) before the cooldown and range checks. Fixes bug_0001;
  playtests 0013 and 0017.
- Combat only resolves a **living** caster. A corpse keeps its `Gcd`, so this check is what
  stops dead players attacking.
- Dead players' look input is gated server-side (`accumulate_look`, `Without<Dead>`). The
  owner's local corpse simulation is stopped too (`hide_dead` removes
  `AhoyCharacterController`).
- The broadcast `Attack.attacker`/`Kill.killer` now carry the **player character** entity,
  which is what client attack animations key off.
- **Death-path panic** (bug_0002, playtest 0016): `--no-render` disabled `RenderPlugin` and
  with it `SyncWorldPlugin`. The corpse despawn's render-sync hook then panicked inside the
  replication receiver. The fix adds `SyncWorldPlugin` explicitly in `--no-render` mode.

### 3. Replication correctness (`bf2a5c8`, `6d59fa0`, `4c66e40`, `c2c4b76`)

- **Remote interpolation** (playtest 0014): `client/src/gameplay/interpolated_remotes.rs`
  inserts lightyear's `Interpolated` marker on every replicated, non-predicted,
  non-`RigidBody::Static` body. Lightyear_avian's existing Hermite interpolation rules then
  apply. No server change.
- **Room-tagged spawns** (bug_0003, playtest 0015): standalone cubes/NPCs carry
  `Rooms::single(game_room)`.
- **Spawn visuals** (`4c66e40`): cubes/NPCs get their model through a `ClientWorldAsset` (an
  NPC child entity, the cube itself) instead of client-side spawning code.
- **`Transform` is no longer replicated** (`c2c4b76`). Bodies sync through
  `Position`/`Rotation`. Non-body children that need a local offset carry a replicated
  `ModelOffset(Vec3)`, which the client turns into a `Transform` (`apply_model_offset`).
- **KCC rollback** (bug_0004, playtest 0020): `CharacterControllerState` is registered with
  lightyear's `local_rollback()`. `AccumulatedInput` is deliberately not registered, because
  it is re-derived from the replayed input every tick.

### 4. Per-client netcode tokens over a TOFU-pinned TLS endpoint (`81d75bf`, `89e44cc`, `31d19fd`)

- The client no longer holds the netcode private key.
- **Server key file:** the server loads or creates a random key at
  `server/assets/network/netcode.key`. Its path resolution mirrors `bevy_asset`'s:
  `BEVY_ASSET_ROOT`, then the runtime `CARGO_MANIFEST_DIR`, then the executable's directory.
- **Token endpoint:** an HTTPS endpoint on `:6001` (`GET /connect_token`) mints a fresh
  per-client connect token with a 30 s expiry. TLS uses a self-signed identity generated
  with `rcgen` on the `aws-lc-rs` provider, which is already in the dependency graph via
  `rmcp`.
- **Client pinning:** the client pins the certificate's SHA-256 fingerprint on first use, in
  `client/assets/network/token-tls-fingerprint.txt`. Playtest 0019 covers the endpoint.
- **Token address** (bug_0005): the token's server-address whitelist is the address the
  client used to reach the endpoint, taken from the HTTP `Host` header. The fallbacks are the
  host's default-route IP, then the peer IP. Using the peer IP told LAN clients the server
  was at their own address.
- **Server address check** (bug_0006): `server_addr_check: true` is restored, which reverses
  ADR 0013 §1. `additional_expected_addresses` lists `127.0.0.1:6000` and the default-route IP
  `:6000`, because the wildcard bind `0.0.0.0:6000` can never match a concrete whitelist
  entry.
- **Key rotation:** the original key and TLS identity had been committed to git by accident.
  They were rotated, and the network identity files moved out of the root `./assets/` into
  each crate's own `assets/network/` (`31d19fd`).

### 5. Player identity and agent QA (`a8c3b1d`, `8c851b8`, `6d2fb0a`)

- Player names are generated as "Adjective Noun" (`shared::player::generate_player_name`),
  seeded from `tick ^ connection bits` per ADR 0013's determinism rule. Collisions get `#2`,
  `#3`, … suffixes. These are the repo's first unit tests.
- The agent tool API gained `game/select` (`{"entity"}` or `{"nearest": true}`) and
  `game/trigger attack|kill`. Headless clients can now fight without crosshair raycasting,
  which needs a window.

### 6. Defect tracking as a first-class artifact (`1f6a99f`, `f486cd1`)

- Bug reports are numbered typst files under `docs/agents/bug_reports/`, written to
  `docs/agents/skills/bugreport.md`.
- All agent-facing docs (ADRs, skills, playtests, bug reports) moved under `docs/agents/`.

## Alternatives considered

- **Keep `server_addr_check: false`** (ADR 0013's choice). Rejected: once tokens carry a real
  address, the check is the standard netcode guarantee that a token is only valid for this
  server. Listing the expected addresses fixes the wildcard mismatch without dropping the
  check.
- **Hardcode the LAN IP in `additional_expected_addresses`.** Rejected: DHCP changes it. The
  default-route lookup at startup tracks it automatically.
- **Use the peer IP as the token's server address.** Shipped first in `81d75bf`, then reverted
  by bug_0005: the peer IP is the client's own address, not the server's.
- **CA-signed certificate, or an asymmetric LAN key exchange, instead of TOFU.** Deferred to
  production. TOFU encrypts the token fetch and detects later certificate swaps, but by
  definition it trusts whoever answers first (playtest 0020, next steps).
- **Server-side `InterpolationTarget::to_clients(AllExceptSingle(owner))`** instead of the
  client-side `Interpolated` marker. Not used: the marker gets the same result without a
  server change for each spawned entity type.
- **Snapshot ahoy's KCC internals manually, or patch ahoy, for rollback.** Unnecessary: ahoy
  0.2 already derives the `Component + Clone` that `local_rollback()` requires. The earlier
  "needs ahoy-side exposure" assessment was wrong (bug_0004).
- **Keep sibling path dependencies and document a checkout layout.** Rejected: CI and any
  second machine would need the exact sibling tree. Pinned git branches keep the patches
  reproducible.

## Consequences

**Gained**

- All six bug reports filed in the span are `Fixed`, each verified live or by unit test.
- Combat, spawning, death and room isolation work end-to-end, including on two `--no-render`
  clients (playtest 0017).
- The netcode private key never reaches a client.
- A fresh clone compiles without sibling checkouts, and CI produces a Steam-shippable client.

**Costs and known regressions**

- `client/src/ui/framework.rs` (added in `c4cd561`) is an unwired 9-slice button demo with
  `main`/`setup` functions. It contributes to the 38 dead-code warnings in `client`.
- The connection now needs a second port (`6001/tcp`) to be reachable alongside `6000/udp`.

**Still open**

- Disconnected players' characters (`Lifetime::Persistent`) are never despawned, which blocks
  a clean reconnect.
- All players spawn at the same point, so a joining player stacks on an idle one (playtest
  0018).
- The replay movement-rate mismatch from ADR 0013 is unchanged.
- Production netcode posture: a CA-signed certificate on a real backend.

## Also since 0013

- `b2ded4a`, `d333a09`: `AGENTS.md`/`README.md` refreshes, including the stale-doc sweep after
  the bug ledger closed.
- `7fb8e30`: playtest 0018, two-player stacking investigation. It established that the
  "joiner hover" is caused by the shared spawn point, not a KCC bug.
- `e9a2344`: `steam_deck_toolbox.sh` moved to `scripts/`.
