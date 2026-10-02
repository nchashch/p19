#set document(
  title: "Bug 0004 — KCC-internal state is not rollback-registered (ghost jumps after corrections)",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Bug 0004 — KCC-internal state is not rollback-registered (ghost jumps after corrections)

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Bug*], [bug_0004],
  [*Date discovered*], [Long-standing; documented in AGENTS.md's movement gap list since the ahoy migration (M0–M2)]
  ,
  [*Commit (state actually running)*], [Open at `81d75bf` "Issue actual per client tokens when clients connect" + uncommitted docs. Present in every commit since the ahoy migration]
  ,
  [*Discovered by*], [opencode agent (GLM-5.3-Flash), AGENTS.md gap review; behavior consistent with the ahoy migration's design notes],
  [*Component*], [`bevy_ahoy` KCC × `lightyear` prediction/rollback (`CharacterControllerState`, `AccumulatedInput`)],
  [*Severity*], [S3 — visible input glitching for the affected player after corrections; no crash],
  [*Status*], [*Open* — needs ahoy-side exposure of its internal-state components for rollback registration; cannot be fixed purely from this repo],
  [*Related*], [AGENTS.md movement "Remaining gaps" bullet; playtest 0014 (interpolation work notes the gap adjacent to its fix)],
)

= Summary

Only `Position`/`Rotation`/`LinearVelocity`/`AngularVelocity` are registered for lightyear's
prediction rollback (via `lightyear_avian`'s registrations). Ahoy's KCC-internal state —
`CharacterControllerState` (coyote-time/jump-buffer stopwatches, grounded-hit data) and
`AccumulatedInput` — is *not*, so when a server correction rewinds and re-simulates the
predicted player, `Position`/`Velocity` snap correctly but the controller's internal
timers/state keep their post-correction values. Consequence: a jump buffered or started
during the mispredicted window can fire again after the correction ("ghost jump"), or a
legitimate buffered jump can be silently swallowed.

= Steps to reproduce

1. Two clients in-game on the same server (A established, B joins — the acute
   `server_late_input_mismatch` correction burst documented in AGENTS.md widens the window).
2. On the corrected client, input a jump in the same frame window as a correction (buffered
   jump via the jump-buffer: press jump a few ticks before landing, then force a correction —
   e.g. the second client's join burst).
3. Observe the corrected client's jump behavior across the correction.

*Expected:* after rewind + re-simulation, the KCC's jump-buffer/coyote state reflects the
re-simulated history — the jump fires exactly once, at the re-simulated landing.
*Actual:* the jump can double-fire (ghost jump) or be swallowed, because
`CharacterControllerState`'s stopwatches were not rewound with the physics state.

= Evidence

Structural, from the registration surface: AGENTS.md's movement bullet records that only the
four physics components are rollback-registered (lightyear_avian's
`register_physics_components`), and ahoy's state components are absent from every rollback
registration call. The symptom class (post-correction jump anomalies) is consistent with this;
a deterministic repro has *not* been built — the corrections window is narrow and the acute
burst is intermittent. Filing as Open with the structural evidence; a repro harness would
need to force corrections on demand (e.g. artificial latency + input replay).

= Root cause

`bevy_ahoy` does not expose its `CharacterControllerState`/`AccumulatedInput` components in a
form this repo can hand to lightyear's rollback registration (they are not
`Clone + PartialEq`-reflected for `SyncComponent`-style registration, and the crate offers no
opt-in API). `AccumulatedInput` additionally resets every tick by design, which makes naive
registration wrong — the correct registration unit is ahoy's *decision state*, not the raw
accumulator.

= Fix

Not implementable from this repo alone. Required: ahoy exposing (a) a serializable snapshot
of its decision-relevant internal state, and (b) either registration hooks or documentation
of the intended lightyear integration. Then this repo registers the snapshot component for
rollback alongside the existing physics registrations.

= Follow-ups

- When ahoy exposes the state, also revisit the movement-rate discrepancy noted in
  `server::replay`'s module docs (`fired_secs`/`elapsed_secs` hardcoded to `0.0` in replay's
  manual `Fire<A>` construction) — the two gaps share the "KCC internals are opaque" cause.
- Re-test the acute-join correction burst (AGENTS.md input-timeline bullet) after the fix:
  ghost-jump reports should disappear.
