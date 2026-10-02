#set document(
  title: "Agent Playtest 0006 — Add game/gamepad, Verify the Literal Crash Path",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0006

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 01:37 -- 01:42 UTC],
  [Commit], [Started at `05a416c` "Fix clent crash on disconnect"; the `game/gamepad` addition
  described here is not yet committed as of this report],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP, editing code between runs],
  [Client], [`target/debug/client --mcp` -- dev profile + `dev-tools` feature, rebuilt between runs],
  [Server], [`target/release/server`],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
)

= Purpose

Requested directly by the project owner: "implement an API for MCP that would allow you to mock
the gamepad inputs, so that they would go through the same codepaths as the human generated
gamepad inputs." Motivated directly by `playtest_0005`'s own limitation, stated in its own
findings: `game/input` only mocks the three ahoy gameplay actions at the *action* level
(post-binding), so there was no way to reach UI navigation (the pause menu, the exact surface
`playtest_0005`'s bug was on) through the tool API at all -- that report's verification of the
`return_to_main_menu` fix had to go through an equivalent code path, not the literal one. This
run closes that gap and then immediately uses it to re-verify that exact fix for real.

= Design

Investigated `bevy_input::gamepad::Gamepad` (a component, `#[require(GamepadSettings)]`,
`Default`-derivable -- no dependency on `bevy_gilrs`/real hardware detection to exist) and
`bevy_enhanced_input`'s `GamepadDevice` resolution
(`bevy_enhanced_input-0.26.0/src/context.rs:911-936`): a context with no explicit `GamepadDevice`
component defaults to `GamepadDevice::Any` ("input will be read from all connected gamepads") --
confirmed none of this project's contexts set one, so a bare `Gamepad` component on *any*
entity, real controller or not, is read identically. This meant a synthetic, agent-owned gamepad
entity, spawned lazily on first use, would be picked up by the exact same binding-resolution
code a physical controller drives -- no wiring changes needed anywhere else in the app.

New BRP method `game/gamepad` (+ MCP tool `gamepad_input`), alongside the existing `game/input`,
not replacing it: `{"input":"button","button":"South","pressed":true}`,
`{"input":"axis","axis":"LeftStickX","value":0.8}`, `{"input":"reset"}`. Level-triggered
(press/release, no `ticks`/duration concept) to match how a real controller actually reports
state, rather than inventing a new auto-expiry mechanism.

= A real bug caught by testing, not by review

First implementation used `gamepad.digital_mut().press(button)`/`.release(button)` --
compiled clean, `cargo clippy` clean, looked correct by every static check available. Ran it
live anyway (habit, not suspicion): pressed gamepad `Start` in-game, expecting the pause modal
to open.

#figure(
  image("screenshots/playtest_0006/1790127488401-gamepad-modal-open.png", width: 65%),
  caption: [After pressing and releasing `Start` with the `digital_mut()` implementation: no
  modal, no error, nothing -- just the same in-game frame as before. Silent failure, not a
  crash -- the kind of bug that's easy to miss if you only check "did it compile" or "did it
  panic."],
)

Read `bevy_enhanced_input-0.26.0/src/context/input_reader.rs` directly rather than guessing
further: `Binding::GamepadButton`'s value-reading arm (line 132-151) calls `gamepad.get(button)`
-- which, per `bevy_input::gamepad::Gamepad::get`'s own doc comment, reads the *`analog`*
field, not `digital`/`ButtonInput`. `GamepadButton` is a valid `GamepadInput` variant specifically
so a button's *pressure* can share the same `Axis` map real analog triggers use; BEI's reader
never touches `digital` for buttons at all. Fixed to `gamepad.analog_mut().set(button, if
pressed {1.0} else {0.0})`.

= Verification: the literal crash path

Rebuilt, fresh server + client, connect -> select level -> play -> confirmed `game_state:
"InGame"` with a real player. Then, for the first time, drove the actual reported interaction
instead of an equivalent one:

#figure(
  image("screenshots/playtest_0006/1790127689078-gamepad-modal-open2.png", width: 65%),
  caption: [`{"input":"button","button":"Start","pressed":true}` then `pressed:false` -- the
  pause modal opens for real, through the actual `PlayerControls` binding
  (`bindings![KeyCode::Escape, GamepadButton::Start]`). "Resume" is focused, matching its
  `AutoFocus`.],
)

#figure(
  image("screenshots/playtest_0006/1790127710297-gamepad-modal-navigated.png", width: 65%),
  caption: [`DPadUp` press+release: focus moves from "Resume" to "Main Menu" above it, through
  `MenuControls`'s real `Cardinal::dpad()` binding and `AutoDirectionalNavigator`.],
)

`{"input":"button","button":"South","pressed":true}` then `pressed:false` -- `UiConfirm`'s real
binding, activating the focused "Main Menu" button, calling `return_to_main_menu` through
`modal_menu.rs`'s actual `on(main_menu_button)` observer, not `game/trigger disconnect`'s
different entry point. Result: `game/state` read back
`{"game_state":"MainMenu","player_despawned":true}`, and `ps aux` confirmed the client process
(both the `timeout` wrapper and the binary itself) still running. No crash, no panic in the log
-- the exact sequence the project owner originally reported crashing the real windowed client
now completes cleanly.

= Findings <findings>

- *`game/gamepad` implemented and working*, closing `playtest_0005`'s own stated limitation.
  Reaches UI navigation (previously unreachable through this tool API at all), not just the
  three ahoy gameplay actions.
- *`return_to_main_menu`'s fix (from the session that produced `playtest_0005`) is now verified
  against the literal reported crash path*, not only an equivalent one. Confidence upgraded from
  "very likely correct by code inspection + equivalent-path testing" to "directly confirmed."
- *A real, non-obvious bug, caught only by actually running the code*: `bevy_enhanced_input`
  reads gamepad buttons through the `analog` field, not `digital`/`ButtonInput`. Both the tool's
  own doc comment and `docs/agents/skills/playtest.md` §5a now record this explicitly so it doesn't get
  quietly "fixed" back to the wrong-but-obvious choice on a future refactor.
- *A minor, deliberately unfixed cosmetic gap*: `InputDeviceState` doesn't flip to `Gamepad` from
  this mock (it tracks real input *events*, not polled component state, and this mock only
  writes persistent state) -- control-tip icons in the modal/HUD stayed keyboard-styled
  throughout this run despite the input being gamepad-sourced. Doesn't affect any actual
  gameplay/UI behavior; not investigated further since it's out of scope for this task.
- *Process note*: this report covers two build/run cycles (the failed `digital_mut()` attempt,
  then the fixed `analog_mut()` one), filed as one report per this session's own "one continuous
  investigation arc, not N independent playtests" convention (see `playtest_0003`'s precedent).

= Artifacts & bookkeeping

- Screenshots: `docs/agents/playtests/screenshots/playtest_0006/` (LFS-tracked, all three captures
  above -- including the failed attempt, kept deliberately since it's the concrete evidence a
  silent failure looks identical to "nothing happened yet", not an error), plus the tool's raw
  capture staging directory `docs/agents/playtests/dist/screenshots/` (gitignored).
- Tool API surface used: `game/trigger` (`connect`, `play`), `game/select_level`, `game/state`,
  `game/gamepad` (new), `game/screenshot[+ /get]`, plus direct reading of
  `bevy_enhanced_input-0.26.0`/`bevy_input-0.19.0` registry source for the design and the bug fix
  -- see `client/src/dev/tool_api.rs`, `docs/agents/skills/playtest.md` §5a, ADR 0009.
- Code changed: `client/src/dev/tool_api.rs` (`game/gamepad` BRP method + `gamepad_input` MCP
  tool, `AgentVirtualGamepad` marker, `parse_gamepad_button`/`parse_gamepad_axis`).
- Living documentation updated this session: `AGENTS.md`'s dev-tools bullet (the new method) and
  `return_to_main_menu` bullet (upgraded verification claim); `docs/agents/skills/playtest.md` (new
  §5a).
