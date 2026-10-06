# Agent Playtest 0032 — Per-Feature `data-*` Attributes Instead of Shared `data-with`

| Field | Value |
|---|---|
| Date | 2026-10-06 01:58 – 02:00 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0031, and this change: tooltips read `data-tooltip` / `data-tooltip-args` / `data-tooltip-placement`, selector toggles `data-selector`, via `ElementSignal::data`; the shared `data-with` blobs in `main_menu.html`, `lobby.html`, `hotbar.html` are gone); bevy_markup path dependency at `9d84990` + uncommitted `HtmlElement::dataset` / `ElementSignal::data` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | — (menus and lobby) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup's `HtmlElement` now carries the element's `data-*` attributes (`dataset`), and every
`ElementSignal` exposes them (`signal.data(key)`). The client's buttons used to pack tooltip and
selector keys into one `data-with` JSON object per element; each feature now has its own
attribute.

## Verification

| Step | Observed |
|---|---|
| Hover Connect | tooltip `Right` at `left 348, top 340`, text "Connect to the server." (as playtest 0029) |
| Hover Options (`data-tooltip-placement="above"`) | tooltip `Above`, `left 60, bottom 414` (as playtest 0029) |
| Click Options / Language (`data-selector`) | popups open: `Stub Option A, B, …` / `English, Русский, 日本語` |
| Lobby: hover Play, click Level | tooltip `Right` at `348/340`; popup `Minimal` |

## Findings

**F1 — Behaviour unchanged**; templates are shorter and each feature reads its own attribute.
Not exercised: `data-tooltip-args` (only the hotbar uses it, and `HOTBAR_ENABLED = false`).
