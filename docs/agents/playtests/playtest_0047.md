# Agent Playtest 0047 — Mouse Sensitivity in the Pause Menu

| Field | Value |
|---|---|
| Date | 2026-10-07 08:20 – 08:25 +0400 |
| Commit | acdb56f "Decouple look replication from physics" + uncommitted slider (playtest 0046) and its pause-menu copy |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (rendered) |
| Server | `target/release/p19-server`, fresh |
| Network | private namespace (`env -u DISPLAY unshare -rn`), loopback |

## Purpose

The pause menu (Esc in game) gained the same Mouse sensitivity slider as the options screen,
now a shared template component (`ui.mouse_sensitivity` in `components.html`). Verify both
screens render it, that it works in the pause menu by mouse and keyboard, and that the change
applies on resume.

## Verification

| Step | Observed |
|---|---|
| Main menu → Options | label `Mouse sensitivity: 1.00×`, slider present |
| In game, Escape | `Main Menu`, `Resume`, the slider; focus `pause-resume` |
| Drag the slider from 20% to 60% | 1.84 (= 0.1 + 0.6 × 2.9), label follows |
| Camera yaw during the drag | 0.0 before and after: the camera doesn't turn under the menu |
| ArrowDown | focus `mouse-sensitivity` |
| ArrowRight × 2 | 1.94 |
| Escape | menu closed, still `InGame` |
| Mouse dx 100 | camera and server both turn 0.9700 rad (0.005 × 100 × 1.94) |

![pause_menu_slider.png](screenshots/playtest_0047/pause_menu_slider.png)

## Findings

**F1 — Works in the pause menu by mouse and keyboard; the value is shared with Options.** No
errors in either log.

**F2 — Fixed during the run:** bevy_markup component arguments must be quoted or `{expr}`;
bare `value=mouse_sensitivity` failed to parse (`Expected "string" or {expression}`), and
both templates failed to load until it was `value={mouse_sensitivity}`.
