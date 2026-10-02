#set document(
  title: "Agent Playtest 0002 — Bug Reproduction Pass (Caster-Resolution + Headless Camera)",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0002

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-22 ~23:25 UTC],
  [Commit], [`329c9bc` "Add playtest.md skill" — HEAD throughout this run],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP from a shell],
  [Client], [`target/debug/client --mcp` — dev profile + `dev-tools` feature, freshly built],
  [Server], [`target/release/server` — freshly built and started, empty state],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
  [Agent netcode client id], [`1790119541657318124` (connection entity `481v0`)],
  [Player entity (BRP id)], [`8589933379`],
)

= Purpose

Requested directly by the project owner: "run the playtesting harness and poke around in it."
Unlike playtest 0001 (a from-scratch tour verifying every reachable state), this run's goal
was to *re-verify specific items already on record* — the caster-resolution regression
(`AGENTS.md`'s gap list) and the frozen headless in-game rendering investigation — with harder
evidence than the previous report supplied, since a re-read of playtest 0001 alongside the
current `AGENTS.md` and a direct source read of `server/src/spawn.rs` turned up a discrepancy:
0001 captioned its `spawn_cube` step as confirmed without ever querying for the resulting
entities.

= Method

+ Build both binaries from a clean tree, confirm zero errors on each
  (`cargo build -p client --features dev-tools`, `cargo build -p server --release`).
+ Teardown any stray processes, verify ports free, launch server then client per the
  documented recipe (`docs/agents/skills/playtest.md` §2).
+ Drive the canonical get-in-game sequence via `game/trigger`/`game/select_level`/`game/state`.
+ Directly query `world.query` for `shared::cube_spawner::Cube` entities *before and after*
  firing `game/trigger spawn_cube`, rather than trusting the trigger's own "sent" response —
  the discrepancy this run exists to resolve.
+ Query the live camera entities over BRP (`bevy_camera::camera::Camera` +
  `Camera2d`/`Camera3d`/`IsDefaultUiCamera`) to look for a harder signal than "the screenshot is
  one color" for the frozen in-game rendering bug.
+ Screenshot at main menu and in-game; teardown cleanly.

= State tour

== Main menu

#figure(
  image("screenshots/playtest_0002/1790119533573-poke-mainmenu.png", width: 78%),
  caption: [`GameState::MainMenu`, fresh client boot. 292 unique pixel colors (measured via
  PIL) confirm the feathers panel renders correctly into the offscreen target — consistent with
  playtest 0001.],
)

== Lobby → in-game

`game/trigger {"event":"connect"}` → `Lobby` (confirmed via `game/state`, though its own
`connected` field lagged to `false` even in `Lobby` — the documented quirk, not a bug).
`game/levels` returned all three server levels (`spawn`, `minimal`, `outpost`). `game/select_level`
+ `game/trigger {"event":"play"}` on `levels/minimal.level.ron` produced a clean spawn:

```
{"connected":true,"game_state":"InGame","grounded":true,"hit_points":100,
 "position":[0.0,0.915,0.0],"velocity":[0.0,0.0,0.0],"gcd_remaining_secs":0.0}
```

No movement/input-injection re-verification this run — playtest 0001 already established
that path solidly through both state readback and a live two-client session (per `AGENTS.md`),
and re-driving it wasn't this run's purpose.

= Bug reproduction: `spawn_cube` caster resolution <caster-bug>

`AGENTS.md`'s gap list states every client→server "act" message (`spawn_cube`/`spawn_npc`/
`AttackAttempt`/`KillAttempt`) is silently dropped, because the affected systems still look the
caster's `Gcd` up on the *connection* entity — a leftover from before the player character
became a separate entity — rather than resolving the connection's player character first. This
run tested it directly rather than trusting the request-was-sent response:

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Query*], [*Result*],
  [`Cube` entities before any `spawn_cube` trigger], [`[]` (zero)],
  [3× `game/trigger {"event":"spawn_cube"}`], [all three returned `{"triggered":"spawn_cube"}`],
  [`Cube` entities after], [`[]` (zero — unchanged)],
  [Server log, the entire trigger window], [*no lines at all* — no "spawn", no error, silence],
)

This confirms the regression precisely as documented: the request is sent, accepted by the
client, forwarded to the server, and dies there with no trace. It also directly contradicts
playtest 0001's own framing of its equivalent step ("the three `spawn_cube` triggers …
the server spawns them … confirmed") — that report verified the message left the client, not
that anything spawned server-side. Recorded here so a future reader trusts this run's entity-query
evidence over 0001's unverified caption for this specific claim.

= Bug reproduction: frozen headless in-game rendering <camera-bug>

#figure(
  image("screenshots/playtest_0002/1790119585777-poke-ingame.png", width: 78%),
  caption: [In-game, fresh spawn. Single unique color `(26, 26, 38)` — the clear color, not a
  rendered frame — reproducing playtest 0001's finding exactly.],
)

Went one step further than 0001 by querying the three live camera entities directly over BRP
rather than only inspecting pixels:

#table(
  columns: (auto, auto, auto, auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  align: left,
  [*Entity*], [*Components*], [*Order*], [*`target_info`*], [*Renders?*],
  [`4294966308`], [`Camera2d`, `IsDefaultUiCamera`], [1], [`{1280, 720}` (populated)], [Yes — menu/lobby UI],
  [`8589933367`], [`Camera3d`, `IsDefaultUiCamera`], [5], [`null` — stays `null` on repeat query], [No — this is the player camera],
  [`4294966253`], [`Camera2d` only], [−1], [`{256, 256}` (populated)], [Yes — unrelated UI-quad target],
)

All three carry a correctly-set `RenderTarget` (the two offscreen-claimed cameras both point at
the same shared `OffscreenRenderTarget` image; the quad camera targets its own dedicated
texture, untouched by the retarget sweep by design). The player camera's `clear_color` is
correctly `None` (order ≠ 1, matching `retarget_cameras_to_offscreen`'s documented behavior).
The one field that differs between the working camera and the broken one is
`computed.target_info`: populated for both cameras that render, `null` and never populated for
the one that doesn't — re-queried several seconds later with no change.

This is a sharper, more specific lead than what's currently on record (`AGENTS.md`/ADR 0009
name the `IsDefaultUiCamera` ambiguity and the KTX2-skybox kill as the live suspects) — both the
working order-1 camera and the broken order-5 camera carry `IsDefaultUiCamera`, so that alone
doesn't explain the divergence. Whatever populates `ComputedCameraValues.target_info` per-camera
during Bevy's camera-target-extraction step is failing specifically for the player's `Camera3d`
against the shared offscreen image, despite an apparently-correctly-set `RenderTarget`/`Camera`.
Root cause still not found — this narrows *where* to look, not the fix itself.

= Findings <findings>

- *Caster-resolution regression: confirmed live, with harder evidence than before.* Zero `Cube`
  entities before or after three `spawn_cube` triggers; total server-log silence. Supersedes
  playtest 0001's unverified "confirmed" caption for the same action — see #ref(<caster-bug>).
- *Headless in-game rendering: still broken, new diagnostic lead recorded.* The player camera's
  `computed.target_info` never resolves, unlike every other camera sharing the same or a
  different valid render target — see #ref(<camera-bug>). Root cause still open.
- *Get-in-game flow: solid, reproduces cleanly.* Second independent run (different agent,
  different day) through connect→lobby→select→play produced the same clean spawn state
  playtest 0001 recorded — no regression in the core flow.
- *New, previously unrecorded log line*: `ICU4X data error: No segmentation model for language: ja`
  appears once during client boot, non-fatal (client continued normally). Not investigated
  further this run — likely related to `ja-JP` locale text segmentation (Fluent/parley), not
  confirmed. Worth a future session's attention if `ja-JP` text behaves oddly.
- *Process hygiene*: build-clean confirmed for both binaries before driving anything (`playtest.md`
  §1's rule); clean teardown afterward, ports verified free.

= Artifacts & bookkeeping

- Screenshots: `docs/agents/playtests/screenshots/playtest_0002/` (LFS-tracked, curated from this
  run — `1790119533573-poke-mainmenu.png` and `1790119585777-poke-ingame.png`), plus the tool's
  raw capture staging directory `docs/agents/playtests/dist/screenshots/` (gitignored, every capture).
- Tool API surface used: `game/trigger`, `game/levels`, `game/select_level`, `game/state`,
  `game/screenshot[+ /get]`, plus direct `world.query`/`world.get_components` BRP calls for the
  two bug-reproduction sections — see `client/src/dev/tool_api.rs`, ADR 0009, and
  `docs/agents/skills/playtest.md`.
- No living documentation updated this run beyond this report — the findings here refine
  (rather than contradict) `AGENTS.md`'s existing gap-list entries for both bugs; a future
  session fixing either should update `AGENTS.md` alongside the fix, not this report.
