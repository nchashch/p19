# 4. Decouple the player character from the client connection entity

Date: 2026-09-21

## Status

Accepted

## Context

Since early in this project's history, a client's own server-side connection entity doubled
as its player character — `shared::player::player()`'s bundle was inserted directly onto the
same entity that represented the network connection. This was documented as a deliberate
design choice (identity for network-authoritative actions like `Movement`/`Jump` could be
resolved straight from the connection with no separate lookup).

Implementing room-based interest management (ADR 0003) surfaced a real bug in this design:
`ControlledBy { owner: connection_entity }`, needed to establish ownership for lightyear's own
replication-visibility filtering, was being inserted onto that *same* entity — i.e. `owner`
pointed at the entity it was already on. Bevy's relationship-validation system silently strips
a self-referential relationship like this (confirmed via a logged warning:
`"the ... ControlledBy(...) relationship on entity ... points to itself ... has been
removed"`). Worse, confirmed by testing rather than assumed: this wasn't just losing
`ControlledBy` itself — it was blocking replication of *sibling* components inserted in the
same bundle, including a newly-added `ClientInGame` marker the client needed to ever transition
into the in-game state at all.

## Decision

Stop treating the connection entity as the player character. `in_game_request` now spawns the
player character as a genuinely separate entity: `ChildOf`-parented under `InGameRoot`
(inheriting its room automatically via the cascade from ADR 0003), owned by the connection
entity via a real, non-self-referential `ControlledBy { owner: connection_entity, .. }`. The
connection entity itself is untouched by this bundle — it separately gets `Rooms::single(...)`
inserted directly, since it isn't a descendant of anything and so doesn't inherit a room from
a parent.

Client-side identity resolution ("which replicated entity is *my* player character") already
existed independently of this change and did not need to be rewritten: `lightyear::prelude::
Controlled` is auto-inserted client-side, by lightyear's own replication machinery, on
whichever entity a client actually owns — `client::gameplay::player_character`'s existing
`LocalPlayer` resolution (`Query<Entity, (With<PlayerCharacter>, Added<Controlled>)>`) was
already built around this and simply started working correctly once `ControlledBy` was fixed
server-side.

### Alternative considered and rejected: patch lightyear to allow self-reference

Bevy's relationship system supports opting into self-referential relationships via
`#[relationship(allow_self_referential)]` on the type definition. Since `ControlledBy` is
defined in the vendored `lightyear_replication` crate, this project already has a precedent
for exactly this kind of local patch (see `bevy_mod_outline`/`bevy_oxr` in `CLAUDE.md`'s
"Dependency layout" section). This was considered and explicitly not taken: it would have kept
the *identity conflated with ownership* design, which fights lightyear's own ownership model
(`ControlledBy.owner` is documented as naming a distinct entity with its own
`ReplicationSender`) rather than working with it, and self-referential ownership doesn't have
an obviously correct meaning to a future reader regardless of whether Bevy technically permits
it.

## Consequences

- Fixes the replication bug outright, with no vendored patch needed.
- More idiomatic use of lightyear's ownership model going forward — `ControlledBy`/`Controlled`
  now work the way lightyear's own documentation describes them, rather than being worked
  around.
- Breaks a previously load-bearing assumption: code that assumed "the connection entity that
  sent this message *is* the simulated character" specifically (as opposed to "resolve *who
  sent this* from the connection entity," which is unchanged and still correct for
  `Movement`/`Jump`/`AttackAttempt`/`KillAttempt`) needs updating. In particular, any future
  character-controller work (see ADR 0005) that wants to go from "which connection sent this
  input" to "which entity should this input be applied to" needs to resolve through
  `ControlledByRemote` (the relationship-target listing what a connection entity owns), not
  assume they're the same entity.
- A second, distinct `ControlledBy`-related issue was hit and is **not** fully explained: even
  once non-self-referential, having `ControlledBy` in the *same initial spawn bundle* as other
  newly-replicated components (specifically `ClientInGame`) still blocked those siblings from
  reaching the client — confirmed by testing (removing just `ControlledBy` from the bundle
  fixed it), but the underlying mechanism in `lightyear_replication` was not identified with
  certainty. The applied workaround — spawn the entity first, then `insert(ControlledBy {
  .. })` as a separate, subsequent command — works and is in place, but this is worth a closer
  look (or an upstream report) if the same symptom recurs elsewhere.
- New cleanup gap introduced: the player character is now spawned with `ControlledBy {
  lifetime: Lifetime::Persistent, .. }`, which means it is *not* despawned when its owning
  connection disconnects (unlike the old single-entity design, where disconnection handling
  and character cleanup were the same event by construction). This is currently unaddressed —
  see `CLAUDE.md`'s gap list.
