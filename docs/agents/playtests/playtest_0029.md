# Agent Playtest 0029 — Tooltips and Selector Popups Anchored by bevy_markup's `HtmlAnchor`

| Field | Value |
|---|---|
| Date | 2026-10-05 22:02 – 22:03 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0028, and this change: `ui/markup.rs`'s `show_tooltips` and `ui/selector.rs`'s popup spawn use `HtmlAnchor` instead of computing rects, viewport clamps and UI cameras; the popup width moved to `.selector-root` in `theme.css`; `POPUP_WIDTH` and the toggle-gone cleanup removed); bevy_markup path dependency at `ebfbf22` + uncommitted `src/anchor.rs` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest), relaunched once after reflecting `HtmlAnchor` for BRP |
| Server | the already-running `target/release/p19-server` (pid 1350053, not started by this session) |
| Level | — (menus and lobby only) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup's new `HtmlAnchor { element, placement, gap }` keeps an overlay root beside an
element every frame (right/left/above/below), clamps it into the viewport by its own size,
renders it on the element's UI camera and despawns it with the element. The client's tooltip
and selector popup used to compute all of that by hand at spawn time.

## Verification

Overlays read with `world.query` on `HtmlAnchor` (+ `Node` insets, `ComputedNode` size).

| Step | Observed | Playtest 0028 (hand-placed) |
|---|---|---|
| Hover Connect | tooltip `Right`, `left 348, top 340`, 204×34 | `left 348, top 340` |
| Hover Quit | tooltip moved to `top 502` | — |
| Click Options | popup `Right`, `left 348, top 394`, width 280 (CSS) × 228; Options tooltip `Above`, `left 60, bottom 414` (= 800 − 394 + 8) | popup `left 348, top 394`, width 280 (code) |
| Pointer away | tooltip gone (leave), popup stays | — |
| Connect while the popup is open | popup despawned with its toggle (main menu torn down) | was `refresh_open_popups`' job |
| Lobby, hover Play | tooltip `left 348, top 340` | same |

![popup_and_tooltip.png](screenshots/playtest_0029/popup_and_tooltip.png)

*Identical to playtest 0028: popup right of Options, the Options tooltip above it.*

![lobby_tooltip.png](screenshots/playtest_0029/lobby_tooltip.png)

## Findings

**F1 — Anchored placement reproduces the hand-computed positions exactly** (table above), and
the popup's width is now plain CSS since the clamp uses the overlay's measured size.

**F2 — `HtmlAnchor` wasn't queryable over BRP at first** (not `Reflect`); now derived like
bevy_markup's other public components. Not exercised: render-to-texture (VR wrist) tooltips
and viewport clamping in the client (no surface near an edge); the clamp is covered by
bevy_markup's `anchored_overlay_follows_clamps_and_despawns`.
