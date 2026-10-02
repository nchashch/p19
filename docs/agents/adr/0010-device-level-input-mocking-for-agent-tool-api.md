# 10. Device-level input mocking (gamepad, keyboard, mouse) for the agent tool API

Date: 2026-09-23

## Status

Accepted — implemented (`game/gamepad`, `game/keyboard`, `game/mouse` BRP methods + matching
MCP tools, all behind the existing `dev-tools` feature from
[0009](./0009-agent-tool-api-via-brp.md)).

## Context

[0009](./0009-agent-tool-api-via-brp.md) built `game/input`: agent input riding `ActionMock` on
the replicated ahoy action entities (`Movement`/`Jump`/`RotateCamera`). That is genuinely
end-to-end for *gameplay* — the mocked values flow through the real replicated-BEI pipeline, the
server's authoritative sim, and back as corrected prediction — but it mocks at the **action**
layer, downstream of `bevy_enhanced_input`'s own binding resolution. It can only ever drive the
three ahoy actions it targets. Everything upstream of an action — which raw input maps to which
binding, dead zones, `GamepadDevice` selection, and critically **all UI** (`MenuControls`'s
`UiNavigate`/`UiConfirm`, every `FeathersButton`'s native pointer/keyboard handling, the pause
modal, every `selector.rs` popup) — was categorically unreachable through the tool API. An agent
could play the game once already in a match, but could not open the pause menu, click a button,
switch languages, pick a level from the lobby, or reproduce any bug that lives in the
input-to-UI path rather than the input-to-character path.

This mattered concretely, not hypothetically: the project owner reported a real bug — clicking
"Play" in the lobby with a mouse sometimes failed to enter the game, while keyboard/gamepad
confirmation worked — and the existing tool API had no way to even attempt a real mouse click to
investigate it. `docs/agents/playtests/playtest_0005`'s own report notes this exact gap explicitly:
verifying the `return_to_main_menu` crash fix could only exercise "an equivalent code path"
(`game/trigger disconnect`), not the literal reported interaction (open the pause menu, click
its button), because nothing in the tool API could simulate an actual keypress or button click.

## Decision

Add three more BRP methods (+ MCP tools) at the layer **below** `game/input` — mocking the raw
device state `bevy_enhanced_input`'s own binding readers consume, so agent-injected input flows
through *real* binding resolution exactly like a human's peripheral, reaching UI as well as
gameplay:

- **`game/gamepad`** — a synthetic, lazily-spawned `bevy_input::gamepad::Gamepad` entity.
  `bevy_enhanced_input`'s `GamepadDevice` resolution defaults to `Any` ("read from all connected
  gamepads") when a context sets none — true of every context in this project — so a bare
  `Gamepad` component on any entity is read identically to a real controller, no extra wiring
  needed. `{"input":"button","button":"South","pressed":true}` / `{"input":"axis", ...}` /
  `{"input":"reset"}`; level-triggered (held until released), matching a real controller.
- **`game/keyboard`** — direct `ButtonInput<KeyCode>` press/release, matching exactly how
  `bevy_enhanced_input`'s `Binding::Keyboard` reads it. `KeyCode` deserializes straight from its
  own `serde` impl (already enabled project-wide), so every one of Bevy's 160+ variants works
  without a hand-maintained name list.
- **`game/mouse`** — `ButtonInput<MouseButton>` for raw mouse-bound gameplay bindings, **plus**
  real `bevy_picking` `PointerInput`/`PointerAction` events on the pointer
  `bevy_picking::input::spawn_mouse_pointer` already spawns unconditionally at `Startup`, headless
  or not (`PointerId::Mouse` — no new entity, no `uuid` dependency, no lazy-spawn bookkeeping;
  it's the same pointer a human's mouse would drive). This is the mechanism that makes clicking
  an actual UI button *by screen position* possible — `move_to`/`motion`/`button`/`wheel`/`reset`,
  positions in the same 1280×720 pixel space `game/screenshot` captures, so a button's on-screen
  rect from a screenshot maps directly to a click.

Two implementation findings recorded here, in the same spirit as 0009's, so they aren't
re-tripped:

- **`bevy_enhanced_input`'s `GamepadButton` binding reads `Gamepad`'s `analog` field, not
  `digital`/`ButtonInput`** — `GamepadButton` is a valid `GamepadInput` variant specifically so a
  button's *pressure* can share the same `Axis` map real analog triggers use, and BEI's reader
  never touches `digital` for buttons at all. A first implementation using
  `digital_mut().press()` compiled and ran with zero errors and did precisely nothing —
  found only by actually driving a gamepad-bound action and watching it not fire.
- **`AccumulatedMouseMotion`/`AccumulatedMouseScroll` cannot be set with a direct
  `world.insert_resource(...)`** — Bevy's own `accumulate_mouse_motion_system`/
  `accumulate_mouse_scroll_system` unconditionally overwrite these resources from
  `MouseMotion`/`MouseWheel` **events** every single frame ("reset to zero every frame", per
  their own doc comments), silently discarding a direct write before
  `bevy_enhanced_input`'s reader ever sees it. Fixed by injecting the real events instead
  (`world.write_message(...)`), letting Bevy's own systems compute the accumulated value on
  their own schedule — the same mechanism a real winit event uses.

A third, more structural finding surfaced once this layer existed and made it possible to test
literal-Enter UI confirmation for the first time: `bevy_input_focus::dispatch_focused_input`
(the system that turns a raw `KeyboardInput` event into the `FocusedInput<KeyboardInput>` a
focused `FeathersButton`'s native key handler reacts to) hard-requires a `PrimaryWindow` entity,
and silently no-ops its entire body without one. `--mcp` headless mode never creates a
`PrimaryWindow` — confirmed via `world.query` returning `[]` — so **literal keyboard Enter can
never confirm a `FeathersButton` through this tool API**, structurally, no matter what
`game/keyboard` injects; only mouse click and gamepad South (`ui.rs`'s `on_ui_confirm`, which
triggers `Activate` directly rather than going through the focus-dispatch pipeline) can. This is
recorded as a testing-harness ceiling, not a bug to fix — not believed to affect a real windowed
client (which always has a `PrimaryWindow`), and independently confirmed by the project owner
running the equivalent flow by hand on a real client, Enter included, once a fix landed
downstream of this work (see "Consequences").

## Alternatives considered

- **Stay at the action-mocking layer, extend `game/input` to more action types.** Doesn't reach
  UI at all, regardless of how many actions it covers — `MenuControls`'s `UiNavigate`/
  `UiConfirm` and every `FeathersButton`'s pointer/keyboard handling sit entirely outside the
  ahoy action set `game/input` mocks. Rejected: the actual gap was structural (a missing layer),
  not a missing action.
- **Drive a real windowed client with an OS-level input-injection tool** (`xdotool`/`ydotool`/
  `wtype`) instead of mocking device state in-process. Would exercise the literal OS input path,
  closing the `dispatch_focused_input`/`PrimaryWindow` gap above for real — but breaks the
  GPU-less headless-fleet story `--mcp` mode exists for (0009's "same binary runs on a VPS with
  lavapipe" scenario), needs a display server and a new external dependency, and tests a
  *different* code path per platform/compositor rather than one uniform mechanism. Rejected for
  the tool API itself; noted as a possible narrow follow-up if the `PrimaryWindow` gap ever needs
  closing for real rather than documented around.
- **A synthetic `PointerId::Custom(Uuid)` for mouse mocking**, as `bevy_picking` explicitly
  supports "for mocking inputs." Rejected once `bevy_picking::input::spawn_mouse_pointer` was
  found to already spawn a real `PointerId::Mouse` unconditionally, headless or not — reusing it
  needs no new entity, no extra dependency, and is arguably more faithful (it *is* the mouse, not
  a stand-in for it).

## Consequences

- Agents can now drive the entire input surface a human peripheral would — gameplay *and* every
  menu, popup, and pause-modal interaction — through the real binding-resolution/picking
  pipeline, not a scripted shortcut. `game/trigger`/`game/select_level`/`game/input` remain for
  fast test setup, but a reachability audit can now deliberately avoid them and still reach
  everything.
- **This directly found and enabled fixing a real production bug**, not a hypothetical one:
  `docs/agents/playtests/playtest_0008` used `game/mouse` to reproduce, then root-cause, a
  100%-reproducible failure where a genuine mouse click never activated the lobby's "Play" or
  "Main Menu" button (`client/src/ui/lobby.rs` was listening for the wrong of two
  identically-named `Activate` event types — gamepad/keyboard-Enter accidentally worked via an
  unrelated compatibility bridge, masking the bug for those two paths only). This is direct
  evidence the layer earns its keep: the bug lived entirely in the click-delivery path
  `game/input`'s action-level mocking cannot reach at all, and was independently confirmed fixed
  by the project owner on the real windowed client.
- **A real, documented ceiling, not swept under the rug**: literal-Enter confirmation of any
  `FeathersButton` cannot be exercised through this harness (see the `dispatch_focused_input`
  finding above). Reachability audits must use mouse click or gamepad South for that specific
  interaction and note the gap rather than claim full coverage; `docs/agents/skills/playtest.md` and
  `AGENTS.md` both carry this caveat so it isn't silently forgotten or "fixed" the wrong way
  (e.g. by faking a `PrimaryWindow`, which would reopen the class of headless-rendering bugs
  0009's own investigation had to work around).
- Same feature-gating posture as 0009: all three methods live behind `dev-tools`, client-only,
  localhost-only BRP — no new attack surface beyond what 0009 already accepted.
- Two more "compiles clean, runs clean, silently does nothing" gotchas are now permanently
  recorded (in code doc comments, `docs/agents/skills/playtest.md`, and persistent agent memory)
  specifically so a future refactor doesn't reintroduce either — both were caught only by
  actually running the code and watching for the *absence* of an expected effect, not by review.
