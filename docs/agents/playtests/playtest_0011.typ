#set document(
  title: "Agent Playtest 0011 — Vision tooling, client-configuration matrix, server-side BRP/MCP, and the multi-client look/movement desync",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0011

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-24 ~17:20 → 20:30 UTC (local +04: Sep 24 evening → Sep 25 morning; one long session, several restarts)],
  [Commit], [`f4180af` "Fix no dev-tools feature build failure" — HEAD at the end of the session; the session's work is *uncommitted* (see §7)],
  [Agent], [opencode session, GLM-5.3-Flash, driving the tool API over loopback HTTP, with the project owner co-driving a windowed client],
  [Clients], [production `target/debug/client --mcp` variants: rendered headless (60 fps), `--no-render`, `--headless-render` (2 fps observer), full-render 60 fps for the desync repro — all with `--brp-port`/`--mcp-port` fleet ports where noted; plus the owner's windowed `target/release/client`],
  [Server], [`target/release/server` — restarted several times; **rebuilt mid-session** to add the server-side BRP/MCP debugging surface (§4)],
  [Level], [`levels/minimal.level.ron` (the only level — the owner deleted `spawn`/`outpost` mid-session after we traced the client-side fall-through to `start.glb`'s missing replication markers, see #ref(<findings>))],
  [Transports], [game: UDP/netcode :6000 · client QA: BRP :15702 + MCP :15710 (fleet ports :1600N/:1700N) · **server QA (new): BRP :15701 + MCP :15711**],
)

= Purpose

Three linked goals, all verified against live sessions:

+ **Vision tooling for the agent** (ADR-0011 follow-through): make the observation loop
  data-first and verify the new surfaces — `game/ui` (accessibility-tree dump), the visible
  agent-cursor overlay, state-fused screenshots (`.json` sidecars), `crop` +
  unchanged-frame suppression, the 1280×800 Steam-Deck viewport, and the server-side
  BRP/MCP endpoint (`server/state`) that turns the *server* into the desync ground truth.
+ **Client-configuration matrix**: exercise every launch mode (`--mcp` rendered headless,
  `--no-render`, `--headless-render` observer, fleet ports) end-to-end and document their
  particularities (§1a of the playbook).
+ **The multi-client look/movement desync**: reproduce the owner's report ("camera pitch
  dead, movement locked to one axis" when a second client joins), characterize it, and bound
  where the corruption lives (client-local vs server-authoritative).

= Method

Standard playbook flow: teardown → fresh server → clients via the tool API → device-level
input (`game/keyboard`, `game/mouse` — deliberately *not* `game/input`, whose action-level
mocks bypass the binding layer the desync lives in) → `game/state` + `server/state` sampled
after every action → BRP `world.query` for player/entity census → screenshots only where
inherently visual. Fleet clients got dedicated ports per §9 (the first attempt taught the
launch-loop port-formatting gotcha recorded in #ref(<findings>)).

= Part 1 — vision tooling and client configurations

== `game/ui`, agent cursor, fused captures

- `game/ui` dump verified at every state (menu, lobby, in-game, selector popups): buttons
  come back as `{rect, text, clickable: true, interaction: Idle|Hovered|Pressed}` rows in
  the exact `game/mouse move_to` pixel space.
- Agent-cursor overlay renders as a 14×14 px crosshair centered exactly on the mocked
  pointer (pixel-verified in captures): red idle → yellow hovering (parked on the "Level"
  button) → white left-held. `Pickable::IGNORE` verified — clicks pass through it.
- Full mouse-only session, every click aimed from `game/ui` rects: Connect → Level popup →
  "Spawn" row → Play → in-game. No coordinate guessing anywhere.
- Fused captures: `game/screenshot/get` now returns `{ready, png_base64, path, state}` and
  writes `<capture>.json` sidecars (#figure link in the table below); unchanged-frame
  suppression answered `unchanged: true` with no image on identical re-polls and on a fresh
  capture of a static menu.
- Crop: a Connect-button crop `[61,281,200,24]` landed as a **200×24 PNG, 1.8 KB** vs a
  962 KB full frame (~190× fewer vision tokens); out-of-bounds crop clamped to 80×20;
  malformed crop rejected cleanly.

#figure(
  image("screenshots/playtest_0011/1790273438257-connect-crop.png", width: 40%),
  caption: [The Connect button captured via `game/screenshot {"crop":[61,281,200,24]}` —
  200×24, 1.8 KB base64. The crop coordinates came straight off a `game/ui` dump row.]
)

== 800p viewport and client-configuration matrix

- The headless offscreen target moved to **1280×800** (Steam Deck native): verified via
  `game/ui` (`target_size [1280,800]`), root node rect, and the PNG itself. The 16:10
  re-centering moved the menu panel down by 80 px (Connect y 281 → 361) — clicked it at the
  *dump's* coordinates without re-deriving anything.
- Client matrix exercised (§1a): rendered headless, `--no-render` (no wgpu, no Vulkan
  driver, ~0.7 core / ~0.3 GB vs ~1.3 cores / ~1.1 GB rendered), `--headless-render`
  (2 fps observer), fleet ports with per-client screenshot dirs. `game/client_info` (new)
  reports the effective flags and ports; verified on no-render and windowed clients.
- `--headless-render` observer verified end-to-end: joins via `ObserveRequest` (server logs
  "no player spawned"), receives the full replicated world, and
  `game/screenshot {"camera": …}` renders *that camera's* view after a one-frame
  draw-order borrow (see #ref(<findings>) for the fresh-image trap this replaced).

#figure(
  image("screenshots/playtest_0011/1790281230656-observer-player-vision.png", width: 78%),
  caption: [The observer's aimed capture: `ObserverCamera` repositioned to (0, 1.4, 4)
  looking at the fleet client's replicated player capsule (server truth (0, 0.92, 0)),
  rendered via `game/screenshot {"camera": …}`. Brightness profile varies with rows
  (lit terrain) — a real 3D view of the replicated world, on demand.]
)

== Server-side BRP/MCP

`server::tools` (new): BRP :15701 + MCP :15711, unconditional (the server is trusted). The
first live `server/state` call immediately paid for the feature — see §4.

#figure(
  image("screenshots/playtest_0011/1790274035894-deck800.png", width: 78%),
  caption: [First 800p capture, full frame. Same as the agent "sees" on the Steam Deck's
  native resolution.]
)

= Part 2 — the multi-client look/movement desync

== The owner's report

With ≥2 live clients connected and acting, the owner's windowed client showed: **camera
pitch dead, movement locked to a single axis** ("any WASD key → the character just moves
right in a straight line"), and — refined later — mouse-look "working but at weird
discrete-feeling jerky angles". The lock appeared **when the owner joined while fleet
clients were already in-game**, and went away on reconnect.

== Reproduction attempts

#table(
  columns: (auto, auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Setup*], [*First client*], [*Second client*],
  [2× `--no-render`, fresh server, B played after A (device-level input)] , [works (mouse dx=200 → yaw −1.0/pitch −0.3 exact; KeyW walks ~11 m along facing)], [**works** — same exact numbers as A],
  [2× `--mcp` full-render, exact owner sequence], [works], [**BROKEN at spawn** — see below],
  [2× `--mcp` full-render, exact-sequence rerun], [works], [works — intermittent, ~1-in-2],
)

The broken second client, *at spawn and before any input*: `look_yaw = −1.71`,
`look_pitch = +1.5707` — pinned at the accumulator's clamp maximum (`±π/2 − 0.0001`).
Its subsequent mouse deltas produced erratic jumps (−1.71 → 2.26 → 0.95) instead of clean
−1.0/−0.3 responses; its KeyW walk followed the corrupted facing. The first client stayed
exact throughout (re-checked after the second client joined: unchanged position, no drift).

Server-side ground truth (`server/state`, new) during the same window: the *first* client's
authoritative walk was healthy — server position (0, 0.92, −18.67) while that client's local
view read (0, 0.915, −18.67), agreeing to ~0.005. Earlier in the session the same
client-vs-server comparison exposed the opposite case: a `--no-render` client's *local
prediction* falling to y = −2364 while `server/state` held it standing at y = 0.92 walking
correctly forward — the local-prediction fall-through, separate from the look corruption.

== Findings <findings>

**F1 — Reproduced (intermittent, ~1-in-2) on full-render `--mcp` clients: the second client
spawns with its look already corrupted (yaw −1.71, pitch pinned at the +π/2 clamp) before
any input.** The first client is unaffected. Two prime-suspect code paths, both real smells
independent of the repro:

- `controls.rs::rotate_camera` — a *global* observer on `Fire<AhoyRotate>` with **no
  context keying**, applying `rotate.value` to `fps_camera.single_mut()` and finishing with
  `camera_transform.look_at(fps_camera.direction)` — using a unit *direction* as a *target
  point*. `Transform::look_at(point)` orients the rig toward a spot ~1 m from the anchor in
  parent-local space, not along the direction; combined with `update_character_look`
  mirroring the camera euler back into `CharacterLook`, a pitch that reaches the clamp
  becomes a **self-sustaining trap** (vertical direction → look_at of a vertical point →
  still clamped), which reads exactly as "can't pitch".
- `controls.rs::update_character_look` reads the camera's **global** rotation — which
  includes the player entity's own (server-replicated) `Rotation`. Any server-side body
  rotation contaminates the euler and desyncs `CharacterLook` from what the player sees —
  "WASD walks in a fixed direction".

**F2 — Intermittency**: the exact-sequence rerun of the broken session stayed clean,
so the trigger is timing-sensitive (spawn-frame ordering, join burst, or replication
arrival racing camera-rig decoration). Reproduced once in ~2 attempts on the affected
configuration; not reproduced at all on `--no-render` clients (2 attempts) or in 2 of 3
full-render attempts.

**F3 — `--no-render` second-player crash (found *during* the repro, fixed)**: the first
two-client no-render session crashed both clients the moment the second player replicated
in: `decorate_other_players` spawns the rig-model child, whose world serialisation needs
`bevy_mod_outline` resources that `--no-render` (deliberately) does not add. Fixed by
gating the decoration behind `not(resource_exists::<NoRenderMode>)` in
`gameplay/player_character.rs`. Verified: two no-render clients connect, play, and walk
simultaneously.

**F4 — Zombie-player pollution is now a measured, live-world fact**: client census via
BRP showed 4 static `PlayerCharacter` corpses from earlier disconnected sessions in the
replicated world while 3 live players moved. Every disconnected session adds another.
This inflates join-burst payload (and possibly the desync trigger). The server-side
disconnect-despawn gap is now the top candidate fix before more fleet sessions.

**F5 — Device-level input verification works headless**: `game/keyboard` +
`game/mouse` on `--no-render` clients exercise the *binding* layer (`game/input`'s
action-mocks bypass it), giving the desync hunt a clean probe on all client modes.

**F6 — Fleet-port formatting gotcha**: a launch loop using `--brp-port 16$i` builds
*1613*, not *16013* — the clients bound fine and the "dead" BRP was just misaddressed.
The playbook's §9 example uses `1600$N` for this reason.

**F7 — Tool-API gaps (for the next desync round)**: `server/state` lacks per-player
`CharacterLook` (server-authoritative yaw/pitch — the single most useful field for the
look-corruption diagnosis); `game/state` lacks binding/bindings introspection (whether
`bind_replicated_ahoy_actions` actually bound the second client's context). Both should
land before the instrumented rerun.

== Next steps (agreed with the owner)

Add temporary instrumentation to `rotate_camera` / `update_character_look` (log
`FpsCamera.pitch/yaw`, the camera's global euler, and the player entity's replicated
`Rotation`), then loop the repro until the second client breaks — the logs will show which
of F1's two smells corrupts first. Likely fix shape: replace the `look_at(direction-as-point)`
with a direct rotation assignment (`Quat` from yaw/pitch) and/or read the camera's
local-to-parent rotation instead of global in `update_character_look`.

= Conclusion

The vision/observation loop is now data-first end-to-end (client *and* server), every
client configuration is exercised and documented, and the desync is bounded: reproduced
intermittently on full-render clients, with the second client's look corrupted at spawn
(pitch pinned at the clamp) and two concrete code smells identified as the mechanism. The
server-side endpoint turned "the server thinks otherwise" from a guess into a one-call
answer, and the zombie-player gap — now measured in the live world — is the other fix this
investigation has queued up.
