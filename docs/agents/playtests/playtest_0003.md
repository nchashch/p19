# Agent Playtest 0003 — Diagnose and Fix the Headless Camera Rendering Bug

| Field | Value |
|---|---|
| Date | 2026-09-22 23:00 – 2026-09-23 00:15 UTC (multiple build/run cycles) |
| Commit | Started at `329c9bc` "Add playtest.md skill" (the still-broken state reproduced in [A follow-on issue found, attempted, and reverted](#a-follow-on-issue-found-attempted-and-reverted)'s regression screenshot and earlier in this run); the fix landed as `e5fe6c6` "Make progress on fixing –mcp client rendering bugs", committed after this run's verification – no single commit covers this whole run, since it documents behavior on both sides of a code change made during it, not a static state |
| Agent | Claude (Sonnet 5), driving the tool API over loopback HTTP, editing code between runs |
| Client | `target/debug/client --mcp` – dev profile + `dev-tools` feature, rebuilt repeatedly |
| Server | `target/release/server`, restarted alongside each client rebuild |
| Level | `levels/minimal.level.ron` ("Minimal level") |
| Transports | game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710) |
| Files changed | `client/src/controls/camera.rs`, `client/src/main.rs`, `AGENTS.md`, `docs/agents/skills/playtest.md` |

## Purpose

Requested directly by the project owner, following up on playtest 0002's confirmation that
in-game rendering was still broken in `--mcp` mode: "try to chase down and fix the camera
rendering bug in the `--mcp` client. The main menu, the lobby, and the in game state are
supposed to render things." This report covers the whole investigation arc – diagnosis, a real
fix, an attempted follow-on fix that was tried and reverted, and the final verified state –
not just the successful end state, per the "record observations/concerns, not only confirmed
findings" guidance added to `docs/agents/skills/playtest.md` earlier this session.

## Root cause

Found via a direct read of `bevy_render::camera::camera_system`
(`bevy_render-0.19.0/src/camera.rs`), not by trial and error: every camera in this app spawns
pointed at the default `RenderTarget::Window(Primary)`. Headless mode never has a primary
window (`WindowPlugin { primary_window: None, .. }`), so `RenderTarget::normalize` returns
`None` for any camera still in that state, and `camera_system` silently skips its **entire**
per-camera body – including the `Camera.computed.target_info` recompute – with no error.
Critically, merely running that `Query` item still consumes the camera's one-tick `is_added()`
window even though the skipped body never reads it. By the time `retarget_cameras_to_offscreen`
(this project's own system, `client/src/controls/camera.rs`) gets around to fixing a given
camera's `RenderTarget` – anything not present at `Startup`, i.e. the player's camera and every
loaded world's background camera – `is_added()` has already gone false, and nothing in
`camera_system`'s recompute gate (window/image asset events, `is_added()`, projection change,
viewport-size change) ever fires again for that camera. `target_info`, and thus its render
output, stays permanently unresolved.

This was **also** the real cause of the separately-documented "KTX2 skybox/TAA/SSAO kill offscreen
rendering" finding from earlier sessions – that bisection was run against cameras whose
`target_info` was already permanently broken for this unrelated reason, not a real skybox- or
antialiasing-specific bug. Confirmed by testing: with the fix in place, the skybox, TAA, and SSAO
all render correctly unstripped, and the strip-them-in-headless-mode workaround (previously in
`client/src/main.rs`) was removed entirely.

## The fix

One line, in `retarget_cameras_to_offscreen`: call `projection.set_changed()` right when fixing
a camera's `RenderTarget`, forcing `camera_system`'s `camera_projection.is_changed()` recompute
condition to be true on the very next pass, regardless of the `is_added()`/`AssetEvent` race
above. No value mutation needed, just the change-detection flag.

![1790122139920-final2-mainmenu.png](screenshots/playtest_0003/1790122139920-final2-mainmenu.png)

*Main menu after the fix: the real `.glb` background renders (spheres, a glowing panel, a mug – 3332 unique pixel colors, up from 292 before), with the UI panel on top.*

![1790122166331-final2-ingame.png](screenshots/playtest_0003/1790122166331-final2-ingame.png)

*In-game after the fix: the real starfield HDRI skybox renders (was: a single flat clear color for the whole frame). The floor is black – see [A separate, unrelated finding: the "minimal" level has no light](#a-separate-unrelated-finding-the-minimal-level-has-no-light), a separate, unrelated finding.*

## A follow-on issue found, attempted, and reverted

*Fixed in `playtest_0004`, once the real cause (a `bevy_ui` `IsDefaultUiCamera` ambiguity,
not an ordering problem) was found – see that report. The section below is kept as-recorded,
including the three approaches that didn't work and why, since that's exactly the kind of thing
worth not re-attempting blind.*

Fixing `target_info` surfaced a second, previously-latent problem: once the menu/lobby
background actually renders, it draws **over** the UI panel instead of under it, since the
background camera arrives on a later frame than the UI camera and, per the existing ordering
scheme (`camera.order` increases with arrival, most-recent-wins), draws on top of it with no
clear.

![1790121136105-orderfix-mainmenu.png](screenshots/playtest_0003/1790121136105-orderfix-mainmenu.png)

*Before this was reverted: the UI panel correctly composited on top of the real background, via bumping the UI camera's order to always stay above whatever last claimed.*

Three variations on "make the UI camera draw last" were tried, in order:

1. Give the UI camera a permanent `ClearColorConfig::Default` (since it happened to claim order 1
  first) and bump only its **order** upward each time something else claimed. This left it both
  highest-order **and** still clearing – it wiped the entire shared target after everything else
  had already drawn, regressing in-game rendering to a single flat clear color again.
1. Re-derive clear ownership fresh every tick instead – "whichever currently-claimed camera has
  the lowest order gets `Default`". Confirmed correct via BRP every time (the right camera had
  the right `clear_color`/`order`), but in-game rendering stayed blank regardless.
1. Give the UI camera a large fixed sentinel order once, never reassigned, to sidestep repeatedly
  touching `camera.order`. Also confirmed "correct" configuration via BRP, and also still
  produced blank/stale rendering in-game – worse, in one run it showed the **previous** (lobby)
  frame's content frozen in place rather than either a clean render or a flat color, meaning the
  player camera wasn't drawing at all despite reporting `is_active: true` and a resolved
  `target_info`.

![1790121537326-v3-ingame.png](screenshots/playtest_0003/1790121537326-v3-ingame.png)

*Attempt 2 (re-derive clear ownership every tick), in-game: config confirmed correct via BRP (player camera order 8 `Default`, UI camera order 9 `None`), yet still a single flat clear color – the concrete evidence that this wasn't actually about clear-ownership logic at all.*

None of the three were kept. All regressed the higher-value "renders at all" fix to chase a
cosmetic ordering issue, and the third attempt's evidence (BRP-correct configuration, still
broken rendering, in one case showing frozen stale content) points at something downstream of
`camera.order` and `clear_color` – suspected but **not confirmed**: frustum/visible-entities
computation, which likely carries a similarly-gated recompute condition to the one this
session's actual fix targets, and which isn't directly BRP-inspectable to confirm, since
`bevy_camera::visibility::VisibleEntities` is `#[reflect(ignore)]`. Reverted cleanly back to the
known-good ordering scheme (`client/src/controls/camera.rs` now matches the pre-attempt version
except for the `target_info` fix itself); `AGENTS.md` and `docs/agents/skills/playtest.md` both
document this as a known, deliberately unfixed follow-on rather than silently dropping it.

## A separate, unrelated finding: the "minimal" level has no light

While verifying the fix, the in-game floor rendered solid black. Confirmed via BRP this is
**not** a rendering bug: the floor's `Mesh3d`/`MeshMaterial3d` are both present (`code: -23402` –
present but unserializable over BRP, not absent) at a normal `GlobalTransform`, with
`Visibility: Inherited`. Separately, `world.query` for `PointLight`/`DirectionalLight`/
`SpotLight`/`AmbientLight` all returned `[]` – zero light entities exist anywhere in
`levels/minimal.level.ron`'s loaded content (`server/assets/rigs/minimal_level.glb` has no
`KHR_lights_punctual` extension at all; the client-visible `rigs/visuals.glb` it Skein-references
has none either). This would render identically black on a windowed desktop client – a
content-authoring gap in this specific level, not investigated further since it's outside the
rendering-pipeline bug this run was chasing.

## Findings

- **Root cause found and fixed**: `camera_system`'s `target_info` recompute gate silently starves
  any camera retargeted after `Startup` in headless mode. One-line fix
  (`projection.set_changed()`), verified across main menu, lobby, and in-game.
- **Two previously-separate-seeming bugs were actually the same one**: the "KTX2 skybox/TAA/SSAO
  kill offscreen rendering" finding from an earlier session was this same bug, not a distinct
  wgpu/asset-format issue. The skybox/TAA/SSAO strip workaround in `client/src/main.rs` is
  removed.
- **A real follow-on ordering bug exists and is intentionally unfixed**: UI renders under the
  background in menu/lobby in `--mcp` mode specifically. Three fix attempts, all reverted –
  see [A follow-on issue found, attempted, and reverted](#a-follow-on-issue-found-attempted-and-reverted). Worth revisiting with render-world instrumentation (e.g. a temporary
  extract-schedule system logging `VisibleEntities` counts) rather than only BRP-probing main-
  world `Camera` state, which reported "correct" even on runs where nothing rendered.
- **Unrelated finding, not a bug**: `levels/minimal.level.ron` has no light source anywhere in its
  content – floor renders black on any client, not just headless. See [A separate, unrelated finding: the "minimal" level has no light](#a-separate-unrelated-finding-the-minimal-level-has-no-light).
- **Process note**: this report covers roughly six build-rebuild-test cycles across \~75 minutes,
  consolidated into one report per this session's own updated guidance (`docs/agents/skills/playtest.md`
  §10) rather than filed as six separate ones, since they were one continuous investigation
  arc, not six independent playtests.

## Artifacts & bookkeeping

- Screenshots: `docs/agents/playtests/screenshots/playtest_0003/` (LFS-tracked, a representative subset
  – final working state plus the clearest before/after pair for the reverted ordering attempt),
  plus the tool's raw capture staging directory `docs/agents/playtests/dist/screenshots/` (gitignored,
  every capture from every cycle this run).
- Tool API surface used: `game/trigger`, `game/select_level`, `game/state`,
  `game/screenshot[+ /get]`, plus extensive direct `world.query`/`world.get_components` BRP
  calls – see `client/src/dev/tool_api.rs`, ADR 0009, and `docs/agents/skills/playtest.md`.
- Living documentation updated this session: `AGENTS.md`'s `--mcp` gap-list bullet (rewritten:
  the "frozen investigation" is fixed, the ordering follow-on and the floor-lighting finding are
  now documented); `docs/agents/skills/playtest.md` §7's "known visual divergences" (rewritten to
  match); `docs/agents/skills/playtest.md` §10 (the "file a report every session" rule, added earlier
  this session before this run started).
