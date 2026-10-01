# Skill: Playtesting prototype_19 with the MCP + BRP harness

Read this before driving the game as an agent. It is the distilled, battle-tested
playbook for launching the game headlessly, driving it through the QA tool API,
observing state, capturing what you see, and avoiding every trap hit so far.
Supplements (does not replace) `AGENTS.md` and `docs/adr/0009-agent-tool-api-via-brp.md`.

**Default input method: gamepad** (§5) — you are emulating a Steam Deck player,
the primary target platform. Use keyboard+mouse only when the task explicitly
targets them.

**Default observation method: data, not pixels** (§7, rationale in
[ADR 0011](../adr/0011-agent-vision-and-fleet-improvements.md)) — understand the world
via `game/state`/`game/ui`/BRP queries; screenshot only when asked or when the
thing under test is inherently visual. If the data surface is missing what you
need, report the API gap rather than falling back to screenshots.

## 1. The three processes

| Process | Binary | Env | Ports |
|---|---|---|---|
| Game server | `target/release/server` | `BEVY_ASSET_ROOT=$PWD/server` | UDP :6000 (game) |
| Client (headless agent host) | `target/debug/client --mcp` | `CARGO_MANIFEST_DIR=$PWD/client BEVY_ASSET_ROOT=$PWD/client` | BRP HTTP :15702 · MCP :15710 |
| You (the agent) | shell + `curl`/python | — | talks to :15702 |

`--no-render` client variant: everything in this playbook works **except
screenshots** — it is the rendered headless mode with the render plugins (and
the wgpu/Vulkan instance) disabled, so it needs no GPU driver at all and runs
at a fraction of the CPU (~0.7 core vs ~1.3 per client). UI layout, `game/ui`,
hover/clicks, input mocking, and netcode are identical (`bevy_ui` 0.19's
layout/picking is render-free logic; a shim feeds the one camera value UI reads
back from the render side). Default choice for gameplay/UI/logic fleets on
small boxes; use rendered mode when a check is inherently visual (§7).
Rationale and implementation notes:
[ADR 0012](../adr/0012-no-render-agent-client-mode.md).

## 1a. Client configurations — call `game/client_info` first

Every client reports its own launch configuration via
`game/client_info` (mode flags + surface ports + `screenshots_available`).
**Call it before anything else on a fresh session** — it tells you which tools
are meaningful here, without trusting whatever launch line someone else used.

| Configuration | Launch flags | Renders? | Screenshots? | World visuals? | Typical use |
|---|---|---|---|---|---|
| Headless agent host (default) | `--mcp` | yes, offscreen 1280×800 via lavapipe | ✓ (crop, unchanged-suppression) | ✓ load + replicate | Full playtesting, visual checks included |
| **GPU-less agent host** | `--mcp --no-render` | **no** (no Vulkan needed at all) | ✗ clean error — use `game/ui` + `game/state` | ✗ never load (implies `--no-common-assets`) | Gameplay/UI/logic fleets on small boxes; ~0.7 core + ~0.3 GB vs ~1.3 cores + ~1.1 GB |
| `--no-common-assets` | `--no-common-assets` (alone or implied) | yes | ✓ | none via manifest — content only via `ClientWorldAsset`s by path; fonts/sounds/icons fall back to embedded/`None` | Plaintext-asset-root playtesting (§10) |
| Windowed dev client | none (dev build has `dev-tools`) | yes, real window | ✓ via `Screenshot::primary_window` | ✓ | Human-visible sessions; BRP still on :15702, but `game/mouse move_to`/clicks are `--mcp`-only |
| Fleet member | `--mcp --brp-port N --mcp-port N` | per above flags | ✓ (isolated `screenshots/client-N/` dir) | per above flags | Many clients, one server (§9) |
| **Observer** | `--mcp --headless-render` | yes, at **2 fps** (logic still 60 Hz via catch-up) | ✓ on demand | ✓ load + replicate | The fleet's *eye*: ~1.3 cores + GPU VRAM, but one observer serves vision for a fleet of `--no-render` clients |

Observer mode particularities (`--headless-render`):

- Join the world with `game/trigger {"event":"observe"}` instead of `play` —
  this sends `ObserveRequest`, which joins the game room (full replicated world
  state) but spawns **no player character**. `game/state` will never show a
  `position` on the observer; that's correct, not a bug.
- `game/cameras` lists every camera (including the spawned `ObserverCamera` at
  0, 2, 8). Aim it via `world.mutate_components` on its `Transform`, then
  capture `game/screenshot {"camera": <entity-id>}` — that renders *that
  camera's* view (its `RenderTarget` is borrowed for one frame, then restored).
- The 2 fps loop means logic runs in batched catch-up ticks; replication and
  state stay correct, but don't use the observer for timing-sensitive
  measurement.

Particularities worth remembering:

- `--no-render` implies `--mcp` + `--no-common-assets`; the reported flags in
  `game/client_info` are the **effective** ones (implications included), so
  trust them over the launch line.
- Only `bevy_mod_outline`/`bevy_hanabi`/FPS-overlay plugins are skipped under
  `--no-render` — selection visuals and particles don't exist there, which also
  means "particle effect fired" / "outline appeared" cannot be tested in this
  mode (use a rendered client).
- Windowed clients keep the OS cursor; the agent-cursor crosshair is
  headless-only.

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
| `game/state` | — | Structured dump: `connected`, `game_state`, `position`, `velocity`, `look_yaw`, `look_pitch`, `grounded`, `crouching`, `dead`, `hit_points`, `max_hit_points`, `gcd_remaining_secs`, `player_entity`, `selected` (current attack target, when set) |
| `game/trigger` | `{"event":"connect"\|"play"\|"observe"\|"disconnect"\|"spawn_cube"\|"spawn_npc"\|"attack"\|"kill"}` | Fires the app's own client-local events — the same ones the menu buttons / hotkeys fire. `connect` opens the netcode connection, `play` sends `InGameRequest`, `attack`/`kill` send `AttackAttempt`/`KillAttempt` for whatever `game/select` targeted |
| `game/select` | `{"entity": <u64 id this client reports>}` or `{"nearest": true}` or `{"name": "<string>"}` | Injects crosshair targeting headlessly (`Selected` = that entity, validated `Selectable`). `nearest` picks the closest *other* player; `name` matches a player's generated unique name (e.g. `"Brisk Falcon"`, suffixed `#2`/`#3` on collision). Pair with `game/trigger attack\|kill` for combat QA — the crosshair raycast itself needs a real window |
| `game/levels` | — | Lists the server-replicated `Levels` singleton (`asset_path` + `name`). Lobby only |
| `game/select_level` | `{"asset_path":"levels/minimal.level.ron"}` | Sends `LoadLevelRequest` (the lobby level-picker's exact message) |
| `game/input` | see §5 | Mocks a replicated BEI action entity for `ticks` fixed ticks |
| `game/screenshot` | `{"label":"..."}`, `{"crop":[x,y,w,h]}` optional | Async capture; PNG written under `docs/playtests/dist/screenshots/` (persistent, raw staging — not curated). `crop` saves only that sub-rect — same pixel space as `game/ui` rects, clamped to frame bounds. Prefer cropped captures of a `game/ui` rect when inspecting one element: fewer vision tokens, and no provider downscale on the region of interest |
| `game/screenshot/get` | — | `{"ready":true,"png_base64":...,"path":...,"state":{...game/state...}}` for the newest capture; **does not consume it**. The `state` is the same payload as `game/state`, sampled at poll time, so every capture arrives with its ground truth attached — never OCR the HUD. If the newest capture's pixels are identical to the last one served in full, responds `{"ready":true,"unchanged":true,"path","state"}` WITHOUT `png_base64` — don't re-request; read the state |
| `game/ui` | — | Accessibility-tree-style UI dump: every visible UI node's `rect` `[x,y,w,h]` **in the same pixel space `game/mouse move_to` consumes**, its text (button labels), `clickable: true` on real buttons, `interaction` (`Pressed`\|`Hovered`\|`Idle`), `pointer_hovered`, and the mocked pointer's position. Back-to-front render order. Read this to decide *what to click and where* — and crop screenshots to these rects — instead of estimating from pixels |

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

**Default to gamepad-style input unless the task is specifically about
keyboard+mouse.** The Steam Deck is this project's primary/min-spec target, so
a playtesting agent emulates a Deck player: drive UI with `game/gamepad`
(DPad/stick navigation + South to confirm) and gameplay with `game/input` /
`game/gamepad` — both ride the exact replicated-BEI path a real Deck controller
drives. Gamepad actions are also the most agent-friendly: they're discrete,
level-triggered presses (hold/release — no pixel coordinates, no analog
calibration, no 100ms timing races like a mouse click's press/release pair).
Reach for `game/mouse`/`game/keyboard` only when the task explicitly targets
those devices (e.g. verifying a real mouse click on a button, or a keyboard
binding) — §5b documents their extra gotchas.

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
#   negative pitch_delta → look_pitch increases → looks UP (look_pitch positive = UP;
#   positive pitch_delta = look DOWN — verified visually against a known scene)
{"action":"rotate","yaw_delta":-1.5708,"ticks":30}
```

The mock **bypasses binding modifiers** (dead zones, `Scale`): values go through
as-is, which is why rotate is radians-direct rather than mouse-pixels. Read back
effects via `game/state` — position/velocity/yaw/pitch/grounded are
server-authoritative and reliable.

## 5a. Gamepad-level input (`game/gamepad`) — for UI navigation, or true binding fidelity

`game/input` (above) mocks at the *action* level — it can't reach anything
`game/trigger`/`game/input` don't already cover, most importantly **UI
navigation** (the pause menu, main menu, any `selector` popup), since those are
driven by `MenuControls`'s own `UiNavigate`/`UiConfirm` actions, not the three
ahoy ones. `game/gamepad` mocks a real gamepad's button/axis state instead —
`bevy_input::gamepad::Gamepad` on a synthetic, lazily-spawned entity (created on
first use, not at boot) — so the value flows through `bevy_enhanced_input`'s
*real* binding resolution exactly like a human's controller: dead zones, which
context currently owns a shared physical input, `require_reset`, all of it.
This is the only method here that can drive UI at all.

```sh
# press Start (opens the in-game pause modal — bound in PlayerControls); release
# it right after, like a human would (see the level-triggered note below)
{"input":"button","button":"Start","pressed":true}
{"input":"button","button":"Start","pressed":false}

# navigate focus (Cardinal::dpad() binds DPad directions to UiNavigate octants);
# "Resume" auto-focuses when the modal opens, DPadUp moves focus to "Main Menu"
# above it
{"input":"button","button":"DPadUp","pressed":true}
{"input":"button","button":"DPadUp","pressed":false}

# confirm whatever's focused (South is UiConfirm's binding)
{"input":"button","button":"South","pressed":true}
{"input":"button","button":"South","pressed":false}

# axes: roughly -1.0..1.0, e.g. left stick forward
{"input":"axis","axis":"LeftStickY","value":1.0}

# release/zero everything in one call — cheap insurance against a forgotten
# release leaving something stuck for the rest of the session; reach for this
# between unrelated test scenarios
{"input":"reset"}
```

**The one trap that will cost you real debugging time if you don't know it
going in**: `bevy_enhanced_input`'s gamepad-button reader calls `Gamepad::get`,
which reads the **`analog`** field — *not* `digital`/`ButtonInput`, despite that
being the obviously-correct-looking choice for a boolean button (confirmed by
testing: an implementation using `digital_mut().press()` compiled fine, ran
with no error, and the modal menu simply never opened — silent, not a panic).
`game/gamepad`'s own implementation already gets this right; this note is here
so nobody "fixes" it back to `digital` on a future refactor without re-reading
this.

Gamepad state is **level-triggered, not duration-based** — unlike `game/input`'s
`ticks`, a button/axis you set stays exactly as you left it until you
explicitly change it again. This matches a real controller (you don't get to
say "press for 30 frames then auto-release," you press and later release), so
budget an explicit release call for everything you press, or use
`{"input":"reset"}` once you're done with a scenario.

Standard button names (19, matches `GamepadButton`, `Other(u8)` not exposed):
`South`, `East`, `North`, `West`, `C`, `Z`, `LeftTrigger`, `LeftTrigger2`,
`RightTrigger`, `RightTrigger2`, `Select`, `Start`, `Mode`, `LeftThumb`,
`RightThumb`, `DPadUp`, `DPadDown`, `DPadLeft`, `DPadRight`. Standard axis names
(6, matches `GamepadAxis`): `LeftStickX`, `LeftStickY`, `LeftZ`, `RightStickX`,
`RightStickY`, `RightZ`.

Known limitation: this doesn't flip `InputDeviceState` to `Gamepad` (the
control-tip icons in the modal/HUD stay keyboard-styled even while driving
input this way) — that state tracks real input *events*
(`GamepadButtonChangedEvent` etc.), and this mock writes persistent component
state directly rather than emitting events. Cosmetic only; every actual
gameplay/UI effect of the input is real.

## 5b. Keyboard + mouse (`game/keyboard`, `game/mouse`) — real UI clicks too

Device-level, same idea as `game/gamepad`: `game/keyboard` mocks
`ButtonInput<KeyCode>` directly, `game/mouse` mocks `ButtonInput<MouseButton>`
plus real cursor motion/position through `bevy_picking`'s own event pipeline —
this is the only method here that can click an actual UI button by position
(as opposed to navigating focus with a gamepad/keyboard and confirming).

```sh
# hold W (bound to Movement's forward axis) for real, through the actual
# KeyCode binding, then release
{"key":"KeyW","pressed":true}
{"key":"KeyW","pressed":false}

# release every held key
{"reset":true}
```

`key` is the exact Rust `KeyCode` variant name (`KeyW`, `Digit1`, `Escape`,
`Space`, `Enter`, `Tab`, `ArrowUp`, `ShiftLeft`, `ControlLeft`, …) — deserialized
directly via `KeyCode`'s own `serde` impl, so every one of Bevy's 160+ variants
works, not a hand-picked subset. Level-triggered like `game/gamepad`, not
duration-based — budget a release call, or use `{"reset":true}`.

`game/mouse` is discriminated by `input`:

```sh
# move the cursor to an absolute pixel position — the SAME 1280x800 (Steam Deck 800p) space
# game/screenshot captures, so you can click exactly what you see in a
# screenshot. Read the target's rect off `game/ui` (its rects are in this exact
# space) rather than estimating from pixels.
{"input":"move_to","x":161,"y":327}

# press then release Left — this is a REAL click: it updates both
# ButtonInput<MouseButton> (for mouse-bound gameplay actions) AND fires a
# bevy_picking PointerInput on the pointer bevy_picking's own
# spawn_mouse_pointer already spawns at Startup (headless or not) — so it
# actually activates whatever UI node is under the cursor, through the real
# hit-testing/Interaction/Activate pipeline, not a shortcut
{"input":"button","button":"Left","pressed":true}
{"input":"button","button":"Left","pressed":false}

# relative motion (mouse-look) — dx/dy, like a real MouseMotion delta
{"input":"motion","dx":200,"dy":0}

# scroll wheel
{"input":"wheel","x":0,"y":1,"unit":"Line"}

# release all buttons + zero motion/scroll accumulators (does not recenter
# the cursor)
{"input":"reset"}
```

**Verified live, not just by inspection**: clicking "Connect" on the main menu
at its actual screenshot pixel coordinates transitioned the client into
`Lobby` through the real `bevy_ui`/`bevy_picking` pipeline (confirmed via
`game/state`), and clicking "Options" opened/closed its real `selector` popup
(confirmed via the `client::ui::selector` log lines it emits). This is the
first method in this API that reaches UI by *position* rather than by
navigating focus and confirming.

**The gotcha that cost real debugging time, same shape as `game/gamepad`'s
analog/digital one**: `AccumulatedMouseMotion`/`AccumulatedMouseScroll` cannot
be set with a direct `world.insert_resource(...)` — Bevy's own
`accumulate_mouse_motion_system`/`accumulate_mouse_scroll_system` unconditionally
overwrite them from `MouseMotion`/`MouseWheel` **events** every single frame
(their own doc comments say "reset to zero every frame"), so a direct write is
silently wiped before `bevy_enhanced_input`'s reader ever observes it —
confirmed live: `look_yaw` in `game/state` stayed exactly `0.0` after a
`dx:200` motion call with the resource-write approach, no error anywhere.
Fixed by writing real `MouseMotion`/`MouseWheel` events instead (`world.
write_message(...)`), letting those systems compute the accumulated value on
their own schedule, same as a real winit event would. `game/mouse`'s own
implementation already does this correctly; don't "fix" it back to a direct
resource write on a future refactor.

**A hard ceiling, not a bug — literal keyboard Enter can never activate a
`FeathersButton` (main menu, lobby, any `selector.rs` popup/row) through this
harness.** `bevy_input_focus::dispatch_focused_input` (the system that turns a
raw `KeyboardInput` event into the `FocusedInput<KeyboardInput>` a focused
widget's native key handler reacts to) requires a `PrimaryWindow` entity to
exist — confirmed via `world.query` for `bevy_window::window::PrimaryWindow`
returning `[]` in `--mcp` mode — and silently no-ops its entire body otherwise,
no error. `--mcp` mode never creates one (`WindowPlugin { primary_window:
None }`), so this is unfixable from `game/keyboard`'s side; not believed to
affect a real windowed client. **Use `game/mouse` or `game/gamepad`'s South
button to test any `FeathersButton`'s click/confirm path instead** — both
verified to work fine headlessly (mouse via `bevy_picking`'s own pipeline,
gamepad via `ui.rs`'s `on_ui_confirm` direct-trigger bridge). Literal Enter
*does* still work headlessly on the *old* hand-rolled `widgets::button()`
surfaces (the HUD, the pause modal) — those go through a different,
window-independent activation path (`on_ui_confirm_enter`'s `LegacyActivate`
trigger) — so don't conflate "Enter doesn't work" there with this limitation;
if Enter fails on a `widgets::button()`-based surface, that's a real bug, not
this ceiling.

## 5c. Real desktop-window testing (no `--mcp`) — xdotool/ydotool/wtype quirks

Everything above (§5-§5b) is the `--mcp` headless harness: agent-injected input
mocked at the ECS level, no real window, no real OS input device. Sometimes you
need the *other* thing — a real windowed client (`target/debug/client`, no
`--mcp` flag, still needs `--features dev-tools` built in) driven by genuine
synthetic OS input (a real uinput/Wayland device, indistinguishable from actual
hardware to the app) — e.g. to close the loop on something `--mcp` can't test
(the `dispatch_focused_input`/`PrimaryWindow` ceiling above is the standing
example: literal Enter on a `FeathersButton` needs a real `PrimaryWindow` to
work, so the only way to *prove* it works is a real window). BRP/MCP still
work identically in this mode — `game/state`, `game/screenshot`
(falls back to `Screenshot::primary_window()` when `OffscreenRenderTarget`
doesn't exist), `game/keyboard`/`game/gamepad`, `world.query`/
`world.get_resources`/`world.trigger_event` — only `game/mouse`'s `move_to`/
click are `--mcp`-only right now (see below).

This was tried for the first time on a real Sway (wlroots) session with two
monitors; take the specifics with a grain of salt on a different compositor,
but the *shape* of each gotcha (especially the acceleration one) is likely to
recur anywhere.

**Tool availability, this compositor**: `xdotool` **does not work at all** —
it's X11/XWayland-only, and this game's window is a native Wayland surface
(`xdotool search --name ...` finds nothing). `wtype` (the Wayland
virtual-keyboard-protocol tool) installed and ran with no error, but its key
events **never reached the app** — no visible effect, not even the dev
console toggle (backtick) — despite the window holding real compositor
keyboard focus (confirmed via `swaymsg -t get_tree`'s `focused: true`). Didn't
root-cause this (Sway config restricting the virtual-keyboard protocol to
specific clients is one guess), just confirmed `ydotool` works in its place —
don't burn time on `wtype` first if it's not already known-working on the
target compositor.

**`ydotool` is the one that reliably works**, but needs setup, all one-time
per session:
```sh
# ydotoold needs /dev/uinput access — check for an existing ACL first
getfacl /dev/uinput   # if it grants your user rw, no sudo needed at all
ydotoold --socket-path=/tmp/.ydotool_socket --socket-own=$(id -u):$(id -g) &
disown
export YDOTOOL_SOCKET=/tmp/.ydotool_socket   # needed by every ydotool call after this
```
`ydotool key <code>:1 <code>:0` (press+release) and `ydotool key <code>:1`/
`<code>:0` (hold/release separately) take raw Linux keycodes from
`/usr/include/linux/input-event-codes.h` (`KEY_W`=17, `KEY_ENTER`=28,
`KEY_ESC`=1, `KEY_UP`=103, `KEY_DOWN`=108, `KEY_GRAVE`=41, …) — not X11
keysyms, not the `KeyCode` names `game/keyboard` uses. `ydotool click <mask>`
buttons are **bit-flag hex**, not plain enum values — read the mask, don't
guess: `0x00` alone means "left button, do nothing" (down bit *and* up bit
both unset — a real, easy-to-make mistake, confirmed by testing: it compiles/
runs/exits 0 and produces literally no click at all); `0xC0` is a real
down-then-up left click; `0x40`/`0x80` are down-only/up-only (for a deliberate
held-drag). Mouse movement (`ydotool mousemove`) is **relative-only on this
compositor** — checking `cat /sys/class/input/eventNN/device/uevent` for the
`ydotoold virtual device` node showed `EV=7` (SYN|KEY|REL, no ABS bit at all),
meaning `--absolute` isn't backed by real absolute positioning hardware and is
at best a software approximation on top of relative deltas — don't trust it
for pixel-accurate targeting.

**The real gotcha, the one that cost the most time**: `ydotool mousemove`'s
relative deltas get warped by **libinput pointer acceleration** by default
("adaptive" profile) — a single large synthetic jump (e.g. "move by 900px to
reach a button") does not land 900px away, because acceleration curves are
tuned for continuous human motion, not one instantaneous synthetic delta.
Confirmed by testing: an absolute-feeling two-step move (pin to a corner with
a huge relative jump, then move by the exact target offset) landed wildly off
target with acceleration on, then landed pixel-exact once acceleration was
disabled. Fix, **per input device**, no restart needed:
```sh
swaymsg input "9011:26214:ydotoold_virtual_device" accel_profile flat
swaymsg input "9011:26214:ydotoold_virtual_device" pointer_accel 0
```
(get the exact device identifier from `swaymsg -t get_seats`, under
`ydotoold virtual device` — the vendor:product pair shown above is what this
session's `ydotoold` happened to register as, not guaranteed stable). With
acceleration flat, the pin-then-move-by-exact-delta pattern is reliable:
```sh
ydotool mousemove -x -5000 -y -5000   # slams into the top-left corner (0,0), any compositor clamps this
ydotool mousemove -x <target_x> -y <target_y>   # now a true relative delta from a known origin
```
**Re-pin before every click**, don't reuse a previously-computed origin — the
compositor's cursor-position bookkeeping does not appear to survive every
`grab_mode` transition (cursor lock/unlock, e.g. entering/leaving the game
world) cleanly; a move that worked right after pinning silently no-op'd once
the cursor had been locked (in-game, camera-look mode) and unlocked again
(pause menu) in between, even with acceleration still flat. When in doubt,
`grim -o <output>` (see below) and re-derive the cursor's actual last-known
position from the image rather than trusting your last computed target.

**Confirm target pixel coordinates from a *real* screenshot of the *actual*
resolution**, not a guess scaled from a downsampled preview — a rendered chat
image's stated "displayed at WxH, multiply by N" note is for *your* viewing
math only; once you load the actual PNG file (`PIL.Image.open(...)`), its
`.size` **is already the real resolution** — multiplying by the display
scale factor *again* on top of that is a real, easy mistake (confirmed by
testing: sampled the wrong pixels searching for a button, found nothing,
before realizing the file was already full-res). `grim -o <output-name>`
(`swaymsg -t get_outputs` for the name, e.g. `DP-2`) grabs the *whole
desktop*, not just the game window — use it over `game/screenshot` whenever
you need to see the **real OS cursor** (a real screenshot shows the actual
system cursor arrow; `game/screenshot`'s in-app capture does not, since the
cursor is compositor-side, not part of the rendered frame) or confirm a
window's actual on-screen position/size (`swaymsg -t get_tree`, cross-checked
against `swaymsg -t get_outputs` for the output's own origin offset if there's
more than one monitor — a window's own `rect`/`geometry` is relative to its
output, not the global compositor space, unless that output happens to sit at
`(0,0)`).

**Real-window-only quirks confirmed distinct from `--mcp`'s**:
- On a cold-started windowed client, `InputFocus` (`world.get_resources` on
  `bevy_input_focus::InputFocus`) started at `None` even though `AutoFocus` is
  present on the main menu's "Connect" button (`world.query` for
  `bevy_input_focus::autofocus::AutoFocus` found it) — unlike `--mcp` mode,
  where `AutoFocus` reliably grants focus immediately. `game/keyboard`
  arrow-key navigation and `game/gamepad` D-pad navigation both had nothing to
  move *from* until a real click happened once; after that, focus tracking
  behaved normally. Not root-caused (a window-focus-timing race between the
  compositor actually granting the new window focus and the UI scene spawning
  is the leading guess); if a mock-input script targets a windowed client
  immediately after launch and nothing seems to respond, try one real click
  first, or don't assume `--mcp`'s "focus already works" baseline transfers.
- `game/mouse`'s `move_to`/`button` (the click-injection path) require the
  `OffscreenRenderTarget` resource that only exists in `--mcp` mode — in a
  real window it errors cleanly (`move_to`) or silently skips the
  `PointerInput` firing (`button`, still sets the raw `ButtonInput<MouseButton>`
  resource but never generates a real click). Not fixed — `game/keyboard`/
  `game/gamepad` have no such dependency and work identically in both modes;
  for real UI clicks on a real window, use `ydotool`/real hardware, not
  `game/mouse`.

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

**Screenshots are a last resort, not your eyes.** Understand the world through
the structured data surfaces first: `game/state`, `game/ui` (labeled rects +
text for everything on screen), `game/levels`, and BRP's `world.query`/
`world.get_components`. They are cheaper, exact, and stable in a way pixels
never are (a vision model reading an 800p frame is the least reliable
instrument in this toolbox). Take a screenshot **only** when:
- the task explicitly asks for one, or
- the thing under test is *inherently* visual — rendering, lighting, materials,
  particles, camera framing, UI compositing/layout. (Even then, `game/ui` is
  the right tool for UI *content*; pixels only answer "did it render right".)

For gameplay, UI, and logic testing, plain MCP/BRP data should be sufficient —
and if it isn't, **that's an API gap to report and fix** (a field missing from
`game/state`, a query the dump doesn't expose), not a reason to fall back to
reading pixels. File the gap in your playtest report (§10) instead of
squelching it with screenshots.

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
both the "nothing renders at all" bug and the follow-on UI-render-order bug are fixed):

- Menu/lobby: UI and the real `.glb` background both render correctly, UI properly
  composited on top (was: UI drawn underneath the background — a `bevy_ui`
  `IsDefaultUiCamera`-ambiguity bug, not an ordering problem; see AGENTS.md). The
  in-game HUD (including the crosshair) also renders now, for the same reason — it
  never did before this fix either. Top of frames still cut off (2× `UI_SCALE` vs
  720p, unrelated, cosmetic).
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

**Fleet mode — many headless clients on one machine** (`--brp-port` /
`--mcp-port`): every extra `--mcp` client past the first needs its own ports
(the defaults are per-host singletons — a second client on the defaults fails
with `AddrInUse`):

```sh
# client N: BRP on 1600N, MCP on 1700N
./target/debug/client --mcp --brp-port 1600$N --mcp-port 1700$N
# address client N's BRP at http://127.0.0.1:1600$N (JSON-RPC POST, same methods)
```

A non-default BRP port also isolates captures into a per-client
`docs/playtests/dist/screenshots/client-<port>/` dir (a shared dir would make
one client's `game/screenshot/get` return another's capture). Routing needs no
proxy — BRP is stateless JSON-RPC POST, so "talk to client N" is just
addressing its port. Capacity: each client is a software-Vulkan (lavapipe) Bevy
instance; measured ~10 clients ≈ 13 cores + ~1 GB each — budget accordingly,
and note server-side player count + the zombie-player disconnect gap (§8).

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

**Isolate the run's assets too (since playtest 0009)**: ship what the run needs under
`playtest_assets/playtest_NNNN/{server,client}/assets/` as plaintext — hand-written JSON
`.gltf` whose content is Skein components (`ClientReplicate`, `ClientWorldAsset`,
`ColliderConstructor`, `MeshPrimitive` for zero-baked-data visuals), a per-run
`collections/common_assets.assets.ron` remap, `.level.ron`, `config.toml`, one en-US locale.
Launch with `BEVY_ASSET_ROOT` aimed at those directories. **Fully plaintext since playtest
0010**: `CommonAssets`'s furniture fields (fonts, WAVs, the skybox, the atlas PNGs) are
`#[asset(key = "…", optional)]` `Option<Handle<T>>` now, so the manifest can list only the
five world keys and the run needs *no* copied engine furniture at all — every consumer
degrades gracefully (Bevy's embedded default font, no skybox pass, no sample playback, empty
icon-atlas fallbacks). There's also a pre-sync `--no-common-assets` CLI flag for the even
barest boot: no loading state at all (the manifest is never read), a
`CommonAssets::placeholder()` resource, and an immediate `AssetLoading → MainMenu`
transition — in-game visuals still arrive via the `ClientWorldAsset` path, which loads by
path, not through the manifest. The recommended playtest mode is still the manifest-driven
one (trimmed manifest, no flag) since it keeps the `MeshPrimitive` world visuals; see
`playtest_assets/playtest_0009/` (stripped to 100% plaintext by 0010) as the template and
its report, plus `docs/playtests/playtest_0010/`, for the mechanism and the gotchas.

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
**Tool-API gaps belong here too**: if `game/state`/`game/ui`/BRP didn't expose
something you needed to understand the world and you were tempted to read it off a
screenshot instead (§7), write down exactly what was missing — those gaps get fixed in
`dev::tool_api`, and every one reported makes the data-first workflow (§7) cover more.
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

**Also update `docs/playtests/index.typ`** — add a new entry (newest first) with the date,
agent, report path, and a short explanation of what the playtest covered and found, in the
same style as the existing entries. This is part of filing a report, not an optional later
chore — the index only stays useful for navigation if every playtest actually lands in it.
Recompile it too (`typst compile --root docs/playtests docs/playtests/index.typ
docs/playtests/dist/index.pdf`) and check the exit code.

**Both the report's own metadata table and its index entry need a `Commit` field** — the git
`HEAD` the run was actually performed against, so the chronology and the actual code under
test stay unambiguous later. Don't guess from memory or approximate dates: cross-reference the
run's own screenshot timestamps (millisecond epoch in the filename — `date -d @<ms/1000>` or
equivalent) against `git log --format="%h %ci %s"`, then confirm the candidate commit's changed
files actually match what the session touched (`git show --stat <hash>`) before trusting a
timestamp match alone. If code changed *during* the run (a diagnose-and-fix playtest, not a
pure state tour), a single hash can be actively misleading — record both the starting commit
and the one any fix landed as, and say so explicitly, rather than picking one and implying it
covers the whole run.

## 11. Practical flow summary

1. Teardown (`pkill -x` both), verify ports free.
2. Build both binaries; confirm zero errors.
3. Start server (env + `sleep 4`), start client (env, `timeout 300+`, `sleep 8`),
   confirm `mcp tool server listening` in the log.
4. Menu: `game/ui` + `game/state` (screenshot only if the task is visual — §7).
5. `connect` → poll Lobby → `game/levels`.
6. `select_level` (fresh server only) → `play` → poll for `position`.
7. Drive with `game/input`/`game/gamepad`; sample `game/state` after each action.
   Screenshots only on explicit request or for inherently visual checks (§7);
   report any data-surface gap you hit (§10).
8. Capture logs from both processes; teardown when done (or leave the pair for
   the user, saying which processes are yours).
9. Write the playtest report (see §10 for the three-location layout); compile with `--root docs/playtests`; never inspect the PDF.
