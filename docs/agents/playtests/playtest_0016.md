# Agent Playtest 0016 — Death-Path Panic Root-Caused and Fixed: a Missing SyncWorldPlugin, Not a Replication Bug

| Field | Value |
|---|---|
| Date | 2026-10-02 02:30 – 03:10 local |
| Commit | `012b7c8` "Only build steamrt4 client" + this session's change (uncommitted at run time): `SyncWorldPlugin` added to the `--no-render` branch of `client/src/main.rs` |
| Agent | opencode session, GLM-5.3-Flash |
| Clients | 1x `target/debug/client --mcp --no-render` (dev-tools feature), host ports; 240-300 s `timeout` lifetimes |
| Server | `target/release/server` — fresh per round |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · client QA: BRP :15702 + MCP :15710 · server QA: BRP :15701 |

## Purpose

Fix the top blocker from playtest 0015 (F3): killing a player panicked the victim's client —
symptomatic as `lightyear_replication::client::sync_last_confirmed_checkpoint` requiring
`Res<ServerMutateTicks>` that didn't exist, preceded by `VisualCorrection` remove-command
errors racing the corpse despawn. Ablation-proven pre-existing; blocks all end-to-end death
verification.

## Root cause (the visible panic was a decoy)

Source-tracing + live probing established the real chain:

1. `--no-render` disables `RenderPlugin`. `RenderPlugin::build` is what adds `ExtractPlugin`,
   and `ExtractPlugin::build` is what adds `bevy_render::sync_world::SyncWorldPlugin` — the
   only inserter of `PendingSyncEntity`. So `--no-render` clients never have it.
2. Separately, any component registered with `ExtractComponentPlugin`/`SyncComponentPlugin`
   gets an **on-remove hook** that does `world.resource_mut::<PendingSyncEntity>()`. Those
   hooks are registered by main-app plugins that **do** run in `--no-render`.
3. Killing the player → 1 s corpse despawn → replication applies the despawn → the hook fires
   → `PendingSyncEntity` missing → **panic inside
   `bevy_replicon::client::receive_replication`** — mid-function.
4. `receive_replication` removes `ServerMutateTicks` (and most of its other resources) at
   function start and re-inserts them at the end; the unwind skipped the re-inserts,
   **stranding the world without them**. The visible `sync_last_confirmed_checkpoint` /
   `ServerMutateTicks` failure on the next frame was collateral damage, not the bug.

Measurement trap documented along the way: `world.list_resources` only lists **reflected**
resources — `PendingSyncEntity` and `ServerMutateTicks` are non-reflected and never appear
there, so a live probe falsely suggested they were missing before any connection (both were
invisible in every run). The panic itself was the only reliable signal.

## The fix

One line in `client/src/main.rs`'s `--no-render` branch:
`add_plugins(bevy::render::sync_world::SyncWorldPlugin)`. Its `build` is main-world-only
(`init_resource::<PendingSyncEntity>` + add/remove observers for `SyncToRenderWorld`); with no
render app nothing drains the pending queue, but it grows only per despawned sync-entity —
bounded by session activity on a headless agent host. Rendered modes unaffected (`RenderPlugin`
already adds it).

## Verification

End-to-end death loop, single clean session (fresh server + client):

| Step | Observed |
|---|---|
| Alive rotate baseline | `game/input rotate` moved server-side `CharacterLook.yaw` 0.0 → 1.0 (measurement path proven) |
| Kill: server BRP mutate `HitPoints.hit_points` → 0 | `kill_zero_hp` fired; client `game/state`: `dead: true`, `hit_points: 0`, still `InGame` (within the 1 s corpse window) |
| Dead rotate | Server-side `CharacterLook.yaw` **unchanged at 1.0** — the `Without<Dead>` look gate (playtest 0015's compile-only check) now runtime-verified |
| Corpse despawn (1 s `Dead` timer) | Client cleanly transitions to `Lobby` (`player_despawned: true`) |
| Panic count | **0** (playtest 0015: guaranteed crash on the same path) |

Server log showed the clean despawn flow (`client despawn request` at disconnect, no errors).

## Findings

**F1 — Death path fully unblocked.** Kill → `Dead` → gated look → despawn → clean Lobby
transition, zero panics. The dead-player window is now closed for movement (pre-existing
`RigidBody` removal), look (`Without<Dead>` gate, runtime-verified), and local corpse
simulation (`hide_dead` controller removal). Dead-**attacker** gating remains correctly deferred
to the combat caster-resolution fix.

**F2 — The error that points at a resource is not always the resource's bug.** The
`ServerMutateTicks` validation failure was real but **secondary**: the same unwind that stranded
it had first panicked inside `bevy_render`'s despawn hook. Worth generalizing: when a Bevy
system reports a missing resource that some plugin should have inserted, check whether an
**earlier** system in the same frame panicked mid-way through a remove-and-reinsert scope
(`receive_replication`'s structure makes this class possible by design). The definitive
attribution here came from ordering the client log: the `bevy_render` panic at line N
precedes the `ServerMutateTicks` failure at line N+1, and only the `bevy_render` one recurs
across runs.

**F3 — `world.list_resources` is reflected-only; it cannot be used to assert a resource's
absence.** Non-reflected resources (`PendingSyncEntity`, `ServerMutateTicks`) are invisible to
it in **every** state. This session's first probe concluded (wrongly) that both were missing
pre-connect; the panic-vs-no-panic behavior is the only trustworthy signal for this class.
Candidate `dev::tool_api` improvement: a raw `world.contains_resource::<T>`-style BRP method
that works for non-reflected resource types.

**F4 — Scope note: the panic was `--no-render`-specific.** Rendered clients initialize
`PendingSyncEntity` via the normal render stack, so real players were never affected — this
was an agent-fleet-only crash. The fix is still correct for rendered modes (idempotent:
`RenderPlugin` already adds the plugin there, and plugins are unique by type).

## Next steps

1. **Joiner hover** (playtest 0014's F3, unchanged) — second joiner floats at spawn height
   until first input; server-side KCC never ticks without an input stream.
2. **Combat caster resolution** — unblocks real combat, the deferred dead-attacker gate, and
   gameplay-reachable deaths (currently only reachable via BRP mutation).
3. **KCC-internal rollback registration** — unchanged.
4. **Netcode posture** — unchanged.

## Conclusion

The death path is fixed end-to-end and verified: kill → `Dead` → gated look → clean corpse
despawn → Lobby transition, zero panics. Root cause was a `--no-render` bootstrap gap
(missing `SyncWorldPlugin`), with the alarming `ServerMutateTicks` error as collateral of
`receive_replication`'s remove-reinsert structure being unwound mid-scope. Two measurement
traps documented for future sessions (F2's mid-scope-unwind attribution rule, F3's
reflected-only resource listing). AGENTS.md updated: the dead-player gap entry now records the
fix and root cause, and the `combat.rs` bullets/TODO reflect the closed blocker.
