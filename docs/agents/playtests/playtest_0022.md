# Agent Playtest 0022 — bevy_markup UI Migration: Every Surface Driven Through Mouse, Gamepad and Enter

| Field | Value |
|---|---|
| Date | 2026-10-05 05:47 – 06:05 UTC |
| Commit | `ecf2349` "Replicate Transforms" + the uncommitted bevy_markup UI migration (ADR 0015): `crates/client/src/ui/**` (incl. new `ui/markup.rs`, `ui/html/*`), `dev/tool_api.rs` (`game/ui`), `main.rs`, `controls/actions.rs`, `assets/collections.rs`, `events.rs`, `lifecycle/{lobby,networking}.rs`, workspace + client `Cargo.toml`, `Cargo.lock` (bevy 0.19.1), deleted `crates/client/build.rs`, `ui/widgets.rs`, `ui/framework.rs`. The controls-tips icon fix (F1) landed mid-run; the rows below say which build each check ran on |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | `target/debug/p19-client --mcp` (dev-tools, rendered on an RTX 4070 SUPER, full manifest), then `target/debug/p19-client --no-render` (implies `--no-common-assets`) |
| Server | `target/release/p19-server`, restarted fresh before the final `--mcp` run |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

The owner asked for every client UI surface to be reimplemented on `bevy_markup`, their own
HTML/CSS/Fluent crate (ADR 0015). This run drives each migrated surface the way a player would,
and checks that no surface rebuilds itself every frame. Inputs used: real pointer clicks
(`game/mouse`), gamepad navigation and confirm (`game/gamepad`), and keyboard Enter/Tab
(`game/keyboard`).

## Method

A throwaway Python driver (not committed) called the BRP methods in sequence and printed
`game/ui` button labels and texts. For focus it queried the focused element with
`world.query` (`bevy_markup::html::HtmlElement` filtered `with` `bevy_ui::ui_node::Outline`;
the focus ring is an `Outline`). For stability it took two `HtmlElement` entity sets 0.5 s apart
and counted the survivors.

## Verification

| Step | Observed |
|---|---|
| Main menu boot | `game/ui` buttons: `Connect, Options, Credits, Quit, Language`; `Connect` holds focus (`autofocus`), ring hidden until directional input |
| DPadDown ×2, DPadUp ×2 | focus `options` → `credits` → `connect` |
| Click `Language`, click `日本語` | the popup lists `English, Русский, 日本語`; after the pick, every label reads `接続, オプション, クレジット, 終了, 言語` and the TUI panel text is Japanese too |
| Gamepad South on the focused `Language` toggle, DPadUp ×2, South | the popup reopens on the last pick; focus `slot-0`; English restored |
| Hover `Connect` | a `game/ui` text row `Connect to the server.` (tooltip) |
| Click `Connect` | `Lobby`; buttons `Play, Level, Main Menu` |
| Click `Level`, click `Minimal` | the popup row `Minimal` (a Fluent key, localized); the pick sends `LoadLevelRequest`; focus returns to the `level` toggle |
| DPadUp, South | focus `play` → `InGame`, player `Brisk Ash` at (0, 0.915, 0), 100 HP |
| Tab | data frame: `Press ` for console`, `HP: 100/100`, `Damage: 49`, `Attack range: 10m`, `Select range: 50m`, `GCD: 0.5s`, `Grounded: true`, `Hovered: n/a`, `Distance: n/a` |
| Start | buttons `Main Menu, Resume`; focus `pause-resume`; tips texts `Move` … `Stats` (11 rows) |
| DPadUp / DPadDown / South | `pause-main-menu` / `pause-resume` / resumed (`InGame`, no UI buttons) |
| Start, DPadUp, South | back to `MainMenu`, focus `connect` |
| Options popup (6 stub rows), DPadDown ×7 | slots 1–4, then the window pages to `B … F` and focus stays on `slot-4` (edge paging) |
| Reopen, wheel up ×2 / down ×1 | the window resumes at the last pick, `A … E` with focus `slot-0`, then `B … F` with `slot-4` |
| `--no-render` client: menu, gamepad Connect, South Play, Start | menu, lobby, game and pause all work; 11 tip labels shown label-only (no atlases, no panic: bug_0008); `game/keyboard` Enter resumes |
| Rebuild stability, final build | main menu 13/20 (the 7 changing entities are the TUI panel's once-per-second readout, by design), lobby 10/10, in-game + data frame 19/19, pause 61/61, `--no-render` pause 51/51 |

![main_menu.png](screenshots/playtest_0022/main_menu.png)

*Main menu: the bottom-left panel (Connect is `.button.primary`) and the top-right TUI panel ported from ratatui.*

![language_popup.png](screenshots/playtest_0022/language_popup.png)

*The Language selector popup: its own `HtmlUi` root placed right of the toggle, with the discrete scrollbar on its right edge.*

![main_menu_ja.png](screenshots/playtest_0022/main_menu_ja.png)

*After picking 日本語: `ActiveLocale` follows `Locale` and every `data-l10n-id` re-localizes.*

![tooltip.png](screenshots/playtest_0022/tooltip.png)

*Hover tooltip (`data-on-enter="tooltip"`). The gold outline on Language is the focus ring left by the previous gamepad pick.*

![level_popup.png](screenshots/playtest_0022/level_popup.png)

*Lobby Level selector. The toggle's tooltip uses `tooltip_above`, so it draws above the toggle and covers Play while the pointer stays on Level.*

![data_frame.png](screenshots/playtest_0022/data_frame.png)

*In-game: the crosshair dot (centre) and the Tab data frame (400×400 at top-right, as before).*

![pause.png](screenshots/playtest_0022/pause.png)

*Pause menu (Main Menu above Resume, Resume focused) and the controls tips with atlas icons, after the F1 fix.*

![nameplates.png](screenshots/playtest_0022/nameplates.png)

*Nameplate over an NPC (name + health bar). Taken from a throwaway build with `NameplatesVisible(true)` as the default (reverted afterwards), because the console toggle can't be driven headlessly (playtest 0021 F2).*

## Findings

**F1 — Runaway rebuild loop from an app `ImageNode` on a built element (found and fixed in
this run).** The first build attached the controls-tips icons as `ImageNode`s directly on
bevy_markup elements. The tips then rebuilt every frame, and `game/ui` never listed their
labels: 11 tip-label entities were replaced within 0.5 s, and ids reached generation-sized
values. Cause: bevy_markup's restyle-in-place check (`build.rs::same_shape`) compares
`ImageNode` presence. The `PseudoState` insert after every build triggers a restyle, the shape
check fails on the app-inserted image, and that forces a full rebuild, which attaches the icons
again. Fix: each icon now sits on a child that is itself an empty `HtmlUi` (`icon.html`), and
the shape check skips nested UIs. After the fix the pause menu was 61/61 stable and the tips
appeared in `game/ui`. This is worth fixing upstream in bevy_markup; AGENTS.md now carries it
as a rule.

**F2 — The old Enter ceiling is gone.** `game/keyboard` Enter resumed the game from the pause
menu on `--no-render`. Every button is now confirmed through `UiConfirm` (South + Enter), with
no `PrimaryWindow` dependency. This retires playtest 0021's and the skill's "literal Enter can
never activate a FeathersButton" note.

**F3 — bug_0008 is fixed.** The pause menu opened under `--no-render` (no icon atlases) with no
panic, and its rows rendered label-only.

**F4 — No NPC sign quad was seen; the cause is not established.** `npc_ui_quad.rs` builds its
`npc_sign.html` into the texture: `HtmlElement`s `npc-sign`/`npc-sign-label` exist on the
texture camera, and the label is 96×77. But the spawned NPC never got its quad child.
`world.list_components` on the NPC shows no `ChildOf` and no `Decorated`, and the client logged
`B0004 … has a parent (the NPC entity) without InheritedVisibility`. So
`gameplay/npc_spawner.rs::decorate_npcs` (unchanged by this migration) never ran. Its gates are
`resource_exists::<NpcUiQuad>`, `Res<CommonAssets>` and `Single<InGameRoot>`. `InGameRoot`
isn't `Reflect`, so BRP couldn't count it. Hypothesis (not confirmed): the `Single` fails.
Discriminating experiment: log the `InGameRoot` count on the client, or run the same spawn on
`ecf2349` without the migration.

**F5 — `Two or more Entities with IsDefaultUiCamera` warning on joining InGame**, logged once.
None of the migrated code inserts `IsDefaultUiCamera` (`controls/camera.rs` owns it); not
chased.

**F6 — Tool-API gaps hit.** `NpcUiQuadMesh`/`InGameRoot`/`NameplatesVisible` aren't reflected,
so BRP can't see the quad, the root count or the nameplate toggle. `game/screenshot {"camera": id}`
is ignored outside `--headless-render`, so the NPC sign's render texture can't be captured
directly.

## Not covered

- Windowed client and VR wrist panels (no headset).
- The hotbar, which is not spawned (`HOTBAR_ENABLED = false`).
- The crosshair GCD ring, which needs a GCD-starting action.
- The Russian locale (only English ↔ Japanese was switched).
