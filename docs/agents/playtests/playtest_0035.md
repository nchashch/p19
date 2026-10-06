# Agent Playtest 0035 — Buttons Routed by Name (`on_html_click`)

| Field | Value |
|---|---|
| Date | 2026-10-06 02:11 – 02:12 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0034, and this change: `ui/ui.rs`, `ui/lobby.rs`, `ui/modal_menu.rs` register their buttons with `app.on_html_click(name, system)` instead of one `MessageReader<ElementSignal>` + `match` per surface; pick handling stays a `MessageReader<SelectorPicked>`); bevy_markup path dependency at `9d84990` + uncommitted `HtmlSignalsExt` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup routes `ElementSignal`s to systems by name (`on_html_click` for clicks and
activations, `on_html_signal` for any trigger), run in `PostUpdate` before rendering. The
client's three button-handling systems became one small handler per button.

## Verification

| Step | Observed |
|---|---|
| Enter on Connect (keyboard activation) | `Lobby` |
| Click lobby Main Menu | `MainMenu` |
| South on Connect (gamepad activation) | `Lobby` |
| Click Play (level loaded) | `InGame` |
| Start, South on Resume | pause menu closed |
| Start, click Main Menu | `MainMenu` |

## Findings

**F1 — Every routed button works by pointer, keyboard and gamepad.** Not exercised: Credits and
Quit (Quit would end the client), and the VR wrist panel's Main Menu (no headset; it shares the
pause menu's handler).
