# Agent Playtest 0033 — Built-in `data-tooltip` Tooltips

| Field | Value |
|---|---|
| Date | 2026-10-06 02:02 – 02:03 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0032, and this change: `ui/markup.rs` inserts `HtmlTooltips(tooltip.html)` and lost its tooltip signal, `Tooltip` component and `show_tooltips`; the templates dropped `data-on-enter/leave="tooltip"`); bevy_markup path dependency at `9d84990` + uncommitted `src/tooltips.rs` (and the `dataset` change of playtest 0032) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | — (main menu) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup now shows tooltips itself, like a browser's `title`: an element with
`data-tooltip="key"` (plus `data-tooltip-args` / `data-tooltip-placement`) gets the app's
`HtmlTooltips` template anchored beside it while hovered. The client's own tooltip signal
plumbing is gone.

## Verification

| Step | Observed (playtest 0032, hand-rolled) |
|---|---|
| Hover Connect | `Right`, `left 348, top 340`, "Connect to the server." (same) |
| Hover Options (`above`) | `Above`, `left 60, bottom 414`, "Not implemented yet." (same) |
| Hover Quit | `Right`, `top 502`, "Quit the game." |
| Pointer away | no tooltip left |
| Click Options | popup opens beside the button (the tooltip stays above it) |

![options_tooltip_popup.png](screenshots/playtest_0033/options_tooltip_popup.png)

## Findings

**F1 — Identical behaviour, about 60 fewer client lines.** Tooltip args (hotbar only) still
not exercised: `HOTBAR_ENABLED = false`.
