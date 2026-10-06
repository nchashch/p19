# Agent Playtest 0036 — Nameplates on bevy_markup's `HtmlWorldAnchor`

| Field | Value |
|---|---|
| Date | 2026-10-06 02:15 – 02:17 UTC |
| Commit | `0da1fcb` "Use bevy_markup for UI" + uncommitted client UI changes (playtests 0023–0035, and this change: `ui/nameplate.rs` spawns plates with `HtmlWorldAnchor` and keeps only name/health/fade; the projection, visibility mirroring and target-gone cleanup moved upstream; `NameplatesVisible` and full fade-out are a `hidden` root class → `display: none`); bevy_markup path dependency at `9d84990` + uncommitted `HtmlWorldAnchor` / `HtmlWorldAnchorView` |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered, full manifest) |
| Server | `target/release/p19-server`, fresh |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

`HtmlWorldAnchor` keeps a UI root's pivot (default bottom center) over a 3D entity's position
plus an offset, hides it off screen / behind the camera / over an invisible target, despawns it
with the target, and reports `HtmlWorldAnchorView { distance, on_screen }`. The client's
nameplates moved onto it.

## Verification

Plates read with `world.query` (`HtmlWorldAnchorView`, `Node`, `Visibility`, `ComputedNode`).

| Step | Observed |
|---|---|
| NPC spawned, `NameplatesVisible` false | NPC plate `display: None` (anchor still measuring: distance 4.1, on screen) |
| Toggle on (BRP) | `display: Flex`, `left 610 = 640 − 60/2`, `top 132 = 162 − 30` (bottom center on the projected head) |
| Own player's plate | distance 1.0, off screen → `Hidden` |
| Walking back 4 → 28 m from the NPC | plate shown up to 27 m, `display: None` at 32 m (fade end 30 m) |

![nameplate_centered.png](screenshots/playtest_0036/nameplate_centered.png)

## Findings

**F1 — Visible change: plates are centered over their target** (before they hung from the
projected point to the right, playtest 0030's screenshot). Intentional: the anchor's default
pivot.

**F2 — Not exercised:** a target despawning while its plate is shown (covered by bevy_markup's
`world_anchor_projects_hides_and_despawns`), VR cameras.
