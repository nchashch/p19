# 9. Expose an agent tool API on the client via the Bevy Remote Protocol, MCP-wrappable

Date: 2026-09-22

## Status

Accepted — implemented (the `dev-tools` cargo feature on the client: BRP with the `game/*`
methods + the in-process rmcp MCP server; the BRP server itself is already provided by
`bevy_skein`'s default-on `brp` feature in dev builds, which the plugin reuses when present).

## Context

The ahoy migration (see [0008](./0008-adopt-bevy-ahoy-character-controller.md)) made the game
client-driven-testable in principle: connect, spawn, move, jump, look. Exercising all of that
is still manual — every QA loop (does X break Y?) costs a human a play session. An LLM agent
could do that work, but it needs an API surface: read game state, take a rendered screenshot,
inject input, run scenarios — the "eyes and hands" pattern.

Two standards cover this landscape now:

- **MCP (Model Context Protocol)** — the de-facto LLM tool-API standard, stewarded by the
  Linux Foundation's Agentic AI Foundation (the same home as AGENTS.md). An MCP server exposes
  **tools** (actions), **resources** (data), and **prompts** over JSON-RPC, with two transports:
  stdio (the agent spawns the process) and **Streamable HTTP** (the application listens on a
  port). Every major agent harness — Claude Code, Codex, Cursor, opencode — is an MCP client.
  The game-engine world has converged on this shape: Roblox Studio ships a built-in MCP server
  with playtest automation, and the Godot ecosystem has several (godot-mcp, Open Godot MCP,
  GoPeak) whose core tool set is exactly the target here — scene/state inspection, input
  injection, screenshot capture, deterministic playtesting.
- **BRP (Bevy Remote Protocol)** — Bevy's built-in answer, `bevy_remote`: **JSON-RPC 2.0 over
  HTTP** (`RemotePlugin` + `RemoteHttpPlugin`, default `127.0.0.1:15702`, plus a second port
  for the render subapp when `bevy_render` is on). Built-in methods cover ECS
  inspection/mutation: `bevy/query` (reflected queries with filters), `bevy/get_components`,
  `bevy/list` (+ `+watch` long-poll variants), `bevy/spawn`/`despawn`/`insert`/`remove`,
  `bevy/registry/schema` — and `register_remote_method` adds arbitrary custom handlers. Every
  component this project replicates is already `Reflect + Serialize` (replication requires it),
  so they are BRP-inspectable essentially for free.

Project-specific constraints shape the design:

- **The client is untrusted by architecture** (the server is authoritative — see the top of
  `AGENTS.md`). Any tool API living in the client is a cheat surface for the multiplayer game
  and must never ship in player-facing builds.
- The BEI **`ActionMock`** mechanism exists already (built for VR locomotion) — programmatic
  action-value injection that overrides bindings. An agent's "input" can ride the exact same
  replicated-BEI pipeline a gamepad does.
- The dev console (`chill_bevy_console`) already establishes the "debug surface, dev-only,
  gated" precedent (`tracy` is the cargo-feature precedent).

## Decision

Build the tool API as **three layers, BRP as the foundation**:

1. **BRP as the data layer (nearly free).** Add `RemotePlugin` + `RemoteHttpPlugin` behind a
   new cargo feature (`dev-tools`, following the `tracy` feature's opt-in pattern) on the
   client. This alone makes the whole reflected ECS world readable (and mutable) over
   `127.0.0.1:15702` with `curl` — no agent framework required.
2. **Custom BRP methods as the game tools**, registered under the same feature:
   - `game/state` — a curated snapshot (local player entity/position/velocity, `HitPoints`,
     `Gcd`, `Dead`, grounded, current level, connection state). Agents do much better with a
     small structured view than raw `bevy/query` dumps; raw BRP remains available underneath.
   - `game/screenshot` — capture the main render target → PNG → base64 in the response.
   - `game/input` — inject player input via `ActionMock` on the replicated ahoy action
     entities (`Movement`/`Jump`/`RotateCamera` values for N ticks). This is the load-bearing
     choice: the agent's input flows through BEI → the replicated-BEI stream → the server's
     authoritative sim → back as corrected prediction. The agent *plays the game as a player*
     and every QA run also exercises the prediction/reconciliation path — the injection is
     end-to-end, not a debug teleport.
   - `game/console` (optional, later) — expose `chill_bevy_console`'s commands as tools.
3. **An MCP wrapper, deliberately deferred.** A thin MCP server (Python/TypeScript, ~100
   lines; or Rust via `rmcp`) proxying tool calls to BRP would make the surface native to
   every agent harness with schemas — but agent harnesses can already hit BRP over plain HTTP,
   and the wrapper is only worth writing once the tool set has stabilized. Defer; nothing in
   layers 1–2 depends on it.

Deterministic stepping (freeze the clock, step exact ticks — the Godot tools' gold standard)
is explicitly **out of scope**: it fights the live replicated timeline and the fixed-tick input
delay machinery. Input injection through the real pipeline is the honest approximation for a
networked game, and covers the QA cases that matter.

Security posture: the HTTP transport binds **localhost only** (BRP's default; it is
unauthenticated by design — never exposed beyond loopback), the entire surface is behind the
`dev-tools` feature so player-facing release builds contain none of it, and none of it runs on
the server binary (a server-side tool API is a possible follow-up for authoritative
inspection, with the same feature-gating).

Two implementation findings recorded here so they're not re-tripped:

- **The MCP server must NOT bind port 15703** — that's `bevy_remote`'s **render-subapp BRP
  port** (`DEFAULT_RENDER_PORT`, live whenever `bevy_render` runs). Binding our listener there
  made the render app's BRP bind fail and the **main** BRP pipeline hang (accepted TCP, no
  response) in release builds. MCP now uses **15710**.
- **BRP's builtin method names are `world.*`-namespaced in Bevy 0.19** (`world.query`,
  `world.get_components`, `world.list_components`, `world.spawn_entity`, `world.insert_components`,
  `+watch` long-poll variants, `world.registry.schema`, …) — the older `bevy/*` names seen in
  third-party docs don't exist anymore.

## Alternatives considered

- **In-process MCP server only (no BRP).** Ties the tool surface to MCP clients and a specific
  SDK; loses the curl-able ergonomic, the Bevy editor/inspector ecosystem that already speaks
  BRP, and the free built-in ECS methods. BRP-first makes MCP a thin proxy rather than a
  parallel implementation.
- **Hand-rolled socket protocol (extending the dev console).** Reinvents BRP (JSON-RPC,
  transport, reflection serialization, method registry) with nothing gained; the console stays
  for interactive human use.
- **Computer-use agents (OS-level screen capture + input injection).** Works with zero game
  code, and remains the fallback for cross-app QA — but it's slow, flaky, OS-dependent, and
  structurally can't read game state except through pixels. The in-process API is
  deterministic and structured; computer-use can still layer on top for visual-only checks.
- **Server-side tool API instead of client-side.** Authoritative inspection is attractive, but
  the rendering (screenshots), the input devices, and the QA surface all live client-side; the
  server's state is already visible *through* the client's replicated copy. Follow-up material.

## Consequences

- Agents get eyes (`game/screenshot`), hands (`game/input` through the real replicated pipeline),
  and state access (`game/state` + raw BRP) — QA and regression testing become scriptable, and
  agent-assisted development (the primary consumer, given this repo's AGENTS.md workflow) stops
  needing a human to ferry screenshots and observations.
- **Feature-gating discipline is load-bearing**: the tool API in a multiplayer client is a
  cheat surface; the `dev-tools` feature must be absent from every player-facing build. A
  leak here is a security bug, not an oversight.
- **BRP is not a stable public API** — reflection paths and method shapes can change between
  Bevy minor releases (Bevy's own docs say so); tooling built on it should declare a Bevy
  version range.
- BRP is not high-throughput (JSON-RPC per request, no streaming of large payloads beyond the
  screenshot case) — fine for QA cadence, not for realtime telemetry.
- Open questions deferred to implementation: the exact `game/*` method schemas; the
  screenshot mechanism (main-world capture vs the render-subapp port); whether the MCP wrapper
  is ever actually needed; headless server-side inspection.
