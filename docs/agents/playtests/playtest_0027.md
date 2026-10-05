# Agent Playtest 0027 — bevy_markup Custom Elements Replace the Client's Build Hooks

| Field | Value |
|---|---|
| Date | 2026-10-05 20:55 – 21:00 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0026, and this change: `is="…"` elements in `crosshair.html`/`hotbar.html`/`controls_tips.html`/`nameplate.html`/`tui_panel.html`, `define_html_element` definitions in `ui/{hud,modal_menu,nameplate,tui_panel}.rs` replacing their `HtmlUiBuilt` observers, `NameplatesVisible` reflected); bevy_markup path dependency at `9721039` + uncommitted `src/custom_elements.rs` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest), restarted once after the `NameplatesVisible` change |
| Server | an already-running `target/release/p19-server` (pid 1350053, not started by this session; ports 6001/15711 were taken, so this run joined it instead of starting one) |
| Level | `levels/minimal.level.ron` (already loaded on that server) |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

bevy_markup now supports customized built-in elements: `<div is="name" data-…>` runs the system
the app defined with `app.define_html_element("name", system)` (input `In<ElementConnected>`:
entity, UI root, `data-*` dataset) on every spawn of the element, before `HtmlUiBuilt`. The
client's five "re-attach after every rebuild" observers became definitions:

| Element | Definition | Attaches |
|---|---|---|
| `crosshair.html` ring | `crosshair-gcd-ring` | `CrosshairGcdRing`, `MaterialNode<CrosshairGcdMaterial>`, hidden |
| `crosshair.html` dot | `crosshair-dot` | `CrosshairDot` |
| `hotbar.html` `.hotbar-gcd` | `gcd-overlay` | `MaterialNode<GcdOverlayMaterial>` |
| `controls_tips.html` icon | `input-icon` (`data-icon="<name>"`) | the atlas `ImageNode` (was parsed from an `icon-<name>` class) |
| `nameplate.html` fill | `nameplate-fill` | `HealthFill` + width from the root's `Nameplate` |
| `tui_panel.html` fill | `tui-gauge-fill` | `TuiGaugeFill` + the current gauge width |

## Verification

| Step | Observed |
|---|---|
| Main menu, `.tui-gauge-fill` `Node.width` 0.5 s apart | `0.49%` → `3.66%` (marker attached; per-frame update finds it) |
| In game, idle | `crosshair-ring` `Hidden`, `crosshair-dot` `Inherited` |
| `spawn_cube` (starts the GCD), +0.12 s | ring `Inherited`, dot `Hidden`; screenshot shows the ring material; after the GCD back to dot |
| Pause (Start) | 14/14 `.controls-tip-icon` elements carry an `ImageNode`; glyphs render |
| NPC attacked to 51 HP, nameplates shown via BRP (`world.insert_resources NameplatesVisible true`) | its fill `51%` |
| That plate rebuilt (BRP: `HtmlDebugOutline` inserted, then removed) | old fill entity gone; the new fill entity is `51%` at once (CSS says `100%`, and `track_nameplates` only writes on a health change, so the definition set it) |

![crosshair_gcd_ring.png](screenshots/playtest_0027/crosshair_gcd_ring.png)

*GCD ring (the `crosshair-gcd-ring` definition's material) right after a spawn.*

![controls_tip_icons.png](screenshots/playtest_0027/controls_tip_icons.png)

*Controls tips with `is="input-icon"` glyphs.*

![nameplate_fill_rebuilt.png](screenshots/playtest_0027/nameplate_fill_rebuilt.png)

*The damaged NPC's plate after a forced rebuild: half-full bar.*

## Findings

**F1 — Every converted hook works and survives rebuilds.** Covered above; the HUD, pause menu,
nameplate and main-menu TUI behave as before.

**F2 — Playtest 0022 F6 partly resolved: `NameplatesVisible` is reflected.** BRP can now show
nameplates (`world.insert_resources`), which this run needed to see the fill. `NpcUiQuadMesh`
and `InGameRoot` remain unreflected.

**F3 — Leftover state on a long-running server** (known gap, "Disconnected players are never
cleaned up"): 36 nameplates for leftover player characters, and a second NPC spawn was refused
by the overlap check because the previous session's NPC stood on the spawn spot.

**F4 — Not converted: the selector's `focus_built_popup`.** It focuses a computed slot after a
popup rebuild. Restore-by-`id` + `autofocus` can't express wheel paging (focus moves to the edge
row, not the remembered `id`), so it stays an `HtmlUiBuilt` observer. The nameplate's fade-base
recording also stays on `HtmlUiBuilt`/`HtmlUiRestyled` until bevy_markup has `opacity`.
