#set document(
  title: "Agent Playtest 0008 — Full Keyboard+Mouse / Gamepad Reachability Sweep",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0008

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 13:35 -- 14:09 UTC],
  [Commit], [Started at `736ec01` "Add keyboard+mouse input injection capability tp MCP"; the
  `lobby.rs` fix described here is not yet committed as of this report],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP, editing code between runs],
  [Client], [`target/debug/client --mcp` -- dev profile + `dev-tools` feature],
  [Server], [`target/release/server`],
  [Levels], [`levels/minimal.level.ron`, `levels/spawn.level.ron`, `levels/outpost.level.ron`],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
  [Human verification], [Yes -- the project owner independently ran the fix on the real windowed
  client (Connect -> select level -> click Play, all with the mouse; literal Enter also
  confirmed) and reported it works. See "Independently confirmed..." under Findings.],
)

= Purpose

Requested directly by the project owner: "test that all the functional elements of the current
implementation are reachable by both gamepad and keyboard+mouse -- only relying on inputs a
human user could actually enter, so not manually triggering events." The owner also flagged a
known bug for context: "if you click 'Play' with the mouse in the lobby after loading a level,
under certain conditions, you won't get in game -- but navigating to it with arrow keys+Enter or
a gamepad, it works." Scope: every menu, popup, lobby control, and in-game action, driven
exclusively through `game/keyboard`/`game/mouse` (device-level) and `game/gamepad`
(device-level) -- deliberately *not* `game/trigger`/`game/select_level`/`game/input`, since those
bypass the real UI/input pipeline a human's peripheral drives.

= Method

Built the reachability matrix from the exact bindings in `client/src/controls/actions.rs`/
`controls.rs`/`ui/ui.rs`/`ui/lobby.rs`/`ui/selector.rs`, then drove each element via all
applicable input methods, cross-checking outcomes against `game/state`, client/server logs, and
targeted BRP `world.query`/`world.get_resources` calls when an outcome needed deeper diagnosis
than a screenshot could show.

= Findings <findings>

== A real, 100%-reproducible bug: mouse clicks never activated lobby "Play"/"Main Menu"

Reproduced the owner's exact scenario (select a level, then click Play) and it failed --- but
follow-up testing showed the failure has *nothing* to do with timing or load state: a cold click
on "Play", with zero prior interaction and the server already `ServerState::InGame`, also
failed, every single time, across two independent fresh server+client pairs. "Main Menu"
(the lobby's own disconnect button) failed identically. Meanwhile `game/gamepad`'s South button
activated both successfully every time.

#figure(
  image("screenshots/playtest_0008/1790171017643-mouse-play-race-fail.png", width: 60%),
  caption: [Level selected, "Play" clicked immediately after (the reported scenario) -- still
  in `Lobby`, no `InGameRequest sent` log line at all.],
)

Diagnosis (BRP `world.query`, no code changes yet): a real click correctly produced
`Pointer<Press>`/`Pointer<Release>` and the `bevy_ui::interaction_states::Pressed` marker
correctly appeared then disappeared on the button entity every time -- ruling out a
picking/hit-testing failure or the documented `ServerState::Loading` race (the server was
already `InGame` in most repro attempts). But `Activate` itself never fired. Injecting the
*correct* `bevy_ui_widgets::Activate` directly via `world.trigger_event` onto the button entity
did not fire the handler either -- proof of a listener-type mismatch, not an event-delivery
problem.

Root cause, found by reading source: this project has two identically-named `Activate` event
types -- `crate::ui::widgets::Activate` (hand-rolled, correct for the old `widgets::button()` --
HUD, pause modal) and `bevy::ui_widgets::Activate` (the real event a `FeathersButton`/
`menu_button()` press emits). `client/src/ui/lobby.rs` imported the *wrong one* for its own two
direct handlers, `lobby_play_button`/`lobby_main_menu_button` -- both `menu_button()`-based, so a
genuine click only ever produces the type they *weren't* listening for. It only looked
input-method-dependent because `ui.rs`'s `on_ui_confirm` (gamepad South) and `on_ui_confirm_enter`
(literal Enter) both *also* fire the legacy type as a compatibility bridge for old-style widgets
-- accidentally satisfying the (wrong) listener and masking the bug for every path except a real
click.

*Fixed*: one-line import change (`bevy::ui_widgets::Activate` in place of
`crate::ui::widgets::Activate`), matching how `selector.rs`/`ui.rs` already do it correctly.

#figure(
  image("screenshots/playtest_0008/1790172166050-fix-verify-ingame.png", width: 60%),
  caption: [Post-fix, fresh server+client: select a level then immediately click "Play" with the
  mouse (the exact reported scenario) -- reaches `GameState::InGame` with a real player, confirmed
  via `game/state`.],
)

"Main Menu" was verified fixed the same way (mouse click now correctly disconnects back to
`MainMenu`). As an expected side effect of the fix, literal keyboard Enter on these two buttons
no longer activates them *in headless `--mcp` testing* -- see the next finding; this is correct,
not a regression, matching how every other `FeathersButton` in the app already behaved.

*Independently confirmed by the project owner, by hand, on the real windowed client* (not
`--mcp`): Connect -> select a level -> click Play, all with the mouse, now reaches the game --
the exact end-to-end flow the original bug report described. The owner also confirmed literal
Enter now works there too, which is exactly what the harness-ceiling finding below predicts:
`dispatch_focused_input` only fails without a `PrimaryWindow`, and a real windowed client always
has one -- so the fix generalizes to real hardware, not just this agent's `--mcp` reproduction.

== A harness ceiling, not a bug: literal Enter can't activate any `FeathersButton` headlessly

While isolating the bug above, literal `Enter` also failed to open the main menu's "Options"
popup -- on a totally unrelated, never-fixed code path, confirming this is systemic, not specific
to lobby.rs. Root cause: `bevy_input_focus::dispatch_focused_input` (turns a raw `KeyboardInput`
event into the `FocusedInput<KeyboardInput>` a focused widget's native key handler reacts to)
requires a `PrimaryWindow` entity, confirmed via `world.query` for
`bevy_window::window::PrimaryWindow` returning `[]` in `--mcp` mode -- and silently skips its
entire body without one. `--mcp` mode (`WindowPlugin { primary_window: None }`) never creates one,
so `bevy_ui_widgets::button::button_on_key_event`'s literal-Enter/Space handling can never fire
headlessly, for *any* `FeathersButton`, no matter what `game/keyboard` injects.

Confirmed this is specifically a `FeathersButton` limitation, not a blanket "keyboard confirm is
broken" one: the pause modal's Resume/Main Menu buttons (the *old* hand-rolled
`widgets::button()`, not `FeathersButton`) correctly respond to literal Enter headlessly, because
their activation (`on_ui_confirm_enter`'s direct `LegacyActivate` trigger) never goes through
`dispatch_focused_input` at all.

#figure(
  image("screenshots/playtest_0008/1790172206051-kb-pause-modal-open.png", width: 55%),
  caption: [Pause modal opened via literal `Escape`; "Resume"/"Main Menu" (hand-rolled widgets)
  both later confirmed reachable via literal `Enter` alone -- no gamepad, no mouse.],
)

Not believed to affect a real windowed client (which always has a `PrimaryWindow`) -- and now
directly confirmed by the project owner (see above): literal Enter works on the real client.
Documented in `docs/agents/skills/playtest.md` and `AGENTS.md` as a testing-harness note, not a gap
list item to fix.

== Full reachability results

#table(
  columns: (auto, auto, auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Element*], [*Mouse*], [*Keyboard*], [*Gamepad*],
  [Main menu: Connect], [✓ click], [✓ arrows+Enter\*], [✓ D-pad+South],
  [Main menu: Options popup + row], [✓ click], [✓ nav / ✗ confirm\*\*], [✓ D-pad+South],
  [Main menu: Credits], [✓ click], [✓ nav / ✗ confirm\*\*], [✓ (by analogy)],
  [Main menu: Language popup + row (verified ja-JP)], [✓ click], [✓ nav / ✗ confirm\*\*], [✓ (by analogy)],
  [Main menu: Quit], [not exercised (would end the session)], [--], [--],
  [Lobby: Level popup + row], [✓ click], [✓ nav / ✗ confirm\*\*], [✓ D-pad+South],
  [Lobby: Play], [✓ click (*fixed this session*)], [✓ nav / ✗ confirm\*\*], [✓ D-pad+South],
  [Lobby: Main Menu (disconnect)], [✓ click (*fixed this session*)], [✓ nav / ✗ confirm\*\*], [✓ D-pad+South],
  [In-game: Movement (WASD / left stick)], [--], [✓], [✓],
  [In-game: Jump (Space / South)], [--], [✓], [✓],
  [In-game: Look (mouse motion / right stick)], [✓], [--], [✓],
  [In-game: Select/Deselect (L/R click / thumbsticks)], [✓ fires, no target available -- see below], [--], [not
  independently re-tested, same binding shape],
  [In-game: Attack/Kill/Shoot/Spawn NPC (F/T/E/R)], [--], [✓ fires client-side; server-side spawn/attack
  known-broken, see gap list], [not independently re-tested],
  [In-game: Toggle stats (Tab / Select)], [--], [✓], [not independently re-tested],
  [In-game: Pause modal (Escape / Start)], [--], [✓ open], [✓ open],
  [Pause modal: Resume / Main Menu], [not exercised (hand-rolled widgets, not part of this
  sweep's UI-click focus)], [✓ nav+Enter, confirmed *both* rows], [✓ D-pad+South, confirmed in
  `playtest_0006`],
)

\* Connect is `AutoFocus`ed, so keyboard-Enter/gamepad-South work on it directly without needing
`dispatch_focused_input` in most cases tested here, since focus was already correct -- but see
\*\* for the general rule.

\*\* Literal-Enter *navigation* (arrow keys moving `InputFocus`) works headlessly on every
`FeathersButton`; literal-Enter *confirmation* does not, per the harness-ceiling finding above.
Mouse click and gamepad South both confirm every one of these reliably.

Attack/Kill/Shoot/Spawn-cube/Spawn-NPC could only be confirmed to *fire client-side* (the BEI
action observers run, `SpawnCube`/`SpawnNpc` trigger, `AttackAttempt`/`KillAttempt` would send if
a `Selected` target existed) -- actually spawning a `Cube`/`Npc` to select is blocked by the
already-documented caster-resolution regression (`AGENTS.md`'s gap list), not anything found in
this sweep. Not re-verified as a new finding.

= Artifacts & bookkeeping

- Screenshots: `docs/agents/playtests/screenshots/playtest_0008/` (LFS-tracked, six curated captures out
  of ~20 taken this session), plus the tool's raw capture staging directory
  `docs/agents/playtests/dist/screenshots/` (gitignored, all captures).
- Tool API surface used: `game/trigger` (`connect` only, as a fast baseline step -- not counted
  toward the reachability verdicts, which all used `game/keyboard`/`game/mouse`/`game/gamepad`),
  `game/keyboard`, `game/mouse`, `game/gamepad`, `game/state`, `game/screenshot[+ /get]`, plus
  diagnostic-only `world.query`/`world.get_resources`/`world.trigger_event` BRP calls (not
  counted toward reachability -- used only to root-cause the Play/Main-Menu bug).
- Code changed: `client/src/ui/lobby.rs` (the `Activate` import fix + an explanatory doc comment
  on `lobby_play_button`).
- Living documentation updated this session: `AGENTS.md` (two new gap-list entries -- the fixed
  bug, and the harness-ceiling note); `docs/agents/skills/playtest.md` (a new paragraph in §5b about the
  `dispatch_focused_input`/`PrimaryWindow` limitation).
- Human verification: the project owner ran the fixed build themselves on the real windowed
  client and confirmed both the mouse-only Connect -> select-level -> Play flow and literal Enter
  now work end-to-end -- the first finding in this report to be cross-checked outside the `--mcp`
  harness entirely, closing the loop the harness-ceiling finding could only reason about from
  source.
