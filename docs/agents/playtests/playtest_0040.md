# Agent Playtest 0040 — Credits Screen

| Field | Value |
|---|---|
| Date | 2026-10-07 02:00 – 02:04 +0400 |
| Commit | `75b9817` "Update docs" + uncommitted credits screen (`ui/credits.rs`, `credits.html`) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered) |
| Server | none (main menu only) |
| Transports | client QA: BRP :15702 |

## Purpose

The main menu's Credits button was a stub. It now opens a modal listing the third-party (CC0)
assets from `assets/CREDITS.md`. Verify it opens and closes by mouse, keyboard and gamepad,
blocks the menu underneath, returns focus, and is translated.

## Verification

Input through `game/mouse`, `game/keyboard` and `game/gamepad` (real bindings); focus read from
`bevy_input_focus::InputFocus` over BRP.

| Step | Observed |
|---|---|
| Main menu | buttons `Connect, Options, Credits, Quit, Language`; focus `connect` |
| Click Credits | credits open; focus `credits-back` |
| Click Connect (under the backdrop) | state stays `MainMenu`, credits stay open |
| Click Back | closed; focus `credits` |
| Enter on Credits / Escape | open / closed, focus `credits` |
| Gamepad South / East | open / closed, focus `credits` |
| Escape with nothing open | stays in `MainMenu`, menu unchanged |
| Russian, Japanese | title, intro, "by", usage lines, outro and Back translated; asset titles and authors stay as published |

![credits_en.png](screenshots/playtest_0040/credits_en.png)
![credits_ru.png](screenshots/playtest_0040/credits_ru.png)
![credits_ja.png](screenshots/playtest_0040/credits_ja.png)

## Findings

**F1 — Works on every input path.** No errors in the client log.

**F2 — Fixed during the run:** the first version spawned the `CreditsControls` input context as
a child of the `HtmlUi` root; bevy_markup owns a root's children and removed it, so Escape and
East did nothing. It is now a separate entity, despawned with the root.

**F3 — The TUI demo panel shows at the top right while the credits are open.** It is correctly
stacked under the backdrop (dimmed; `ComputedStackIndex` 15 vs the credits root's 23) and
partly covered by the panel. Not a defect.

**F4 — Not tested here:** VR (the wrist panel omits Credits, since a screen-space modal wouldn't
show in the headset) and the windowed client.
