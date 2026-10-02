#set document(
  title: "Agent Playtest 0020 — KCC Rollback Registration: bug_0004 Fixed via lightyear's Built-in Local Rollback API",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0020

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-10-02 04:55 -- 05:20 local],
  [Commit (local state actually running)], [`1f6a99f` "Add ./docs/bug_reports and ./docs/skill/bugreport.md" + uncommitted working tree, per file: `client/src/gameplay/player_character.rs` (*the fix*: `local_rollback` registration for `CharacterControllerState`) · `client/src/lifecycle/networking.rs` + `server/src/networking.rs` (TLS provider switched ring → aws-lc-rs, caught during verification) · `docs/bug_reports/bug_0004.typ` (status → `Fixed`, root cause corrected) · `AGENTS.md` (movement gap corrected)],
  [Agent], [opencode session, GLM-5.3-Flash],
  [Clients], [2x `target/debug/client --mcp --no-render` (dev-tools feature); client B on fleet ports :15712/:15713],
  [Server], [`target/release/server` — fresh],
  [Level], [`levels/minimal.level.ron`],
  [Transports], [game: UDP/netcode :6000 · token endpoint: HTTPS :6001 (aws-lc-rs provider) · client QA: BRP :15702 + MCP :15710 · server QA: BRP :15701],
)

= Purpose

Fix the last technical crack, `docs/bug_reports/bug_0004` (status `Open` at session start):
ahoy's KCC-internal state (`CharacterControllerState` — coyote/jump-buffer stopwatches,
grounded-hit data — plus `AccumulatedInput`) was not registered for lightyear's rollback, so
server corrections rewound `Position`/`Velocity` while controller-internal decision state kept
post-correction values → ghost jumps / swallowed buffered jumps. The working assumption had
been that this *needs ahoy-side exposure and can't be fixed from this repo*.

= Root cause re-examination (the premise was wrong)

Direct read of `bevy_ahoy` 0.2.0's source: `CharacterControllerState` derives
`Component + Clone (+ Reflect, PartialEq)` with mutable mutability — exactly the requirement of
lightyear 0.30's built-in *local rollback* API,
`PredictionBuilderExt::local_rollback()` ("enable local rollback for a component … not handled
by Replicon's prediction marker writes", bounds: `Component<Mutability = Mutable> + Clone`).
No ahoy-side change, no fork, no snapshot plumbing needed. The old assessment was written
against an assumed need for serializable/exposed internals; `local_rollback` requires none of
that.

= The fix

One registration in `PlayerCharacterPlugin::build`:

```rust
app.component::<CharacterControllerState>().local_rollback();
```

- Makes lightyear maintain `PredictionHistory<CharacterControllerState>` on predicted entities
  (snapshotted in `FixedPostUpdate`'s `UpdateHistory`, restored on rewind) *without replicating
  the state to anyone*.
- Ordering requirement: `local_rollback` registers rollback metadata only if `PredictionRegistry`
  already exists (created by `PredictionPlugin` inside `ClientPlugins`) — satisfied because
  `PlayerCharacterPlugin` is added later in `main.rs`'s tuple. Documented in-code.
- `AccumulatedInput` deliberately *not* registered: it is cleared and re-filled from the
  replayed input stream every tick, so it re-derives correctly during rollback — snapshotting
  it would only store post-consumption zeros.
- Scope: client-only by construction (`local_rollback` is a no-op wherever
  `PredictionRegistry` is absent, i.e. on the authoritative server).

= Verification

Two `--no-render` clients, fresh server: A connects → selects level → plays; B joins (its
join-burst triggers the documented acute correction window on A — the exact condition the
bug needs).

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Check*], [*Observed*],
  [History registration], [A's own player entity carries `PredictionHistory<bevy_ahoy::CharacterControllerState>` *alongside* the four physics histories (`Position`/`Rotation`/`LinearVelocity`/`AngularVelocity`) — present only on the local predicted player, per `world.list_components`],
  [Movement sanity through corrections], [A moves via `game/input movement` after B's join-burst corrections: position updates correctly ((0, 0.92, −8.67) after 40 ticks), `grounded: true`, no desync symptoms],
  [Panics/errors], [0 across both clients and the server],
)

= Findings <findings>

*F1 — bug_0004 is fixed, and the fix is one line + a comment.* The "needs ahoy-side
exposure" assessment in the report and AGENTS.md was wrong: ahoy 0.2 already derived
`Component + Clone` on `CharacterControllerState`, which is all `local_rollback()` requires.
Lesson recorded in the bug report and AGENTS.md: before declaring an upstream-exposure
requirement, check the *derives* against the target API's actual trait bounds.

*F2 — Provider mismatch caught during verification.* The client's `rustls` resolved with the
*aws-lc-rs* crypto provider (via rmcp's feature set) while the token-endpoint code written
last turn used `rustls::crypto::ring::default_provider()` — a compile-time catch
(`could not find ring in crypto`). Both binaries switched to
`rustls::crypto::aws_lc_rs::default_provider()` (already in the graph via rmcp; one provider
everywhere, so client and server negotiate identically). Server re-verified over the full
HTTPS token flow after the switch (token #1 issued, connect → InGame, zero failures).

*F3 — Behavioral verification limit, stated honestly.* The *structural* fix (history
registration on the predicted player) is live-verified; the *behavioral* symptom (ghost
jump vs swallowed jump across a correction) is not deterministically observable through the
harness — it requires catching a jump input inside the narrow mispredicted window. The
registration is the mechanism lightyear documents for exactly this state class, the history
is confirmed present and per-tick updated, and movement through the correction burst is
clean. A targeted ghost-jump repro (scripted jump input timed against forced corrections)
would be the follow-up if symptoms are ever reported in practice.

= Next steps

1. *Spawn-point separation* (design item, playtest 0018) — joiners still stack on an idle
   predecessor.
2. *Production netcode upgrades* (explicitly deferred): CA-signed cert on a real backend;
   asymmetric first-connection exchange if first-use MITM safety is ever required (TOFU
   trusts its first observer by definition).
3. *KCC rollback behavioral repro* (F3) — only if ghost-jump symptoms are ever reported.
4. Player names are generated handles ("Adjective Noun" + collision suffix); real name sourcing
   remains an open design item when identity matters beyond uniqueness.

= Conclusion

The last technical crack is closed: ahoy's KCC decision state participates in rollback via
lightyear's built-in local-rollback registration, live-verified on the predicted player, with
the TLS provider mismatch caught and fixed during the same pass. The bug ledger now shows
bug_0001–bug_0004 all `Fixed`, with the remaining items being explicit design decisions
(spawn separation, name sourcing, production cert/keychain posture) rather than defects.
AGENTS.md's movement gap bullet is corrected to match.
