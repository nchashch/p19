# 17. Look is client-owned absolute input, sent with the tick's inputs and replicated as facing

| Field | Content |
|---|---|
| ADR | 0017 |
| Title | Look is client-owned absolute input, sent with the tick's inputs and replicated as facing |
| Date | 2026-10-07 07:50 +0400 |
| Author | Claude Opus 5.5 (Anthropic), via omp — on the project owner's direction |
| Commit | `4892d4d` Fix mouse look bug + uncommitted look rework |
| Status | Accepted |
| Related | bug_0009, bug_0010, bug_0011; playtests 0011, 0012, 0043, 0044 |

## Context

Before this change, look was two independent accumulators of one delta stream:

- **Client:** the camera rig (`FpsCamera` + `Transform::look_at`) summed mouse/stick deltas
  from two replicated ahoy `RotateCamera` actions (marked `MouseLook` / `StickLook`).
  `controls.rs::update_character_look` then copied the camera's *global* rotation into the
  local `CharacterLook`.
- **Server:** `p19_server::input::accumulate_look` summed the same deltas, as received, into the
  server's `CharacterLook`, which ahoy's KCC steers by.

Any delta that only one side applied left a permanent offset; nothing ever re-synchronized.
Three ways it happened were found:

- **bug_0009:** a menu opened in the same frame as mouse motion. The client dropped that tick's
  delta, the server applied it. That fix (ungating the camera observer) closed only that path.
- **bug_0010:** standing on a spinning cube. ahoy's `spin_character_look` (an `Update` system,
  `bevy_ahoy` 0.2 `kcc.rs`) rotates `CharacterLook.yaw` by the floor's angular velocity on the
  server. Measured: server yaw 0 → +0.035 with the client at 0, and W then walked 0.035 rad
  off the camera; the drift grows with time on the cube.
- **bug_0011:** the two sides used opposite pitch signs. Camera euler pitch was −1.571 (looking
  down) where the server had +1.571.

The project owner's direction: look is not physics and not server-decided. It is a vector the
client sends alongside its BEI inputs; the server uses it and replicates it so other clients
know where a character faces. Hitscan and animation will need it too.

## Decision

1. **A `Look` input action carries the absolute direction.**
   - `p19_shared::inputs::Look` (`Vec2(yaw, pitch)`, ahoy's convention: yaw 0 faces −Z,
     pitch positive looks up) is one of the player's replicated action entities, spawned in
     `player()` in place of the two `RotateCamera`s.
   - The owning client has no bindings on it. `bind_replicated_ahoy_actions` gives it an
     enabled `ActionMock` (`MockSpan::Manual`) plus lightyear's `InputMarker`.
     `write_look_input` (`FixedPreUpdate`, before `EnhancedInputSystems::Update`) writes
     `FpsCamera`'s yaw/pitch into the mock every tick.
   - So look rides lightyear's `BEIStateSequence` with movement: same tick, same input delay,
     same rollback replay, same loss handling (the server repeats the last input).
2. **Both sides apply it in `apply_look`.** This `Fire<Look>` observer lives in `p19_shared`,
   which both binaries load.
   - It validates the value with `LookDirection::from_input`: finite, yaw wrapped to `[−π, π)`,
     pitch clamped to `MAX_LOOK_PITCH`. The server never trusts the client beyond that shape.
   - It writes the KCC's `CharacterLook` and a new replicated `LookDirection` component (with
     `set_if_neq`). `Dead` characters are skipped, as `accumulate_look` did.
   - `p19_server::input` is deleted.
3. **`LookDirection` is the replicated facing.**
   - It is registered in `SharedInputsPlugin` and present in `player()`.
   - Clients turn other players' models to its yaw (`face_look_direction`; +π because the
     rig's front faces +Z).
   - ahoy's `spin_character_look` still edits `CharacterLook` between ticks, but never
     `LookDirection`, and the next tick's `Look` overwrites `CharacterLook` before the KCC
     (`FixedPostUpdate`) runs.
4. **The client camera is plain local state.**
   - `FpsCamera { yaw, pitch }` uses the same convention. `orient_fps_camera` derives the rig's
     `Transform` from it, replacing the `look_at`-a-direction-as-a-point logic and the
     `yaw: −π` seed (playtest 0012 F1).
   - Mouse and stick deltas are a client-local `RotateCamera` action in `PlayerControls`,
     gated with the other gameplay observers.
   - VR sets `FpsCamera` from the headset's global rotation, replacing the head-delta mock.
   - Cube aim uses `FpsCamera::forward`. Replay records and injects `Look` instead of the two
     rotate actions.

## Alternatives considered

- **Server-authoritative look.** The owner first asked for this, then withdrew it. The server
  has no camera; it can only echo or accumulate client input, which is what failed.
- **Keep deltas, add periodic absolute corrections.** Still two sources of truth, just
  bounded. Rejected for the single-source design.
- **A separate client→server message (`LookUpdate`).** Messages aren't tick-aligned with
  inputs, so prediction and rollback would steer by a look from a different tick than the
  server's.
- **Replicate ahoy's `CharacterLook` instead of a new component.** It needs ahoy's `serialize`
  feature, and the server's copy carries `spin_character_look`'s between-tick edits. On the
  predicted owner, replicated writes would also land on the component its KCC steers by.

## Consequences

**Gained**, measured with the playtest 0044 harness (camera vs `server/state` look, both
prediction modes):

- Δ = 0.0000 after the cube launch and spin;
- Δ = 0.0000 after 5 pause and 5 console cycles with motion on the open frame;
- Δ = 0.0000 under `netem` 40 ± 15 ms delay and 2% loss over 120 mouse bursts;
- pitch has the same sign on both sides;
- W walks exactly along the camera heading in every case;
- other clients see the look: `LookDirection` matches, and the model faces it, checked by
  screenshot;
- replay reproduces the live look exactly (yaw −1.40, pitch 0.10).

**Cost:**

- 8 bytes of absolute look per input tick (snapshots are diffed, so an idle look costs little
  `[INFERENCE]`).
- The predicted client now steers its KCC by the input-delayed look (2 ticks), the same one the
  server uses, instead of the current camera. That is consistent with movement and removes
  corrections, but the predicted turn lags the camera by those ticks.
- Old replay recordings (`RotateCameraMouse` / `RotateCameraStick`) no longer parse.

**Open:**

- Pitch isn't shown on remote models: no aim or head bones are driven yet.
- `LookDirection` isn't interpolated for remote clients; it updates at the tick rate.
- Turning with a spinning floor is gone: the client would have to apply the platform's spin
  to its own camera, if that feature is wanted.
- VR look wasn't tested on a headset.
