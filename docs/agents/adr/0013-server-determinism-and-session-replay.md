# 13. Server-side determinism hardening and session recording/replay for post-release debugging

Date: 2026-09-25

## Status

Accepted — implemented (`server/src/main.rs`'s `build_app` refactor +
`TaskPoolOptions`/`SingleThreadedExecutor`; `server::replay`; the independent bugfixes listed
below). One known open gap, not yet resolved: replayed movement's magnitude doesn't yet match
the live session it was recorded from (see Consequences).

## Context

Since [0012](./0012-no-render-agent-client-mode.md), work moved from the client to `server`.
A run of independently-discovered, unrelated bugs got fixed along the way (a LAN connection
whitelist rejecting every non-loopback client, a debug-build startup panic, an unbounded
busy-loop burning ~200% CPU, a missing analog-stick deadzone). Separately, a real deployment
goal came up: pack many small game sessions onto one shared VPS, one OS process per session,
as cheaply as possible. Answering that — how to size the server's Bevy task pools and system
scheduler for a shared box instead of a dedicated one — led to a second, unplanned benefit:
a server whose *only* remaining source of nondeterminism was wall-clock time (nothing here
does real I/O during a live tick) is also a server whose entire session can be recorded
(every client message plus every tick's resolved input) and later *re-derived byte-for-byte* —
which is genuinely valuable for a bug reported after release, where there's no way to attach a
debugger to something that already happened. Two further fixes were needed to actually reach
that guarantee, and building the recorder/replay itself surfaced a real, non-obvious discovery
about how BEI input mocking does (and doesn't) work server-side.

## Decision

### 1. Independent server hardening
- **LAN whitelist**: `NetcodeConfig::server_addr_check` was validating an incoming connect
  token's embedded server address against the server's own bound `LocalAddr` (`0.0.0.0`) —
  which no real client's address ever equals, so every non-loopback connection was silently
  rejected. Set `server_addr_check: false` (`server::networking::start_endpoint`).
- **Debug-build startup panic**: `bevy_skein::SkeinPlugin`'s `handle_brp` default
  (`cfg!(debug_assertions)`) collided with `server::tools::ServerToolsPlugin`'s own
  unconditional `RemotePlugin` add. Set `handle_brp: false` explicitly, mirroring the client's
  pre-existing identical fix.
- **Unbounded CPU**: `MinimalPlugins`' default `ScheduleRunnerPlugin` is
  `RunMode::Loop { wait: None }` — a genuine busy loop, no sleep ever. Measured ~200% CPU with
  zero clients connected. Fixed with `.set(ScheduleRunnerPlugin::run_loop(1/60s))`, matching
  `server::ServerPlugins`' own tick duration.
- **Missing deadzone**: the right stick (camera look) had no `DeadZone`, unlike the left
  stick/UI navigation actions — added the same `DeadZone { kind: Radial, .. }` modifier the
  others already use.
- **Observability**: enabled `lightyear`'s `debug` feature, giving structured JSONL tracing of
  the replicated input pipeline (`server_input_message_recv` and friends) via
  `$LIGHTYEAR_DEBUG_FILE`.

### 2. Single-threaded, right-sized task pools
`TaskPoolOptions`' default sizes every pool (IO/async-compute/compute) off the *host's* total
core count, not a per-process budget — fine for one server per machine, but every instance
sharing a VPS would size its pools off the same host-wide count, oversubscribing threads long
before CPU is the actual bottleneck. `TaskPoolOptions::with_num_threads(1)` pins every pool to
one worker.

Separately, for a different reason: Bevy's default `MultiThreadedExecutor` gives no execution
order guarantee for two systems with no explicit `.before()`/`.after()` between them — an
"ambiguity" whose resolution order is free to vary between runs given identical ticks and
inputs, which would silently break any deterministic-replay effort. `SingleThreadedExecutor`,
installed on `Update`/`FixedPostUpdate`/`PhysicsSchedule` (`avian3d` already does this for its
own inner `SubstepSchedule`), removes that class of nondeterminism outright. The two decisions
compound naturally — with every task pool down to one worker there's nothing left to run in
parallel with anyway.

### 3. Two further fixes, required specifically for replay
- `server::spawn`'s "random" angular velocity/spawn-angle was seeded from
  `Time::elapsed_secs()` — real wall-clock time, impossible to reproduce from a recorded log.
  Reseeded from `tick.0 ^ caster.to_bits()`.
- `server::combat`'s `Gcd`/`Dead` timers ticked `Res<Time>::delta()` in `Update` — real/virtual
  elapsed time, subject to OS scheduling jitter. Moved to `FixedUpdate` (runs exactly once per
  simulation tick, unlike `Update` which runs once per real frame regardless of tick count) and
  ticked by the fixed `TickDuration` instead.

### 4. `server::replay` — session recording + deterministic debug-replay
- **Recording** (`SERVER_REPLAY_RECORD=/path/to/session.jsonl`, always registered, no-op
  otherwise): every client→server gameplay message — 7 of the 9 `shared::replication` registers
  (`AttackAttempt`/`KillAttempt`/`SpawnCubeRequest`/`SpawnNpcRequest`/`LoadLevelRequest`/
  `InGameRequest`/`ClientDespawn`; not `LobbyRequest`, which has no handler anywhere in
  `server/src`, or `ObserveRequest`, which only touches replication-targeting bookkeeping with
  no effect on authoritative state) — plus every tick's resolved Movement/Jump/RotateCamera(×2)
  action value, tick-stamped, keyed by the connecting client's `PeerId` (stable across a
  session, unlike the ephemeral connection `Entity`).
- **Replay** (`server --replay /path/to/session.jsonl`): boots the identical `App` — `main.rs`'s
  plugin registration was refactored into a shared `build_app(networking_plugin)` both the live
  path and replay call, so every plugin/schedule-executor/asset-registration line is shared
  verbatim — driven by `TimeUpdateStrategy::FixedTimesteps(1)` (a real Bevy testing primitive,
  not a hand-rolled clock) instead of wall-clock-paced `ScheduleRunnerPlugin`, decoupling every
  simulated tick from real elapsed time entirely. Freezes the tick counter itself
  (`TimeUpdateStrategy::ManualDuration::ZERO`) while `ServerState::Loading`, since a level's
  `.glb` load is real async I/O whose completion can't be forced onto a specific tick.
- **Non-obvious finding, confirmed live**: `ActionMock` — the documented BEI input-mocking
  mechanism, and the one `client::dev::tool_api`'s `game/input` successfully uses — does **not**
  get consumed on this project's headless server. Confirmed via three separate diagnostic
  passes: the mock was inserted correctly every tick with the correct value; the same action
  entity's own `ActionValue`/`TriggerState` read back completely unchanged for the entire span;
  the player's `Transform` stayed static the whole time. Real server-side movement goes through
  a different code path entirely — `lightyear_inputs::server::get_action_state`, which
  transitions `ActionState` directly from the replicated input buffer, not through BEI's generic
  per-context `update()` system that `ActionMock` hooks into. The fix: trigger the exact
  `Fire<A>` event directly instead (`context`/`action`/`value`/`state` are all public fields) —
  the three real consumers (`apply_movement`, `apply_jump`, `server::input::accumulate_look`)
  all read straight from the event's own fields, not by re-querying the action entity, so this
  is a complete substitute for those three specifically.

## Alternatives considered

- **Rely on `TaskPoolOptions::with_num_threads(1)` alone**, skipping `SingleThreadedExecutor`.
  Starves the default executor of anything to race against in practice, but doesn't touch its
  code path — the executor swap is the actual guarantee, and cheap enough to just also do.
- **Client-recorded state replay** (record replicated `Position`/`Rotation` and play them back
  like a video) instead of server-side input replay. Explicitly not what this ADR is for: it
  needs none of the determinism work and is a genuinely simpler, different feature (a
  player-facing "watch my session" killcam) — but can't let a developer step through the
  *simulation* with a debugger, which is the actual ask.
- **Populate `MessageReceiver<T>` directly during replay**, so the existing live-handling
  systems run completely unmodified. Not possible: `receive()`/`receive_with_tick()` are the
  only public ways to get messages *out* of a `MessageReceiver<T>`, and there is no public way
  to push one *in* — it's populated exclusively by lightyear's own wire-deserialization
  pipeline. Instead, each message's business logic in `combat.rs`/`spawn.rs`/`networking.rs`
  was extracted into a plain `apply_*` function callable from both the live system and the
  replay driver directly.
- **Skip `networking::NetworkingPlugin` entirely in replay mode** (no real client ever connects
  during a replay, so binding a real socket seemed pointless). Reverted after a live panic:
  `lightyear_replication::server::receive_server_packets` depends on a resource
  (`RepliconChannelMap`) only inserted by `NetworkingPlugin`'s `Startup` bootstrap
  (`start_endpoint` → `server::Start`). Replay includes the real plugin — it still binds
  `0.0.0.0:6000`, unused — so don't run a replay alongside a live server on the same
  machine/port.

## Consequences

- Idle server CPU: ~200% → ~5% (measured via `/proc/<pid>/stat` across three separate windows,
  including one with an actively-moving client — confirming the original cost was never
  replication).
- Task-pool threads per instance bounded to a handful regardless of host core count — N
  instances now cost roughly N × (a few threads), not N × host-cores, the actual VPS-density
  goal.
- Deterministic replay confirmed live: the identical recorded log, replayed twice, produced
  bit-identical results both times (same entity IDs, same final `Transform`) — connect → level
  load → player spawn (correct `HitPoints`/`ControlledBy`) → movement in the correct direction
  all reproduce correctly.
- **Known, unresolved gap**: replayed movement's *rate* doesn't match the live session it was
  recorded from — live covered ~12.7 units over 60 held-movement ticks; the identical 60 ticks,
  replayed, covered under 1 unit (confirmed via per-tick position sampling across the whole
  span, not just comparing endpoints). Separately confirmed as *expected*, not a bug:
  `bevy_ahoy`'s `ground_accelerate` has no deceleration term at all when wish-velocity is zero
  (`Dir3::new_and_length` on a zero vector returns `Err`, leaving velocity untouched) — a
  character coasts in a straight line forever once input stops, live or replayed, so comparing
  "final position" is only meaningful at matched tick numbers, not after an arbitrary settle
  period. The rate mismatch itself is still open — leading suspect is the manually-triggered
  `Fire<A>`'s `fired_secs`/`elapsed_secs` fields, hardcoded to `0.0` here, feeding into
  `ground_accelerate`'s acceleration math differently than a real per-tick resolution would —
  not chased further this session; `server::replay`'s own module doc comment has the fullest
  detail on where to keep looking.
- Directly affects an existing open design note (AGENTS.md's NPC/AI-input plan: "server-side
  BEI mocking of an NPC context — ahoy's stated design goal"). That plan needs revisiting now
  that `ActionMock` is confirmed not to work server-side as currently wired — the `Fire<A>`
  -triggering approach built for replay is the more likely real path forward for NPC/AI input
  too.

## Also since [0012] (not independently significant enough for their own record)

- `scripts/deploy_steam_deck.sh` — builds and stages a release client for the SteamOS Devkit
  Client toolbox upload workflow.
- `client::ui::tui_panel` (`bevy_tui_texture` + `ratatui`, vendored with a local patch — see
  AGENTS.md's dependency-layout notes) — a small ratatui-rendered-to-texture demo panel on the
  main menu.
