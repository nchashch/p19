#set document(
  title: "Bug 0003 — Spawned cubes/NPCs bypass room filtering and replicate to every client",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Bug 0003 — Spawned cubes/NPCs bypass room filtering and replicate to every client

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Bug*], [bug_0003],
  [*Date discovered*], [2026-10-02 (architecture review; present since `server::spawn` was migrated to lightyear)],
  [*Commit (state actually running)*], [Discovered at `bf2a5c8` "Add interpolation for physics replication" + uncommitted work tree. *Fixed in* `6d59fa0` "Fix NPC and Cubes replication rooms"],
  [*Discovered by*], [opencode agent (GLM-5.3-Flash)],
  [*Component*], [`server::spawn` (`apply_spawn_npc`/`apply_spawn_cube`), lightyear room-based interest management],
  [*Severity*], [S3 — degraded experience/correctness at scale; harmless at two clients],
  [*Status*], [*Fixed in* `6d59fa0` — verified live: in-game client sees its spawns; a lobby-held client sees neither],
  [*Related*], [AGENTS.md "Fixed: cubes/NPCs bypassed room filtering" entry + the room-based interest-management convention note (an entity with no `Rooms` component bypasses filtering entirely)],
)

= Summary

`spawn_cube`/`spawn_npc` entities were spawned standalone (no `ChildOf` under `InGameRoot`,
so the room-membership hierarchy cascade never reached them) and carried no `Rooms` tag. In
lightyear's room-based interest management, an entity with *no* `Rooms` component bypasses
room filtering entirely — so every spawned cube/NPC replicated to *every* connected client,
including clients still sitting in the lobby room, and the room system was effectively
inert for the highest-churn entity class.

= Steps to reproduce

1. Launch the server and two clients (A and B).
2. A: connect → select `levels/minimal.level.ron` → play. B: connect, but stay in the
   Lobby (never press play).
3. A: trigger `spawn_cube` and `spawn_npc`.
4. Query both clients for `shared::cube_spawner::Cube` / `shared::npc_spawner::Npc`.

*Expected:* A sees the spawns; B (lobby room) sees none.
*Actual (pre-fix):* B sees every spawn A makes — and vice versa for anything B spawns while
A is in-game.

= Evidence

Live two-client run post-fix: A `cubes: 1, npcs: 1`; B (lobby) `cubes: 0, npcs: 0`. Pre-fix
the same query on B returned every spawn (see AGENTS.md's corrected gap entry; also
playtest 0017's combat loop relied on this fix — headless attackers select targets that must
exist per-room).

= Root cause

Room membership on *content* and on the *watching client's connection* are two independent
things that must both be set. Level geometry and player characters get their room via the
`InGameRoot` hierarchy cascade (`HierarchySendPlugin::<ChildOf>`); spawned cubes/NPCs are
standalone entities, so nothing ever tagged them. The netcode convention: an entity with no
`Rooms` component replicates to every client its `Replicate`/`NetworkTarget` allows.

= Fix

- `6d59fa0` — both `apply_spawn_*` resolvers insert `Rooms::single(game_room.0)` (the same
  room the spawning player's connection joins in `in_game_request`); `GameRoom` threaded
  through both live systems and the replay driver's `replay_spawn`.

= Follow-ups

- Room-scoped cleanup: cubes/NPCs keep their room tag after their spawner disconnects
  (despawn-on-disconnect for non-`Persistent` owned entities is a separate known gap).
- When per-player rooms exist, spawned entities should join their *spawner's* room set, not a
  single global game room.
