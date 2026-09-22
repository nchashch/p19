# 8. Adopt bevy_ahoy as the character controller, over lightyear-replicated BEI input

Date: 2026-09-22

## Status

Accepted (implements the rebuild that [0005](./0005-gut-character-controller-for-prediction-rewrite.md)
gutted for; that ADR's "rewrite using lightyear's own prediction" direction is realized here)

## Context

ADR 0005 deliberately gutted the from-scratch kinematic character controller (`shared::character_controller.rs`,
originally an avian3d-example lift) pending a client-side-prediction rewrite. The rewrite needed
two things lightyear provides and the old controller lacked: replicated *input* (not just
replicated state) and rollback/reconciliation. Separately, the project's character-authoring
design (see `AGENTS.md`) wants one movement system for players, NPCs, and AI alike.

`bevy_ahoy` (0.2) is a kinematic character controller for exactly this stack — Bevy 0.19 /
avian3d 0.7 / `bevy_enhanced_input` 0.26, fixed-tick only, BEI-native (its input observers read
BEI actions and write an `AccumulatedInput` component the KCC consumes), and its design notes
call out BEI's input-mocking API as the intended path to "treat player and NPC input the same
way". `lightyear` 0.30 ships `lightyear_inputs_bei` — BEI action-state replication with
tick-buffered, packet-loss-protected, rollback-aware input — plus the `PredictionPlugin`
infrastructure this project had explicitly disabled for lack of an input plugin.

## Decision

Replace the old controller with **ahoy's KCC running on both binaries over one replicated BEI
input stream**, in three verified steps (M0–M2):

1. **M0** — `lightyear_inputs_bei` registered on both binaries (per-binary `client`/`server`
   features; the shared registration in `shared::inputs`), `PredictionPlugin` un-disabled. The
   meta-crate needs its `input_bei` feature for the `lightyear_inputs?/prediction` passthrough
   — without it `LastConfirmedInput` never initializes and the client panics at connect.
2. **M1** — ahoy's KCC drives the local player client-side: `CharacterController` +
   `CharacterLook` inserted on the local player, ahoy-typed actions bound.
3. **M2** — server-authoritative + prediction: `player()`'s bundle carries ahoy's
   `CharacterController`/`CharacterLook`, the `PlayerInputContext` context, and **bare,
   unbound** action entities; they replicate to the owning client (via the `ActionOf` hierarchy
   sender); the owning client's polling binder inserts the real bindings (which auto-adds
   `InputMarker` — starting the `BEIStateSequence` client→server stream); the server-side BEI
   mocking fires ahoy's observers there; `PredictionTarget::to_clients(Single(remote_id))`
   scopes prediction to the owner.

The old surface is deleted: the gutted `CharacterControllerPlugin` and its six `todo!()`
systems, the `Movement`/`Jump` client→server messages and their server-side translators, the
legacy `FpsCameraRotation`/`Movement`/`Jump` BEI actions, and the old controller's data
components (`CharacterMovementSettings`/`CharacterCollisions`/`GroundDetection`/`DesiredMotion`).
`Grounded` survives as a replicated marker, now written by a small bridge
(`character_controller::bridge_grounded`) from ahoy's `CharacterControllerState` on both
binaries — its consumers (grounded/idle animation transitions, the HUD indicator) were reading
state nothing had written since the gut. VR locomotion mocks the same replicated actions
(`ActionMock`): the left stick as local movement, the head's global-rotation deltas as look.

## Findings (each confirmed by testing during the migration)

- **ahoy authors `Transform`** (its `run_kcc` final write), so `LightyearAvianPlugin` must run
  `sync_to_transform: true` on both binaries, or `Position` — the replicated, physics-canonical
  state — never receives character motion. This also fixed player spawning: the bundle
  positions via `Transform`, which the disabled import had never let through to `Position`.
- **`RigidBody` must be replicated**: without it, replicated colliders are loose client-side
  (no `ColliderOf`), and ahoy's collision query (requires `Position`/`Rotation`/`ColliderOf` on
  collider entities) sees no level geometry — characters fall through everything.
- **BEI gives a binding's input to the first action that reads it each tick**; other actions
  reading the same input get zero. Duplicate look bindings (the legacy + replicated actions,
  both on mouse motion) split the mouse deltas nondeterministically (~70-90%/~10-30%, measured
  via wall-time-aligned dual logs), starving the server's accumulated look — "movement locked
  to one axis". One consumer per input, always.
- **The client must lead the server's timeline**: `InputTimelineConfig::default()` is
  `no_input_delay()`, so tick-N input arrived after the server simulated tick N (even on
  loopback — send in PostUpdate, arrive next PreUpdate) → `server_late_input_mismatch`
  corrections fighting the prediction. A 2-tick minimum delay (`balanced()` preset) covers the
  structural scheduling latency; the auto-recomputed delay covers real RTT up to ~50ms.
- Ahoy's input observers write the **context entity's** `AccumulatedInput` and its KCC runs on
  the `CharacterController` entity — one entity, both roles.

## Consequences

- **Movement works end-to-end**: server-authoritative, client-predicted, latency-tolerant, with
  ahoy's full feature set (sprint/crouch/surf/bhop/tic-tacs/mantling…) available by binding
  more of ahoy's existing action types — no controller code to write.
- **The NPC/AI symmetry the character-authoring design wants is now one step away**: ahoy's
  stated input-mocking pattern means server-side AI can drive an NPC context's mocked actions
  exactly like a player's replicated ones.
- Known remaining gaps: ahoy's KCC-internal state (`CharacterControllerState`/`AccumulatedInput`)
  is not rollback-registered (only the avian physics components are), so coyote/jump-buffer
  stopwatches don't rewind on corrections; other players aren't interpolated (no
  `InterpolationTarget`); during fast camera flicks the server's look transiently lags (per-tick
  look deltas partly lost under burst load) and re-converges; VR + keyboard simultaneously
  degrades to mock-only movement (mocks override bindings).
- The dev console's `kcc_debug` command (added during the migration) dumps the whole
  input→movement chain and stays as a permanent tool.
