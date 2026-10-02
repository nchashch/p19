# Agent Playtest 0013 — Spawn Caster-Resolution Fix: NPC/Cube Spawning Verified End-to-End

| Field | Value |
|---|---|
| Date | 2026-10-01 22:30 – 23:20 local |
| Commit | `9d36146` "Add CI" + this session's fix (uncommitted at run time): caster resolution in `server::spawn`, `owned_players` made `pub(crate)`, `Npc` reflection |
| Agent | opencode session, GLM-5.3-Flash |
| Clients | `target/debug/client --mcp --no-render` (dev-tools feature), fresh per round |
| Server | `target/release/server` — fresh per round, restarted between fix iterations |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · client QA: BRP :15702 + MCP :15710 · server QA: BRP :15701 |

## Purpose

Fix and verify the documented caster-resolution regression: `server::spawn`'s
`apply_spawn_npc`/`apply_spawn_cube` looked the caster's `Gcd` up **on the connection entity**
(`casters.get_mut(caster)`), but since the player became a separate `ControlledBy`-owned
entity (M2), the connection has no `Gcd` — every spawn request died silently at the guard.
AGENTS.md recorded this as live-confirmed ("`game/trigger spawn_cube` produces no
`Cube`/`Npc` entities at all"); no fresh reproduction was run against the broken code — the
pre-fix behavior is ground truth from that earlier session's documentation.

## The fix

- `server/src/spawn.rs` — both `apply_spawn_*` resolvers now iterate the connection's owned
  entities (`ControlledBy { owner: connection }`) and take the first that carries `Gcd`
  before the cooldown check. A `debug!` line logs the drop when no `Gcd`-carrying player
  exists — the silent-guard failure class was the original complaint. `spawn_npc`/`spawn_cube`
  and `replay.rs`'s `replay_spawn` all pass the new `Query<(Entity, &ControlledBy)>` through.
- `server/src/networking.rs` — `owned_players` made `pub(crate)` (reused, not duplicated; the
  same helper the disconnect-cleanup path already used).
- Deliberately unchanged: the RNG seed still mixes the **connection** entity's bits
  (`caster.to_bits()`), so replay determinism against existing recordings is preserved.

## Verification rounds

| Round | Procedure | Result |
|---|---|---|
| 1 (post-fix) | Fresh pair → connect → `select_level minimal` → play → `game/trigger spawn_cube` → `world.query Cube` → `spawn_npc` | Cube: **spawns** (1 entity). NPC: **0** — initially read as failure |
| 2 (post-fix) | NPC retry after cooldown; camera rotated π via `game/input rotate`; player walked to (0, 0.92, −9.46) and retried; cube spawn sanity-check | Cube: spawns every time (and flies \~100 u/s — see F3). NPC: 0 every time, **in every location** |
| 3 (server-side truth) | Direct BRP against the **server's** QA surface (:15701): `world.list_components` per entity on suspicious dynamic bodies | **NPCs were spawning all along** — bodies at the exact spawn points carrying full `npc()` bundles (`Npc`/`Idle`/`Character`/`HitPoints{100}`/`Selectable`/`Name`). The client-side and server-side `world.query` for `shared::npc_spawner::Npc` both returned 0 despite this |
| 4 (post-reflection) | `Npc` given `Reflect` + `#[reflect(Component)]`; both binaries rebuilt; **fresh** pair: connect → level → play → spawn NPC → spawn cube | **`world.query` sees the NPC**: entity at (−0.00003, 0.90, −4.00) — the exact camera-forward spawn point, settled resting on the floor (y = 0.9). Cube: spawns. Both verified through the standard QA surface |

## Findings

**F1 — Root cause confirmed and fixed (spawn half).** The `Gcd`-on-the-wrong-entity
regression was exactly as documented. The fix reuses `networking::owned_players` (the
connection → owned-entities relationship walk the disconnect-cleanup path already used) rather
than introducing a second resolution convention. **Still open**: `server::combat`'s
`resolve_attack`/`resolve_kill` have the same regression one level up (`Gcd` **and** `Transform`
for `ATTACK_RANGE` read off the connection) — every attack/kill request still dies silently.
Same fix shape applies; not touched this session (scope was spawning).

**F2 — QA-surface bug, not a gameplay bug: `Npc` was invisible to BRP because it lacked
reflection.** `bevy_remote` resolves component names through `AppTypeRegistry`;
`world.query`'s required-component filter for `shared::npc_spawner::Npc` silently matched
**nothing** — on both the client (:15702) and the server (:15701) — even on entities that
carried the component (provable via per-entity `world.list_components`). `Cube` works because
it derives `Reflect` + `#[reflect(Component)]`; `Npc` did not. Fixed by matching `Cube`'s
derive set. This was **masking** the spawn fix during verification — several rounds were spent
chasing location geometry, cooldowns, and clip-checks before the server-side component dump
showed the spawns had been succeeding all along. **Heuristic worth keeping**: if a marker
component "doesn't exist" in BRP queries but the simulation clearly spawned it, check for
reflection before doubting the simulation. Related trap in the same investigation:
`avian3d` 0.7's `Collider` component lives at
`avian3d::collision::collider::parry::Collider` — a query using the older
`avian3d::collision::collider::Collider` path **silently returns nothing** in an `option` list
rather than erroring (a wrong path in the **required** list also returns an empty result set
instead of an error). Verify component `TypePath`s via `world.list_components` on a known
entity before trusting a negative BRP query.

**F3 — Observation, not chased: spawned cubes launch at \~100 u/s.** A cube triggered at the
camera-forward point was 145 units away 1.5 s after spawn (and the session-1 cube coasted to
z ≈ −1035). The `aim_direction`-derived velocity plus `random_angular_component`'s (−10, 10)
scaling is doing something aggressive; whether the intended "cube launcher" feel is \~100 u/s
is a design question, not investigated. NPCs (no `LinearVelocity` input) stay at their spawn
point and settle to the floor at y = 0.9 — correct-looking behavior.

**F4 — Environment gotchas, harness-relevant:**
- Backgrounding game processes from this tool harness: a plain `(... & )` subshell launch gets
  reaped when the shell command exits; the server died this way once. `setsid env ... & disown`
  survives. The skill's §2 recipe should be updated if this reproduces for other agents.
- The skill's `timeout 300` client lifetime kills the client **including agent thinking time
  between tool calls** — this run lost a client mid-round that way (and the process died
  silently with an empty log when relaunched via the reaped-subshell pattern, costing another
  round). Budget generously; verify the MCP listening line after every relaunch.
- A client killed by `timeout` while the server lives is **not** the zombie-player scenario the
  skill warns about: the reconnecting client got a fresh player (server-side
  `on_client_disconnected` despawn path working — a `ClientInGame`-inheritance failure would
  have shown as InGame-without-position and did not).

**F5 — Pre-existing log noise noted, not chased:** at client spawn,
`bevy_enhanced_input` logs repeated "`action Movement/RotateCamera expects Axis2D, but got
Bool`" warnings and one LOOK-DIVERGENCE first-line (see playtest 0012's F1 for that
instrumentation's meaning). Both appear unrelated to spawning and were left alone.

## Next steps

1. **Combat caster resolution** — `resolve_attack`/`resolve_kill` still read the caster's
   `Gcd`/`Transform` off the connection entity; same `owned_players`-based fix. Every
   attack/kill is currently a silent no-op.
2. Consider clamping or tuning the cube launch velocity (F3) if \~100 u/s is unintended.
3. `docs/agents/skills/playtest.md` §2: add the `setsid env ... & disown` backgrounding pattern and
   a reminder that wrong-`TypePath`/unreflected components make BRP queries **silently empty**
   (F2) — both cost real debugging time this session.
4. Optionally: a `game/state`-style aggregate (or BRP helper) that lists spawned
   `Npc`/`Cube` counts, so "did it spawn" never again depends on reflection metadata being
   present.

## Conclusion

The documented spawn regression is fixed and verified end-to-end through the standard QA
surface on a fresh server+client pair: NPCs and cubes spawn server-side as full, correct
bundles at the camera-forward spawn points, and are visible to `world.query`. The verification
detour (F2) surfaced and fixed a second, independent QA-surface bug (`Npc` reflection) that
would have made **every** future agent playtest blind to NPCs. Combat's identical
caster-resolution bug remains the top open item.
