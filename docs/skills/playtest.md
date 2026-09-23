# Skill: Playtesting prototype_19 with the MCP + BRP harness

Read this before driving the game as an agent. It is the distilled, battle-tested
playbook for launching the game headlessly, driving it through the QA tool API,
observing state, capturing what you see, and avoiding every trap hit so far.
Supplements (does not replace) `AGENTS.md` and `docs/adr/0009-agent-tool-api-via-brp.md`.

## 1. The three processes

| Process | Binary | Env | Ports |
|---|---|---|---|
| Game server | `target/release/server` | `BEVY_ASSET_ROOT=$PWD/server` | UDP :6000 (game) |
| Client (headless agent host) | `target/debug/client --mcp` | `CARGO_MANIFEST_DIR=$PWD/client BEVY_ASSET_ROOT=$PWD/client` | BRP HTTP :15702 · MCP :15710 |
| You (the agent) | shell + `curl`/python | — | talks to :15702 |

- Build first, and **verify the build actually succeeded**:

  ```sh
  cargo build -p client --features dev-tools 2>&1 | grep -cE "^error"   # must print 0
  cargo build -p server --release 2>&1 | grep -cE "^error"              # must print 0
  ```

  A failed build leaves the *previous* binary in place; rerunning then silently
  tests stale code. This bit us more than once.
- The env vars matter: bevy_asset resolves asset roots from **runtime**
  `CARGO_MANIFEST_DIR`/`BEVY_ASSET_ROOT`, so running the binaries bare breaks
  asset loading. Always launch with the env set.
- The tool API is gated by the **`dev-tools` cargo feature** (works in dev *and*
  release — the gate is the feature, never the profile). `--mcp` additionally
  switches the client to the headless agent host (no window at all).

## 2. Launch / teardown recipe

```sh
pkill -x client 2>/dev/null; pkill -x server 2>/dev/null; sleep 1
ss -tunap | grep 6000          # server port free?
ss -tlnp | grep -E "15702|15710"  # QA ports free?

# server
(BEVY_ASSET_ROOT=$PWD/server ./target/release/server > /tmp/opencode/server.log 2>&1 &)
sleep 4                        # wait for the bind

# client — put a hard lifetime on it (timeout N) and keep N big enough for the
# whole test; a dead client answers BRP with connection-refused (silent with -s)
(CARGO_MANIFEST_DIR=$PWD/client BEVY_ASSET_ROOT=$PWD/client \
  timeout 300 ./target/debug/client --mcp > /tmp/opencode/client.log 2>&1 &)
sleep 8                        # menu reachable ~2s; give slack

# sanity: the MCP server must have come up
grep -i "mcp tool server listening" /tmp/opencode/client.log
```

Hard rules learned the hard way:

- **Never `pkill -f "debug/client"`** — the pattern matches your *own shell's*
  command line and kills the test command itself (no output, no log file). Use
  `pkill -x client` / `pkill -x server` (exact process names).
- **Stray clients hold :15702/:15710.** The next client's MCP then logs
  `mcp server stopped: Os { code: 98, kind: AddrInUse }` and its BRP bind can
  fail too. Always teardown first, verify with `ss`.
- `timeout N` kills the client at N seconds wall-clock *including your thinking
  time between tool calls*. Budget generously; restart if it expires mid-test.
- Restart **both** server and client between test rounds unless you specifically
  want to test against existing state. A long-lived server accumulates zombie
  players (see §8).

## 3. The tool API surface

JSON-RPC 2.0 over HTTP POST to `http://127.0.0.1:15702`:

```sh
curl -s -m 6 http://127.0.0.1:15702 -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"game/state","params":{}}'
```

Custom `game/*` methods (see `client/src/dev/tool_api.rs`):

| Method | Params | Effect |
|---|---|---|
| `game/state` | — | Structured dump: `connected`, `game_state`, `position`, `velocity`, `look_yaw`, `look_pitch`, `grounded`, `crouching`, `dead`, `hit_points`, `max_hit_points`, `gcd_remaining_secs`, `player_entity` |
| `game/trigger` | `{"event":"connect"\|"play"\|"disconnect"\|"spawn_cube"\|"spawn_npc"}` | Fires the app's own client-local events — the same ones the menu buttons / hotkeys fire. `connect` opens the netcode connection, `play` sends `InGameRequest`, the spawns go through the client observers into server requests |
| `game/levels` | — | Lists the server-replicated `Levels` singleton (`asset_path` + `name`). Lobby only |
| `game/select_level` | `{"asset_path":"levels/minimal.level.ron"}` | Sends `LoadLevelRequest` (the lobby level-picker's exact message) |
| `game/input` | see §5 | Mocks a replicated BEI action entity for `ticks` fixed ticks |
| `game/screenshot` | `{"label":"..."}` optional | Async capture; PNG written under `docs/playtests/dist/screenshots/` (persistent, raw staging — not curated) |
| `game/screenshot/get` | — | `{"ready":true,"png_base64":...,"path":...}` for the newest capture; **does not consume it** |

Bevy builtins are **`world.*`-named** in 0.19 (`world.query`, `world.get_components`,
`world.list_resources`, `world.get_resources`, `world.list_components`,
`world.mutate_components`, `world.spawn_entity`, `world.insert_components`,
`world.remove_components`, `world.reparent_entities`, `world.trigger_event`, …).
`world.query` schema:

```json
{"data":{"components":["<exact::TypePath>", ...],"option":["<exact::TypePath>", ...]}}
```

→ array of `{"components":{"<path>":{...}},"entity":<id>}`. Required list = all
must be present; `option` list adds those *only when present*.

## 4. Canonical get-in-game sequence

```sh
# 1. connect → wait for Lobby
curl … "game/trigger" '{"event":"connect"}'
# poll game/state every ~2s until game_state == "Lobby" (first poll usually)

# 2. list levels (Lobby only) and pick the asset_path
curl … "game/levels"     # levels/minimal.level.ron is the only working level

# 3. select level (only if the server isn't already InGame with it — see §8)
curl … "game/select_level" '{"asset_path":"levels/minimal.level.ron"}'
sleep 3

# 4. play → poll until "position" appears in game/state (that's your player)
curl … "game/trigger" '{"event":"play"}'
# player spawns at (0, 0.92, 0), grounded, 100 HP
```

Timings on a fresh pair: connect→Lobby ≈ 2s; select→server `Loading`→`InGame`
≈ 1–2s; play→player-spawn ≈ 2s. Total ~15–20s including client boot.

Important state-machine facts:

- `InGameRequest` is handled **only while the server is in `ServerState::InGame`**
  and is **silently dropped otherwise** (e.g. while `Loading`). If the server is
  already in-game with the level you want, **skip `select_level` and just `play`**.
- `select_level` while in-game triggers the level-reload regression (a fresh
  `WorldAssetRoot` per request, no dedup) whose multi-second hitch can drop the
  netcode connection entirely.
- `game/state`'s `position` only appears once the client has `Controlled` on its
  player (`LocalPlayer` set). `game_state:"InGame"` *without* `position` means
  you inherited a zombie player's `ClientInGame` on reconnect — restart the
  server (§8).
- `connected:false` in `game/state` can lag reality; trust the client log's
  `connected to server` line instead.

## 5. Input injection semantics

`game/input` mocks the server-spawned **replicated** BEI action entities
(`Movement`/`Jump`/`RotateCamera`) with `ActionMock` — the exact pipeline a real
gamepad drives, including prediction/reconciliation. All actions take `ticks`
(1 tick ≈ 16.7 ms; `MockSpan::Updates(n)` — the mock fires on *every* tick it is
active).

```sh
# walk forward 2s (y=1 is forward; x is strafe; values are a held stick, not a rate)
{"action":"movement","x":0,"y":1,"ticks":120}      # ≈24.7 units at full deflection

# jump: a 2-tick press. Airtime is shorter than a capture round-trip — sample
# immediately if you want mid-air state
{"action":"jump","ticks":2}

# turn: yaw_delta/pitch_delta are RADIANS TOTAL for the call, spread across ticks
# (the method divides by ticks because BEI re-fires per tick). Calibrate signs by
# injecting a small amount and reading look_yaw/look_pitch in game/state — the
# conventions are empirically inverted relative to intuition:
#   negative yaw_delta → look_yaw increases (view rotates toward −X from +Z)
#   negative pitch_delta → look_pitch increases (looks down)
{"action":"rotate","yaw_delta":-1.5708,"ticks":30}
```

The mock **bypasses binding modifiers** (dead zones, `Scale`): values go through
as-is, which is why rotate is radians-direct rather than mouse-pixels. Read back
effects via `game/state` — position/velocity/yaw/pitch/grounded are
server-authoritative and reliable.

## 6. Probing the world (BRP)

Type paths must be **exact and fully qualified**. When in doubt, grep the source:

```sh
grep -rn "pub struct Mesh3d" ~/.cargo/registry/src/*/bevy_mesh-0.19*/src/
```

Known-correct paths (0.19): `bevy_camera::camera::Camera`,
`bevy_camera::camera::RenderTarget`, `bevy_camera::components::Camera3d`/`Camera2d`,
`bevy_camera::projection::Projection`, `bevy_mesh::components::Mesh3d`,
`bevy_pbr::mesh_material::MeshMaterial3d<bevy_pbr::pbr_material::StandardMaterial>`,
`bevy_transform::components::transform::Transform`,
`bevy_transform::components::global_transform::GlobalTransform`,
`bevy_ecs::hierarchy::Children` (not `relationship::`),
`bevy_world_serialization::components::WorldAssetRoot`,
`shared::cube_spawner::Cube`, `shared::assets::level::ClientWorldAsset`,
`client::controls::fps_controller::FpsCamera`.

**The single biggest trap: serialization failure ≠ absence.** Components holding
asset handles (`Mesh3d`, `RenderTarget`, `WorldAssetRoot`, …) cannot BRP-serialize
(`Arc<StrongHandle>` lacks `ReflectSerialize`) — they come back `null` in
`world.query` and as errors in `world.get_components`. To decide "is component X
present", call `world.get_components` with X listed explicitly and read the
`errors` map:

- `code:-23402 … did not register ReflectSerialize` → **present, unserializable**
- `code:-23403 … not present in Entity` → genuinely absent

`Assets<Image>` / `Assets<Mesh>` are not BRP-reflected at all — you cannot count
asset stores; count *entities* carrying handles instead.

Entity ids appear in **two formats**: BRP returns a large u64 bit-pattern, while
the client *log* prints `index+generation` form (`1224v1`). They are the same
entity — e.g. BRP `8589933367` ≡ log `1224v1` (roughly `2^33 − 1225`). Don't
conclude "different entity" when comparing a log line to a query result; convert
via the ±index relationship instead.

Rapid-fire BRP calls can transiently fail (empty body / `KeyError: 'result'`).
Retry once after ~2s before concluding anything.

## 7. Screenshots

- `game/screenshot {"label":"ingame-cubes"}` →
  `docs/playtests/dist/screenshots/<millistamp>-ingame-cubes.png` (persistent, never
  consumed). Poll `game/screenshot/get` until `ready:true` (~1–3s), or just
  `ls docs/playtests/dist/screenshots/`.
- The capture reads the **offscreen texture** in `--mcp` mode = exactly what the
  agent "sees" (the most recently arrived claimed camera's view).
- Annotate captures by measuring pixels, not by eyeballing memory:
  unique-color counts via PIL tell you instantly whether a frame rendered
  (hundreds of colors), is the clear color (1 color), or is the menu
  (~200 colors).
- Never open/inspect compiled PDFs in this session — the environment breaks with
  "Functionality not supported". Compile typst reports, but verify only that the
  compile command exits 0 (§10).

Known visual divergences in `--mcp` (see AGENTS.md for the full root-cause writeup —
the "nothing renders at all" version of this is fixed; one cosmetic ordering issue
remains):

- Menu/lobby: both UI *and* the real `.glb` background now render correctly. The UI
  panel currently draws *underneath* the background rather than on top of it
  (background arrives on a later frame than the UI camera and draws over it, per
  the retarget scheme's ordering rules) — a known, deliberately unfixed follow-on,
  not the original bug. Top of frames still cut off (2× `UI_SCALE` vs 720p, unrelated,
  cosmetic).
- In-game: the real starfield HDRI skybox and level geometry now render correctly.
  `levels/minimal.level.ron`'s floor renders black specifically because that level's
  content has zero light entities anywhere — not a `--mcp` bug, confirmed via BRP,
  would be black on a windowed client too.

## 8. Known failure modes & recovery

| Symptom | Cause | Recovery |
|---|---|---|
| Empty curl bodies | Client dead (timeout expired) or malformed shell quoting | `ps aux | grep -c "[d]ebug/client"`; rebuild the curl |
| Curls return nothing *and* client alive | **zsh doesn't word-split unquoted vars** — `H='-H …'; curl $H …` passes one giant arg | Always write literal URLs/headers in curls |
| Next client logs `mcp server stopped: AddrInUse` | Stray client holds :15710/:15702 | `pkill -x client`; verify `ss` |
| `game/state` says InGame but no `position`, forever | Zombie-player inheritance: a previous client's `Lifetime::Persistent` player replicated its `ClientInGame` to your fresh connection without `Controlled` | Restart the server (the parked reconnect bug) |
| Connection drops mid-session after `select_level` | Level-reload hitch (no-dedup regression) exceeds netcode tolerance | Restart both; avoid re-selecting a loaded level |
| Test results look impossible / old behavior | Stale binary from a failed build | Rebuild, `grep -cE "^error"` must be 0 |
| Server floods `server_late_input_mismatch` when a second client joins | Join-burst replication hitch blows the 2-tick input-delay headroom; self-heals in ~10 ticks | Benign — document, don't fix (AGENTS.md has the episode) |
| `pkill -f` kills your own test command | `-f` matches your shell's own command line | Use `pkill -x` |

## 9. Multi-client testing

The user's windowed client and your headless client can share a server:

1. User starts their client and gets in-game normally.
2. You launch `--mcp`, `connect`, then **skip `select_level`** (the server is
   already in-game; re-selecting triggers the reload drop) and `play` directly.
3. Your capsule spawns at the origin and is visible to the user; movement/turns
   are mutually visible.
4. Expect the benign `server_late_input_mismatch` burst on the server during
   your join (~10 ticks of 1-tick-late input corrections).

Each client instance generates a fresh nanosecond netcode client-id — no
collision by design.

## 10. Reporting (typst playtests)

**Every run gets a report, no exceptions.** Any session where a client gets started,
driven through the MCP/BRP tool API in any way, and then torn down — a full formal state
tour, a five-minute poke to sanity-check one thing, a targeted bug-reproduction pass, a
one-off check while debugging something else — gets a filed report. "This was too small/
informal to write up" is exactly the case this rule exists to rule out: the value is in the
accumulating, searchable history (what was tried, what was observed, on what date, against
what commit), not in any single run being significant. Don't wait to be asked.

**Layout — three separate locations, not one directory, since only the screenshots need
Git LFS and only the PDF needs to stay out of git entirely:**

- `docs/playtests/playtest_NNNN/report.typ` (find the next free number) — the report
  source. Plain text, tracked normally (not LFS).
- `docs/playtests/screenshots/playtest_NNNN/*.png` — the *curated* screenshots this
  report's `report.typ` actually references (copy the relevant ones in from the raw
  capture staging directory, `docs/playtests/dist/screenshots/` — see §7 — don't dump
  every capture from the session, just what's worth keeping). Tracked via **Git LFS**
  (`.gitattributes` covers `docs/playtests/screenshots/**/*.png`) — confirm
  `git lfs status` shows them as LFS objects, not plain git blobs, before committing.
- `docs/playtests/dist/playtest_NNNN.pdf` — the compiled report. **Gitignored**
  (`/docs/playtests/dist` in `.gitignore`) — regenerable from `report.typ`, never commit
  it directly.

Reference screenshots from `report.typ` **relatively**, e.g.
`image("../screenshots/playtest_NNNN/<file>.png", ...)` (the report lives one level
under `docs/playtests/`, the screenshots one level under `docs/playtests/screenshots/`);
use `#figure(image(...), caption:[...])`, a metadata `#table`, numbered `= Sections`, and
a `<findings>` label for the findings block (see `docs/playtests/playtest_0001/report.typ`
for the house style).

**The findings section isn't just confirmed bugs.** Record observations, suspicions,
things that looked odd but weren't chased down, open questions, anything that would help
a *future* session pick up the thread faster — not only what got definitively proven.
Say what's uncertain as uncertain; don't inflate a hunch into a confirmed finding, but
don't omit it either. Cross-reference earlier reports by number when a run confirms,
contradicts, or narrows something an earlier one said (`playtest_0002` superseding
`playtest_0001`'s unverified `spawn_cube` caption is the working example of this).

Compile with (the `--root` matters — a bare `typst compile docs/playtests/playtest_NNNN/report.typ`
fails with "path would escape the project root" the moment it hits a `../screenshots/...`
reference, since typst sandboxes relative paths to the input file's own directory by default):

```sh
typst compile --root docs/playtests \
  docs/playtests/playtest_NNNN/report.typ \
  docs/playtests/dist/playtest_NNNN.pdf
```

Check the exit code only. Do **not** open or read the produced PDF. typst 0.15.1 is at
`/usr/sbin/typst`.

## 11. Practical flow summary

1. Teardown (`pkill -x` both), verify ports free.
2. Build both binaries; confirm zero errors.
3. Start server (env + `sleep 4`), start client (env, `timeout 300+`, `sleep 8`),
   confirm `mcp tool server listening` in the log.
4. Menu: screenshot + `game/state`.
5. `connect` → poll Lobby → `game/levels` → screenshot.
6. `select_level` (fresh server only) → `play` → poll for `position` →
   screenshot.
7. Drive with `game/input`; sample `game/state` after each action; take labeled
   screenshots at each interesting state.
8. Capture logs from both processes; teardown when done (or leave the pair for
   the user, saying which processes are yours).
9. Write the playtest report (see §10 for the three-location layout); compile with `--root docs/playtests`; never inspect the PDF.
