# Agent Playtest 0028 — UI Roots Styled by CSS (`<html class>`), Spawn-Site Placement Removed

| Field | Value |
|---|---|
| Date | 2026-10-05 21:37 – 21:41 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0027, and this change: every template wrapped in `<html class="<surface>-root">`, a "Roots" section in `theme.css`, the roots' `Node`/`GlobalZIndex`/`Pickable`/`BackgroundColor` and the `PAUSE_MENU_Z`/`TOOLTIP_Z`/`POPUP_Z`/`HOTBAR_BOTTOM_PADDING`/`PANEL_MARGIN` constants removed from `ui/{ui,lobby,hud,tui_panel,nameplate,npc_ui_quad,modal_menu,markup,selector}.rs`); bevy_markup path dependency at `d033fe1` + uncommitted root rule (`apply_root`, `CssRoot`) and bug_0021 fix |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest); three launches (see F1, F2) |
| Server | the already-running `target/release/p19-server` (pid 1350053, not started by this session; same as playtest 0027) |
| Level | `levels/minimal.level.ron` (already loaded) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup's root rule now styles the `HtmlUi` entity itself: the `html` rule plus the
document's own `<html id class>` give it layout, position/insets, size, `background-color`,
`z-index` (`ZIndex` among roots) and `pointer-events`, restoring the app's values for anything
no longer declared and never touching undeclared fields. The client moved each root's static
placement, stacking and pickability from spawn code into `theme.css`; only computed values
(nameplate/tooltip/popup `left`/`top`, the popup width) stay in code.

## Verification

Roots read with `world.query` on `HtmlUi` (`Node`, `ZIndex`, `Pickable`, `BackgroundColor`).

| Step | Observed |
|---|---|
| Main menu roots | `screen-root` 100% wide, `Pickable::IGNORE`; NPC-sign `texture-root` 100%; `tui-root` absolute `top/right 20px`, ignored |
| Main menu clicks | Connect/Options/Credits/Quit/Language clickable (`.main-menu-panel { pointer-events: auto }` under the ignored root) |
| Hover Connect | tooltip root absolute at `left 348, top 340` (code), `z=1000` (CSS), ignored |
| Options popup | root absolute, `width 280px` / `left 348, top 394` (code), `z=900` (CSS); rows A–E; pick works |
| Mouse Connect → Lobby | lobby `screen-root`; Play tooltip beside Play |
| In game | crosshair root 100%, ignored, dot centered; data frame absolute `top/right 0`; 16 nameplates absolute with per-frame `left`/`top` |
| Pause (Start) | `pause-root` absolute 100%, `z=100`, dim background; controls tips absolute `0,0`, `z=101`, ignored; focus on Resume; a **mouse** click on Resume closes it |

![menu_popup_tooltip.png](screenshots/playtest_0028/menu_popup_tooltip.png)

*Main menu: options popup (z 900) and the Options tooltip (above it, z 1000), TUI panel top-right.*

![lobby_tooltip.png](screenshots/playtest_0028/lobby_tooltip.png)

![in_game.png](screenshots/playtest_0028/in_game.png)

![pause.png](screenshots/playtest_0028/pause.png)

*Pause: dimmed root (CSS background), controls tips above it.*

## Findings

**F1 — bevy_markup bug_0021 (fixed in this change): despawning a UI in the same frame panicked
`update_pseudo_states`.** First launch: hover Connect (tooltip spawned), click Connect → client
exit 101, `insert<PseudoState>` on a despawned entity. bevy_markup queued plain inserts on
elements the app can despawn in the same `Update`; now `try_insert` (also `try_insert`/
`try_remove` of `AutoDirectionalNavigation` in `focus::sync_navigation`). Unit regression test
`pseudo_states_tolerate_a_same_frame_despawn` fails without the fix. Pre-existing, not caused
by the root rule; earlier runs activated Connect by keyboard/gamepad, not a mouse click after
hovering.

**F2 — Stale binary on the second launch.** The client was relaunched before the rebuild with
the fix had finished linking (binary mtime 10 s after launch) and crashed the same way; the
third launch ran the fixed build and passed everything above. Tooling note only.

**F3 — No visible change** from playtests 0025–0027: layout, focus, stacking and
clickability are as before.
