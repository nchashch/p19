# Agent Playtest 0042 — Client-Side Prediction Toggle

| Field | Value |
|---|---|
| Date | 2026-10-07 02:22 – 02:25 +0400 |
| Commit | `3a58f16` "Move language select to options menu" + uncommitted prediction toggle (`ClientPrediction`, `InGameRequest { predict }`) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered) |
| Server | `target/release/p19-server`, restarted between the two runs |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 (localhost) · client QA: BRP :15702 |

## Purpose

Options gained a Client-side prediction toggle. The client sends it with `InGameRequest`; the
server scopes the character's `PredictionTarget` to the owner only if it is set, and the client
adds its KCC only to a `Predicted` character. Verify both modes reach the game and move, and
that the toggle works and is translated.

## Verification

Input through `game/mouse`, `game/keyboard` (real bindings). Components read over BRP; position
from `game/state` every ~0.05 s while holding W.

| Step | Prediction on (default) | Prediction off |
|---|---|---|
| Options label | `Client-side prediction: On` | clicked → `Off`; `ClientPrediction` = `false` |
| `game/trigger play` | `{"predict": true}` | `{"predict": false}` |
| Client log | — | `local player character spawned predicted=false` |
| Own character | `lightyear_core::prediction::Predicted`, `CharacterControllerState` | `Interpolated`, no `CharacterControllerState` |
| W held: first motion | z −0.24 at 0.10 s | z −0.0007 at 0.10 s, −0.30 at 0.17 s |
| W held ~1.3 s | z −19.07 | z −18.67 |
| Mouse look (off) | — | yaw 0 → −3.0; then W moves along the new heading |

Toggle by keyboard (focus on the toggle, Enter ×3): `false → true → false`, label following
each time, focus stays on the toggle. Japanese: `クライアント側予測：オン`.

![options_prediction_ja.png](screenshots/playtest_0042/options_prediction_ja.png)
![in_game_unpredicted.png](screenshots/playtest_0042/in_game_unpredicted.png)

## Findings

**F1 — Both modes work.** Without prediction, movement starts about one tick later on localhost
(server round trip + interpolation); over a real network the delay grows with latency. No
errors in either log. The client's BEI "expects `Axis2D`, but got `Bool`" and
`LOOK-DIVERGENCE` warnings at spawn are the ones playtests 0012/0013 already recorded.

**F2 — Not tested here:** a laggy link (where the difference shows), switching modes without a
server restart (blocked by the known disconnected-player gap), several clients mixing modes,
and the windowed client.
