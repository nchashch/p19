# Agent Playtest 0024 — bevy_markup CSS Positioning/Borders/Pointer-Events: Client Workarounds Removed

| Field | Value |
|---|---|
| Date | 2026-10-05 14:42 – 14:43 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted: playtest 0023's icon change, and this change's `ui/hud.rs`, `ui/nameplate.rs`, `ui/tui_panel.rs`, `ui/html/theme.css`, AGENTS.md; bevy_markup path dependency at `f99c6ec` + uncommitted CSS support (`position`/insets, `z-index`, `border-radius`, `border-color`, `pointer-events`) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup gained `position`/insets, `z-index`, `border-radius`, `border-color` and
(inherited) `pointer-events`. prototype_19 replaced its code-side stand-ins with CSS: the
`Pickable::IGNORE` descendant loops (crosshair, nameplates, TUI panel; the hotbar GCD overlay's
insert), the crosshair dot's radius re-applied on every build/restyle, and the
"background + 2px padding" fake frames (hotbar slot, TUI panel). This run checks the result.

## Verification

| Check | Observed |
|---|---|
| Main menu buttons | `Connect, Options, Credits, Quit, Language` |
| TUI panel: root element `Pickable` / `BorderColor` | `is_hoverable: false`; top border `#5fd7ff` |
| TUI descendants (`tui-screen`, `tui-text`, gauge, fill) | all unpickable (inherited `pointer-events: none`) |
| Hover Connect | tooltip text row present; tooltip element unpickable |
| Mouse click Connect | `Lobby` (menu still pickable) |
| In game: crosshair dot | `Node.border_radius.top_left = Px(4)`, unpickable; renders round (crop below) |
| Pause menu | `Main Menu, Resume`; 61/61 elements stable over 0.5 s; South resumes |

![menu.png](screenshots/playtest_0024/menu.png)

*Main menu with the hover tooltip; the TUI panel's frame is now a CSS `border-color` border.*

![crosshair_dot_x8.png](screenshots/playtest_0024/crosshair_dot_x8.png)

*The crosshair dot, 40×40 crop scaled 8×: round from CSS `border-radius: 4px`.*

## Findings

**F1 — Workarounds removed, behavior unchanged.** All checks above pass; no rebuild loops.

**F2 — Root-level properties stay in code by design.** bevy_markup applies none of the new
properties to the `html` rule (the `HtmlUi` node is the app's), and CSS `z-index` is
sibling-local `ZIndex`, so each root's placement `Node`, `GlobalZIndex` and root `Pickable`
remain spawning-code concerns.

## Not covered

Hotbar (not spawned), nameplates on screen (hidden by default; console toggle not headlessly
drivable), VR.
