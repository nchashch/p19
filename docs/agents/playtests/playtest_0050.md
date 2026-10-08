# Agent Playtest 0050 — Console-Only Options Exposed in Both Menus; Pause Menu Gains an Options Submenu

| Field | Value |
|---|---|
| Date | 2026-10-08 13:10 – 13:22 +0400 |
| Commit | `3870b8f` "Upgrade bevy_markup to 0.4.0; locale bundles to .ftl.ron" + uncommitted bevy_fluent removal and options exposure |
| Agent | omp session, GLM 5.3 Flash (Z.ai) |
| Client | 1× `target/debug/p19-client --mcp` (rendered), rebuilt with `--features dev-tools` (0 errors) |
| Server | the pre-existing idle server on :6000 (was `Lobby`, 0 players), driven through the canonical connect → select → play sequence |
| Transports | BRP client :15702, BRP server :15701 |

## Purpose

The four console-only toggles (FPS overlay, physics debug gizmos, nameplates, HUD) are now
options-screen rows, and the options screen is shared: the main menu opens it as before, the
pause menu opens it as a submenu (its layout is now Resume / Options / Main Menu, the mouse
sensitivity slider having moved into the submenu). The toggles and the console's matching
commands flip the same reflected resources (`FpsOverlayVisible`, `PhysicsGizmosVisible`,
`NameplatesVisible`, `HudVisible`); `apply_fps_overlay`/`apply_physics_gizmos` copy the first
two into Bevy's/avian's engine configs. Verify both surfaces, the shared state, the submenu's
back/Escape layering, and the slider in its new home.

## Verification

Driven by gamepad and mouse (`game/gamepad`, `game/mouse`), read back from `game/ui` and BRP.

| Step | Observed |
|---|---|
| Main menu → Options | Language, slider, prediction, **FPS overlay: Off, Physics debug gizmos: Off, Nameplates: Off, HUD: On** (correct defaults), Back |
| FPS overlay toggle (gamepad) | label flips; the actual overlay appears/disappears — an `FPS: 34.57` text row shows in `game/ui` while on |
| BRP `world.insert_resources` on `FpsOverlayVisible` | overlay row appears/disappears too — engine state follows the resource through the applier |
| Start (gamepad) in game | pause modal: exactly **Resume** (auto-focused), **Options**, **Main Menu** + controls tips |
| Options (gamepad) | the options screen opens as a submenu over the modal (all rows incl. the slider) |
| Nameplates toggle in the submenu | `NameplatesVisible` true → false via BRP, label follows |
| Slider drag (mouse click at track midpoint) | `Mouse sensitivity: 1.55×`; click near the left end restores `1.00×` |
| Back | submenu closes, focus returns to the pause menu's Options button (`pause-options`) |
| Escape (keyboard) with submenu open | submenu closes, **pause menu stays**, focus back on `pause-options` |
| Escape again | pause modal closes, back in game (position unchanged throughout) |
| Start → Resume (South on the auto-focused button) | modal closes, still `InGame` |
| `game/trigger disconnect` | back to `MainMenu` |

![options_main_menu.png](screenshots/playtest_0050/options_main_menu.png)

*The main menu's options screen with the four exposed toggles.*

![pause_menu.png](screenshots/playtest_0050/pause_menu.png)

*The pause menu: three buttons, Resume auto-focused.*

![pause_options_submenu.png](screenshots/playtest_0050/pause_options_submenu.png)

*The options screen as the pause menu's submenu (the pause menu's rows are still listed beneath it in the `game/ui` dump; the submenu's backdrop blocks them).*

## Findings

**F1 — one state, three writers.** UI toggles (both menus), console commands and BRP writes all
flip the same reflected resources; the appliers are the only writers of the engine-side configs
(`FpsOverlayConfig.enabled` + its frame-time graph, `PhysicsGizmos` group). Verified both
directions (UI → engine, BRP → engine).

**F2 — the Escape layering has no double-binding.** Pause-scoped screens bind only gamepad East
for cancel; Escape over the pause modal stays owned by `ToggleModalMenu`, whose handler now
closes the topmost open screen (popup, else the screen) before toggling the modal. Headlessly
verified: submenu first, pause second. (The main-menu scope keeps Escape + East; nothing else
there binds them.)

**F3 — the submenu's Language picker state is per pause session.** The `Selector` is spawned
with the modal (`DespawnOnExit(ModalMenuState::Open)`), so the "resume at last pick" memory
resets when the pause menu closes — unlike the main menu's, which lives as long as the menu.
Unexercised beyond spawning (the popup itself wasn't driven in this run); harmless but worth
knowing.

**F4 — BRP visibility note.** `world.get_resources` resolves the app-owned resources
(`FpsOverlayVisible`, `PhysicsGizmosVisible`, `NameplatesVisible`, `MouseSensitivity`) but not
Bevy's `FpsOverlayConfig` (not reflected, -23501) — by design: the app-owned resource is the
single BRP-visible state, the engine config is an implementation detail of the applier.

**F5 — housekeeping.** The test session's player was despawned server-side after the run
(`world.despawn_entity` on :15701); the server ended with `player_count: 0`, so the
documented "disconnected players are never cleaned up" gap (AGENTS.md "Known gaps") did not
strand a zombie for the next client.

**F6 — incidental:** the FPS overlay's text shows up as a `game/ui` row while enabled, which
makes the toggle verifiable headlessly without screenshots.
