# Agent Playtest 0026 — bevy_markup Skips Identical Renders: Client Value Diffing Removed

| Field | Value |
|---|---|
| Date | 2026-10-05 16:22 – 16:30 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0025, and this change: `ui/hud.rs` drops `DataFrame.values` and builds its context with `tera::Context::from_serialize`, `ui/nameplate.rs` drops `Nameplate.name`, `ui/tui_panel.rs` drops `MainMenuTuiPanel.seconds`); bevy_markup path dependency at `f81acce` + uncommitted `render_templates` identical-render skip |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup's `render_templates` used to rebuild a UI whenever its `TemplateContext` was
touched, even if the rendered HTML was identical. The client therefore kept its own copy of the
last written values and diffed before writing (data frame, nameplate name, TUI seconds). The
library now compares the new render with the previous `HtmlDocument::source` and leaves
`RenderedHtml` untouched when they match. This run checks that the client, now writing its
contexts unconditionally, rebuilds only on real changes.

## Verification

Rebuilds measured as `HtmlElement` entity-set churn between `world.query` snapshots.

| Step | Observed |
|---|---|
| Main menu, 0.5 s and 2.5 s windows | 13/20 elements kept; only the six `tui-*` elements replaced (the readout's second ticked) |
| TUI panel root sampled every 0.1 s for 4 s | 4 rebuilds (once per displayed second; the context is written every frame) |
| TUI readout | `t = 18s` → `t = 19s` |
| In game, data frame shown, idle 1.0 s | 19/19 kept (later run: 23/23); the context is written every frame |
| Moving 60 ticks, nothing hovered | 19/19 kept (displayed values unchanged) |
| Jump | `Grounded: false` seen mid-air, back to `true`; data frame elements replaced (17/27 kept) |
| Pause → Main Menu → Connect → Play | menus and focus as in playtest 0025 |

![data_frame_stable.png](screenshots/playtest_0026/data_frame_stable.png)

*The data frame while idle: written every frame, rebuilt only when a line changes.*

## Findings

**F1 — Identical renders no longer rebuild.** Before this change the TUI panel rebuilt its
six elements once per second because the client diffed `seconds` itself; the data frame
relied on its own `Value` comparison. Both now write every frame and the rebuild rate is
unchanged (TUI: 1/s; data frame: only on a displayed change).

**F2 — Observation (unconfirmed, not filed): spawned cubes end up ~1000 units from the
player.** Two cubes spawned with `game/trigger spawn_cube` were at z ≈ −960 and −1031 while the
player stood at z ≈ 26–30, so `deselect_when_out_of_range` (50 m) immediately clears a
`game/select` of them and the data frame's "Selected" block never shows. Whether this is the
minimal level's `CubeSpawner` placement or the cube's launch along `aim_direction` (see
"KCC has no ground friction" in AGENTS.md for the analogous coasting) was not investigated;
unrelated to the UI change.
