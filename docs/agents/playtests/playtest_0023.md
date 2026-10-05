# Agent Playtest 0023 — bevy_markup bug_0018 Fix: Icons Directly on Built Elements

| Field | Value |
|---|---|
| Date | 2026-10-05 06:57 – 06:59 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted: `crates/client/src/ui/modal_menu.rs` (icons inserted directly), deleted `crates/client/src/ui/html/icon.html`, AGENTS.md; bevy_markup is the path dependency `../../PROTOTYPE_23/bevy_markup` at `5113f7c` + uncommitted bug_0018 fix (`src/build.rs` `CssFrame`, regression test, docs) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

Verify bevy_markup's fix for its bug_0018 (an app `ImageNode` on a built element counted as a
shape change, so every restyle rebuilt — the controls-tips rebuild loop of playtest 0022 F1),
with prototype_19's workaround removed: the tip icons' atlas `ImageNode`s now go directly on
the `controls-tip-icon` elements instead of on nested empty `HtmlUi`s (`icon.html`, deleted).

## Verification

`game/trigger connect` → `game/select_level` → `game/trigger play` → `InGame` at
(0, 0.915, 0); gamepad Start opens the pause menu.

| Check | Observed |
|---|---|
| `HtmlElement` entity sets 0.5 s apart | 61/61 unchanged (playtest 0022 F1, same layout with the image on the element: 11 tip labels replaced within 0.5 s) |
| Elements carrying `ImageNode` (`world.query` `HtmlElement` with `ImageNode`) | 14 — the tip icons |
| `game/ui` tip labels | `Move` … `Stats`, all 11 |

![pause.png](screenshots/playtest_0023/pause.png)

*Pause menu with the atlas icons attached directly to the built elements; stable across
restyles.*

## Findings

**F1 — bug_0018 fixed upstream-side, workaround removed.** The bevy_markup regression test
`restyle_keeps_an_apps_image_on_an_element` fails with the fix reverted and passes with it;
the full bevy_markup test suite passes. In-game the tips no longer rebuild.
