# 11. Vision and fleet improvements for the agent tool API: data-first observation, UI-tree dumps, visible cursor, state-fused/cropped/unchanged-suppressed captures, Steam-Deck 800p viewport, per-client ports

Date: 2026-09-24

## Status

Accepted — implemented (`game/ui` BRP method + MCP tool, the agent-cursor overlay, state-fused
`game/screenshot/get`, `crop`/`unchanged` capture semantics, the 1280×800 offscreen target,
`--brp-port`/`--mcp-port`, and per-client screenshot dirs — all behind the existing `dev-tools`
feature from [0009](./0009-agent-tool-api-via-brp.md), plus `docs/agents/skills/playtest.md` policy
updates).

## Context

[0009](./0009-agent-tool-api-via-brp.md)/[0010](./0010-device-level-input-mocking-for-agent-tool-api.md)
gave agents complete input reach. Observation was the weak half of the loop, in four specific
ways — each one measured or hit in live playtesting, not hypothetical:

1. **Coordinates were guessed, not read.** The weakest link was screenshot → model estimates
   pixel coordinates → `game/mouse move_to`. A vision model reading an 800p frame is the least
   reliable instrument in the toolbox: text at UI scale is thin-stroke and easily misread, and
   every click began as a coordinate guess.
2. **The mocked pointer was invisible.** `game/mouse` moves `PointerId::Mouse`, but nothing drew
   it — the agent could not see where its own cursor was, nor hover state, making hover/click
   verification blind.
3. **Captures carried no ground truth.** `game/screenshot/get` returned only pixels + path; the
   agent had to OCR the HUD (HP, GCD bar, position) or correlate "which `game/state` call came
   after which screenshot". Ground truth was available one tool call away but never attached.
4. **Fleet testing was structurally impossible.** Both surfaces were per-host singletons
   (BRP 15702 via hardcoded plugin defaults, MCP 15710 as a `const`), and — worse and
   non-obvious — `game/screenshot/get` returns the *newest PNG in one shared directory*, so two
   clients would cross-contaminate each other's captures. The playbook's own "stray client
   holds :15710" failure mode was this limitation showing.

Separately, the vision-model economics made raw resolution a dead end: image token cost is
dimension-driven, not byte-driven — re-encoding a screenshot as JPEG saves zero tokens and
damages exactly the thin-stroke text the model struggles with. The lever is *fewer pixels for
the same information*, not *smaller bytes*.

## Decision

Four changes, all shaped by one principle: **pixels are the observation channel of last
resort** — structured data for understanding the world, pixels only for what is inherently
visual.

### 1. `game/ui` — an accessibility-tree-style UI dump

BRP method (+ MCP `ui_tree` tool) walking `UiStack` in back-to-front render order; per visible
node: `rect [x, y, w, h]` computed from `UiGlobalTransform`'s translation ± `ComputedNode.size`
— both in **physical render-target pixels, the exact space `game/mouse move_to` and
`bevy_picking`'s UI backend consume** (verified end-to-end by clicks landing), plus aggregated
subtree text (button labels), `clickable`/`interaction`/`pointer_hovered` state, the hovered
entity set from `HoverMap`, and the pointer position. Noise control: zero-size/invisible/
rotated nodes skipped; clickable rows aggregate descendant text while plain containers emit
own-text only (otherwise the root panel row repeats every label on screen); non-clickable rows
under a clickable ancestor are folded away; the cursor overlay excludes itself.

Interactive detection uses the markers this project's UI **actually** carries:
`bevy_ui_widgets::Button`/`FeathersButton` — **not** `bevy_ui::Interaction`, which feathers
buttons do not carry (confirmed via `world.list_components` on a live button). The hand-rolled
`widgets::button()` has no marker at all (picking observers only); its rows still appear with
labels, just without the flag.

### 2. A visible agent cursor

Headless-only crosshair overlay (two outlined UI bars, `GlobalZIndex(i32::MAX)`), positioned
each frame onto the mocked pointer via the node's own `ComputedNode.inverse_scale_factor` (the
factor `ui_layout_system` derived — target scale × `UiScale` — so the overlay lands exactly on
the pointer whatever the scale), recolored by state: red idle / yellow hovering (`HoverMap`
non-empty for the pointer) / white left-held. `Pickable::IGNORE` on every node so it can never
occlude the UI it exists to hover; excluded from the `game/ui` dump.

### 3. Captures that carry truth and cost less

- **State fusion**: `game_state` refactored into `game_state_snapshot()`;
  `game/screenshot/get` embeds it in every response and writes it once to a `<capture>.json`
  sidecar beside the PNG, so both the agent's response and the human-browsable record carry
  ground truth. Snapshot is taken at poll time — the state the agent wants (world right after
  its action), bounded by the poll granularity.
- **Crop**: `game/screenshot {"crop":[x,y,w,h]}` — bevy's `save_to_disk` can't crop, so a
  custom `save_cropped_to_disk` observer over `ScreenshotCaptured` (which carries the full
  `Image`) does `to_rgb8` → `imageops::crop_imm` → PNG encode, clamped to bounds. Crop coords
  are the same pixel space `game/ui` dumps. Measured: a button crop is 1.8 KB vs 962 KB full
  frame, and ~190× fewer vision tokens — plus the region renders unscaled instead of inside a
  provider-downscaled full frame (the actual legibility win; providers downscale past ~1.5MP
  long-edge, so most of a 720p/800p frame never reaches the model at full resolution).
- **Unchanged-frame suppression**: `game/screenshot/get` answers
  `{"ready":true,"unchanged":true,...}` *without* `png_base64` when the newest capture's PNG
  bytes hash identically to the last full serving (same encoder + same pixels → same bytes, so
  file-hash = frame-hash); fresh `state` still included, since the world can change under a
  static view. Agents polling through state transitions routinely re-read identical frames.
  PNG stays the format deliberately: lossless, and UI text is the worst case for lossy codecs.
- **Steam Deck viewport**: the offscreen target is 1280×800 — the primary target platform's
  native 800p, 16:10 included — so agent captures see exactly what a Deck player would.

### 4. Fleet mode: per-client ports + isolated captures

- `--brp-port N` / `--mcp-port N` (CLI, pre-sync parsed like `--mcp`). `RemoteHttpPlugin`
  already had `with_port`; MCP just needed the const parameterized. The MCP→BRP proxy URL
  follows the same flag.
- Enabler: `main.rs` constructs `SkeinPlugin { handle_brp: false }`. Skein's default BRP add is
  hardwired to port 15702, which would have made the flag release-only; Skein's source shows its
  presets endpoint registers in `finish` whenever `RemoteMethods` exists — which
  DevToolsPlugin (added after Skein) provides. So DevToolsPlugin always owns BRP, dev and
  release alike, nothing lost.
- `screenshots_dir()` puts non-default-port clients in per-client
  `screenshots/client-<port>/` dirs. Routing itself needs no proxy: BRP is stateless JSON-RPC
  POST, so "talk to client N" is just addressing its port.

### 5. Policy, not just plumbing

`docs/agents/skills/playtest.md` now states the doctrine explicitly: gamepad-default input (emulating
a Deck player; discrete level-triggered actions are also the most agent-friendly), and
**data-first observation** — `game/state`/`game/ui`/BRP queries for understanding the world,
screenshots only on explicit request or for inherently visual subjects (rendering, lighting,
compositing). Critically: when the data surface is missing something, the playbook directs
agents to **report the gap in the playtest report so it gets fixed**, rather than falling back
to pixels as a crutch. The observation loop improves itself instead of accreting workarounds.

## Alternatives considered

- **Higher-resolution screenshots for legibility.** Providers downscale past ~1.5MP long-edge,
  so beyond 1080p raw resolution buys nothing without cropping; it costs lavapipe CPU per frame.
  Superseded by crop + dump: a `game/ui` rect crop is both cheaper in tokens *and* rendered at
  full effective resolution.
- **JPEG/WebP capture encoding.** Saves bytes (irrelevant — token cost is dimension-driven,
  transport is loopback), damages thin-stroke text (the actual content agents need to read).
  PNG kept; JPEG remains a possible opt-in only if captures ever cross a real network.
- **A separate UI-only capture layer** (second offscreen texture with just the UI camera). All
  cameras share one texture by design (`retarget_cameras_to_offscreen`'s ordering invariants);
  a second target re-opens that entire class of composited-ordering bugs — the project has the
  scar tissue from three reverted attempts (see AGENTS.md) — for a benefit the `game/ui` dump
  provides with zero render-graph risk. Rejected.
- **A reverse proxy in front of N clients** (path-prefix routing to per-client ports).
  Unnecessary: BRP is stateless HTTP POST, so a port table *is* the router. A ~40-line axum
  proxy remains possible later, but nothing needs it. Podman containers similarly add resource
  limits/crash containment, not port solving — a hardening layer for later, not the mechanism.
- **Keep 720p.** 16:9 is not a platform this project targets; 800p matches the Steam Deck
  exactly, and menu layout/camera framing differ between the aspects — agents now test the
  layout Deck players actually see.

## Consequences

- The observation loop is now: `game/ui` for *what and where* (with `clickable` flags and
  exact rects), `game/state` for *ground truth*, screenshots only for "did it render right" —
  and a click pipeline (dump rect → `move_to` → click) verified end-to-end, twice, including
  across a viewport change that re-centered the menu (where remembered pixel positions would
  have missed).
- The cursor makes hover/press state observable in captures (red/yellow/white), closing the
  "agent can't see its own pointer" blind spot; picking remains unaffected (`Pickable::IGNORE`).
- Every capture is self-describing (embedded `state`, on-disk sidecar), and repeated polls of
  static views cost ~0. Measured: button crop 1.8 KB vs 962 KB full frame; identical-frame
  re-polls carry no image at all.
- Fleet mode works: verified 1 server + 10 clients (`--brp-port 1600N`/`--mcp-port 1700N`),
  all ten in Lobby, per-client screenshot isolation confirmed, no cross-talk. Measured capacity
  on the dev machine: ~13 cores / ~11 GB for 10 software-Vulkan clients — documented in the
  playbook so fleet sizes get budgeted, not discovered.
- Known limits, recorded rather than hidden: provider downscale still bounds full-frame
  legibility (crop is the answer); server-side zombie-player accumulation (documented in
  AGENTS.md) makes long connect/disconnect fleet churn risky until that gap is fixed;
  `screenshots_dir` derives isolation from the *BRP* port, so two clients would need distinct
  BRP ports to share a host even if MCP ports were the only concern (they always are set
  together in practice).
