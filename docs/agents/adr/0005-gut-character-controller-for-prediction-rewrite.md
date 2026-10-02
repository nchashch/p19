# 5. Gut the character controller pending a client-side-prediction rewrite

Date: 2026-09-21

## Status

Accepted

## Context

`shared::character_controller.rs` — a from-scratch kinematic move-and-slide implementation
originally lifted from an `avian3d` example early in the project's history, then adapted over
many commits for server-authoritative multiplayer — has no client-side prediction at all.
Every input round-trips through the server before its effect is visible to the client that
sent it. This is fine on loopback (where all this has been tested) but would feel laggy over
any real network latency.

The project wants to rebuild movement using `lightyear`'s own idiomatic prediction support
(`PredictionPlugin`, one of `lightyear_inputs_native`/`_bei`/`_leafwing`) instead of hand-
rolling client-side prediction on top of the existing bespoke controller. Confirmed by
research before this decision: zero prediction scaffolding exists anywhere in the codebase
today — no `lightyear_inputs_*` dependency, and `PredictionPlugin` is explicitly disabled
client-side already (it otherwise panics, since it assumes one of those input-replication
plugins has already initialized state this project doesn't use).

## Decision

Rip out the character controller's actual movement/physics *logic* now, as a distinct,
deliberate step ahead of (not combined with) the prediction rewrite itself. Every
`FixedUpdate` system body and both input-observer bodies (`update_grounded`, `apply_gravity`,
`integrate_horizontal_linear_velocity`, `apply_movement_damping`, `move_and_slide`,
`apply_forces_to_dynamic_bodies`, `on_movement_input`, `on_jump_input`) become `todo!()`
stubs. Every component/event **type** definition, and the plugin's system registration and
ordering, are kept completely intact.

This was chosen deliberately over the alternatives: leaving the old implementation running
while building prediction alongside it (would mean maintaining two movement systems
simultaneously mid-migration, and made it unclear which one was authoritative at any given
point), or deleting the whole module outright (would have cascaded into rewriting
`shared::player::player()`'s bundle construction, `shared::replication`'s replicated-component
list, and client-side presentation code that reads `Grounded`/`Character`/`Idle` for
animation/HUD display — none of which actually needed to change). Keeping the type surface
stable and only gutting bodies meant the entire rest of the workspace kept compiling
throughout, confirmed by a full-workspace grep beforehand establishing that the client never
runs this plugin at all (it's server-only), so the actual blast radius was fully contained to
one file plus a documentation correction.

## Consequences

- A clean, compiler-enforced scaffold for the rewrite: the six-stage pipeline shape (ground
  detection → gravity → acceleration integration → damping → move-and-slide → external forces)
  remains visible as a checklist in the stubbed function signatures and doc comments, even
  though the eventual lightyear-prediction-based design may not follow the identical stage
  breakdown.
- Zero changes needed anywhere outside `shared/src/character_controller.rs` plus one
  corrected `AGENTS.md` bullet — confirmed by `cargo check --workspace` passing immediately
  after the gut with no other edits.
- **The game is not currently playable past joining, by design.** `update_grounded` runs
  unconditionally every `FixedUpdate` tick for any `CharacterController` entity — not gated on
  receiving input — so the server panics the moment a player character exists, before any
  movement is even attempted. `CharacterControllerPlugin`'s registration is commented out in
  `server/src/main.rs` as a direct consequence, with a `TODO` pointing at this decision.
- Building the actual lightyear-idiomatic prediction-based replacement is **explicitly out of
  scope for this decision** — this ADR covers only the deliberate teardown. The rebuild is
  separate, future work, and does not yet have its own ADR since the concrete design (which of
  `lightyear_inputs_native`/`_bei`/`_leafwing` to use, how rollback/correction should interact
  with the existing move-and-slide algorithm) hasn't been decided yet.
