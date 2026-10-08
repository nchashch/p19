# Playtest Index

One entry per run, as `docs/agents/playtests/playtest_NNNN.md` (see `docs/agents/skills/playtest.md` §10 for
the format and layout these follow). Newest first. Update this file whenever a new playtest is
filed — that's part of filing it, not a separate later chore.

### `playtest_0051` — chill_bevy_console Removed; Options Screens Absorb Its Toggles

| Field | Value |
|---|---|
| **Date** | 2026-10-08 13:42 – 13:52 +0400 |
| **Commit** | `4291336` "Rip out bevy_fluent" + uncommitted chill_bevy_console removal |
| **Agent** | omp session, GLM 5.3 Flash (Z.ai) |
| **Report** | [`playtest_0051.md`](playtest_0051.md) |

Verifies the dev-console removal: bevy_markup is now the only UI framework. The
`FpsOverlayVisible`/`PhysicsGizmosVisible` resources, their appliers and the FPS-overlay/
physics-debug plugin adds moved into `ui/ui.rs`; the `console_closed` gates are gone
(gameplay observers gate on the pause modal alone, `gate_replicated_input_context` included).
Verified in game against a fresh server: data frame without its console hint, the options
screens' FPS toggle still drives the real overlay, pause-menu Escape layering intact, and the
run's player despawned server-side afterwards.

### `playtest_0050` — Console-Only Options in Both Menus; Pause Menu's Options Submenu

| Field | Value |
|---|---|
| **Date** | 2026-10-08 13:10 – 13:22 +0400 |
| **Commit** | `3870b8f` "Upgrade bevy_markup to 0.4.0; locale bundles to .ftl.ron" + uncommitted bevy_fluent removal and options exposure |
| **Agent** | omp session, GLM 5.3 Flash (Z.ai) |
| **Report** | [`playtest_0050.md`](playtest_0050.md) |

Verifies the four console-only toggles (FPS overlay, physics debug gizmos, nameplates, HUD)
exposed as options-screen rows in both menus: the main menu's Options screen and — new — the
pause menu's Options submenu (the pause layout is now Resume / Options / Main Menu, the mouse
sensitivity slider moved into the submenu). Toggles, console commands and BRP writes all flip
the same reflected resources whose appliers drive the engine configs; the submenu's Back and
Escape layering (submenu first, pause second) and the slider in its new home are verified
headlessly. The test player was despawned server-side afterwards, leaving the idle server
clean.

### `playtest_0049` — bevy_fluent Ripped Out: Minimal `LocaleSelection`, bevy_markup-Only Localization

| Field | Value |
|---|---|
| **Date** | 2026-10-08 12:49 – 13:00 +0400 |
| **Commit** | `3870b8f` "Upgrade bevy_markup to 0.4.0; locale bundles to .ftl.ron" + uncommitted bevy_fluent removal |
| **Agent** | omp session, GLM 5.3 Flash (Z.ai) |
| **Report** | [`playtest_0049.md`](playtest_0049.md) |

Verifies the bevy_fluent removal: the language picker now writes a minimal reflected
`LocaleSelection` resource that `sync_active_locale` mirrors into bevy_markup's `ActiveLocale`,
and the console output is plain English literals. Driven by gamepad through the selector
popup, en → ru → ja → en all render correctly (in-place re-render, instant switch-back via the
bundle cache), `world.list_resources` shows `LocaleSelection` and zero bevy_fluent state, and
the user manually confirmed the English-only console on a real window (the agent's wtype
attempt is recorded as an unresolved §5c tooling note).

### `playtest_0048` — Credits Outro States the Original Assets Are CC0

| Field | Value |
|---|---|
| **Date** | 2026-10-08 10:02 – 10:07 +0400 |
| **Commit** | `90b3e2e` "Bump bevy_mcp_harness version" + uncommitted CC0 licensing (ADR 0018) |
| **Agent** | omp session, GLM 5.3 Flash (Z.ai) |
| **Report** | [`playtest_0048.md`](playtest_0048.md) |

Verifies the Credits screen after licensing the original assets CC0 (ADR 0018): driven by gamepad through the unchanged main-menu focus path, the outro now reads "Everything else was made for this game and is likewise public domain (CC0 1.0)" in en-US; the ru-RU/ja-JP values are static-checked (locale mechanism per playtest 0040).

### `playtest_0047` — Mouse Sensitivity in the Pause Menu

| Field | Value |
|---|---|
| **Date** | 2026-10-07 08:20 – 08:25 +0400 |
| **Commit** | acdb56f "Decouple look replication from physics" + uncommitted slider and its pause-menu copy |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0047.md`](playtest_0047.md) |

Verifies the Mouse sensitivity slider in the in-game pause menu (shared component with Options): dragging and arrow keys change the shared value, the camera doesn't turn while dragging under the menu, and after resuming 100 px turns camera and server by exactly 0.005 × 100 × the value.

### `playtest_0046` — Mouse Sensitivity Slider

| Field | Value |
|---|---|
| **Date** | 2026-10-07 08:10 – 08:20 +0400 |
| **Commit** | acdb56f "Decouple look replication from physics" + uncommitted slider |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0046.md`](playtest_0046.md) |

Verifies the continuous Mouse sensitivity slider (0.1×–3.0×, new `ui/slider.rs`): click and drag set the exact mapped value (also past the ends and off the track), left/right and the d-pad step by 0.05 with auto-repeat while up/down still navigate, and 100 px of mouse motion in game turns camera and server by exactly 0.005 × 100 × the value.

### `playtest_0045` — Mouse Sensitivity Option

| Field | Value |
|---|---|
| **Date** | 2026-10-07 08:00 – 08:05 +0400 |
| **Commit** | `acdb56f` "Decouple look replication from physics" + uncommitted mouse sensitivity option |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0045.md`](playtest_0045.md) |

Verifies the Options screen's Mouse sensitivity selector (0.25×–3×): picking by mouse and keyboard updates the label (also translated), and 100 px of mouse motion turns the camera and the server by exactly 0.005 × 100 × the multiplier, including after a mid-game change.

### `playtest_0044` — Client-Owned Look (ADR 0017)

| Field | Value |
|---|---|
| **Date** | 2026-10-07 07:10 – 07:50 +0400 |
| **Commit** | `4892d4d` "Fix mouse look bug" + uncommitted look rework |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0044.md`](playtest_0044.md) |

Verifies the switch to client-sent absolute look: before it, a spinning cube drifted the server's yaw (bug_0010) and pitch was mirrored (bug_0011); after it, camera and server look match exactly (Δ 0.0000) through cube launches, menu toggles, 40 ± 15 ms lag with 2% loss, `game/input`, the right stick and replay, in both prediction modes, and other clients see the replicated facing on the model.

### `playtest_0043` — Look Divergence After Opening a Menu (bug_0009)

| Field | Value |
|---|---|
| **Date** | 2026-10-07 02:40 – 03:00 +0400 |
| **Commit** | `47e7b19` "Add client side prediction toggle" + uncommitted fix |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0043.md`](playtest_0043.md) |

Investigates "W walks at an angle": client and server yaw agree under mouse input (loopback and 40±15 ms/2% loss, prediction on and off), but mouse motion in the frame the pause menu or console opens turned only the server (client 0.0 vs server −1.6 after five cycles). Fixed by ungating the camera observer; verified in both modes.

### `playtest_0042` — Client-Side Prediction Toggle

| Field | Value |
|---|---|
| **Date** | 2026-10-07 02:22 – 02:25 +0400 |
| **Commit** | `3a58f16` "Move language select to options menu" + uncommitted prediction toggle |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0042.md`](playtest_0042.md) |

Verifies the Options screen's Client-side prediction toggle end to end: with it on the own character is `Predicted` and runs the client KCC (motion within 0.1 s); with it off the server skips `PredictionTarget`, the character is `Interpolated` with no client KCC and still moves and turns (motion one tick later on localhost). Toggle works by mouse and keyboard and is translated.

### `playtest_0041` — Options Screen

| Field | Value |
|---|---|
| **Date** | 2026-10-07 02:09 – 02:12 +0400 |
| **Commit** | `c4d3bd8` "Add credits for assets" + uncommitted options screen |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0041.md`](playtest_0041.md) |

Verifies the main menu's Options screen (a modal like Credits, holding the Language selector that used to sit in the main menu): the language popup opens over it, Escape/gamepad East close the popup first and then the screen, focus returns to the opening button, language changes apply with the screen open, and Credits still works.

### `playtest_0040` — Credits Screen

| Field | Value |
|---|---|
| **Date** | 2026-10-07 02:00 – 02:04 +0400 |
| **Commit** | `75b9817` "Update docs" + uncommitted credits screen |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0040.md`](playtest_0040.md) |

Verifies the main menu's new Credits screen (the third-party CC0 assets from `assets/CREDITS.md`): opens and closes by mouse, Enter/Escape and gamepad South/East, blocks the menu underneath, returns focus to the Credits button, and renders in English, Russian and Japanese.

### `playtest_0039` — End to End on the Git-Tracked Assets Alone

| Field | Value |
|---|---|
| **Date** | 2026-10-07 01:38 – 01:40 +0400 |
| **Commit** | `ff3fc3b` "Use system fonts instead of bundled asset fonts" + uncommitted assets (Git LFS) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0039.md`](playtest_0039.md) |

Verifies that the 47 asset files going into git (19 through LFS) are enough to run the game: with fresh network state (new server key and TLS identity, new client pin), the client reached the main menu, connected, listed the level, entered the game, moved and spawned an NPC with its nameplate.

### `playtest_0038` — System Fonts

| Field | Value |
|---|---|
| **Date** | 2026-10-07 00:50 – 00:52 +0400 |
| **Commit** | `9158192` "Use crates.io version of bevy_markup" + uncommitted system-fonts change (ADR 0016) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0038.md`](playtest_0038.md) |

Verifies the client after it stopped loading font files and switched to the system's fonts: the main menu in English (system serif and monospace) and Japanese (CJK fallback, no missing glyphs), lobby → in game, a nameplate, the controls panel and the pause menu all render. The UI's look now follows the host's fonts; a system without a CJK font would not render Japanese.

### `playtest_0037` — In-Place Updates Write Only What Changed

| Field | Value |
|---|---|
| **Date** | 2026-10-06 02:22 – 02:24 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup write-if-different updates on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0037.md`](playtest_0037.md) |

Verifies the client after bevy_markup's in-place updates started writing only changed components (600-element UI with one changing value: 9.85 → 4.06 ms/frame in a release bench): the TUI gauge still animates, focus and tooltips work, and a 51%-HP NPC's bar lays out at 31 of 60 px. Notes bevy_markup bug_0025, a pre-existing flaky property test.

### `playtest_0036` — Nameplates on bevy_markup's `HtmlWorldAnchor`

| Field | Value |
|---|---|
| **Date** | 2026-10-06 02:15 – 02:17 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `HtmlWorldAnchor` on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0036.md`](playtest_0036.md) |

Verifies the client after nameplates moved onto bevy_markup's `HtmlWorldAnchor` (projection, visibility and cleanup upstream; toggle and fade as a `hidden` class): the NPC plate sits bottom-centered over the head (610/132 for a 60×30 plate at 640/162), the own player's off-screen plate hides, the toggle maps to `display: none`, and the plate disappears past the 30 m fade end. Plates are now centered over their targets.

### `playtest_0035` — Buttons Routed by Name (`on_html_click`)

| Field | Value |
|---|---|
| **Date** | 2026-10-06 02:11 – 02:12 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `HtmlSignalsExt` on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0035.md`](playtest_0035.md) |

Verifies the client after its main-menu, lobby and pause-menu buttons moved from one `MessageReader` + name `match` per surface to bevy_markup's `app.on_html_click(name, system)`: Connect (Enter and gamepad South), lobby Main Menu, Play, Resume and pause Main Menu all work.

### `playtest_0034` — Buttons From a Shared Template Component Library

| Field | Value |
|---|---|
| **Date** | 2026-10-06 02:07 – 02:09 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup template composition on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0034.md`](playtest_0034.md) |

Verifies the client after its menus, lobby, pause menu and VR wrist panel started building buttons from `html/components.html` (Tera 2 `ui.button` / `ui.selector_toggle`, included across files — new in bevy_markup): every button has the same id, classes and `data-*` as the hand-written markup, autofocus, tooltips and selectors work, Enter plays and the pause menu resumes.

### `playtest_0033` — Built-in `data-tooltip` Tooltips

| Field | Value |
|---|---|
| **Date** | 2026-10-06 02:02 – 02:03 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `data-tooltip` tooltips on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0033.md`](playtest_0033.md) |

Verifies the client after its tooltip signal plumbing was replaced by bevy_markup's built-in `data-tooltip` (`HtmlTooltips`): Connect, Options (above) and Quit tooltips appear exactly where the hand-rolled ones did, with their translated text, and none remain after the pointer leaves.

### `playtest_0032` — Per-Feature `data-*` Attributes Instead of Shared `data-with`

| Field | Value |
|---|---|
| **Date** | 2026-10-06 01:58 – 02:00 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `HtmlElement::dataset` / `ElementSignal::data` on top of `9d84990`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0032.md`](playtest_0032.md) |

Verifies the client after tooltips and selector toggles moved from one shared `data-with` object per button to their own `data-tooltip*` / `data-selector` attributes (read through bevy_markup's new `ElementSignal::data`): tooltips land where they did in playtest 0029 (right 348/340, above with bottom 414) with their translated text, and the Options, Language and Level selectors open.

### `playtest_0031` — Signal Sources: Activation Input, Primary-Only Clicks

| Field | Value |
|---|---|
| **Date** | 2026-10-05 23:25 – 23:27 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `SignalSource`/`ActivationInput`/`data-on-auxclick` on top of `c8ab00f`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0031.md`](playtest_0031.md) |

Verifies the client after `ElementSignal` gained a `source` and `HtmlFocus::activate` started requiring the activating input (the client reports Enter or the gamepad's South): Enter connects, gamepad South resumes from the pause menu, a left click on Play enters the game, and a right click on Play now does nothing (bevy_markup bug_0024: any mouse button used to fire `data-on-click`).

### `playtest_0030` — In-Place UI Updates, `style` Attributes and `opacity`

| Field | Value |
|---|---|
| **Date** | 2026-10-05 22:31 – 22:34 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup in-place reconciliation, `style` attribute and `opacity` on top of `3c73cba`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0030.md`](playtest_0030.md) |

Verifies the client after bevy_markup started updating UIs in place and the nameplate fade/health and TUI gauge moved into templated `style` attributes: language switches, clock/gauge ticks, popup paging and nameplate fades keep every element entity (20/20, 124/124), focus stays on the same row entity across edge paging, the selected row autofocuses via the template, and a 51%-HP NPC's plate shows a 51% bar while fading 1.0 → 0.18 → hidden. No `HtmlUiBuilt`/`HtmlUiRestyled` observers remain in the client.

### `playtest_0029` — Tooltips and Selector Popups Anchored by bevy_markup's `HtmlAnchor`

| Field | Value |
|---|---|
| **Date** | 2026-10-05 22:02 – 22:03 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `HtmlAnchor` on top of `ebfbf22`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0029.md`](playtest_0029.md) |

Verifies the client after its tooltip and selector popup stopped computing element rects, viewport clamps and UI cameras and use `HtmlAnchor` instead: positions are identical to playtest 0028 (348/340, 348/394, above-placement `bottom 414`), the popup width is CSS, the tooltip follows the hovered element, and an open popup is despawned with its toggle when the menu goes.

### `playtest_0028` — UI Roots Styled by CSS (`<html class>`), Spawn-Site Placement Removed

| Field | Value |
|---|---|
| **Date** | 2026-10-05 21:37 – 21:41 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup root rule + bug_0021 fix on top of `d033fe1`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0028.md`](playtest_0028.md) |

Verifies the client after every template got an `<html class="<surface>-root">` and `theme.css` took over each root's placement, size, stacking (`z-index` 100/101/900/1000) and pickability from the spawn code, which now sets only computed positions: menus, popup, tooltips, HUD, nameplates and pause menu lay out, stack and click as before. Found and fixed bevy_markup bug_0021 (a same-frame UI despawn panicked `update_pseudo_states`; hover-then-click Connect crashed the client).

### `playtest_0027` — bevy_markup Custom Elements Replace the Client's Build Hooks

| Field | Value |
|---|---|
| **Date** | 2026-10-05 20:55 – 21:00 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup `is="…"` custom elements on top of `9721039`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0027.md`](playtest_0027.md) |

Verifies the client after its five per-rebuild `HtmlUiBuilt` hooks (crosshair ring/dot, hotbar GCD overlay, controls-tip glyphs, nameplate fill, TUI gauge) became `is="…"` elements with `define_html_element` systems: all attach on build (GCD ring/dot swap, 14/14 glyphs, animated gauge), and a forced nameplate rebuild comes back at the damaged NPC's 51% (set by the definition, not CSS). `NameplatesVisible` is now reflected (BRP can show nameplates; playtest 0022 F6 partly resolved). The selector's focus hook stays (wheel paging).

### `playtest_0026` — bevy_markup Skips Identical Renders: Client Value Diffing Removed

| Field | Value |
|---|---|
| **Date** | 2026-10-05 16:22 – 16:30 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup identical-render skip on top of `f81acce`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0026.md`](playtest_0026.md) |

Verifies the client after bevy_markup stopped rebuilding on identical renders and the client stopped diffing its own template values (data frame, nameplate name, TUI seconds): written every frame, the TUI panel still rebuilds once a second and the data frame only on a displayed change (idle and moving-without-target: 0 rebuilds; a jump flipping `Grounded` rebuilds). Observed, not investigated: spawned cubes end up ~1000 units away, so selecting one is cleared at once.

### `playtest_0025` — Focus and Navigation Moved into bevy_markup

| Field | Value |
|---|---|
| **Date** | 2026-10-05 15:14 – 15:40 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and uncommitted bevy_markup focus module + astral-tl switch on top of `decd505`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0025.md`](playtest_0025.md) |

Verifies the client after moving focus, directional navigation, restore-by-id, modal scoping and the focus ring into bevy_markup (browser-style `autofocus`/`tabindex`/`:focus-visible`/`outline`), with the client keeping only its input bindings: menu, selector edge paging, pointer-hides-ring, lobby, pause modal, Enter/South activation all match playtest 0022. Found an upstream tl 0.7.8 parser bug (a value-less attribute eats the next attribute's first character), fixed by switching bevy_markup to astral-tl (its bug_0019).

### `playtest_0024` — bevy_markup CSS Positioning/Borders/Pointer-Events: Client Workarounds Removed

| Field | Value |
|---|---|
| **Date** | 2026-10-05 14:42 – 14:43 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (and the uncommitted bevy_markup CSS support on top of `f99c6ec`) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0024.md`](playtest_0024.md) |

Verifies the client after replacing code-side `Pickable::IGNORE` loops, the crosshair dot's code-set radius and fake 2px frames with bevy_markup's new `pointer-events`, `border-radius` and `border-color`: TUI panel, tooltip and crosshair subtrees are unpickable while menus still click, the dot renders round, the TUI frame is a real border, and the pause menu stays stable.

### `playtest_0023` — bevy_markup bug_0018 Fix: Icons Directly on Built Elements

| Field | Value |
|---|---|
| **Date** | 2026-10-05 06:57 – 06:59 UTC |
| **Commit** | `0da1fcb` "Use bevy_markup for UI" + uncommitted `modal_menu.rs` icon change (and the uncommitted bevy_markup path-dependency fix) |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0023.md`](playtest_0023.md) |

Verifies bevy_markup's bug_0018 fix (a `CssFrame` marker so an app `ImageNode` on a built element is no longer a shape change) with prototype_19's nested-`HtmlUi` icon workaround removed: the pause menu's 61 elements stay stable for 0.5 s with 14 icon `ImageNode`s attached directly, and all 11 tip labels show in `game/ui`.

### `playtest_0022` — bevy_markup UI Migration: Every Surface Driven Through Mouse, Gamepad and Enter

| Field | Value |
|---|---|
| **Date** | 2026-10-05 05:47 – 06:05 UTC |
| **Commit** | `ecf2349` "Replicate Transforms" + the uncommitted bevy_markup UI migration (ADR 0015; every `crates/client/src/ui/` file, `game/ui`, bevy 0.19.1 lock); the controls-tips icon fix (F1) landed mid-run |
| **Agent** | omp session, Claude Opus 5.5 (Anthropic) |
| **Report** | [`playtest_0022.md`](playtest_0022.md) |

Verifies the client UI rewrite on bevy_markup (HTML templates + CSS + Fluent; ADR 0015) on `--mcp` and `--no-render` clients. Checks main menu, both selectors (gamepad edge paging, wheel paging, resume at the last pick), language switching to Japanese, tooltips, lobby, level pick, play, data frame, pause menu with controls tips, and nameplates. Mouse clicks, gamepad navigation/South and keyboard Enter all confirm buttons, so the old windowless-Enter gap is gone, and bug_0008 is fixed (label-only tips without atlases). Found and fixed a runaway rebuild loop: an app `ImageNode` on a built element fails bevy_markup's restyle shape check, so icons now sit on nested empty `HtmlUi`s. Open: the NPC sign quad was never attached because `decorate_npcs` didn't run (cause unconfirmed), plus one `IsDefaultUiCamera` warning.

### `playtest_0021` — Console/Modal Input Lock: Replicated Context Deactivation (bug_0007)

| Field | Value |
|---|---|
| **Date** | 2026-10-03 16:00 – 16:20 UTC |
| **Commit** | `ac6dc0a` "Restructure the workspace" + uncommitted working tree, per file: `client/src/controls/controls.rs` (the fix — `gate_replicated_input_context`), AGENTS.md, bug_0007/bug_0008; plus staged font-swap and nameplate files not under test |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0021.md`](playtest_0021.md) |

Fixes the owner-reported leak where typing WASD/Space in the open dev console (or with the pause modal open) moved the character: the `console_closed` convention gates only observers, while movement flows continuously through BEI's binding readers on the replicated ahoy actions. The fix deactivates the local player's `PlayerInputContext` via BEI's `ContextActivity::INACTIVE` while a UI surface owns the keyboard (bindings survive; Escape/Tab live in a separate context and keep working). Verified live through device-level input on a `--mcp` client: W moves with no surface, freezes while the modal is open, moves again after close — with the stated limit that the console-open state itself is not headlessly drivable (`game/keyboard` can't reach `just_pressed` consumers — new documented harness gap). The run also found bug_0008: a `--no-common-assets` client panics when the pause modal opens (missing icon atlas in `input_icons.rs`).

### `playtest_0020` — KCC Rollback Registration: bug_0004 Fixed via lightyear's Built-in Local Rollback API

| Field | Value |
|---|---|
| **Date** | 2026-10-02 04:55 – 05:20 local |
| **Commit** | `1f6a99f` "Add ./docs/bug_reports and ./docs/skill/bugreport.md" + uncommitted working tree, per file: `client/src/gameplay/player_character.rs` (the fix — `local_rollback` registration for `CharacterControllerState`), the ring → aws-lc-rs TLS provider switch caught during verification, bug_0004 status/root-cause correction, AGENTS.md movement-gap correction |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0020.md`](playtest_0020.md) |

Fixes bug_0004 by re-examining its premise: ahoy 0.2's `CharacterControllerState` already derives `Component + Clone`, exactly what lightyear 0.30's built-in `local_rollback()` requires, so the "needs ahoy-side exposure" assessment was wrong and the fix is one registration in `PlayerCharacterPlugin` (ordered after `PredictionPlugin`), with `AccumulatedInput` deliberately unregistered (it re-derives from the replayed input stream every tick). Verified on two `--no-render` clients: the predicted player carries `PredictionHistory<CharacterControllerState>` alongside the four physics histories, and movement stays sane through client B's join-burst correction window with zero panics — while stating honestly that the ghost-jump symptom itself is not deterministically observable through the harness (a scripted repro is the proposed follow-up if symptoms are ever reported). A crypto-provider mismatch caught at compile time during verification switched both binaries to `rustls::crypto::aws_lc_rs`, and the token flow was re-verified over it.

### `playtest_0019` — Token Endpoint over Self-Signed HTTPS: LAN Encryption Closed

| Field | Value |
|---|---|
| **Date** | 2026-10-02 04:05 – 04:45 local |
| **Commit** | `8c851b8` "Improve MCP quality of life - add select nearest player API" + uncommitted working tree, per file: `rustls`/`rustls-pemfile`/`rcgen`/`sha2` promoted to direct deps, `server/src/networking.rs` (TLS identity load-or-create, HTTPS :6001, `server_addr_check: true`), `client/src/lifecycle/networking.rs` (TOFU fingerprint pinning), `game/state` player `name`, docs |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0019.md`](playtest_0019.md) |

The connect-token endpoint now speaks HTTPS with a self-signed `rcgen` certificate (blocking `rustls`, an explicit TLS `close_notify` after the response — rustls surfaces its absence as a read error), the client pins the cert's SHA-256 fingerprint trust-on-first-use and refuses a mismatch with an explicit MITM/rotation message, and `server_addr_check` is restored to `true` because every token now embeds the requester's real IP as its server-address whitelist (supersedes the `false` posture). Verified end-to-end on a `--no-render` client: first-run cert generation + fingerprint logging, token fetch over TLS, connect → Lobby → level → play → InGame with a generated name (`Valiant Sparrow`), a tampered-fingerprint reconnect refused cleanly with zero panics, then a full-loop regression. The LAN half of the netcode posture is closed; a CA-signed backend and first-connection MITM safety are the explicitly deferred remainder.

### `playtest_0018` — 'Joiner Hover' Re-Verified: Spawn-Point Stacking, Not a KCC Bug

| Field | Value |
|---|---|
| **Date** | 2026-10-02 03:15 – 03:35 local |
| **Commit** | `8c851b8` "Improve MCP quality of life - add select nearest player API" + one uncommitted file: `AGENTS.md` (doc correction only — **no code changes this session**) |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0018.md`](playtest_0018.md) |

Re-verification the project owner requested of playtest 0014's F3 ("joiner hover = the joiner's KCC gets no ticks until input flows"), both directions on two `--no-render` clients: joining over an idle first player lands the joiner at exactly (0, 2.73, 0) with `grounded: true` — the precise capsule-stacking height, standing on the first player's head — and once the first player walks away (the joiner still sending nothing) the joiner falls to 0.95 normally, with no input of its own. Playtest 0014's F3 mechanism is retracted; "joiner hover" is reclassified from technical bug to shared-spawn-point design issue (AGENTS.md corrected; both reports kept as the historical record). Bonus finding: a stacked joiner is a convenient stationary, in-range combat target for the harness recipe.

### `playtest_0017` — Headless Combat Harness: Full Kill-to-Despawn Loop Verified Without a Window

| Field | Value |
|---|---|
| **Date** | 2026-10-02 02:55 – 03:40 local |
| **Commit** | `9377b41` "Fix dead player bug" + uncommitted working tree, per file: `client/src/events.rs` (new `AttackSelected`/`KillSelected` triggers), `controls.rs` (hotkey observers re-routed; shared send observers), `dev/tool_api.rs` (`game/select`, `attack`/`kill` triggers, `selected` in `game/state`), `gameplay/combat.rs` (idempotent `hide_dead`), `server/src/combat.rs` (combat caster resolution), `server/src/replay.rs`, docs |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0017.md`](playtest_0017.md) |

Combat was unreachable headlessly because crosshair targeting needs a real window: `game/select` (entity id / nearest / name) now injects `Selected` on `--no-render` clients and `game/trigger attack|kill` fires the same send observers the hotkeys use (hotkey observers re-routed through shared `AttackSelected`/`KillSelected` triggers, so both paths share one send implementation). The full kill-to-despawn loop verified on two `--no-render` clients: two GCD-spaced attacks drove HP 100 → 51 → 2 through the new caster-resolution path, the kill variant dropped it to 0, the corpse despawned, the victim returned to Lobby — zero panics across all three processes. Also found and fixed mid-verification: per-frame `hide_dead` command spam racing the replicated corpse despawn on a remote observer (now idempotent).

### `playtest_0016` — Death-Path Panic Root-Caused and Fixed: a Missing SyncWorldPlugin, Not a Replication Bug

| Field | Value |
|---|---|
| **Date** | 2026-10-02 02:30 – 03:10 local |
| **Commit** | `012b7c8` "Only build steamrt4 client" + uncommitted working tree, per file: `SyncWorldPlugin` added to the `--no-render` branch of `client/src/main.rs` |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0016.md`](playtest_0016.md) |

Fixes bug_0002 — the death-path panic of playtest 0015's F3 — by root-causing the visible `ServerMutateTicks` failure as a decoy: `--no-render` (no `RenderPlugin` → no `ExtractPlugin` → no `SyncWorldPlugin`) never gets `PendingSyncEntity`, so the corpse despawn's sync on-remove hook panicked mid-`receive_replication`, and the unwind stranded the resources that function removes-then-reinserts by design. One-line fix (`add_plugins(SyncWorldPlugin)` in the `--no-render` branch); end-to-end death loop verified with zero panics, and playtest 0015's dead-look gate is now runtime-verified. Two measurement traps recorded: a missing-resource error may implicate an earlier system that panicked inside a remove-and-reinsert scope, and `world.list_resources` is reflected-only so it can never assert a resource's absence.

### `playtest_0015` — Room-Tagged Spawns and the Dead-Player Window: Two Fixes and a Newly-Exposed Death-Path Panic

| Field | Value |
|---|---|
| **Date** | 2026-10-02 02:00 – 02:50 local |
| **Commit** | `012b7c8` "Only build steamrt4 client" + uncommitted working tree, per file: `Rooms` tags in `server/src/spawn.rs`, dead-look gate in `server/src/input.rs`, corpse-sim stop in `client/src/gameplay/combat.rs`, doc updates |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0015.md`](playtest_0015.md) |

Fixes bug_0003: cubes/NPCs are standalone entities the `ChildOf` room cascade never reaches, so both spawn resolvers now insert `Rooms::single(game_room)` — verified as 1 cube + 1 NPC visible to the in-game client and zero to a lobby-held client. The dead-player window closes: movement (pre-existing `RigidBody` removal), server-side look (`Without<Dead>` gate — compile-verified here, runtime-verified in playtest 0016), and the owner's local corpse sim (`hide_dead` removes `AhoyCharacterController`); dead-attacker gating is correctly deferred to the combat caster-resolution fix. Verification exposed a new pre-existing blocker — killing a player panicked the victim's `--no-render` client (bug_0002: missing `ServerMutateTicks`, the ~1 s corpse despawn racing lightyear corrections) — fixed in playtest 0016. Also documents `world.mutate_components`' reflect sub-path schema and `Dead` being invisible to BRP (unreflected).

### `playtest_0014` — Remote-Entity Interpolation: Fix, Verification, and a New Joiner-Freeze Finding

| Field | Value |
|---|---|
| **Date** | 2026-10-02 01:00 – 02:10 local |
| **Commit** | `012b7c8` "Only build steamrt4 client" + uncommitted working tree, per file: `client/src/gameplay/interpolated_remotes.rs` (the fix), `main.rs` plugin-tuple rebalance |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0014.md`](playtest_0014.md) |

Remote players/cubes/NPCs snapped to each replicated update; the fix is one marker — a polling system inserts lightyear's `Interpolated` on every replicated body that is neither `Predicted` nor `RigidBody::Static` — because lightyear 0.30 (interpolation a default feature; `LightyearAvianPlugin`'s `AvianReplicationMode::Position`) already registers the history buffers and Hermite rules, and 0.30 interpolates in place. Marker assignment verified per-entity with `world.list_components`. Two side findings: BRP `option` queries can match components under stale TypePath aliases (`Predicted` matched under an old path that per-entity listing showed as absent — the silently-wrong sibling of playtest 0013's silently-empty trap), and a second-joining client's player hovered at spawn height until its first input, attributed by ablation to pre-existing behavior and later reclassified as spawn-point stacking (playtest 0018's F1).

### `playtest_0013` — Spawn Caster-Resolution Fix: NPC/Cube Spawning Verified End-to-End

| Field | Value |
|---|---|
| **Date** | 2026-10-01 22:30 – 23:20 local |
| **Commit** | `9d36146` "Add CI" + uncommitted working tree, per file: caster resolution in `server/src/spawn.rs` (reusing `networking::owned_players`, made `pub(crate)`), `Reflect` on `Npc` |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0013.md`](playtest_0013.md) |

Fixes the spawn half of bug_0001: `apply_spawn_npc`/`apply_spawn_cube` looked the caster's `Gcd` up on the connection entity, which has carried none since the player became a separate `ControlledBy`-owned entity; both resolvers now walk the connection's owned entities via `networking::owned_players` (the RNG seed deliberately stays connection-derived, preserving replay recordings). NPCs and cubes verified spawning as full, correct bundles at the camera-forward point on fresh pairs. The detour found a second, QA-surface bug that had masked the fix: `Npc` lacked `Reflect`, so BRP `world.query` silently matched nothing even while per-entity `world.list_components` proved the spawns were succeeding (reflection heuristic recorded; avian 0.7's `Collider` TypePath trap noted). Cubes launch at ~100 u/s (observed, not chased); the same-class combat bug in `server::combat` stayed open — fixed in playtest 0017's span.

### `playtest_0012` — Desync Reproduction Attempt: Instrumented, Multi-Mode, Intermittent

| Field | Value |
|---|---|
| **Date** | 2026-09-25 00:30 – 01:45 local (+04) |
| **Commit** | `f4180af` + temporary desync instrumentation in `controls.rs` (marked for removal) as of this entry |
| **Agent** | opencode agent (GLM-5.3-Flash) |
| **Report** | [`playtest_0012.md`](playtest_0012.md) |

Loops playtest 0011's intermittent multi-client look/movement desync across client modes with
new instrumentation (LOOK-DIVERGENCE / ROTATE-FIRE logging in `controls.rs`). Finds a real
spawn invariant violation (`FpsCamera::new()` yaw = −π vs the identity rig transform),
eliminates the remote-action cross-fire theory (zero ROTATE-FIRE on the second client while
the first rotated), confirms the server's ground truth stayed correct in every sample, and
records the windowed-client harness gap (BRP unreachable at poll time) as the blocker for an
instrumented reproduction on the owner's exact configuration.

### `playtest_0011` — Vision Tooling, Client Configurations, Server BRP/MCP, and the Multi-Client Desync

| Field | Value |
|---|---|
| **Date** | 2026-09-24 17:20 – 20:30 UTC (one long session, several restarts) |
| **Commit** | `f4180af` "Fix no dev-tools feature build failure" at the end; the session's work (server tools, `ObserveRequest`, no-render decorate fix, playbook updates) uncommitted as of this entry |
| **Agent** | opencode agent (GLM-5.3-Flash), with the project owner co-driving a windowed client |
| **Report** | [`playtest_0011.md`](playtest_0011.md) |

Three linked verifications on live sessions: the data-first vision tooling (`game/ui`,
agent cursor, state-fused/cropped/unchanged-suppressed captures, 1280×800 viewport), the
client-configuration matrix (rendered headless, `--no-render`, `--headless-render`
observer, fleet ports) including a live two-client desync reproduction on full-render
clients — second client spawns with its look pitch-pinned at the clamp, first client
unaffected — and the new server-side BRP/MCP surface (`server/state`) whose very first
use caught a client-vs-server divergence red-handed. Findings include the no-render
second-player crash (found and fixed during the session), a measured zombie-player census,
and the next-step instrumentation plan for the desync.

### `playtest_0010` — Optional `CommonAssets` Furniture / `--no-common-assets`

| Field | Value |
|---|---|
| **Date** | 2026-09-23 17:23 – 17:32 UTC |
| **Commit** | `5e71215` "Implement Skein mesh primitives for agent testing"; the `CommonAssets` changes not yet committed as of this entry |
| **Agent** | opencode agent (GLM-5.3-Flash) |
| **Report** | [`playtest_0010.md`](playtest_0010.md) |

Implements playtest 0009's closing proposal: the 9 engine-furniture fields of `CommonAssets`
(fonts, WAVs, KTX2 skybox, atlas PNG pairs) became `#[asset(key = "…", optional)]`
`Option<Handle<T>>` fields — a manifest that omits the keys resolves them to `None` and
every consumer degrades gracefully (Bevy's embedded default font, no skybox pass, no sample
playback, an empty icon-atlas fallback) — plus a pre-sync `--no-common-assets` CLI flag
for the even-bareer boot (no manifest read at all, a `CommonAssets::placeholder()`
resource, and an immediate `AssetLoading → MainMenu` transition; the dev console's hardcoded
font path suppressed too). Verified in three modes: playtest 0009's tree stripped **in place**
to 100% plaintext (only text files remain, full loop works, clear color replaces the
skybox), the flag mode (UI-only menu, in-game `ClientWorldAsset` visuals still render —
that path loads by path, not manifest), and a production regression check (skybox renders,
zero degradation warns). Also noted: the manifest's `lobby_background` key is dead in every
mode (the code aliases it to `menu_background` — pre-existing), and a pre-existing
connect-time `Disconnected` re-entry into `MainMenu` observed in both modes.

### `playtest_0009` — Isolated, All-Plaintext Playtest Assets

| Field | Value |
|---|---|
| **Date** | 2026-09-23 16:00 – 16:40 UTC |
| **Commit** | `5e71215` "Implement Skein mesh primitives for agent testing" |
| **Agent** | opencode agent (GLM-5.3-Flash) |
| **Report** | [`playtest_0009.md`](playtest_0009.md) |

First experiment with the owner's isolated-asset idea: each playtest ships its own
server/client assets under `docs/agents/playtests/playtest_assets/playtest_NNNN/` as **plaintext** — hand-written
JSON `.gltf` scenes whose only content is Skein components (`ClientReplicate`,
`ClientWorldAsset`, `ColliderConstructor`, and the new `MeshPrimitive` for zero-baked-data
visuals), a per-playtest dynamic-asset manifest, config, and a single en-US locale. The whole
get-in-game loop runs on the isolated assets: level list, colliders (the player grounds on the
authored Skein collider), MeshPrimitive visuals rendering, starfield skybox, HUD — with the
only non-text files being copied engine furniture (fonts, WAVs, KTX2, atlas PNGs). Two
self-inflicted asset bugs found and documented en route (a glTF node-index typo; the pitch
sign convention), plus a new playtest technique: injecting lights via BRP
`world.insert_resources`.

### `playtest_0008` — Full Keyboard+Mouse / Gamepad Reachability Sweep

| Field | Value |
|---|---|
| **Date** | 2026-09-23 13:35 – 14:09 UTC |
| **Commit** | Started at `736ec01`; the `lobby.rs` fix not yet committed as of this entry |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0008.md`](playtest_0008.md) |

Requested directly: verify every functional element (main menu, lobby, in-game) is reachable by
both keyboard+mouse and gamepad, using only device-level input injection (no `game/trigger`/
`game/select_level`/`game/input` shortcuts). Reproduced and root-caused a real,
100%-reproducible bug the project owner had independently noticed: a genuine mouse click on the
lobby's "Play" or "Main Menu" button never worked, at all – `client/src/ui/lobby.rs` imported
the wrong of two identically-named `Activate` event types, so these `FeathersButton`-based
handlers only ever received `Activate` via a gamepad/keyboard-Enter compatibility bridge meant
for old-style widgets, never via a real click's actual `bevy_ui_widgets::Activate`. Fixed with a
one-line import change; verified post-fix with a fresh server+client pair that a mouse click on
Play immediately after selecting a level (the exact reported scenario) now reaches
`GameState::InGame` – and independently confirmed by the project owner on the real windowed
client (Connect -> select level -> click Play, all with the mouse; literal Enter also works
there). Also found and documented a separate, unfixable **harness** limitation:
`bevy_input_focus::dispatch_focused_input` requires a `PrimaryWindow` entity that `--mcp`
headless mode never creates, so literal keyboard Enter can never confirm a `FeathersButton`
through this tool API (mouse click and gamepad South both work fine) – not believed to affect a
real windowed client. Full reachability matrix in the report covers every menu/lobby/in-game
control across all three input methods.

### `playtest_0007` — Add game/keyboard + game/mouse, Verify Real UI Clicks

| Field | Value |
|---|---|
| **Date** | 2026-09-23 08:33 – 08:41 UTC |
| **Commit** | Started at `b340541`; not yet committed as of this entry |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0007.md`](playtest_0007.md) |

Extended `game/gamepad` (`playtest_0006`) to keyboard and mouse: `game/keyboard` mocks
`ButtonInput<KeyCode>` directly (every one of Bevy's 160+ variants, via `KeyCode`'s own `serde`
impl); `game/mouse` mocks `ButtonInput<MouseButton>` plus drives `bevy_picking`'s real
`PointerInput` pipeline for cursor motion/position/clicks – the first method in this API that
reaches UI by screen position rather than by navigating focus and confirming. First
implementation of mouse motion/wheel set `AccumulatedMouseMotion`/`AccumulatedMouseScroll` via a
direct resource write – compiled and ran with no error, but silently did nothing (`look_yaw`
stayed `0.0`), because Bevy's own per-frame reset-from-events systems unconditionally overwrite
those resources every frame. Fixed by injecting real `MouseMotion`/`MouseWheel` events instead.
Verified live: a real mouse click on "Connect"/"Options" at their actual screenshot pixel
coordinates drove the genuine `bevy_ui`/`bevy_picking` pipeline (state transitions and
`selector` popup open/close confirmed independently via `game/state` and log lines); `KeyW`
moved the in-game player through the real replicated movement pipeline; mouse motion turned the
camera only after the event-based fix.

### `playtest_0006` — Add game/gamepad, Verify the Literal Crash Path

| Field | Value |
|---|---|
| **Date** | 2026-09-23 01:37 – 01:42 UTC |
| **Commit** | Started at `05a416c`; not yet committed as of this entry |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0006.md`](playtest_0006.md) |

Added `game/gamepad` (mocks real `bevy_input::gamepad::Gamepad` button/axis state on a
synthetic entity, flowing through `bevy_enhanced_input`'s actual binding resolution) precisely
to close `playtest_0005`'s own stated gap: no way to reach UI navigation through the tool API,
only the three ahoy gameplay actions via `game/input`'s action-level mocking. First
implementation used the wrong `Gamepad` field (`digital`, not `analog` – BEI's button reader
calls `Gamepad::get`, which is the analog map; confirmed by testing, a silent no-crash failure,
not an error) and was caught by actually running it, not by review. Fixed, then used
immediately: drove the **literal** reported crash path for the first time – gamepad Start (open
pause modal) -> DPadUp (navigate focus to "Main Menu") -> South (activate) – confirming
`playtest_0005`'s fix against the real interaction, not an equivalent one. No crash, clean
`GameState::MainMenu` transition.

### `playtest_0005` — Fix the return_to_main_menu Crash

| Field | Value |
|---|---|
| **Date** | 2026-09-23 01:22 – 01:24 UTC |
| **Commit** | Started at `80b6f0f`; fix not yet committed as of this entry |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0005.md`](playtest_0005.md) |

User-reported crash on the real windowed client: in-game -> pause modal -> "Main Menu" ->
client panics. Matched a documented gap: `return_to_main_menu` was a live `todo!()`. Fixed to
mirror `lobby_main_menu_button`'s already-working pattern (`commands.trigger(Disconnect);
commands.set_state(GameState::MainMenu);`), routing through the crate-local `Disconnect` event
and its established two-part-disconnect handler rather than a fresh implementation. Verified via
the equivalent code path (`game/trigger disconnect` while genuinely in-game) since the tool API
has no way to simulate the actual keypress/button-click yet – no crash, clean return to
`MainMenu`, UI correctly rendered. Also fixed a stale doc comment in `modal_menu.rs` caught
along the way (no behavior change). **Re-verified against the literal crash path in
`playtest_0006`**, once `game/gamepad` closed the tool-API gap this report's own verification
had to work around.

### `playtest_0004` — Fix the Headless UI Render-Order Bug

| Field | Value |
|---|---|
| **Date** | 2026-09-23 00:53 UTC |
| **Commit** | Started at `41e29a9`; fix not yet committed as of this entry |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0004.md`](playtest_0004.md) |

Root-caused and fixed `playtest_0003`'s open follow-on: the menu/lobby UI panel rendering
**under** the background instead of on top of it. Real cause: `bevy_ui`'s `DefaultUiCamera::get()`
fallback only ever considers `Window(Primary)`-targeting cameras – structurally dead in `--mcp`
mode, where everything targets `Image` – so headless mode's permanent bootstrap UI camera and
`player_camera()`'s own self-tagged `IsDefaultUiCamera` collided the moment a player existed,
breaking `bevy_ui`'s camera selection entirely (not an ordering problem, despite looking like
one). Very likely explains why all three of `playtest_0003`'s ordering-only fix attempts
regressed in-game rendering in ways that resisted explanation at the time. Fixed by actively
maintaining "exactly one live `IsDefaultUiCamera` holder" as an invariant, handed off between
the bootstrap camera and `player_camera()` as they come and go. Verified end-to-end: main menu,
lobby, in-game (HUD now renders too, not just the world), and – newly tested, not covered by
any earlier playtest – the full disconnect-back-to-main-menu round-trip all composite
correctly.

### `playtest_0003` — Diagnose and Fix the Headless Camera Rendering Bug

| Field | Value |
|---|---|
| **Date** | 2026-09-22 23:00 – 2026-09-23 00:15 UTC |
| **Commit** | Started at `329c9bc`; fix landed as `e5fe6c6` (committed after this run's verification) – spans both sides of a code change, not a static state |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0003.md`](playtest_0003.md) |

Root-caused and fixed the bug `playtest_0001`/`playtest_0002` both hit: `--mcp` mode's cameras
never rendered anything, because `bevy_render::camera::camera_system`'s `target_info` recompute
silently never fires for a camera retargeted after `Startup` (a one-line fix,
`projection.set_changed()`, in `retarget_cameras_to_offscreen`). This also explained the
previously-separate "KTX2 skybox kills offscreen rendering" finding – same root cause, not a
real skybox bug; the skybox/TAA/SSAO strip workaround is removed. Surfaced a follow-on
UI-panel-renders-under-the-background ordering bug in menu/lobby specifically; three fix
attempts were tried and reverted here (each one regressed in-game rendering worse than the
ordering bug itself) – see the report's "A follow-on issue found, attempted, and reverted"
section for what was tried and why. **Fixed in `playtest_0004`**, once the real (non-ordering)
cause was found. Also found, unrelated: the "minimal" level has no light source anywhere in its
content (renders black on any client, not a `--mcp`-specific issue).

### `playtest_0002` — Bug Reproduction Pass (Caster-Resolution + Headless Camera)

| Field | Value |
|---|---|
| **Date** | 2026-09-22 \~23:25 UTC |
| **Commit** | `329c9bc` "Add playtest.md skill" |
| **Agent** | Claude (Sonnet 5) |
| **Report** | [`playtest_0002.md`](playtest_0002.md) |

A targeted re-verification pass, not a full state tour: checked two things `AGENTS.md`'s gap
list already claimed, with harder evidence than `playtest_0001` had supplied for the same
claims. Confirmed live: `spawn_cube` is silently dropped server-side (zero `Cube` entities
before/after the trigger, total server-log silence) – **still open, not fixed by any playtest
since**, see `AGENTS.md`'s caster-resolution gap entry for the fix shape. Also found a sharper
diagnostic lead for the headless camera bug (`Camera.computed.target_info` staying `null`
specifically for cameras claimed after `Startup`) that `playtest_0003` went on to root-cause
and fix.

### `playtest_0001` — Headless (–mcp) Client State Tour & Input Drive

| Field | Value |
|---|---|
| **Date** | 2026-09-23 \~02:38 local (2026-09-22 22:38 UTC) |
| **Commit** | `89f5eca` "Implement headless –mcp mode for client" |
| **Agent** | opencode session, GLM-5.3-Flash |
| **Report** | [`playtest_0001.md`](playtest_0001.md) |

The first formal playtest: verified the agent/QA tool API (ADR 0009) end-to-end against a fresh
server and a fresh headless client – drove every reachable client state, injected input through
the real replicated pipeline, and recorded what the agent sees vs. what a human on a windowed
client sees. Confirmed the movement/input/replication data path is fully solid headless (server-
authoritative sim, client prediction, all verified through state reads across a full menu→game
flow). First recorded the in-game blank-rendering bug and the KTX2-skybox-kill finding – both
later found to be the same root cause, fixed in `playtest_0003`.
