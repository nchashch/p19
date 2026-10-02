#set document(
  title: "Agent Playtest 0018 — 'Joiner Hover' Re-Verified: Spawn-Point Stacking, Not a KCC Bug",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0018

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-10-02 03:15 -- 03:35 local],
  [Commit (local state actually running)], [`8c851b8` "Improve MCP quality of life - add select nearest player API" + one uncommitted file: `AGENTS.md` (doc correction only — *no code changes this session*)],
  [Agent], [opencode session, GLM-5.3-Flash],
  [Clients], [2x `target/debug/client --mcp --no-render` (dev-tools feature); client B on fleet ports :15712/:15713],
  [Server], [`target/release/server` — fresh],
  [Level], [`levels/minimal.level.ron`],
  [Transports], [game: UDP/netcode :6000 · client QA: BRP :15702/:15712 + MCP :15710/:15713],
)

= Purpose

The project owner challenged playtest 0014's F3 interpretation ("joiner hover = the server's
KCC gets no ticks for the joiner until input flows, so gravity never applies"), offering a
simpler one: all players spawn at the *same point*, so a joiner whose predecessor stood still
spawns overlapping their capsule and physics stacks the joiner on top. If true, moving the
first player away — with *zero* input from the joiner — should make the joiner fall normally.
This session verifies both directions of that claim.

= Verification

Two `--no-render` clients, fresh server: A connects → selects level → plays and *stands still*
at the spawn; B connects → plays, sends nothing afterwards.

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Test*], [*Observed*],
  [1 — B joins over idle A], [B's position: exactly (0, *2.73*, 0), `grounded: true`. 2.73 = A's center (0.915) + two capsule half-extents — the precise capsule-stacking height. B is *standing on A's head*, legitimately grounded],
  [2 — A walks away (~8.7 units via `game/input movement` on A only); B sends *nothing*], [B falls: (0, *0.95*, 0), `grounded: true`. Gravity and KCC ticks were applying the entire time — B was resting on A's capsule, not frozen],
)

Both results match the owner's explanation exactly. Conclusion: the "joiner hover" is
spawn-point overlap → capsule stacking — a game-design issue (shared spawn point), not a
technical bug. Fix direction when wanted: separated spawn positions per joiner (e.g.
index/ring offsets around the spawn in `in_game_request`), not input/KCC changes.

= Findings <findings>

*F1 — Playtest 0014's F3 mechanism is retracted.* The "no KCC ticks until input flows /
gravity never applies" interpretation was wrong. The decisive discriminator (which 0014 never
ran): move the *first* player away while the joiner stays idle. 0014 only ever observed the
stacked state (both clients idle), which is equally consistent with either explanation; this
session's TEST 2 disambiguates in favor of stacking. The `grounded: true`-while-mid-air
observation that fed the misdiagnosis is *correct behavior* for an entity standing on another
entity.

*F2 — Corrected in AGENTS.md* (the interpolation bullet's "new live finding" paragraph now
states the spawn-stacking explanation, with the misdiagnosis struck through and the
verification recorded). Playtest 0014's report itself is left as the historical record — its
F3 "mechanism" text is superseded by this document.

*F3 — Bonus observation for combat QA*: a stacked joiner is a *convenient* stationary
target (directly overhead, inside `ATTACK_RANGE`, selectable via `game/select {"nearest":
true}`), so the harness recipe from playtest 0017 still works unchanged even before
spawn-point separation exists.

= Next steps

1. *Spawn-point separation* (design decision, when wanted): offset each joiner's spawn
   (join-order index around the spawn point) in `in_game_request`'s `player(name, at)` call —
   removes the stacking and makes headless combat targets ground-level.
2. *KCC-internal rollback registration* — unchanged: ahoy's `CharacterControllerState`/
   `AccumulatedInput` (coyote/jump-buffer stopwatches, grounded-hit data) aren't
   rollback-registered, so corrections rewind `Position`/`Velocity` but not controller-
   internal state.
3. *Unique player names* — unchanged: `in_game_request` hardcodes `name: "player name"`,
   so `game/select {"name": …}` can only answer "ambiguous" (verified playtest 0017), and
   nameplates/kill-feed semantics are degraded.
4. *Netcode posture* — unchanged: hardcoded `PROTOCOL_ID`/`PRIVATE_KEY` +
   `server_addr_check: false`; LAN-fine only.

= Conclusion

The owner's explanation is verified in both directions: an idle first player stacks the
joiner on their head (2.73, grounded), and the joiner falls normally the moment the first
player moves — no joiner input involved. "Joiner hover" is therefore reclassified from
technical bug to game-design issue (shared spawn point), AGENTS.md is corrected, and the
remaining technical cracks are the three unchanged items above.
