#set document(
  title: "Playtest Index",
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Playtest Index

One entry per run in `docs/playtests/playtest_NNNN/` (see `docs/skills/playtest.md` §10 for
the format and layout these follow). Newest first. Update this file whenever a new playtest is
filed — that's part of filing it, not a separate later chore.

#outline(title: none, indent: auto)

== `playtest_0007` --- Add game/keyboard + game/mouse, Verify Real UI Clicks

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 08:33 -- 08:41 UTC],
  [*Commit*], [Started at `b340541`; not yet committed as of this entry],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0007/report.typ`],
)

Extended `game/gamepad` (`playtest_0006`) to keyboard and mouse: `game/keyboard` mocks
`ButtonInput<KeyCode>` directly (every one of Bevy's 160+ variants, via `KeyCode`'s own `serde`
impl); `game/mouse` mocks `ButtonInput<MouseButton>` plus drives `bevy_picking`'s real
`PointerInput` pipeline for cursor motion/position/clicks -- the first method in this API that
reaches UI by screen position rather than by navigating focus and confirming. First
implementation of mouse motion/wheel set `AccumulatedMouseMotion`/`AccumulatedMouseScroll` via a
direct resource write -- compiled and ran with no error, but silently did nothing (`look_yaw`
stayed `0.0`), because Bevy's own per-frame reset-from-events systems unconditionally overwrite
those resources every frame. Fixed by injecting real `MouseMotion`/`MouseWheel` events instead.
Verified live: a real mouse click on "Connect"/"Options" at their actual screenshot pixel
coordinates drove the genuine `bevy_ui`/`bevy_picking` pipeline (state transitions and
`selector` popup open/close confirmed independently via `game/state` and log lines); `KeyW`
moved the in-game player through the real replicated movement pipeline; mouse motion turned the
camera only after the event-based fix.

== `playtest_0006` --- Add game/gamepad, Verify the Literal Crash Path

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 01:37 -- 01:42 UTC],
  [*Commit*], [Started at `05a416c`; not yet committed as of this entry],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0006/report.typ`],
)

Added `game/gamepad` (mocks real `bevy_input::gamepad::Gamepad` button/axis state on a
synthetic entity, flowing through `bevy_enhanced_input`'s actual binding resolution) precisely
to close `playtest_0005`'s own stated gap: no way to reach UI navigation through the tool API,
only the three ahoy gameplay actions via `game/input`'s action-level mocking. First
implementation used the wrong `Gamepad` field (`digital`, not `analog` -- BEI's button reader
calls `Gamepad::get`, which is the analog map; confirmed by testing, a silent no-crash failure,
not an error) and was caught by actually running it, not by review. Fixed, then used
immediately: drove the *literal* reported crash path for the first time -- gamepad Start (open
pause modal) -> DPadUp (navigate focus to "Main Menu") -> South (activate) -- confirming
`playtest_0005`'s fix against the real interaction, not an equivalent one. No crash, clean
`GameState::MainMenu` transition.

== `playtest_0005` --- Fix the return_to_main_menu Crash

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 01:22 -- 01:24 UTC],
  [*Commit*], [Started at `80b6f0f`; fix not yet committed as of this entry],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0005/report.typ`],
)

User-reported crash on the real windowed client: in-game -> pause modal -> "Main Menu" ->
client panics. Matched a documented gap: `return_to_main_menu` was a live `todo!()`. Fixed to
mirror `lobby_main_menu_button`'s already-working pattern (`commands.trigger(Disconnect);
commands.set_state(GameState::MainMenu);`), routing through the crate-local `Disconnect` event
and its established two-part-disconnect handler rather than a fresh implementation. Verified via
the equivalent code path (`game/trigger disconnect` while genuinely in-game) since the tool API
has no way to simulate the actual keypress/button-click yet -- no crash, clean return to
`MainMenu`, UI correctly rendered. Also fixed a stale doc comment in `modal_menu.rs` caught
along the way (no behavior change). *Re-verified against the literal crash path in
`playtest_0006`*, once `game/gamepad` closed the tool-API gap this report's own verification
had to work around.

== `playtest_0004` --- Fix the Headless UI Render-Order Bug

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 00:53 UTC],
  [*Commit*], [Started at `41e29a9`; fix not yet committed as of this entry],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0004/report.typ`],
)

Root-caused and fixed `playtest_0003`'s open follow-on: the menu/lobby UI panel rendering
*under* the background instead of on top of it. Real cause: `bevy_ui`'s `DefaultUiCamera::get()`
fallback only ever considers `Window(Primary)`-targeting cameras -- structurally dead in `--mcp`
mode, where everything targets `Image` -- so headless mode's permanent bootstrap UI camera and
`player_camera()`'s own self-tagged `IsDefaultUiCamera` collided the moment a player existed,
breaking `bevy_ui`'s camera selection entirely (not an ordering problem, despite looking like
one). Very likely explains why all three of `playtest_0003`'s ordering-only fix attempts
regressed in-game rendering in ways that resisted explanation at the time. Fixed by actively
maintaining "exactly one live `IsDefaultUiCamera` holder" as an invariant, handed off between
the bootstrap camera and `player_camera()` as they come and go. Verified end-to-end: main menu,
lobby, in-game (HUD now renders too, not just the world), and -- newly tested, not covered by
any earlier playtest -- the full disconnect-back-to-main-menu round-trip all composite
correctly.

== `playtest_0003` --- Diagnose and Fix the Headless Camera Rendering Bug

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-22 23:00 -- 2026-09-23 00:15 UTC],
  [*Commit*], [Started at `329c9bc`; fix landed as `e5fe6c6` (committed after this run's
  verification) -- spans both sides of a code change, not a static state],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0003/report.typ`],
)

Root-caused and fixed the bug `playtest_0001`/`playtest_0002` both hit: `--mcp` mode's cameras
never rendered anything, because `bevy_render::camera::camera_system`'s `target_info` recompute
silently never fires for a camera retargeted after `Startup` (a one-line fix,
`projection.set_changed()`, in `retarget_cameras_to_offscreen`). This also explained the
previously-separate "KTX2 skybox kills offscreen rendering" finding -- same root cause, not a
real skybox bug; the skybox/TAA/SSAO strip workaround is removed. Surfaced a follow-on
UI-panel-renders-under-the-background ordering bug in menu/lobby specifically; three fix
attempts were tried and reverted here (each one regressed in-game rendering worse than the
ordering bug itself) -- see the report's "A follow-on issue found, attempted, and reverted"
section for what was tried and why. *Fixed in `playtest_0004`*, once the real (non-ordering)
cause was found. Also found, unrelated: the "minimal" level has no light source anywhere in its
content (renders black on any client, not a `--mcp`-specific issue).

== `playtest_0002` --- Bug Reproduction Pass (Caster-Resolution + Headless Camera)

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-22 ~23:25 UTC],
  [*Commit*], [`329c9bc` "Add playtest.md skill"],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0002/report.typ`],
)

A targeted re-verification pass, not a full state tour: checked two things `AGENTS.md`'s gap
list already claimed, with harder evidence than `playtest_0001` had supplied for the same
claims. Confirmed live: `spawn_cube` is silently dropped server-side (zero `Cube` entities
before/after the trigger, total server-log silence) -- *still open, not fixed by any playtest
since*, see `AGENTS.md`'s caster-resolution gap entry for the fix shape. Also found a sharper
diagnostic lead for the headless camera bug (`Camera.computed.target_info` staying `null`
specifically for cameras claimed after `Startup`) that `playtest_0003` went on to root-cause
and fix.

== `playtest_0001` --- Headless (--mcp) Client State Tour & Input Drive

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 ~02:38 local (2026-09-22 22:38 UTC)],
  [*Commit*], [`89f5eca` "Implement headless --mcp mode for client"],
  [*Agent*], [opencode session, GLM-5.3-Flash],
  [*Report*], [`docs/playtests/playtest_0001/report.typ`],
)

The first formal playtest: verified the agent/QA tool API (ADR 0009) end-to-end against a fresh
server and a fresh headless client -- drove every reachable client state, injected input through
the real replicated pipeline, and recorded what the agent sees vs. what a human on a windowed
client sees. Confirmed the movement/input/replication data path is fully solid headless (server-
authoritative sim, client prediction, all verified through state reads across a full menu→game
flow). First recorded the in-game blank-rendering bug and the KTX2-skybox-kill finding -- both
later found to be the same root cause, fixed in `playtest_0003`.
