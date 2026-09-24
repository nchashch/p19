# 12. A `--no-render` agent-client mode: the headless host without wgpu/Vulkan

Date: 2026-09-24

## Status

Accepted — implemented (`--no-render` CLI flag; `config::is_no_render_presync`, the
`mcp_headless` sub-branch in `client/src/main.rs`, `NoRenderMode` +
`shim_camera_computed` in `controls/camera.rs`, guards in `dev::tool_api` and
`lifecycle::loading`).

## Context

[0011](./0011-agent-vision-and-fleet-improvements.md) made fleet testing possible
(`--brp-port`/`--mcp-port`, per-client capture isolation) but each `--mcp` client still ran the
full render stack: a wgpu instance on lavapipe (software Vulkan) drawing 1280×800 frames at
60 Hz that no one looks at. Measured cost on the dev machine: **~1.3 cores and ~1.1 GB RSS per
client** — the CPU bill is almost entirely rendering, and the driver requirement (a Vulkan
loader + mesa/lavapipe installed) is exactly what a "cheap GPU-less VPS" lacks. Since
[0011] also established data-first observation — `game/state`, `game/ui`, and BRP queries as
the understanding channel, pixels only for inherently visual checks — most fleet clients don't
need pixels at all. Rendering them anyway paid the most expensive line item for the least-used
feature.

## Decision

Add a `--no-render` CLI flag (implies `--mcp` + `--no-common-assets`; wins over VR like `--mcp`
does) that composes the headless branch with the render plugins disabled:

- `DefaultPlugins.build().disable::<…>()` for `RenderPlugin`, `ImagePlugin`,
  `CorePipelinePlugin`, `AntiAliasPlugin`, `SpriteRenderPlugin`, `UiRenderPlugin`,
  `GizmoRenderPlugin`, `PbrPlugin`, and `PostProcessPlugin` (the last one loads a `Shader`
  handle in `build()` and panics on the uninitialized asset otherwise). No wgpu instance is
  ever created; no Vulkan loader is needed.
- `Assets<Image>` / `Assets<Shader>` / `Assets<StandardMaterial>` / `Assets<Mesh>` are
  `init_asset`-ed by hand — normally render-side plugins register them, but logic-side systems
  (feathers' shader handles, the NPC-quad setup) insert handles unconditionally. They become
  inert stores.
- **The one real shim**: `shim_camera_computed` (an `Update` system) writes each camera's
  `Camera.computed.target_info` (`RenderTargetInfo { 1280×800, scale_factor 1.0 }`) by hand,
  because the system that normally computes it lives in `bevy_render`. This is the only thing
  `bevy_ui`'s camera propagation/layout and `bevy_picking`'s UI backend read back from the
  render side — with it, UI layout, the `game/ui` dump, hover, and clicks are pixel-for-pixel
  identical to rendered headless mode (same coordinate space). Notably, `bevy_ui` 0.19 contains
  **no render module at all** (UI rendering is the separate `UiRenderPlugin`) — layout was
  render-free all along.
- Feature plugins that hard-require `RenderApp` are skipped under the flag (checked presync):
  `bevy_mod_outline`'s `OutlinePlugin` (unwraps `RenderApp` in `build`; outlines are selection
  *visuals*, the targeting logic/raycast/`Hovered`/`Selected` are independent) and
  `bevy_hanabi`'s `HanabiPlugin` + particle authoring (its `finish` requires `RenderApp`).
  `bevy_dev_tools`' FPS overlay is likewise skipped (its `setup` needs render-side
  `Assets<ShaderBuffer>`).
- Consumers adapt: `game/screenshot` returns a clean error ("no rendering enabled
  (`--no-render`): … use game/ui for on-screen content and game/state for ground truth") via a
  `NoRenderMode` marker resource instead of polling a capture that can never complete;
  `lifecycle::loading`'s client-world-visual system is gated off (its `.glb` loads need the
  disabled image/mesh machinery, and there is nothing to show them on).

Everything else is untouched and verified: `game/state`, the full `game/ui` dump with correct
rects, hover, clicks, `game/gamepad`/`game/mouse`/`game/input`, netcode, prediction, and the
fleet port flags.

## Alternatives considered

- **Compile-time exclusion** (split the client's `bevy` features so `bevy_render` isn't linked).
  Slimmer binaries and a compile-enforced guarantee, but heavy `#[cfg]` surgery across
  `main.rs`/`tool_api.rs`/`camera.rs` for a v1; runtime plugin exclusion achieves the actual
  goal (no wgpu at runtime) with one branch. Revisit only if binary size or a hard guarantee
  matters.
- **Throttle rendering instead of removing it** (e.g. `run_loop(1/10)` or a frame-skip). Cuts
  CPU ~6× but keeps the Vulkan driver requirement — which, not the CPU alone, is what excludes
  cheap VPSes — and still burns memory on textures/swapchain-like resources. The data-first
  doctrine ([0011]) makes the rendered frames waste for most fleet members anyway.
- **Fix `bevy_mod_outline` to guard `RenderApp` instead of skipping the plugin.** More general
  (the vendored crate already carries local patches worth upstreaming), but not required for
  this mode to work; noted as a follow-up alongside the existing `pipeline_key.rs` patch.

## Consequences

- **Measured (9800X3D, dev build):** one `--no-render` client idles at **~0.7 core / ~0.3 GB**
  vs **~1.3 cores / ~1.1 GB** rendered — roughly 2× client density from CPU alone, ~4× from
  memory, and the host needs no Vulkan driver at all. The "fleet of dozens on one cheap box"
  scenario from [0011] is now realistic; a 10-client fleet costs ~7 cores + ~3 GB instead of
  ~13 cores + ~11 GB.
- The agent workflow is unchanged except screenshots: the playbook's data-first default
  ([0011]) is *enforced by the environment* in this mode — if an agent reaches for pixels, it
  gets a precise error pointing at `game/ui`/`game/state` instead of a timeout.
- Verified end-to-end render-less: boot → menu dump → hover → click Connect → Lobby; 3-client
  no-render fleet all in Lobby against one server; rendered headless mode re-verified
  unregressed (boot, dump, full-frame capture).
- Still open: long fleet churn remains limited by the server-side zombie-player gap (AGENTS.md
  top-level list) — a render-less client makes churn *cheaper*, which makes fixing that gap
  more urgent, not less.
