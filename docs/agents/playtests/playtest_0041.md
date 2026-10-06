# Agent Playtest 0041 — Options Screen

| Field | Value |
|---|---|
| Date | 2026-10-07 02:09 – 02:12 +0400 |
| Commit | `c4d3bd8` "Add credits for assets" + uncommitted options screen (`ui/menu_screen.rs`, `options.html`) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered) |
| Server | none (main menu only) |
| Transports | client QA: BRP :15702 |

## Purpose

The main menu's Options button opened a stub selector, and Language sat in the main menu. Options
now opens a modal screen like Credits (both built by `ui/menu_screen.rs`), and the Language
selector lives on it. Verify the screen, the selector popup on top of it, layered cancel
(Escape closes the popup, then the screen), focus return, and that Credits still works.

## Verification

Input through `game/mouse`, `game/keyboard` and `game/gamepad` (real bindings); focus read from
`bevy_input_focus::InputFocus` over BRP.

| Step | Observed |
|---|---|
| Main menu | `Connect, Options, Credits, Quit` (no Language); focus `connect` |
| Click Options | screen open (`Language`, `Back`); focus `options-back` |
| Click Language | popup `English, Русский, 日本語` over the screen; focus `slot-0` |
| Escape | popup closed, screen stays; focus `language` |
| Click Language, Русский | whole UI in Russian, screen stays open; focus `language` |
| Escape | screen closed; focus `options` |
| Enter, ArrowUp, Enter, ArrowDown ×2, Enter | screen → Language → popup (focus on the current pick, `slot-1`) → 日本語 picked; UI in Japanese |
| Gamepad East | screen closed; focus `options` |
| Gamepad South, DPadUp, South / East / East | screen → popup (`slot-2`) / popup closed (focus `language`) / screen closed (focus `options`) |
| Credits, Escape | credits open (`credits-back`), closed (focus `credits`) |
| Escape with nothing open | stays in `MainMenu` |

![options_en.png](screenshots/playtest_0041/options_en.png)
![options_language_popup.png](screenshots/playtest_0041/options_language_popup.png)

## Findings

**F1 — Works on every input path.** No errors in the client log.

**F2 — Fixed during the run:** the Language tooltip, placed right of the button, covered the
popup's first row (it opens to the right too). It is now placed above.

**F3 — Not tested here:** VR (the wrist panel keeps the Language selector directly and has no
Options or Credits) and the windowed client.
