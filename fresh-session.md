# fresh-session.md — handoff for continuing the `--no-common-assets` work

You are picking up mid-task from a previous session that had to be abandoned for an
environment limit (see "Why a fresh session" below). Everything is saved on disk; nothing
was lost. Read this fully, then read `docs/skills/playtest.md` (the full playtest harness
playbook) and `AGENTS.md` (the architecture doc) as needed.

## Why a fresh session

The previous session accumulated ~20 `read` calls on screenshot PNGs. The harness embeds
image bytes (base64) into the conversation even when the model cannot process them, and the
transcript is re-sent with every request — the cumulative images (~13 MB encoded) exceeded
the provider's 8 MiB request-body limit and requests started failing.

**Rule for this session: NEVER call `read` on an image/PNG/PDF file.** Analyze screenshots
with python PIL instead (unique-color counts, top-color percentages — tiny text outputs).
This is also what `docs/skills/playtest.md` §7 recommends.

## The task: gate `CommonAssets` furniture behind `--no-common-assets`

Goal (project owner's request): a `--no-common-assets` CLI flag so playtest asset roots
(`playtest_assets/playtest_NNNN/client/assets/`) can be **100% plaintext** — no textures,
no fonts, no sounds, no skyboxes. Playtest 0009 (`playtest_assets/playtest_0009/`) is the
template: text `.gltf` scenes whose only content is Skein components + `MeshPrimitive`
visuals; it currently still carries copied "engine furniture" binaries that this task
removes the need for.

### Settled design (do not relitigate without cause)

1. **The 9 furniture fields of `CommonAssets` become `Option<Handle<T>>`**:
   `crunch`, `explosion`, `skybox`, `serif_font`, `noto_sans_jp_font`,
   `keyboard_mouse_atlas_image`, `keyboard_mouse_atlas_manifest`,
   `steam_deck_atlas_image`, `steam_deck_atlas_manifest`.
   The 5 world keys stay non-`Option`: `cube_world`, `rig_world`, `rig_gltf`,
   `menu_background`, `lobby_background`.
   Rationale: `bevy_asset_loader` resolves `Option` fields only if the key exists in the
   dynamic manifest — so a playtest manifest that simply **omits** the furniture keys gets
   `None` for them (graceful degradation) while still loading the text `.gltf` worlds.
   The normal `client/assets` manifest lists every key → production behavior unchanged.
2. **Degradation mapping** (all verified against bevy 0.19 semantics):
   - `serif_font`/`noto_sans_jp_font` = `None` → `Handle::<Font>::default()` is Bevy's
     **embedded default font** (ASCII) — `TextFont { font: handle.unwrap_or_default() }`
     renders fine. This is exactly the owner's "Bevy includes a default font" reasoning.
   - `skybox` = `None` → `Skybox.image` is already `Option<Handle<Image>>`; pass it
     through and the skybox pass simply doesn't run (clear color shows).
   - `crunch`/`explosion` = `None` → skip spawning the `SamplePlayer`.
   - atlas fields = `None` → build an empty/fallback `IconAtlas` (default image handle,
     empty index map) — the icon-lookup functions already use `.get(name)?` and return
     `None`, so icon quads degrade to absent/blank while labels still render.
3. **The `--no-common-assets` flag** (CLI-only, pre-sync, like `--mcp`): skips the whole
   `CommonAssets` collection load (no manifest read at all) and instead inserts a
   placeholder `CommonAssets` (world fields: dangling `Handle::default()`; furniture:
   `None`), plus an immediate `GameState::AssetLoading → MainMenu` transition (normally
   the loading state's completion does this). This is the "barest boot" mode — content
   then arrives only via the `ClientWorldAsset` path (which loads by path, not manifest).
   The **recommended playtest mode is actually WITHOUT the flag** (trimmed manifest keeps
   the MeshPrimitive world visuals); the flag is the extra minimal mode.

## State: done so far (uncommitted working tree)

- `client/src/assets/collections.rs`: the 9 fields are `Option`-ified (with a doc comment
  explaining the playtest-assets mode), and both font-override systems are adapted:
  `override_default_font` and `override_feathers_button_font` now early-return when
  `serif_font` is `None`.
- The build currently FAILS with the expected downstream type errors — that is the
  remaining work.

## Remaining work (exact sites)

1. `client/src/controls/camera.rs` ~line 46 (`fn skybox`): change
   `image: Some(common_assets.skybox.clone())` → `image: common_assets.skybox.clone()`
   (pass the Option through; `Skybox.image` is `Option<Handle<Image>>`).
2. `client/src/gameplay/combat.rs` lines ~36, ~41, ~55: wrap each
   `commands.spawn(SamplePlayer::new(common_assets.explosion/crunch.clone()))` in
   `if let Some(handle) = &common_assets.… { … }`.
3. `serif_font` clone sites → `.clone().unwrap_or_default()`:
   `client/src/ui/widgets.rs` ~243, `client/src/ui/npc_ui_quad.rs` ~124,
   `client/src/ui/hud.rs` ~202 and ~294.
4. `client/src/ui/input_icons.rs` `finalize_input_icon_atlases` (~250–270): make it
   `Option`-aware — when an atlas image/manifest pair is `None`, construct the
   `IconAtlas` fallback (default image handle, empty `indices` map) instead of the
   `.expect(...)` path. The private `IconAtlas` struct may need an `empty()` constructor.
5. `client/src/assets/collections.rs`: add a `CommonAssets::placeholder()` constructor
   (world fields `Handle::default()` — verify `Handle<A>: Default` exists in bevy_asset,
   else `Handle::Weak(AssetId::default())`; furniture fields `None`).
6. `client/src/main.rs`: parse the flag pre-sync (follow `config::is_mcp_mode_presync`'s
   CLI-first pattern in `client/src/config.rs`; CLI-only is fine). When set: skip the
   `.add_loading_state(LoadingState::new(GameState::AssetLoading)…)` registration for
   `CommonAssets`, `insert_resource(CommonAssets::placeholder())`, and add a `Startup`
   system that sets `NextState(GameState::MainMenu)`.
7. Optional nicety: `client/src/dev/console.rs` ~45 hardcodes the console font path
   (`fonts/mono/IBMPlexMono-Regular.ttf`, a direct load, not `CommonAssets`) — make
   `font_path` `None` when the flag is set so no font file is needed at all.
8. `cargo build -p client --features dev-tools` until `grep -cE "^error"` prints 0
   (**always verify the build this way** — a failed build leaves a stale binary and you
   test old code silently).

## Verification plan (after it compiles)

1. **Strip playtest_0009 to fully plaintext**: delete
   `playtest_assets/playtest_0009/client/assets/{fonts,audio,skyboxes,textures}/` and trim
   its `collections/common_assets.assets.ron` to only the five world keys
   (`cube_world`, `rig_world`, `rig_gltf`, `menu_background`, `lobby_background` —
   pointing at the existing text `.gltf` files). After this, `find
   playtest_assets/playtest_0009 -type f` should show only text files.
2. **Run WITHOUT the flag** (`BEVY_ASSET_ROOT` at the playtest client, server at the
   playtest server, per `docs/skills/playtest.md` §2): expect the menu with the sphere
   backdrop rendering (MeshPrimitive from text gltf), lobby with the torus, in-game floor
   + floating sphere + HUD crosshair, UI text in Bevy's default font, **no** starfield
   skybox (clear color instead — the ktx2 is gone), blank icon tips. Drive the full
   connect → `game/levels` → `game/select_level` → `game/trigger play` flow; take labeled
   screenshots; analyze with PIL (never `read` them!).
3. **Run WITH `--no-common-assets`**: expect the barest boot — menu UI without the
   background world, still reaching `MainMenu` (the placeholder + immediate transition),
   and in-game `ClientWorldAsset` visuals (the level's floor/sphere) still rendering
   because that path loads by path, not via the manifest.

## Bookkeeping after verification

- File `docs/playtests/playtest_0010/report.typ` (see `playtest_0009`'s for the house
  style), curated screenshots in `docs/playtests/screenshots/playtest_0010/`, an entry at
  the TOP of `docs/playtests/index.typ`, compile with
  `typst compile --root . docs/playtests/playtest_0010/report.typ docs/playtests/dist/playtest_0010.pdf`
  (never open/inspect the PDF). Typst gotchas learned the hard way: bold is *single*
  stars; a single-backtick code span must not wrap across a line break; `` \` `` does not
  escape inside raw text; images outside the report dir need `--root`.
- Update `AGENTS.md` (the `--mcp`/tool-API area): the flag, the `Option` furniture fields,
  the fully-plaintext playtest-assets mode, and `CommonAssets::placeholder()`.
- Update `docs/skills/playtest.md`: §5c/§10 — the flag, and that playtest assets can now
  be 100% plaintext (drop the "copied furniture" caveat from the 0009-era notes).
- All changes stay uncommitted; the project owner handles staging/commits.

## Runtime gotchas that bit the previous sessions (condensed)

- `pkill -x client` / `pkill -x server` — NEVER `pkill -f "debug/client"` (matches your
  own shell's command line and kills your own command).
- Write curl URLs/headers literally — zsh does not word-split unquoted variables, and
  `$H='-H …'` silently breaks curl.
- Launch with `BEVY_ASSET_ROOT=$PWD/playtest_assets/playtest_NNNN/{client,server}` (bevy
  appends `assets/` — the tree nests one level deeper than you first expect) and
  `CARGO_MANIFEST_DIR=$PWD/client` for the client binary; put a generous `timeout N` on
  the client (wall-clock includes your thinking time between tool calls).
- Restart BOTH server and client between rounds: a long-lived server accumulates zombie
  players whose replicated `ClientInGame` traps fresh clients in a broken InGame state.
- Input conventions (empirically verified): `look_pitch` **positive = up** (positive
  `pitch_delta` = look down); negative `yaw_delta` → `look_yaw` increases. Calibrate with
  a small injection + `game/state` before trusting any sign.
- BRP: components holding handles serialize as `null`/error `-23402` — that means
  **present**, not absent; `-23403` means genuinely absent. Entity ids: BRP prints
  `2^(gen+1) − (index+1)`-style u64s, logs print `index+gen` (`1224v1`).

## Pointers

- Full harness playbook: `docs/skills/playtest.md` (read §1–§5 and §10 before driving).
- Architecture, known bugs, conventions: `AGENTS.md` (top-of-file gap list + the `--mcp`
  and tool-API bullets).
- The experiment this continues: `docs/playtests/playtest_0009/report.typ` and
  `playtest_assets/playtest_0009/`.
- Still-open known bugs (do not fix unless asked): the caster-resolution regression (all
  client→server "act" messages silently dropped), `InGameRequest` dropped during
  `ServerState::Loading` + level-reload dedup, zombie-player cleanup,
  `return_to_main_menu`-adjacent items are FIXED — check AGENTS.md's gap list for current
  status before assuming either way.
