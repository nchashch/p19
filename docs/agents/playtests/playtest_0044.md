# Agent Playtest 0044 — Client-Owned Look (ADR 0017)

| Field | Value |
|---|---|
| Date | 2026-10-07 07:10 – 07:50 +0400 |
| Commit | `4892d4d` "Fix mouse look bug" + uncommitted look rework (ADR 0017) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | `target/debug/p19-client` `--no-render` (measurements), plus a rendered `--mcp` observer for the facing check |
| Server | `target/release/p19-server`, fresh per run |
| Level | `levels/minimal.level.ron` |
| Network | private namespace (`env -u DISPLAY unshare -rn`); plain loopback and `tc netem delay 40ms 15ms loss 2%` |

## Purpose

The owner reported W walking at an angle again after a cube launch, and asked for look to be
plain client input sent with the BEI inputs and replicated to other clients. This run
reproduces the cube case on the old code, then verifies the rework: camera direction
(`game/state.camera_*`) vs the server's look (`server/state`), and the walk heading, in both
prediction modes.

## Verification

Δ is camera minus server, in radians. The heading is computed from the server's position
change while W is held for 1 s.

| Scenario | Before (`4892d4d`) | After, prediction off | After, prediction on |
|---|---|---|---|
| Look straight down (mouse dy +600) | client pitch −1.571, server **+1.571** | Δ 0.0000, both −1.561 | Δ 0.0000 |
| Launch onto a spinning cube (two `spawn_cube` under the player) | server yaw drifts **+0.035**, camera 0 | Δ 0.0000 | Δ 0.0000 |
| Walk after the cube | heading 0.035 off the camera | off 0.0000 | off 0.0000 |
| 5 × (pause, console) with motion on the open frame | — (fixed by bug_0009) | Δ 0.0000 | Δ 0.0000 |
| 120 random mouse bursts, netem 40 ± 15 ms, 2% loss | — | worst Δ 0.00000 | worst Δ 0.00000 |
| `game/input rotate` yaw +0.5, pitch +0.2 | — | — | camera and server (−0.500, −0.200) |
| Right stick right 0.5 s, up 0.3 s | — | — | Δ 0.0000; stick up looks up |
| Record a session, `--replay` it | — | live look (−1.40, 0.10), replay (−1.40, 0.10) | — |

**Remote facing.** Client B walked 6.3 m ahead of rendered client A, then turned. A's copy of
B's `LookDirection` matched the server's every time (0 → 1.57 → 3.14). B's model faced away,
then left, then toward A:

![remote_facing_yaw_0_half_pi_pi.png](screenshots/playtest_0044/remote_facing_yaw_0_half_pi_pi.png)
![remote_faces_observer.png](screenshots/playtest_0044/remote_faces_observer.png)

## Findings

**F1 — bug_0010, fixed by ADR 0017:** ahoy's `spin_character_look` turned the server's
accumulated look while the player stood on a spinning cube. With the look re-sent as an absolute
value every tick, it can't drift.

**F2 — bug_0011, fixed by ADR 0017:** the client and server used opposite pitch signs.

**F3 — Two mistakes caught during the rework:**
- **The look wasn't being sent.** Every BEI action already carries a disabled `ActionMock`, so
  the binder's `Without<ActionMock>` filter never matched, and the server received no look at
  all (pitch stayed 0). The binder now filters on lightyear's `InputMarker`.
- **The remote model faced backwards and missed its first update.** The rig's front faces +Z
  (+π added), and the model attached after the last look change (now also reacts to
  `Changed<Children>`).

**F4 — Replay movement covers less distance (known gap).** Replay ended at (1.62, −0.28) vs
live (6.78, −1.17), the same heading but a shorter distance. Look is exact.

**F5 — Not tested here:** VR (headset look now goes through `FpsCamera`), the windowed client,
and three or more clients.
