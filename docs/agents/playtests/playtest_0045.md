# Agent Playtest 0045 — Mouse Sensitivity Option

| Field | Value |
|---|---|
| Date | 2026-10-07 08:00 – 08:05 +0400 |
| Commit | `acdb56f` "Decouple look replication from physics" + uncommitted mouse sensitivity option |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (rendered) |
| Server | `target/release/p19-server`, fresh |
| Network | private namespace (`env -u DISPLAY unshare -rn`), loopback |

## Purpose

Options gained a Mouse sensitivity selector: presets 0.25×–3× multiplying the 0.005 rad/px base.
Verify the popup, picking by mouse and keyboard, the label in another language, and that the
turn rate changes on the camera and the server, including mid-game.

## Verification

| Step | Observed |
|---|---|
| Options | `Language`, `Mouse sensitivity: 1×`, `Client-side prediction: Off`, `Back` |
| Open the selector | popup shows `1×`–`3×`: it opens scrolled to the current pick (0.25×–0.75× above it) |
| Click `2×` | label `Mouse sensitivity: 2×`; `MouseSensitivity` = 2.0 |
| Enter, ArrowDown, Enter | label `3×`; resource 3.0 |
| Japanese | `マウス感度：3×` |
| In game at 2×, mouse dx 100 | camera and server both turn 1.0 rad (100 × 0.005 × 2) |
| Set 0.5× over BRP mid-game, dx 100 | both turn 0.25 rad |

![options_mouse_sensitivity_popup.png](screenshots/playtest_0045/options_mouse_sensitivity_popup.png)

## Findings

**F1 — Works by mouse and keyboard; changes apply immediately, in game too.** No errors in either
log.

**F2 — Not tested here:** the windowed client and a real mouse. The setting isn't saved between
launches (same as Client-side prediction).
