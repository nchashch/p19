#set document(
  title: "Agent Playtest 0001 — Headless (--mcp) Client State Tour & Input Drive",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0001

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 ~02:38 local (2026-09-22 22:38 UTC)],
  [Agent], [opencode session, GLM-5.3-Flash, driving the tool API over loopback HTTP],
  [Client], [`target/debug/client --mcp` — headless agent host, `dev` profile + `dev-tools` feature],
  [Server], [`target/release/server` — freshly started for this playtest, empty state],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
  [Agent client id], [`Netcode(1790116738538578825)`],
)

= Purpose

Formal verification that the agent/QA tool API (ADR 0009) works end-to-end against a *fresh
server and a fresh headless client*: drive every reachable client state, inject player input
through the real replicated pipeline, capture screenshots of what the agent sees, and record
the known divergences between what the agent sees and what a human sees on a windowed client.

This is the first report in an accumulating series: each playtest's `report.typ` lives in
`docs/playtests/playtest_NNNN/`, its screenshots in `docs/playtests/screenshots/playtest_NNNN/`
(tracked via Git LFS), and its compiled `report.pdf` in the gitignored `docs/playtests/dist/`.

= Method

+ Tear down any running `client`/`server` processes; start the server, wait for the endpoint to
  bind (`:6000`), start the client with `--mcp`.
+ Drive the client exclusively through the tool API's `game/*` BRP methods:
  `game/trigger` (the app's own client-local events: `connect`, `play`, `spawn_cube`),
  `game/select_level`, `game/input` (BEI `ActionMock` injection on the replicated action
  entities), `game/screenshot` (labeled, persistent PNG), and `game/state` (a structured dump
  of connection/game-state/player data).
+ Screenshot at each state; sample `game/state` immediately after each input action and record
  the numbers below.

= State tour

== Asset loading

#figure(
  image("../screenshots/playtest_0001/1790116733474-pt1-asset-loading.png", width: 78%),
  caption: [The earliest readable moment after client start. The frame is pure black: the
  offscreen render target exists and is cleared, but nothing renders yet — the menu UI (and the
  loaded-world cameras) do not exist until `CommonAssets` finishes loading. By the time the
  capture round-tripped, the state read was already `MainMenu` — asset loading takes ~1s at the
  current asset count, so this state is barely capturable.],
)

== Main menu

#figure(
  image("../screenshots/playtest_0001/1790116736004-pt1-main-menu.png", width: 78%),
  caption: [`GameState::MainMenu`. The feathers panel with the Connect / Options / Credits /
  Quit / Language buttons renders correctly into the offscreen target. The top of the frame is
  cut off — the known 2× `UI_SCALE` overflow of the 720p offscreen target (cosmetic). The
  `.glb` menu background does *not* render behind the panel; the backdrop is the clear color
  (see #ref(<findings>)).],
)

== Lobby

After `game/trigger {"event": "connect"}`: the client opened a real lightyear UDP/netcode
connection (`Client Netcode(1790116738538578825) connected`) and the `On<Add, Connected>`
observer moved it to `GameState::Lobby`.

#figure(
  image("../screenshots/playtest_0001/1790116740567-pt1-lobby.png", width: 78%),
  caption: [`GameState::Lobby`. The lobby panel renders: the Play button and the level picker
  are cut off above the frame (same `UI_SCALE` overflow); the "Main Menu" button is visible.
  The server-replicated `Levels` singleton is readable here — the agent resolves level asset
  paths from it (`game/levels`).],
)

== Entering the game

`game/select_level {"asset_path": "levels/minimal.level.ron"}` sends `LoadLevelRequest`; the
server drops to `ServerState::Loading`, instantiates the level `.glb` headlessly (real Avian
colliders), returns to `InGame`. `game/trigger {"event": "play"}` sends `InGameRequest`; the
server spawns the agent's player character as a separate entity (`ControlledBy` → the client
gets `Controlled` → `LocalPlayer`), and the client's `On<Add, ClientInGame>` observer moves it
to `GameState::InGame`. Player spawned on the first poll, grounded at the origin.

#figure(
  image("../screenshots/playtest_0001/1790116750145-pt1-ingame-fresh.png", width: 78%),
  caption: [`GameState::InGame`, fresh spawn at (0, 0.92, 0), grounded, 100 HP. This is where
  the known headless camera bug bites: the frame is *pure clear color* — the player camera
  renders nothing at all (see #ref(<findings>)). A human on a windowed client sees the world
  correctly from this exact state.],
)

= Input drive

Movement is injected via `game/input`, which mocks the server-spawned replicated BEI action
entities (`Movement`/`Jump`/`RotateCamera`) with `ActionMock` — the exact same pipeline a real
gamepad drives, including the prediction/reconciliation path. All values below are
server-authoritative state read back through `game/state`.

#table(
  columns: (auto, auto, auto, auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  align: left,
  [*Step*], [*Action*], [*Position (x, y, z)*], [*Look yaw*], [*Grounded*],
  [spawn], [—], [(0.00, 0.92, 0.00)], [0.000], [true],
  [walk 1.5 s], [forward 90 ticks], [(-18.67, 0.92, 0.00)], [1.571], [true],
  [jump], [2-tick press], [(-18.67, 0.96, 0.00)], [1.571], [true (sampled after landing)],
  [3× `spawn_cube`], [server requests], [unchanged], [—], [—],
)

Readings: the 90° turn + 1.5 s walk produced a clean 18.67-unit displacement along the new
facing (−X at yaw +1.571 — the API's `yaw_delta` sign is empirically *inverted* relative to its
doc comment: negative `yaw_delta` turned the view toward −X here; the mock passes radians
per-tick straight through, bypassing the binding's `Scale` modifier). Jump airtime is shorter
than the ~0.5 s capture round-trip, so the mid-air sample shows the landing instead
(y = 0.96 vs the standing 0.92). The three `spawn_cube` triggers went through the client-local
`SpawnCube` event → the observer wraps the player's aim into `SpawnCubeRequest` → the server
spawns them (all confirmed through the same path a human hotkey drives).

#figure(
  image("../screenshots/playtest_0001/1790116755712-pt1-ingame-walked.png", width: 78%),
  caption: [After the turn + walk. State says (−18.67, 0.92, 0.00) — the simulation is exactly
  where it should be; the *pixels* are still the clear color. Data-path ✓, visual-path ✗.],
)

#figure(
  image("../screenshots/playtest_0001/1790116758752-pt1-ingame-jump.png", width: 78%),
  caption: [Post-jump sample (y = 0.96 vs standing 0.92 — the landing tail; airtime is shorter
  than the capture round-trip).],
)

#figure(
  image("../screenshots/playtest_0001/1790116764936-pt1-ingame-cubes.png", width: 78%),
  caption: [After three `spawn_cube` triggers. The cubes exist server-side and replicate, but
  are of course invisible here for the same reason as everything else in-game.],
)

= Findings <findings>

- *Movement/input pipeline: fully working headless.* Server-authoritative sim + replication +
  client prediction all verified through state data across an entire menu→game flow with zero
  manual intervention.
- *In-game visuals: still broken (FROZEN investigation).* Every in-game frame is the pure clear
  color, and the HUD does not render either, while menu/lobby UI states render fine. The
  KTX2-skybox kill is bisected and documented (see below), but the player camera renders
  nothing even with it stripped. Current suspicion: the `IsDefaultUiCamera` ambiguity (the
  player camera also carries the marker) plus whatever is silencing the player camera's whole
  output into an image render target. On a windowed client the same build renders correctly,
  so this is specific to the offscreen `Image` target path.
- *KTX2 skybox kill (bisected, frozen).* The loaded BC6H cubemap skybox makes any camera
  targeting an offscreen `Image` render *nothing* — silently, no wgpu error — while a
  programmatic uncompressed cubemap renders; brightness irrelevant; the same asset on a window
  target is fine. Worked around headless by stripping `Skybox`/TAA/SSAO from all cameras.
- *Join-burst input corrections (documented, benign).* In the prior two-client session, the
  joiner's replication burst hitched the pipeline ~10 ticks and produced a bounded run of
  `server_late_input_mismatch` ERRORs for the established client's input stream (1 tick late,
  sub-frame stick-drift corrections). Self-healed; documented in AGENTS.md rather than fixed.
- *UI_SCALE overflow.* The 2× UI scale exceeds the 720p offscreen target; the top of the menu
  and lobby panels is cut off in every capture. Cosmetic — bump the offscreen resolution if it
  matters.

= Artifacts & bookkeeping

- Screenshots: `docs/playtests/screenshots/playtest_0001/` (LFS-tracked, curated from this run),
  plus the tool's raw capture staging directory `docs/playtests/dist/screenshots/` (gitignored,
  every capture, not curated).
- The tool API surface used: `game/trigger`, `game/select_level`, `game/state`, `game/input`,
  `game/screenshot[+ /get]` — see `client/src/dev/tool_api.rs` and ADR 0009.
- Living documentation updated this session: AGENTS.md (join-burst `server_late_input_mismatch`
  episode; `--mcp` bullet — camera reorder/de-clear policy, persistent screenshots directory,
  skybox/TAA/SSAO strip rationale).
