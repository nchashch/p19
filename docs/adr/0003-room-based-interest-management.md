# 3. Room-based interest management for lobby/in-game separation

Date: 2026-09-21

## Status

Accepted

## Context

The project needed a real way to separate "clients in the lobby" from "clients in an active
game session" at the replication level, not just the UI level — a client sitting in the lobby
shouldn't receive level geometry, other players' movement, or combat state, and vice versa.
The explicit longer-term motivation: treat the lobby itself as a real, embodied level rather
than a flat 2D menu screen, since a purely flat UI doesn't make sense once a player is
embodied in VR — and support multiple simultaneous populations on one server (e.g. 3 players
in the lobby, 7 already in-game) without one group's traffic reaching the other.

`lightyear`'s replication layer ships a room-based interest management primitive
(`lightyear_replication::visibility::room`) built for exactly this shape of problem, already
a dependency of this project but not yet used for anything.

## Decision

Use `lightyear`'s built-in `Rooms` component and `RoomPlugin` rather than building a custom
visibility filter. At server `Startup`, allocate exactly two persistent rooms via
`RoomAllocator` — `GameRoom` and `LobbyRoom` — and spawn two persistent, content-agnostic root
entities, `LobbyRoot` and `InGameRoot`, each tagged `Rooms::single(...)` with its own room.
Any entity `ChildOf`-parented under one of these roots inherits that room's membership
automatically, confirmed via `lightyear_replication`'s own `HierarchySendPlugin::<ChildOf>`
cascade — already active by default through `lightyear`'s server plugin group, requiring no
extra registration. A client only ever receives replicated data for whichever room(s) its own
connection entity is currently a member of; moving a client between rooms is done by
*replacing* its `Rooms` component (`Rooms::single(new_room)`), not adding to it.

Every newly-connected client joins `LobbyRoom` immediately (`join_lobby_room_on_connect`);
the server-replicated list of available levels (`Levels`) is tagged `Rooms::single(lobby_room)`
so only lobby clients ever receive it; joining a game (`InGameRequest`) moves a client's own
connection entity into `GameRoom` instead.

## Consequences

- "N players in the lobby, M in-game, same server" requires no custom capacity or filtering
  logic — it falls directly out of room membership, which is exactly what the primitive is
  for.
- Level content parented under `InGameRoot` is fully covered with zero per-entity room
  tagging, by construction.
- **Two real bugs were hit and fixed while building this**, both instructive about the
  mechanism's sharp edges:
  - Room membership on a piece of *content* and room membership on the *watching client* are
    two independent things that both have to be set correctly — a client can be tagged into
    the right room for content to be visible to it, and still see nothing if its own
    connection entity's room membership wasn't updated too (or vice versa). This was hit
    directly: a client joining the game had its new player character correctly parented under
    `InGameRoot` (inheriting the room), but the client's *own* connection entity was initially
    left in `LobbyRoom` — nothing became visible to it despite the character existing
    correctly server-side, until the connection entity's own `Rooms` was explicitly updated
    too.
  - `GameRoom`/`LobbyRoom` were briefly assigned backwards (`LobbyRoot` tagged with
    `GameRoom`'s id and vice versa) — a plain copy-paste-shaped bug, not a design flaw, but
    worth noting since it produced a confusing "in-game root doesn't exist" symptom that was
    actually a *different*, second bug (see below) layered on top of it.
- **Known, not-yet-closed gap**: cubes and NPCs (`server::spawn`) are spawned as standalone
  entities, not `ChildOf`-parented under `InGameRoot`, and carry no `Rooms` tag of their own —
  they currently bypass room filtering entirely and replicate to every connected client
  regardless of room, including clients still in the lobby. Fixing this means giving
  `server::spawn`'s cube/NPC spawn calls a `ChildOf(in_game_root)` the same way the level's
  `WorldAssetRoot` and player characters already get.
- "Lobby as an actual embodied level" (real 3D content under `LobbyRoot`, not just a room tag)
  is the stated longer-term direction but not yet built — only the room-membership plumbing
  exists so far.
- This work also surfaced (but does not itself fix) the fact that `InGameRoot` being a single
  persistent entity, rather than one spawned fresh per level load, meant the level-loading code
  needed a matching update to parent new level geometry under the *existing* root instead of
  spawning a second one — see the "Server" section of `CLAUDE.md` for the current state of
  that fix and the dedup-protection regression it exposed.
