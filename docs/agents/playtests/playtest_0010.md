# Agent Playtest 0010 — Optional CommonAssets Furniture / –no-common-assets

| Field | Value |
|---|---|
| Date | 2026-09-23 17:23 – 17:32 UTC |
| Commit | `5e71215` "Implement Skein mesh primitives for agent testing" — HEAD throughout; the work-in-progress `CommonAssets` changes uncommitted |
| Agent | opencode session, GLM-5.3-Flash, driving the tool API over loopback HTTP |
| Client | `target/debug/client --mcp` (plus one `--no-common-assets` pass), `BEVY_ASSET_ROOT=$PWD/playtest_assets/playtest_0009/client` (and `$PWD/client` for the production check) |
| Server | `target/release/server`, `BEVY_ASSET_ROOT=$PWD/playtest_assets/playtest_0009/server` (and `$PWD/server` for the production check) |
| Level | `levels/playtest.level.ron` (playtest 0009's level) |
| Asset isolation | playtest 0009's tree, now stripped to **100% plaintext** — the furniture copies (fonts, WAVs, KTX2 skybox, atlas PNGs) deleted; manifest trimmed to the five world keys only |
| Transports | game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710) |

## Purpose

Playtest 0009 ended with a proposal: the only non-text files an isolated playtest still
carried were **engine furniture** `CommonAssets` required (two font slots, two WAVs, the KTX2
skybox, two atlas PNG pairs), and "making them optional would need a feature-gated
`CommonAssets` variant" (playtest 0009's findings). This playtest implements and verifies
exactly that — not feature-gated, but **manifest-driven**:

- The 9 furniture fields of `CommonAssets` became `Option<Handle<T>>` with
  `#[asset(key = "…", optional)]` on each: a manifest that **omits** a key resolves the field
  to `None` (confirmed against `bevy_asset_loader` 0.27's derive — `AssetKeys::get_asset`
  returns `Option`, and the derive's `optional` attribute wraps the build in `.map(…)`),
  while the production `client/assets` manifest still lists every key, so production
  behavior is unchanged.
- Every consumer degrades gracefully on `None`: the skybox `Option` passes straight through
  (`Skybox.image` is itself an `Option` — the pass just doesn't run); sounds are skipped
  (`if let Some` before spawning the `SamplePlayer`); fonts fall back to
  `Handle::<Font>::default()` — Bevy's **embedded** default font, exactly the reasoning
  playtest 0009's owner proposed; the icon atlases build an empty `IconAtlas` fallback whose
  every lookup misses (icon quads absent, labels still render).
- A new CLI flag `--no-common-assets` (pre-sync, like `--mcp`) is the even-bareer mode: no
  loading state is registered at all (no manifest read), a `CommonAssets::placeholder()`
  resource is inserted instead (dangling world handles, `None` furniture), and a `Startup`
  system performs the `AssetLoading → MainMenu` transition the loading state's completion
  normally would. The dev console's own hardcoded font path is also suppressed under the
  flag (`font_path: None`), so no font file needs to exist in the asset root at all.

## Verification

Three fresh server+client pairs, all driven through the canonical `connect` →
`game/levels` → `game/select_level` → `game/trigger play` flow:

### Playtest 0009's tree, stripped to 100% plaintext (no flag) — the recommended playtest mode

Deleted `playtest_assets/playtest_0009/client/assets/{fonts,audio,skyboxes,textures}/` and
trimmed its manifest to only the five world keys. `find playtest_assets/playtest_0009
-type f` now lists only text files (`.gltf` JSON, RON, TOML, FTL, WGSL, YAML). The whole
loop still works: menu, lobby, in-game, player spawned and grounded. As expected:

- The in-game view renders the level's floor + floating sphere (the `ClientWorldAsset`
  visuals) with the **clear color** behind them — the KTX2 skybox is gone, and
  `Skybox.image: None` simply skips the skybox pass:

![1790184263865-stripped-ingame.png](screenshots/playtest_0010/1790184263865-stripped-ingame.png)

*In-game on the fully-plaintext assets. Floor + sphere render (flat-shaded grays under the ambient light); behind them is the app's clear color `(26, 26, 38)` — 29% of the frame **exactly** that color, vs 0% in production — where the starfield skybox would be. The white crosshair still composites (44 px of pure white dead center).*

- Menu and lobby render with the UI text in Bevy's embedded default font (the manifest no
  longer names any font), over the `MeshPrimitive` sphere backdrop:

![1790184333674-stripped-menu.png](screenshots/playtest_0010/1790184333674-stripped-menu.png)

*`MainMenu` on the stripped assets: UI panel, button labels, and the sphere backdrop all render without any font/image furniture in the asset root.*

### `--no-common-assets` — the barest boot

Same stripped tree, client launched with the flag. The manifest is never read at all. The
client boots straight to `MainMenu` (the `Startup` transition fires), the menu UI renders
against the placeholder — **no** background world (its handle is dangling by design), and
unlike the no-flag pass, not even a `Path not found` error appears in the log (the console
font suppression at work):

![1790184455686-flag-menu-barest.png](screenshots/playtest_0010/1790184455686-flag-menu-barest.png)

*`--no-common-assets` menu: full feathers UI over bare clear color — the sphere backdrop is gone (82.6% of the frame is the exact clear color), and the UI still works. In-game (see the next figure) the level visuals still arrive because `ClientWorldAsset`s load **by path**, not through the manifest.*

![1790184485475-flag-ingame.png](screenshots/playtest_0010/1790184485475-flag-ingame.png)

*`--no-common-assets` in-game: the playtest level's floor + sphere render exactly as in the no-flag pass, crosshair included — the `ClientWorldAsset` path is fully independent of `CommonAssets`.*

### Production regression check (`client/assets`, no flag)

The owner's manifest still lists every key, so all furniture resolves to `Some` and nothing
should change. Verified: the full loop works, **zero** "input-icon atlas omitted" warns, and
the in-game view has **zero** clear-color pixels — the starfield skybox renders exactly as
before (2891 unique colors vs 101 in the stripped pass):

![1790184631604-prod-ingame.png](screenshots/playtest_0010/1790184631604-prod-ingame.png)

*Production assets after the `Option`-ification: starfield skybox, level floor, HUD — indistinguishable from pre-change behavior, as intended.*

## Findings

- **The optional-furniture design works end-to-end in all three modes** (stripped manifest,
  `--no-common-assets`, production manifest): the degradation map from the design held
  without surprises — fonts to the embedded default, skybox pass skipped, sounds skipped,
  icon atlases to an empty fallback (its `warn!` lines observed in both degraded modes and
  absent in production), console font suppressed under the flag.
- **One derive subtlety worth remembering**: `bevy_asset_loader`'s `Option<Handle<T>>` field
  type alone is not enough — without the `#[asset(key = "…", optional)]` attribute the
  derive generates a plain, non-optional load and the build fails with
  `expected Option<Handle<…>>, found Handle<_>` (one error per distinct handle type, all
  pointing at the derive site). The first attempt omitted the attribute and had to be
  corrected.
- **The manifest's `lobby_background` key is dead in every mode** — pre-existing, not this
  change's doing: `CommonAssets.lobby_background` is **aliased to the `menu_background` key**
  in `collections.rs` (documented in AGENTS.md for the production manifest; the playtest
  manifest's distinct `lobby_background` entry — the torus — is therefore never read). The
  handoff's expectation "lobby with the torus" was accordingly wrong: menu and lobby both
  render the sphere. The two captures differ **only** in the UI panel region (pixel-diff
  bbox `(61, 285)–(261, 441)`); the backdrop is pixel-identical. Worth un-aliasing the field
  someday if the lobby is ever meant to have its own backdrop.
- **Connect-time `Disconnected` re-entry (pre-existing, both modes)**: immediately after
  `on_connect_request`, an "disconnected from server" log line fires and the client
  re-enters `GameState::MainMenu` (observable as `finalize_input_icon_atlases` running a
  second time on `OnEnter(MainMenu)`). Identical in the flag and no-flag runs and unaffected
  by this change — the connect then proceeds normally (Connected → Lobby \~140 ms later).
  Not chased further; noted here so a future session doesn't mistake it for a regression.
- **The `--no-common-assets` boot is quiet**: exactly one benign log line (the pre-existing
  ICU4X "No segmentation model for language: ja" warning) — no missing-path errors even
  though the asset root carries no fonts at all.
- **Remaining sounds/skybox degradation is observable only indirectly** — no assets means no
  `SamplePlayer` spawns (code-verified `if let Some`); no error/panic either way. The
  icon-atlas fallback and font fallback were verified by warn-absence/pixel inspection as
  described above.

## Artifacts & bookkeeping

- Screenshots: curated copies in `docs/agents/playtests/screenshots/playtest_0010/`; the full
  capture stream in the gitignored `docs/agents/playtests/dist/screenshots/`.
- The assets: `playtest_assets/playtest_0009/`, stripped in place to 100% plaintext (the
  furniture is trivially re-copyable from `client/assets` if 0009 ever needs re-running
  with it).
- Code (uncommitted, staged for the project owner): `client/src/assets/collections.rs`
  (`optional` fields + `placeholder()`), `client/src/controls/camera.rs`,
  `client/src/gameplay/combat.rs`, `client/src/ui/{hud,widgets,npc_ui_quad,input_icons}.rs`,
  `client/src/config.rs` (`is_no_common_assets_presync`), `client/src/main.rs` (flag branch),
  `client/src/dev/console.rs` (flag-suppressed font path).
- Docs updated on the same pass: `AGENTS.md` (the `--mcp`/tool-API area: the flag, the
  `Option` furniture fields, the fully-plaintext playtest-assets mode), and
  `docs/agents/skills/playtest.md` §10 (playtest assets can now be 100% plaintext; the furniture
  caveat from the 0009 era dropped).
