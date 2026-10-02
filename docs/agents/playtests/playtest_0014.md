# Agent Playtest 0014 — Remote-Entity Interpolation: Fix, Verification, and a New Joiner-Freeze Finding

| Field | Value |
|---|---|
| Date | 2026-10-02 01:00 – 02:10 local |
| Commit | `012b7c8` "Only build steamrt4 client" + this session's interpolation change (uncommitted at run time): `gameplay/interpolated_remotes.rs`, `main.rs` tuple rebalance |
| Agent | opencode session, GLM-5.3-Flash |
| Clients | 2x `target/debug/client --mcp --no-render` (dev-tools feature); client B on fleet ports :15712/:15713; 200-420 s `timeout` lifetimes |
| Server | `target/release/server` — fresh per round |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · client QA: BRP :15702/:15712 + MCP :15710/:15713 · server QA: BRP :15701 |

## Purpose

Fix crack #1 from the physics/replication architecture review: remote players, cubes, and NPCs
rendered at raw replicated `Position` — arrival-cadence snapping/jitter for anything not owned
by the viewing client. Goal: smooth delayed interpolation for remotes, no behavior change for
the locally predicted player or static level geometry.

## What the fix turned out to be

Source inspection of the vendored lightyear 0.30 crates showed the infrastructure was already
entirely in place — the missing piece was one marker component:

- `interpolation` is a **default** feature of the lightyear meta-crate, so `SharedPlugins`
  (inside the `ClientPlugins`/`ServerPlugins` groups both binaries use) already registers
  `InterpolationMarkerPlugin` + `InterpolationPlugin`. No plugin additions were needed.
- `LightyearAvianPlugin`'s `AvianReplicationMode::Position` auto-registers velocity-aware
  Hermite interpolation rules for the `(Position, Rotation, LinearVelocity, AngularVelocity)`
  bundle plus per-component linear fallbacks. No rule registrations were needed.
- lightyear 0.30's interpolation model applies values **in place** on entities carrying the
  `Interpolated` marker (no visual-copy entities as in older lightyear).

The change is therefore: `client/src/gameplay/interpolated_remotes.rs` — a polling `Update`
system (same replication-arrival reasoning as the other decoration systems) inserting
`Interpolated` on every replicated body that is **not** locally predicted (`Without<Predicted>`)
and **not** `RigidBody::Static`. Players (Kinematic) and cubes/NPCs (Dynamic) interpolate; the
entire static level skips `ConfirmedHistory` maintenance for zero visual benefit. Plugin
registered in `main.rs` (the outer plugin tuple was at Bevy's 15-element `Plugins` cap; the
gameplay/misc tuples are now nested as a 2-element tuple-of-tuples).

Deliberately **not** used: lightyear's server-side
`InterpolationTarget::to_clients(AllExceptSingle(owner))` — equivalent effect, but requires a
per-entity-type server change; the client-side marker achieves the same.

## Verification rounds

| Round | Procedure | Result |
|---|---|---|
| 1 | Server + clients A (host ports) and B (:15712); A connects/selects/plays first, B joins second; BRP `world.query` from B's view for `RigidBody` + option `Interpolated`/`Predicted`/`PlayerCharacter` | A's player on B: Interpolated + `ConfirmedHistory<Position/Rotation/LinVel/AngVel>`, no `PredictionHistory`. B's own player: Predicted-only (`Controlled`, `PredictionHistory`x4), no Interpolated. Static floor: untagged |
| 2 | Disambiguation: `world.list_components` per entity as ground truth | An earlier reading had shown `Predicted` seemingly present on the **remote** player too — ground truth shows it is NOT (see F2: BRP stale-alias artifact). Marker assignment is exactly correct |
| 3 | Freeze investigation: second-join client B's player sat at spawn height 2.73 indefinitely (`grounded: true`); server-side query showed the **authoritative** player also at 2.73 | Joiner's server-side KCC gets no ticks until the joiner sends any input — after a `game/input movement` call, B instantly landed (0.915) and moved. Not caused by this change: see F3's ablation |
| 4 (ablation) | Interpolation change stashed (`git stash -u`), client rebuilt, identical two-client scenario re-run | B **still** froze at 2.73 without the change — the freeze is pre-existing. Stash popped, rebuilt, re-verified the fix working in the same session |

## Findings

**F1 — Remote interpolation works, and was a one-marker change.** The full delayed-
interpolation pipeline (history buffers, `InterpolationTimeline` sampling, Hermite curves for
position+velocity) was already registered by default features + the avian replication mode.
`Interpolated` on non-predicted non-static bodies is the entire fix. Worth internalizing: the
workspace's lightyear feature set gives interpolation "for free"; any future entity type that
moves only needs to (a) carry `RigidBody`/`Position`/`Rotation` replication and (b) not be
locally predicted, and it interpolates automatically.

**F2 — BRP `option` queries can match components under stale TypePath aliases; use
`world.list_components` as ground truth.** Both clients' `world.query` option lists reported
`lightyear_prediction::components::Predicted` present on entities where per-entity
`world.list_components` showed no `Predicted` at all (the real component lives at
`lightyear_core::prediction::Predicted`). The alias resolved via the type registry and
silently matched. This cost one disambiguation round and briefly suggested an
Interpolated+Predicted conflict that does not exist. (Same family as playtest 0013's F2:
unreflected components make queries silently **empty**; here, aliased names make them silently
**wrong**.)

**F3 — NEW pre-existing bug: a second-joining client's player hovers frozen at spawn height
until its first input.** Observed in every two-client round: the joiner's player hangs at
(0, 2.73, 0) server-side **and** client-side indefinitely, reporting `grounded: true` in
`game/state` despite being mid-air; the first joiner falls to 0.915 within a second of playing.
Attributed by ablation (round 4): the freeze reproduces with the interpolation change stashed.
Mechanism (consistent with the evidence, not root-caused): the server's KCC advances per
input-stream tick, and the joiner's replicated input stream doesn't flow until its client
binds the replicated action entities and starts the `BEIStateSequence` stream — so gravity
never applies until the player touches any input (`game/input movement` made B land and move
instantly). Same problem family as the documented acute-join `server_late_input_mismatch`
burst (AGENTS.md's input-timeline bullet), but a harsher symptom: that one self-heals in \~10
ticks, this one persists indefinitely for an idle joiner. Candidate directions when picked up:
why the first joiner's stream flows before any input while the second's doesn't (join-burst
hitch delaying the binding poll? server discarding early input ticks?), and whether the KCC
should tick on absent input at all (gravity with zero input should still fall).

**F4 — Harness notes.** A 200 s client `timeout` expired mid-observation and produced
connection-refused (silent with `curl -s`) — same lesson as playtest 0013's F4; use 300 s+
for multi-round sessions. Also: `git stash push` without `-u` silently skipped the untracked
new module, and a `&&`-chained relaunch after the failed stash still ran — an ablation round
was wasted testing the **fixed** binary against itself. Verify the stash actually moved code
(`git stash list`) before trusting an ablation.

## Next steps

1. **Joiner freeze (F3)** — highest player-visible impact in any two-human session; candidate
   root causes and the input-stream evidence are in F3.
2. **Combat caster resolution** — `resolve_attack`/`resolve_kill` still read `Gcd`/`Transform`
   off the connection entity (carried over from playtest 0013's F1; unchanged this session).
3. **KCC-internal rollback registration** — coyote/jump-buffer stopwatches still don't rewind
   on corrections.
4. **Dead-player window** and **cubes/NPCs room-tagging** — unchanged, see AGENTS.md gaps.

## Conclusion

Remote-entity interpolation is fixed and verified: one small client-side marker system on top
of infrastructure lightyear was already registering, with correct marker assignment confirmed
per-entity and the local player's prediction untouched. The verification pass surfaced a new,
pre-existing, player-visible bug (joiner hover, F3) — cleanly attributed by ablation and
documented for the next session. AGENTS.md's interpolation gap entry is updated; the stale
"interpolated remote entities carry no RigidBody" note is corrected.
