# Agent Playtest 0048 — Credits Outro States the Original Assets Are CC0

| Field | Value |
|---|---|
| Date | 2026-10-08 10:02 – 10:07 +0400 |
| Commit | `90b3e2e` "Bump bevy_mcp_harness version" + uncommitted CC0 licensing of the original assets (ADR 0018) |
| Agent | omp session, GLM 5.3 Flash (Z.ai) |
| Client | 1× `target/debug/p19-client --mcp` (rendered), rebuilt with `--features dev-tools` (0 errors) |
| Server | none — main-menu surface only |
| Transports | BRP :15702 |

## Purpose

The licensing change (ADR 0018: original assets dedicated to the public domain under CC0 1.0)
edits the Credits screen's outro — the `credits-original` Fluent key in all three locales plus
the embedded English fallback in `credits.html`. Verify the screen still builds and the new
outro renders.

## Verification

| Step | Observed |
|---|---|
| Boot to main menu | `game/ui`: Connect / Options / Credits / Quit, all `clickable` |
| Gamepad DPadDown ×2, then South | Credits modal opened (focus path unchanged from playtest 0040) |
| `game/ui` outro node | `Everything else was made for this game and is likewise public domain (CC0 1.0).` |
| Cropped capture of the outro rect | rendered glyphs, not a blank frame (517 unique colors in the 680×94 crop) |

![credits_outro_cc0.png](screenshots/playtest_0048/credits_outro_cc0.png)

*The en-US credits outro after the licensing change.*

## Findings

**F1 — New outro renders live in en-US.** The `credits-original` key resolves from the runtime
locale file; the intro and the three third-party rows are unchanged.

**F2 — ru-RU/ja-JP not driven.** Their `credits-original` values were static-checked for
presence and brace-free syntax; the locale-switch → credits mechanism itself was verified in
playtest 0040 and is untouched by this change. A stale or missing outro in those locales would
therefore indicate a locale-file problem, not a template one.

**F3 — Note for future sessions.** The outro's visible text comes from the runtime `.ftl`, not
the embedded template — the embedded English fallback only shows when the key is missing. An
old binary run against new assets still renders the new copy, so verifying this surface
requires neither a rebuild nor a screenshot.
