# Agent Playtest 0046 — Mouse Sensitivity Slider

| Field | Value |
|---|---|
| Date | 2026-10-07 08:10 – 08:20 +0400 |
| Commit | acdb56f "Decouple look replication from physics" + uncommitted slider (`ui/slider.rs`) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (rendered) |
| Server | `target/release/p19-server`, fresh |
| Network | private namespace (`env -u DISPLAY unshare -rn`), loopback |

## Purpose

The Mouse sensitivity presets (playtest 0045) became a continuous slider, 0.1×–3.0×. bevy_markup
has no range input, so `ui/slider.rs` builds one: pointer press and drag observers on an
`is="slider"` element, and left/right stepping while it's focused. Verify the pointer, the
keyboard, the gamepad, the value mapping, and the in-game turn.

## Verification

Value = 0.1 + fraction × 2.9, kept at 0.01; the step is 0.05. The track is 578 px wide.

| Step | Observed | Expected |
|---|---|---|
| Open Options | label `Mouse sensitivity: 1.00×`, thumb at 31% | — |
| Click at 75% of the track | 2.28 | 2.28 |
| Press at 10% | 0.39 | 0.39 |
| Drag to 50% | 1.55 | 1.55 |
| Drag to 90%, pointer 60 px below the track | 2.71 | 2.71 |
| Drag past the right end | 3.00 | 3.00 (clamped) |
| Release, then move the pointer | 3.00 | unchanged |
| Focus after the drag | `mouse-sensitivity` | the slider |
| ArrowLeft × 3 | 2.85 | 2.85 |
| Hold ArrowLeft 1 s | 2.50 (7 steps: one press, then repeats from 0.4 s every 0.08 s) | — |
| ArrowUp / ArrowDown / ArrowDown | focus `language`, slider, `prediction` | up/down still navigate |
| ArrowRight, then gamepad DPadRight | 2.55, 2.60 | +0.05 each |
| In game at 2.6×, mouse dx 100 | camera and server both turn 1.3000 rad | 1.3000 |

![slider_keyboard_focus.png](screenshots/playtest_0046/slider_keyboard_focus.png)
![slider_hover_tooltip.png](screenshots/playtest_0046/slider_hover_tooltip.png)

## Findings

**F1 — Works by mouse, keyboard and gamepad.** No errors or warnings in the logs.

**F2 — Fixed during the run:** the tooltip, placed above the hovered track, covered the label
and so hid the value while dragging. It is now on the whole row and sits above it.

**F3 — Not tested here:** the windowed client with a real mouse, and a UI scale factor other than
1 (the pointer is converted to physical pixels the same way `bevy_ui`'s picking does).
