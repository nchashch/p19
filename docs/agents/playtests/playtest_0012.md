# Agent Playtest 0012 — Desync Reproduction Attempt: Instrumented, Multi-Mode, Intermittent

| Field | Value |
|---|---|
| Date | 2026-09-25 00:30 – 01:45 local (+04) |
| Commit | `f4180af` "Fix no dev-tools feature build failure" + uncommitted session work; this turn adds **temporary** desync instrumentation (removed-when-diagnosed candidate) in `controls.rs::update_character_look` and `rotate_camera` |
| Agent | opencode session, GLM-5.3-Flash; the project owner reproduces the desync on windowed clients independently |
| Clients | `--mcp` full-render headless (multiple rounds), `--no-render` (earlier turn), and **windowed** dev-tools clients (no `--mcp`) in this turn's rounds — all on fleet ports :1613/:1614 etc. |
| Server | `target/release/server --brp-port 15701` — fresh per round; restarts verified mid-session |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · client QA: BRP :1600N + MCP :1700N · server QA: BRP :15701 |

## Purpose

Follow-up to playtest 0011's findings (F1/F2): the multi-client look/movement desync reproduced
once on full-render `--mcp` clients (second client spawns with `look_yaw = −1.71`,
`look_pitch = +1.5707` — pinned at the accumulator's clamp — and erratic mouse response, while
the first client stayed exact), but was intermittent. This playtest instruments the two suspect
code paths and loops the repro across client modes to catch the corruption moment.

## Instrumentation (temporary, in `controls.rs`)

- `update_character_look` — one-shot **LOOK-DIVERGENCE first** log when the camera-anchor's
  global euler diverges from the `FpsCamera` fields by > 0.02 rad, including the player
  entity's global rotation (where the server-replicated `Rotation` lands) and position;
  re-arms after recovery (**LOOK-DIVERGENCE recovered**).
- `rotate_camera` — **ROTATE-FIRE** log for every non-zero `Fire<AhoyRotate>` with the
  pre-fire `FpsCamera` state. Deliberately unkeyed (the observer applies to
  `fps_camera.single_mut()` regardless of which action context fired) — so any
  **remote** player's action state firing on this client is immediately visible in the log.

## Rounds and results

| Round | Configuration | Second client (B) |
|---|---|---|
| Earlier turn (pre-instrumentation) | 2× full-render `--mcp`; A did mouse (dx=200, dy=60) + KeyW before B joined | **BROKEN at spawn** — yaw −1.71, pitch +1.5707 (clamp), erratic mouse jumps (−1.71 → 2.26 → 0.95); A stayed exact (−1.0/−0.3) |
| This turn, instrumented ×4 (full-render `--mcp`, exact broken-run sequence per round, fresh server each round) | as above | **clean every round** — yaw 0/pitch 0 for 6 s after play |
| This turn, cross-fire check | A rotated (dx=300, dy=−80) while B was in-game | B's look unchanged (yaw 0/pitch 0); **B's log has zero ROTATE-FIRE lines** while A's log has 4 — the remote-action-fire hypothesis is **eliminated** |
| This turn, windowed ×3 (no `--mcp`; real rendering + winit input — closest to the owner's repro) | as above | rounds 1/3: **dead** (BRP unreachable at poll time; clients alive per logs, no panic — unresolved harness gap); round 2: **clean** |

## Findings

**F1 — A real, benign-until-Fire spawn mismatch found and instrumented.** At every client's
spawn (caught by the divergence logger): `camera euler yaw=0.000 pitch=-0.000 | fps yaw=-3.142
pitch=0.000 | player rot [0,0,0,1] | player pos [0.0, 4.229, 0.0]`. `FpsCamera::new()` seeds
`yaw = −π` (and `direction = Vec3::Z`), while the rig anchor spawns with `Transform::IDENTITY`
(yaw 0) — the `FpsCamera` fields and the visual camera disagree from frame one.
`update_character_look` immediately writes the camera's euler (0) into `CharacterLook`, so the
mismatch is invisible until the first Fire event applies deltas on top of the −π-seeded fields.
Not yet proven to **cause** the desync, but it is a real invariant violation at spawn that any
fix should normalize (e.g. `FpsCamera::new()` seeding `yaw`/`direction` consistently with the
identity rig transform).

**F2 — The remote-action cross-fire theory is eliminated.** A's mouse rotations produced
zero `ROTATE-FIRE` lines on client B's log while B was in-game: lightyear_inputs_bei's
remote action-state does not fire `Fire<AhoyRotate>` on other clients (at least not in these
sessions). The unkeyed `rotate_camera` observer remains a smell, but it is not currently being
triggered by remote state.

**F3 — The desync is still intermittent and not yet reproduced in a controlled
agent-driven round**: 1 of 2 full-render attempts (earlier turn) versus 4 clean instrumented
rounds + 1 clean windowed round this turn. The owner reproduces it on **windowed** clients
with a real mouse; the agent's synthetic `game/mouse` injection drives the same event path
but not the same timing as physical hardware input. The windowed agent-driven rounds are the
right vehicle, but they died at poll time in 2 of 3 rounds (see F4).

**F4 — Harness gap: windowed clients' BRP is unreliable at poll time.** In 2 of 3 windowed
rounds the clients were alive, connected to the server ("connected to server" in logs), and
panic-free, but BRP on :1613/:1614 answered nothing when polled. Uninvestigated: whether the
windowed build's BRP bind fails silently (port conflict with the **other** windowed client's
:15702 default? — both used fleet ports, so unlikely), or the windowed app starves the BRP
task while unfocused. Needs a round with slower polling or log-side verification of the bind.

**F5 — Server ground truth confirmed clean in every round**: `server/state`'s authoritative
player positions tracked the first client's movements exactly and never diverged from the
**healthy** client's view. The desync is client-local to the broken client; the server's view
has been correct in every sample across both playtests.

**F6 — The pitch-clamp trap state is self-sustaining** (established in playtest 0011's F1):
once look.pitch reaches ±π/2, the vertical `direction` makes `look_at(vertical-point)` re-pin
the camera euler on every subsequent fire — which is why the owner reads "pitch is dead" and
why a reconnect (fresh `FpsCamera::new()`) clears it. Any fix must both stop the **entry** into
the trap and make the exit possible.

## Next steps (agreed with the owner)

1. **Keep the instrumentation in place** (it is temporary, marked in `controls.rs`) and loop
   the **windowed** repro once F4's poll-time BRP issue is worked around (e.g. poll slower, or
   drive the whole windowed round from a single script with retries) — the first
   LOOK-DIVERGENCE log on the broken client will show which value corrupted first
   (camera euler vs FpsCamera fields vs the player's replicated Rotation).
2. Likely fix shape (unchanged from playtest 0011): replace
   `camera_transform.look_at(fps_camera.direction)` with a direct rotation assignment
   (`Quat` from yaw/pitch — no point-vs-direction ambiguity), and normalize
   `FpsCamera::new()`'s yaw/direction against the identity rig transform (F1).
3. Server-side disconnect-despawn fix for the zombie players (playtest 0011 F4) — queued.
4. Tool-API gaps (playtest 0011 F7) — queued: per-player `CharacterLook` in `server/state`;
   binding introspection in `game/state`.

## Conclusion

The desync did not reproduce in a controlled instrumented round this turn (4 clean headless
rounds, 1 clean windowed round, 1 dead-windowed round), but the instrumentation is now in
place and already yielded the first concrete invariant violation (F1: the `FpsCamera` spawn
mismatch). The remote-action cross-fire theory is eliminated (F2), the server has been
cleared as the corruption source in every sample (F5), and the windowed-client harness gap
(F4) is the blocker between here and an instrumented reproduction on the owner's exact
configuration.
