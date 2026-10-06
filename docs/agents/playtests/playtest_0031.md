# Agent Playtest 0031 — Signal Sources: Activation Input, Primary-Only Clicks

| Field | Value |
|---|---|
| Date | 2026-10-05 23:25 – 23:27 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0030, and this change: `ui/markup.rs` `on_ui_confirm` passes the confirming input — `ActivationInput::Key(Enter)` / `GamepadButton { gamepad, South }` / `Other` — to `HtmlFocus::activate`); bevy_markup path dependency at `c8ab00f` + uncommitted `SignalSource` / `ActivationInput`, `data-on-auxclick`, primary-only `data-on-click` (its bug_0024) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

`ElementSignal` now carries a `source`: the pointer (mouse, touch, custom such as the VR
lasers) with button, position and click count, the hovering pointer, or the input that
activated the focused element — which the app must report (`HtmlFocus::activate(input)`).
`data-on-click` fires for the primary button only; middle/right clicks are
`data-on-auxclick`. This run checks the client's activation and click paths still work and
that a right click no longer activates a button.

## Verification

| Step | Observed |
|---|---|
| Main menu boot | focus `connect` |
| Enter (`game/keyboard`) | `Lobby` (activation through `on_ui_confirm`) |
| Right click on Play (`game/mouse` button `Right`), level loaded | still `Lobby` (no `click`) |
| Left click on Play | `InGame` |
| Start, South on the pause menu | pause menu closed (Resume activated from the gamepad) |

## Findings

**F1 — Right clicks no longer activate buttons** (bevy_markup bug_0024: before, any button's
click fired `data-on-click`). The client has no `data-on-auxclick` hooks, so right/middle
clicks are inert everywhere.

**F2 — Tooling note:** the first Play click was on a fresh server with no level loaded, so the
server silently dropped `InGameRequest` (AGENTS.md: "only while `ServerState::InGame`"); the
retry after `game/select_level` entered the game.
