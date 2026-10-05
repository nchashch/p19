# Agent Playtest 0025 — Focus and Navigation Moved into bevy_markup

| Field | Value |
|---|---|
| Date | 2026-10-05 15:14 – 15:40 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0024, and this change: `ui/markup.rs` cut to input bindings, `HtmlModal`/`HtmlNoFocus`/`FocusEdge` in `ui/{ui,lobby,modal_menu,selector}.rs`, `autofocus` attribute in the templates, `.button:focus-visible` in `theme.css`, `game/ui` clickability in `dev/tool_api.rs`); bevy_markup path dependency at `decd505` + uncommitted `src/focus.rs`, `:focus`/`:focus-visible`/`outline`, and the `astral-tl` switch (its bug_0019) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh per run |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

Focus, directional navigation, focus restore, modal scoping and the focus ring moved from the
client (`ui/markup.rs`) into bevy_markup's new `focus` module (browser-style: `data-on-click`
and `tabindex` focusable, `autofocus` attribute, `:focus-visible` + `outline`, restore by `id`).
The client keeps only the `MenuControls` bindings, which call `HtmlFocus::navigate` /
`activate`. This run drives every focus path the client relied on.

## Verification

Focused element read with `world.query` (`HtmlElement` with `Outline`, i.e. the CSS
`:focus-visible` ring) and `InputFocus` → `HtmlElement.id`.

| Step | Observed |
|---|---|
| Main menu boot | buttons `Connect, Options, Credits, Quit, Language`; focus `connect` (autofocus), no ring |
| DPadDown / DPadUp | `options` with ring / back to `connect` with ring; 20/20 elements stable over 0.5 s |
| South on Options, DPadDown ×5 | popup rows `A…E`, focus `slot-0`; after ×5 window pages to `B…F`, focus `slot-4` (`FocusEdge`) |
| South (pick) | popup closed, focus back on `options` |
| Click Language, click 日本語 | ring hidden after pointer presses; labels `接続 … 言語` |
| Gamepad back to English | labels English |
| Enter on Connect | `Lobby`; focus `play` (autofocus) |
| South | `InGame`; focus `None` (nothing focusable in game) |
| Start | pause `Main Menu, Resume`; focus `pause-resume`; DPadUp → `pause-main-menu`; 61/61 stable |
| Enter | resumed; Start → DPadUp → South → `MainMenu`, focus `connect` |

![focus_ring.png](screenshots/playtest_0025/focus_ring.png)

*The focus ring on Options, now `.button:focus-visible { outline: 2px solid #fac74d;
outline-offset: 2px }` in `theme.css`.*

## Findings

**F1 — Upstream `tl` bug: a value-less attribute ate the next attribute's first character.**
The first run showed Connect missing from `game/ui`'s clickables and initial focus on Options:
the templates now carry `autofocus` *before* `data-on-click`, and tl 0.7.8 parsed
`autofocus data-on-click="…"` as `autofocus` + `ata-on-click`. Confirmed with a standalone tl
probe; fixed by switching bevy_markup to `astral-tl` 0.8.0 (bevy_markup bug_0019, UPSTREAM.md
U11). The second run above is on the fix.

**F2 — Behaviour matches the client-side implementation it replaces** (playtest 0022): same
initial focus, ring heuristic, edge paging, focus return to the selector toggle, modal
confinement and Enter/South activation.
