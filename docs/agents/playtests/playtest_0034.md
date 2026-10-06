# Agent Playtest 0034 — Buttons From a Shared Template Component Library

| Field | Value |
|---|---|
| Date | 2026-10-06 02:07 – 02:09 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0033, and this change: new `html/components.html` with Tera 2 components `ui.button` / `ui.selector_toggle`, embedded in `ui/markup.rs`; `main_menu.html`, `lobby.html`, `pause_menu.html`, `wrist_game.html` include it and call the components instead of repeating the button markup); bevy_markup path dependency at `9d84990` + uncommitted template composition (`{% include %}`/`{% extends %}` across files) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup templates can now include and extend other template files (paths relative to the
template, loaded with it, hot-reloading together), and Tera 2 components defined in an
included file are callable from the including template. The client's button markup moved into
one component library.

## Verification

Buttons read with `world.query` on `HtmlElement` (id, classes, `dataset`).

| Step | Observed |
|---|---|
| Main menu | `Connect, Options, Credits, Quit, Language`; focus `connect` (`autofocus={true}`); `connect` has classes `button primary`, `options` `data-selector="main-menu.options"`, `data-tooltip-placement="above"` — the same attributes as the hand-written markup |
| Hover Options | tooltip `Above` |
| Language toggle | popup `English, Русский, 日本語` |
| Lobby | `Play, Level, Main Menu`; focus `play`; same attributes as before |
| Enter on Play (level loaded) | `InGame` |
| Start | pause menu `Main Menu, Resume`, focus `pause-resume`; South resumes |

![pause_from_components.png](screenshots/playtest_0034/pause_from_components.png)

## Findings

**F1 — The component-built UI is attribute-for-attribute identical** to the hand-written
markup it replaced. The VR wrist panel (`wrist_game.html`) uses the components too but wasn't
exercised (no headset).

**F2 — Tooling note:** an Enter right after closing the Language popup reopened it (focus
returns to the toggle, by design); the driver then connected via `game/trigger connect`.
