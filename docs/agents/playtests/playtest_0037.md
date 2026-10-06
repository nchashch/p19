# Agent Playtest 0037 — In-Place Updates Write Only What Changed

| Field | Value |
|---|---|
| Date | 2026-10-06 02:22 – 02:24 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0036; no client change in this step); bevy_markup path dependency at `9d84990` + uncommitted write-if-different updates (`build.rs` `put`) and the features of playtests 0032–0036 |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup's in-place updates used to re-insert every component of every kept element, so one
changing templated value (a nameplate fade, the TUI gauge) re-laid-out the whole UI. Updates now
write a component only when its value differs. Measured with a throwaway release benchmark
(1200 frames, one templated value changing per frame): a 600-element UI went from 9.85 to
4.06 ms/frame, a 120-element one from 2.09 to 0.97 (idle: 0.44 / 0.19). This run checks the
client's live-updating surfaces still update.

## Verification

Laid-out sizes read from `ComputedNode` (what's actually drawn, not just the `Node`).

| Step | Observed |
|---|---|
| Main menu TUI gauge, 0.5 s apart | laid-out width 40 → 120 px (still animating) |
| DPadDown | focus `options` (the focus ring's restyle still applies) |
| Hover Quit | tooltip "Quit the game." |
| NPC attacked to 51 HP, nameplates on | its fill lays out at 31 px of 60 (51%); the player's own plate 60 px |

![nameplate_after_write_if_different.png](screenshots/playtest_0037/nameplate_after_write_if_different.png)

## Findings

**F1 — Live updates unaffected.** Not measured in p19 itself: frame time (no tool-API probe).

**F2 — bevy_markup bug_0025 filed:** its `ui_state_machine_matches_model` property test fails
~1 in 10 runs; it already did at the committed `9d84990` (1/12), so it predates this change.
