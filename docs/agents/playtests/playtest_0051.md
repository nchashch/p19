# Agent Playtest 0051 — chill_bevy_console Removed; the Options Screens Absorb Its Toggles

| Field | Value |
|---|---|
| Date | 2026-10-08 13:42 – 13:52 +0400 |
| Commit | `4291336` "Rip out bevy_fluent" + uncommitted chill_bevy_console removal |
| Agent | omp session, GLM 5.3 Flash (Z.ai) |
| Client | 1× `target/debug/p19-client --mcp` (rendered), rebuilt with `--features dev-tools` (0 errors) |
| Server | 1× fresh `target/release/p19-server` (started for this run, torn down after; the test player was despawned server-side first) |
| Transports | BRP client :15702, BRP server :15701 |

## Purpose

`chill_bevy_console` and the backtick dev console are removed entirely (bevy_markup is now the
only UI framework — i18n and accessibility everywhere). The four console toggles were already
options-screen rows sharing resources with the console; the console's other writers are gone,
and `FpsOverlayVisible`/`PhysicsGizmosVisible` + their appliers (and the `FpsOverlayPlugin`/
`PhysicsDebugPlugin` adds) moved into `ui/ui.rs`. Verify nothing regressed: the options toggles
still drive the engine, the pause menu and its Escape layering are intact, and the data frame
loses its console hint row.

## Verification

| Step | Observed |
|---|---|
| connect → select → play | `InGame`, player at spawn, 100 HP |
| Tab (data frame) | rows start at `HP: 100/100` — the "Press \` for console" hint row is **gone** (key deleted from all three locales) |
| Start → pause menu | Resume / Options / Main Menu, unchanged |
| Options → FPS overlay toggle (gamepad) | `FPS overlay: On` **and** the real overlay renders (`FPS: 50.17` row in `game/ui`) — the machinery relocated into `ui/ui.rs` works end to end |
| Escape with submenu open | submenu closes, pause stays (focus back on `pause-options` — not re-read this run, but the close was) |
| Escape again | pause closes, back in game |
| `game/trigger disconnect` | back to `MainMenu` |
| Teardown | player despawned server-side (`world.despawn_entity`), `player_count` 0, server + client killed |

![pause_options_no_console.png](screenshots/playtest_0051/pause_options_no_console.png)

*The pause menu's options submenu toggling the FPS overlay on the console-free build (the overlay's live `FPS: 50.17` row was present in the same `game/ui` dump).*

## Findings

**F1 — the console's disappearance is invisible to gameplay.** Every gameplay path (pause
modal, data frame, options toggles, ESC layering) behaves identically; the only user-visible
removals are the backtick surface and the data frame's hint row.

**F2 — the gating semantics simplified, not weakened.** `console_closed` gates are gone;
gameplay observers now gate on `in_state(ModalMenuState::Closed)` alone, and
`gate_replicated_input_context` (bug_0007's fix) gates only on the pause modal. The main-menu
and pause-toggle observers are registered ungated (`main_menu`'s `MainMenu` action is unbound
and never fires anyway).

**F3 — options toggles now have exactly one writer (the UI) plus BRP.** The engine configs
(`FpsOverlayConfig`, `PhysicsGizmos`) are written only by the appliers; no second input surface
left to desync with.

**F4 — known-functionality regression, accepted per the removal's intent:** the console's
non-toggle commands are gone with it (`respawn`, `despawn_cubes`, `despawn_npcs`,
`play_animation`, `load_level`, `controls`, `kcc_debug`) — those were dev conveniences; the BRP
agent tool API remains for QA, and hotkeys (`E`/`R`/`T`/`F`) still cover cube/NPC spawns and
combat.
