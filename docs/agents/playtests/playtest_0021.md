# Agent Playtest 0021 — Console/Modal Input Lock: Replicated Context Deactivation (bug_0007)

| Field | Value |
|---|---|
| Date | 2026-10-03 16:00 – 16:20 UTC |
| Commit (local state actually running) | `ac6dc0a` "Restructure the workspace" + uncommitted working tree, per file: `client/build.rs` + `client/src/ui/tui_panel.rs` (IosevkaSlabMono font swap — staged, not under test), `client/src/ui/nameplate.rs` (nameplate spawn-hidden — staged, not under test), `client/src/controls/controls.rs` (**the fix**: `gate_replicated_input_context`), `AGENTS.md`, `docs/agents/bug_reports/bug_0007.md`+`bug_0008.md` |
| Agent | opencode session, GLM-5.3-Flash |
| Clients | 1x `target/debug/p19-client --mcp` (dev-tools feature, rendered headless, full manifest); first attempt used `--no-common-assets` and found bug_0008 |
| Server | `target/release/p19-server` — the owner's already-running instance was reused untouched (InGame with `levels/minimal.level.ron`, 0 players at join) |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

The project owner reported: with the in-game dev console open, typing WASD or Space moves
and jumps the character — the console's input bar does not lock gameplay input. Fix it and
verify through the harness.

## Root cause

The `add_observers_run_if!(…, console_closed)` convention gates only *observers*. Movement
is not observer-driven: `bind_replicated_ahoy_actions` inserts `Bindings` on the
server-authored replicated `Movement`/`Jump`/`RotateCamera` action entities once, and BEI's
binding readers then consume `ButtonInput<KeyCode>` every PreUpdate tick — console state
never consulted — with lightyear streaming the action state to the server's KCC. The pause
modal had the identical leak (and its build comment claimed gameplay paused under it; only
attack/kill actually were gated).

## The fix

`client/src/controls/controls.rs` — new polling `Update` system
`gate_replicated_input_context`: while the console is open or the modal is open, deactivate
the local player's `PlayerInputContext` via BEI 0.26's `ContextActivity<C>::INACTIVE`
(remove+insert — the component is immutable; written only on state transitions). BEI
transitions the context's action states to zero/release, so the streamed input releases held
keys server-side; the bindings survive for reactivation on close. Escape/Tab live in the
separate local `PlayerControls` context, so the toggles keep working, and the console's own
backtick toggle reads raw `ButtonInput` upstream of BEI, unaffected. The condition mirrors
the attack/kill observers' `console_closed.and_then(in_state(ModalMenuState::Closed))` — one
gate semantics for both UI surfaces and both input kinds.

## Verification

Fresh `--mcp` client (full manifest) against the live server; `connect` → `play` → player
"Lucky Bramble" at (0, 0.915, 0), all four ahoy actions bound. Entirely device-level input
(`game/keyboard` — through BEI's real binding resolution):

| Step | Observed |
|---|---|
| Baseline: `KeyW` held 2 s, no UI surface | position → (−0.92, 0.915, −22.65) — moves (plus the documented post-release coast) |
| Escape → pause modal opens (`game/ui` shows the modal panel); `KeyW` held 2 s | position **unchanged** at (0, 0.915, 0) — frozen |
| Escape → modal closed (`game/ui` empty); `KeyW` held 2 s | position → (0, 0.915, −22.87) — moves again; gate releases cleanly, bindings intact |
| Panics/errors across the run | 0 |

**Verification limit, stated honestly:** the console-open state itself is not headlessly
drivable — chill_bevy_console's `handle_toggle_key` reads `ButtonInput::just_pressed`, which
`game/keyboard` (writing in `RemoteLast`, end of frame) can never satisfy, because the next
frame's `keyboard_input_system` clears `just_pressed` before `handle_toggle_key` (Update)
runs. The live verification therefore exercised the **modal** branch of the same gate
system; the console branch differs only in reading `ConsoleState.open` instead of
`State<ModalMenuState>`. The owner's windowed repro (real key events → real console) follows
the identical code path.

## Findings

**F1 — bug_0007 fixed and verified** (modal branch live; console branch shared-code,
see limit above). The gate is BEI's own mechanism, not a fork: `ContextActivity`
documentation ("similar to hiding an entity instead of despawning") is exactly this use
case. Worth internalizing: BEI has no run-condition-level input lock — observers and
continuous context consumption need separate gates, and `add_observers_run_if!` alone will
always leak the latter.

**F2 — `game/keyboard` cannot drive `just_pressed` consumers headlessly** (new harness gap,
same family as the Enter/`PrimaryWindow` limitation): the mock writes `ButtonInput` in
`RemoteLast`; per-frame `clear()` wipes `just_pressed` before any `Update` system runs.
Level-triggered consumers (`pressed`) work; edge-triggered ones (chill's console toggle)
are unreachable. Filing nothing — it is a verification limit, not a game bug — but the
skill should record it.

**F3 — NEW bug found: `--no-common-assets` client panics when the pause modal opens**
(`spawn_modal_menu` → `input_icons.rs:463` expects the keyboard/mouse icon atlas that this
mode omits). The first launch used `--no-common-assets`, Escape opened the modal, and the
client died. Filed as **bug_0008** (`Open`); re-verified the full run on a full-manifest
client. The modal's control tips are the first optional-furniture consumer that assumes an
atlas handle is present rather than degrading.

**F4 — `ContextActivity` also blocks `game/input`'s action mocks.** BEI deactivates "action
updates from inputs and mocks" — so while the console/modal is open, action-level mocks
freeze too (device-level mocks keep flowing into `ButtonInput`). Playtests that need
movement must close the console/modal first; no harness change needed.

## Next steps

1. **bug_0008** — `--no-common-assets` modal panic (degrade the icon lookup like every other
   optional-furniture consumer).
2. **Spawn-point separation** (playtest 0018 design item) — unchanged.
3. Disconnected-player cleanup — unchanged.
4. Replay movement-rate mismatch — unchanged.

## Conclusion

The reported input leak is fixed: gameplay input freezes while the console or pause modal
owns the keyboard and releases cleanly on close, live-verified through the real replicated
pipeline on a rendered headless client. The gating convention in AGENTS.md now covers the
continuous input path (`gate_replicated_input_context` via BEI's `ContextActivity`), the
defect is filed as bug_0007, and the verification run surfaced one new independent defect
(bug_0008) plus a documented harness limitation (F2).
