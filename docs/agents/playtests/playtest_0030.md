# Agent Playtest 0030 — In-Place UI Updates, `style` Attributes and `opacity`

| Field | Value |
|---|---|
| Date | 2026-10-05 22:31 – 22:34 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0029, and this change: `nameplate.html` uses `style="opacity: {{ alpha }}"` on its root and `style="width: {{ health }}%"` on the fill, `tui_panel.html` `style="width: {{ gauge }}%"`, `selector.html` `autofocus` on the selected row; `ui/nameplate.rs` lost `FadeBase`/`HealthFill`/the fade bookkeeping and both build/restyle observers, `ui/tui_panel.rs` its gauge marker and per-frame `Node` writes, `ui/selector.rs` `focus_slot`/`page_popup`/`focus_built_popup` — no `HtmlUiBuilt`/`HtmlUiRestyled` observers remain in the client); bevy_markup path dependency at `3c73cba` + uncommitted keyed in-place reconciliation, `style` attribute and `opacity` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | the already-running `target/release/p19-server` (pid 1350053, not started by this session) |
| Level | `levels/minimal.level.ron` (already loaded) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup no longer despawns and respawns a UI on content changes: the new document is
reconciled with the existing entities (matched by unique `id`, else by position), updating
kept elements in place. Elements accept `style="…"` (cascaded as in CSS) and CSS `opacity`
(group opacity: the subtree's colors fade). The client's per-frame visuals moved into
template values, and its last build/restyle hooks went away.

## Verification

Entity stability measured as `HtmlElement` entity sets taken apart; focus via `InputFocus`.

| Step | Observed |
|---|---|
| Main menu, TUI gauge `Node.width` 0.5 s apart | `100%` → `91%` (from `style`), same entity |
| All main-menu elements over 2.5 s (gauge and clock ticking) | 20/20 kept |
| Language → 日本語 → English | labels switch (`接続 … 言語` and back); 20/20 elements kept across the switch |
| Options popup | opens focused on `slot-0` (the selected row's `autofocus`) |
| DPadDown ×4, then ×1 at the bottom (edge paging) | rows page to `B…F`; focus stays on `slot-4`, **same entity** |
| Wheel up over the popup | rows `A…E`, focus `slot-0` |
| South (pick) | popup closed, focus on `options` |
| In game, nameplates shown (BRP), idle / walking away | 124/124 plate elements kept over 1 s, both cases |
| NPC attacked to 51 HP, walking back from z 23.9 to 48 | fill `51%` throughout; name alpha 1.0 → 1.0 → 0.82 → 0.5 → 0.18 → hidden; the same name/fill entities in every sample |
| Pause (Start) | `Main Menu, Resume`, focus on Resume; South resumes |

![nameplate_fading.png](screenshots/playtest_0030/nameplate_fading.png)

*NPC plate mid-fade (root `opacity` from the template) with its 51% bar (`style` width).*

![selector_paged.png](screenshots/playtest_0030/selector_paged.png)

## Findings

**F1 — Content updates keep entities.** Language switches, clock ticks, gauge animation, popup
paging and nameplate fades all update the existing elements; focus and hover survive (the
selector no longer needs to re-focus after paging).

**F2 — Per-frame template values only re-render on visible change** (values rounded to 1%;
hidden plates write nothing). Exercised with one visible fading plate (the other 31 targets
were out of range) and the TUI panel. Not measured: frame time with many plates fading at
once (no tool-API frame-time probe).

**F3 — Found and fixed during development (bevy_markup, never committed): duplicate `id`s
looped.** Keying siblings by `id` respawned the second of two equal ids on every update, and
the respawn's new `PseudoState` triggered the next restyle. Caught by the new property test
`content_update_matches_a_fresh_build`; only `id`s unique on both sides key now.
