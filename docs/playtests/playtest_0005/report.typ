#set document(
  title: "Agent Playtest 0005 — Fix the return_to_main_menu Crash",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0005

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 01:22 -- 01:24 UTC],
  [Commit], [Started at `80b6f0f` "Fix stale AGENTS.md"; the fix described here is not yet
  committed as of this report],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP],
  [Client], [`target/debug/client --mcp` -- dev profile + `dev-tools` feature],
  [Server], [`target/release/server`],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
)

= Purpose

Reported directly by the project owner on the real (windowed, non-`--mcp`) client: "When I get
in game on a server -- and then go to the modal menu and then press 'Main Menu' -- the client
crashes." This matches a bug already documented in `AGENTS.md`'s gap list --
`client/src/controls/controls.rs`'s `return_to_main_menu` was a live `todo!()`, unconditionally
panicking whenever reached, whether from the pause-modal button, its VR wrist-panel equivalent,
or the `MainMenu` input action (Escape / gamepad Start).

= The fix

`return_to_main_menu` mirrors `ui/lobby.rs`'s `lobby_main_menu_button` exactly -- the one place
this exact pattern already worked, for the equivalent Lobby-to-MainMenu transition:

```rust
pub(crate) fn return_to_main_menu(mut commands: Commands) {
    commands.trigger(Disconnect);
    commands.set_state(GameState::MainMenu);
}
```

`Disconnect` here is this crate's own local event (`crate::events::Disconnect`, added to the
import list -- it wasn't in scope before), not `lightyear::prelude::Disconnect`. It routes
through `lifecycle::networking::on_disconnect_request`, the only correct way to disconnect in
this codebase: that function's own doc comment documents a previously-confirmed crash
(`Option::unwrap()` panic in `lightyear_udp`) from triggering lightyear's `Disconnect` alone and
skipping `Unlink`, which leaves a stale `Linked` marker blocking the *next* connection attempt.
`return_to_main_menu` needed to go through the same two-part disconnect for the same reason.

Also fixed a stale doc comment in `client/src/ui/modal_menu.rs`: a comment on the
`OnExit(GameState::InGame)` hook that resets `ModalMenuState` claimed "the Main Menu button
already sets it directly" -- not true of this implementation, which only leaves
`GameState::InGame` and lets that same exit hook handle the reset generically. Corrected to
describe what actually happens.

= Verification

The tool API's `game/input` only mocks the `movement`/`jump`/`rotate` BEI actions -- there's no
way to simulate the actual Escape/Tab keypress or a UI button click via `--mcp` today, so the
literal "open pause menu, click Main Menu" interaction couldn't be reproduced pixel-for-pixel
through the agent tooling. What *was* verified directly: `return_to_main_menu` is now a trivial
pass-through to the exact same `Disconnect` event `game/trigger disconnect` already sends, so
triggering that while genuinely in-game (live player, live connection) exercises the identical
code path the pause-menu button now calls.

#figure(
  image("../screenshots/playtest_0005/1790126684711-mainmenu-fix-ingame.png", width: 70%),
  caption: [In-game, immediately before the disconnect trigger -- confirmed via `game/state`
  (`game_state: "InGame"`, real `player_entity`).],
)

`game/trigger {"event": "disconnect"}` fired while in this exact state. Result: no panic, no
crash -- the client process was still running afterward (`ps aux` confirmed both the `timeout`
wrapper and the client binary alive), the client log had zero panic/`todo!()`/`unwrap`-on-`None`
lines, and `game/state` read back `{"game_state": "MainMenu", "player_despawned": true}`.

#figure(
  image("../screenshots/playtest_0005/1790126696512-mainmenu-fix-postdisconnect.png", width: 70%),
  caption: [Main menu after the disconnect: UI correctly on top of the real background (the
  `playtest_0004` render-order fix, unaffected by this change), no crash.],
)

= Findings <findings>

- *Fixed and verified via the equivalent code path*, not the literal UI interaction --
  see the tool-API gap noted above. Confidence is high regardless: `return_to_main_menu` has no
  logic of its own beyond the two calls shown, both already proven correct (`lobby_main_menu_button`
  and `playtest_0004`'s own disconnect-while-in-game test both already exercised them
  successfully before this run).
- *A real, reusable gap in the tool API*: no way to inject a raw key press, gamepad button, or
  UI-node click through `game/input` or any other method -- only the three specific BEI player-
  movement actions. Worth a follow-up ADR/design note if agent-driven testing of menu/UI
  interaction specifically (as opposed to gameplay movement) becomes a recurring need; not
  attempted here since it's out of scope for a single bug fix.
- The VR wrist-panel "Main Menu" equivalent (`modal_menu.rs`'s `spawn_vr_in_game_wrist_panel`)
  was confirmed by reading the code to reuse the *same* `modal_menu` scene and `main_menu_button`
  observer, not a separate implementation -- so this fix covers it too, though VR mode itself
  wasn't (and couldn't easily be) exercised via `--mcp` this run.

= Artifacts & bookkeeping

- Screenshots: `docs/playtests/screenshots/playtest_0005/` (LFS-tracked, the two captures
  above), plus the tool's raw capture staging directory `docs/playtests/dist/screenshots/`
  (gitignored).
- Tool API surface used: `game/trigger` (`connect`, `play`, `disconnect`), `game/select_level`,
  `game/state`, `game/screenshot[+ /get]` -- see `client/src/dev/tool_api.rs`, ADR 0009,
  `docs/skills/playtest.md`.
- Code changed: `client/src/controls/controls.rs` (`return_to_main_menu`'s implementation, the
  `Disconnect` import), `client/src/ui/modal_menu.rs` (the stale doc-comment correction only --
  no behavior change there).
