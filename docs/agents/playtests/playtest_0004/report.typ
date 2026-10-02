#set document(
  title: "Agent Playtest 0004 — Fix the Headless UI Render-Order Bug",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0004

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 00:53 UTC],
  [Commit], [Started at `41e29a9` "Add commit hashes and dates to playtest reports"; the fix
  described here is not yet committed as of this report],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP, editing code between runs],
  [Client], [`target/debug/client --mcp` -- dev profile + `dev-tools` feature],
  [Server], [`target/release/server`],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
)

= Purpose

Requested directly by the project owner as a follow-up to `playtest_0003`'s reverted ordering
attempts: "see if you can figure out the remaining render order bug for the --mcp client, with
the aim being for the --mcp client to be visually identical to the windowed client, and
preferably running the exact same codepaths." That reframing -- match desktop's actual
behavior rather than invent new ordering logic -- is what led to the real root cause, which
none of `playtest_0003`'s three attempts had found.

= Root cause

A forked research pass (not represented as its own playtest -- pure code reading, no client
ever run) found the actual mechanism via a direct read of `bevy_ui::ui_node::DefaultUiCamera::get()`
(`bevy_ui-0.19.0/src/ui_node.rs`): it selects the sole `IsDefaultUiCamera`-bearing entity via
`.single()` when exactly one exists; its fallback (used when zero or more than one exist) only
ever considers cameras targeting `RenderTarget::Window(Primary)` -- structurally excluding
`RenderTarget::Image` entirely. In `--mcp` mode every camera targets `Image`, so that fallback
can never succeed, full stop.

This explains why desktop needs zero custom ordering code at all: it has *no persistent UI camera*, ever -- `IsDefaultUiCamera` is only ever inserted by `player_camera()`
(`client/src/controls/camera.rs:55`) and headless mode's own `Startup`-spawned bootstrap camera
(`client/src/main.rs`, `mcp_headless` branch). During `MainMenu`/`Lobby` on desktop, zero
entities carry the marker, so the fallback runs -- and since the glTF background camera is the
*only* `Window(Primary)` candidate, it wins by elimination. UI renders as an overlay *within
that camera's own render pass*, not via a second, separately-clearing camera competing for
compositing order.

Headless mode's bootstrap camera exists specifically because that fallback can't do the same
job when every target is `Image` -- but it never despawned, and `player_camera()` also self-tags
`IsDefaultUiCamera`, so the instant a player existed there were two simultaneous holders,
`.single()` failed, the fallback couldn't rescue it (dead for `Image` targets regardless), and
`DefaultUiCamera::get()` returned `None` entirely -- explaining not just the UI-under-background
symptom but very plausibly the unexplained in-game rendering regressions all three of
`playtest_0003`'s ordering-only fix attempts hit, since they were tried against this same
standing ambiguity without knowing it was there.

= The fix

Two independent pieces, both in `client/src/controls/camera.rs`:

+ `HeadlessUiCameraBootstrap` marks the bootstrap camera specifically (distinct from
  `player_camera()`, which also carries `IsDefaultUiCamera`).
  `maintain_default_ui_camera` keeps "exactly one live entity carries `IsDefaultUiCamera`" true
  at all times: hands the marker to the bootstrap camera whenever nothing else holds it (boot,
  and every return trip from `InGame` back to `Lobby`/`MainMenu`), strips it the instant a real
  content camera claims it on its own.
+ `keep_ui_camera_drawn_last` re-derives, fresh every tick (not decided once at claim time,
  since *which* entity holds the marker changes over a session), which camera currently holds
  it and keeps it drawn last -- highest order, no clear -- among cameras sharing the offscreen
  target.

A real bug was caught and fixed before ever running the client: the first draft of (2) computed
"highest order" across *all* cameras including the UI camera's own already-bumped value, which
would have incremented its order by one forever, every tick. Fixed by computing the bump target
from non-UI cameras only, and by splitting into three explicit phases (bump, then re-derive
lowest-order fresh, then assign clear colors) rather than reusing a pre-bump snapshot that would
have left one tick per transition uncleared.

= Verification

Full state cycle, driven end-to-end through the tool API -- the first time this specific
round-trip (including the disconnect-back-to-menu direction) has been tested in `--mcp` mode at
all:

#figure(
  image("../screenshots/playtest_0004/1790124794028-orderfix2-mainmenu.png", width: 70%),
  caption: [Main menu: UI panel correctly composited on top of the real `.glb` background.],
)

#figure(
  image("../screenshots/playtest_0004/1790124808556-orderfix2-lobby.png", width: 70%),
  caption: [Lobby: same background asset, Play/Level/Main Menu panel on top, correct.],
)

#figure(
  image("../screenshots/playtest_0004/1790124817618-orderfix2-ingame.png", width: 70%),
  caption: [In-game: the starfield HDRI still renders (no regression from `playtest_0003`'s
  fix), and for the first time ever in `--mcp` mode the HUD also renders on top of the world --
  the small white dot at screen center is the crosshair, which never appeared at all before this
  fix either, since it rides the same `IsDefaultUiCamera`-selected camera pass as the menu UI.],
)

#figure(
  image("../screenshots/playtest_0004/1790124840549-orderfix2-postdisconnect.png", width: 70%),
  caption: [After `game/trigger disconnect` (`game_state` confirmed `MainMenu`,
  `player_despawned: true` via `game/state`): UI correctly on top again, confirming the marker
  hand-*back* to the bootstrap camera works, not just the initial hand-off to the player.],
)

Also checked directly over BRP after the full cycle: final camera state showed exactly one live
`IsDefaultUiCamera` holder (the bootstrap camera, order 7, `clear: None`), the new menu
background camera at order 6 with `clear: Default`, and the unrelated 256x256 quad camera
untouched at order -1 -- no lingering duplicate markers, no zombie cameras. The client log's
`camera {entity} clear -> {Default|None}` transition trace (added as permanent diagnostic
output, not temporary) showed exactly 11 transitions across the whole session, each corresponding
to a real state change (background arrives/despawns, player spawns/despawns, disconnect) --
no oscillation or thrashing between ticks.

= Findings <findings>

- *Root cause found and fixed, verified end-to-end.* Menu, lobby, in-game (including the HUD,
  not just the world), and the disconnect-back-to-menu round-trip all composite correctly now.
  `--mcp` mode's rendering is now a faithful match for what a human on a windowed client sees at
  every state this run exercised.
- *The three `playtest_0003` ordering-only attempts were very likely never actually about
  ordering* -- they were fighting this same `IsDefaultUiCamera` collision the whole time, which
  is consistent with (but not independently re-confirmed against) their specific unexplained
  regressions, since none of them were individually re-tried with just the ambiguity fix applied
  on top of their exact original ordering scheme.
- *A real bug caught before ever touching the client*: the self-referential "bump to
  highest-order + 1" mistake in the first draft of `keep_ui_camera_drawn_last`, which would have
  produced an ever-climbing `camera.order` on every tick. Worth the reminder that "BRP confirms
  the final config is correct" (`playtest_0003`'s own verification method) doesn't catch a bug
  that's only wrong *during* convergence, not at rest.
- *Not exercised this run*: VR mode (`XrCamera`, a separate codepath per `on_xr_camera_added`,
  untouched by any of this), and whether `IsDefaultUiCamera` ever legitimately needs to move to
  a *third* kind of camera beyond bootstrap/player (e.g. a spectator or replay camera, if either
  is ever added) -- the current fix only knows about these two.

= Artifacts & bookkeeping

- Screenshots: `docs/agents/playtests/screenshots/playtest_0004/` (LFS-tracked, the four state-cycle
  captures above), plus the tool's raw capture staging directory
  `docs/agents/playtests/dist/screenshots/` (gitignored, every capture from this run).
- Tool API surface used: `game/trigger` (`connect`, `play`, `disconnect`), `game/select_level`,
  `game/state`, `game/screenshot[+ /get]`, plus `world.query` BRP calls for the final camera-state
  check -- see `client/src/dev/tool_api.rs`, ADR 0009, `docs/agents/skills/playtest.md`.
- Living documentation updated this session: `AGENTS.md`'s `--mcp` gap-list bullet (rewritten:
  the follow-on render-order bug is fixed, not just the original blank-frame bug);
  `docs/agents/skills/playtest.md` §7's "known visual divergences" (rewritten to match);
  `client/src/controls/camera.rs`'s doc comments on `retarget_cameras_to_offscreen` and the two
  new systems.
