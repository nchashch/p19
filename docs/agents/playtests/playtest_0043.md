# Agent Playtest 0043 — Look Divergence After Opening a Menu (bug_0009)

| Field | Value |
|---|---|
| Date | 2026-10-07 02:40 – 03:00 +0400 |
| Commit | `47e7b19` "Add client side prediction toggle" + uncommitted: `ClientPrediction` default off, `rotate_camera` ungated |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client` (`--mcp` rendered and `--no-render`) |
| Server | `target/release/p19-server`, fresh per run |
| Network | private namespace (`unshare -rn`); plain loopback and `tc netem delay 40ms 15ms loss 2%` |

## Purpose

The owner reported W walking at an angle to the camera. Compare the client camera's yaw with
the server's `CharacterLook` yaw (`server/state`) under different inputs, find which path
diverges, fix it, and verify both prediction modes.

## Verification

| Scenario | Prediction | Result (client vs server yaw, walk heading) |
|---|---|---|
| Random mouse bursts ×3, loopback | off / on | equal (Δ 0.0000); walk heading = yaw |
| Random mouse bursts ×4, netem 40±15 ms, 2% loss | off / on | equal; walk heading = yaw |
| Mouse motion in the frame Escape opens the pause menu, ×5 | off (before fix) | **client 0.0 vs server −1.6**; walk 1.6 rad off the camera |
| Same, after fix | off / on | equal (−2.0 / −2.0); walk heading = yaw |
| Same with the console (Backquote), after fix | off | equal; walk heading = yaw |

## Findings

**F1 — bug_0009 (fixed):** the client's camera observer was gated on the pause menu/console
state, while input reaching the server stops one fixed tick later (context deactivation runs in
`Update`). Look input in that tick turned the server but not the camera. `rotate_camera` is now
ungated.

**F2 — Pitch sign mismatch (open, in bug_0009's follow-ups):** client pitch −0.30 vs server
+0.30. Doesn't affect walking.

**F3 — Rendered `--mcp` clients sometimes stalled at GPU init** (2 of 6 launches inside the
network namespace, nothing logged after `SystemInfo`). The numeric runs moved to
`--no-render`. Investigated afterwards: an environment fault, not a game bug. NVIDIA's Vulkan
ICD calls `XOpenDisplay` during instance creation whenever `DISPLAY` is set, and the host's
Xwayland had stopped accepting connections. `env -u DISPLAY` avoids it (playtest skill §8).
