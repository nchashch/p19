#set document(
  title: "Bug 0002 — Killing a player panics the victim's client (--no-render death path)",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Bug 0002 — Killing a player panics the victim's client (`--no-render` death path)

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Bug*], [bug_0002],
  [*Date discovered*], [2026-10-01 (playtest 0015 F3)],
  [*Commit (state actually running)*], [Discovered at `012b7c8` "Only build steamrt4 client" + uncommitted work tree. Ablation-confirmed pre-existing (reproduced with the candidate fix stashed). *Fixed in* `9377b41` "Fix dead player bug"],
  [*Discovered by*], [opencode agent (GLM-5.3-Flash), during dead-player-window verification],
  [*Component*], [`client` render-sync bootstrap × `bevy_replicon::client::receive_replication` × `lightyear_replication::client`],
  [*Severity*], [S1 — crash; made every death end the victim's app],
  [*Status*], [*Fixed in* `9377b41` — verified live: kill → `Dead` → gated look → clean 1 s corpse despawn → back to Lobby, zero panics],
  [*Related*], [bug_0001 (the combat fix that first made deaths reachable), playtest 0015 F3 + playtest 0016 (root cause + fix), AGENTS.md dead-player gap entry],
)

= Summary

Killing a player crashed the victim's client. The visible error was misleading —
`sync_last_confirmed_checkpoint` failing on a missing `Res<ServerMutateTicks>` — but that was
collateral. The real panic happened one frame earlier, inside
`bevy_replicon::client::receive_replication`, when applying the replicated despawn of the
corpse: `bevy_render`'s `SyncComponent` on-remove hook demanded `PendingSyncEntity`, a
resource only the render stack inserts — which `--no-render` mode deliberately never creates.

= Steps to reproduce

1. Launch the server and a `--mcp --no-render` client (the crash is `--no-render`-specific;
   rendered clients have `PendingSyncEntity` via the normal render stack).
2. Connect → select level → play.
3. Server-side, kill the player (the only path available pre-combat-fix was
   `world.mutate_components` on `HitPoints.hit_points` → 0 — the schema is
   `{entity, component, path, value}`, reflect sub-path, not a properties object).
4. ~1 s later the corpse despawns (server `Dead` timer) → the client panics:
   `Requested resource bevy_render::sync_world::PendingSyncEntity does not exist` inside
   `receive_replication`, followed on the next frame by
   `sync_last_confirmed_checkpoint: Parameter Res<'_, ServerMutateTicks> failed validation:
   Resource does not exist`, and the app dies.

*Expected:* the corpse despawns, the client transitions to `Lobby`, no panics.
*Actual:* guaranteed client crash on every death.

= Evidence

- Playtest 0015 F3 (discovery, full panic logs) and playtest 0016 (root cause + fix), with
  the ablation run (candidate fix stashed → identical crash) that proves it pre-existing.
- The `ServerMutateTicks` failure was *collateral*: `receive_replication` removes
  `ServerMutateTicks` (and most of its resources) at function start and re-inserts them at
  the end; the sync-component-hook panic unwound past the re-inserts, stranding the world
  without them.

= Root cause

`--no-render` disables `RenderPlugin`, whose `build` adds `ExtractPlugin`, whose `build` adds
`bevy_render::sync_world::SyncWorldPlugin` — the only inserter of `PendingSyncEntity`. Any
component registered with `SyncComponentPlugin` carries an on-remove hook that does
`world.resource_mut::<PendingSyncEntity>()`; the first despawn of such an entity (the corpse)
therefore panicked the main-world replication receiver.

= Fix

- `9377b41` — the `--no-render` branch of `client/src/main.rs` adds
  `bevy::render::sync_world::SyncWorldPlugin`. Its `build` is main-world-only (resource + two
  observers); with no render app the pending-sync queue is never drained but grows only per
  despawned sync-entity. The same pass made `hide_dead` idempotent (skip already-`Hidden`
  corpses) — a related per-frame command-spam race a remote observer hit on the same path.
- Verified live end-to-end: kill → `Dead` → gated look → clean 1 s despawn → Lobby, zero
  panics (playtest 0016).

= Follow-ups

- Death verification is now possible through gameplay: after bug_0001's combat fix, the full
  kill-to-despawn loop ran clean headlessly (playtest 0017).
- Upstream-worthy observation: rustls-style strictness — bevy_render's sync hook panicking on
  a missing resource in headless builds is a known headless-Bevy friction class; a graceful
  `get_resource_mut` would make `--no-render` builds despawn-safe by default.
