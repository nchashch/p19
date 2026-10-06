# Agent Playtest 0038 — System Fonts

| Field | Value |
|---|---|
| Date | 2026-10-07 00:50 – 00:52 +0400 |
| Commit | `9158192` "Use crates.io version of bevy_markup" + uncommitted system-fonts change (ADR 0016: `register_ui_fonts` maps the CSS generics to `FontSource::Serif` / `SansSerif` / `Monospace` at `Startup`; the three font keys leave `CommonAssets` and the manifest; `override_default_font` removed; FPS overlay on `FontSource::Monospace`; dev console on Bevy's built-in font) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

Text must render with the host's fonts now that the client loads no font files: Latin in the
`serif` and `monospace` generics, Japanese through fallback to an installed CJK font, and every
surface — menus, the TUI panel, in-game nameplates, the controls panel and the pause menu.

## Verification

| Step | Observed |
|---|---|
| Asset loading → main menu | reached `MainMenu`; buttons `Connect, Options, Credits, Quit, Language` |
| Main menu, English | buttons in a system serif; the TUI panel in a system monospace |
| Language → 日本語 | buttons `接続, オプション, クレジット, 終了, 言語`; the TUI panel's Japanese line renders; no missing-glyph boxes |
| Back to English, Connect, select the level, Play | `Lobby`, then `InGame` |
| NPC spawned, nameplates on | nameplate "NPC" renders |
| Start (pause menu) | controls panel and `Main Menu, Resume` render |

![main_menu_en.png](screenshots/playtest_0038/main_menu_en.png)
![main_menu_ja.png](screenshots/playtest_0038/main_menu_ja.png)
![pause_menu.png](screenshots/playtest_0038/pause_menu.png)

## Findings

**F1 — Every surface renders with system fonts**, including Japanese, on this machine (Arch
Linux with Noto fonts and Noto Sans CJK installed). The UI font is now the system serif (a Noto
serif here), not IosevkaSlabQP: a visible change of look.

**F2 — Not verified:** a system without a CJK font (Japanese would show missing-glyph boxes, by
design of ADR 0016), the Steam Deck, the dev console (now on Bevy's built-in FiraMono), and the
FPS overlay (disabled by default).

**F3 — Tooling note:** a `target/release/p19-server` started before this session held ports
6000/6001; the first server start failed with `AddrInUse`, and the client could not connect to
the old instance. That process exited on its own; a fresh server was then started for the run.
