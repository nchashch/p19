# Agent Playtest 0015 — Room-Tagged Spawns and the Dead-Player Window: Two Fixes and a Newly-Exposed Death-Path Panic

| Field | Value |
|---|---|
| Date | 2026-10-02 02:00 – 02:50 local |
| Commit | `012b7c8` "Only build steamrt4 client" + this session's changes (uncommitted at run time): `Rooms` tags in `server::spawn`, dead-look gate in `server::input`, corpse-sim stop in `client::gameplay::combat`, doc updates |
| Agent | opencode session, GLM-5.3-Flash |
| Clients | 2x `target/debug/client --mcp --no-render` (dev-tools feature); client B on fleet ports :15712/:15713; 300-400 s `timeout` lifetimes |
| Server | `target/release/server` — fresh per round |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · client QA: BRP :15702/:15712 + MCP :15710/:15713 · server QA: BRP :15701 |

## Purpose

Fix and verify cracks #4 and #5 from the physics/replication architecture review:

- **#4 Dead-player window** — corpses kept agency during the \~1 s `Dead` window before
  `despawn_dead` removes them (movement, look, and — once combat works — attacks).
- **#5 Room filtering bypass** — spawned cubes/NPCs carried no `Rooms` tag, so they replicated
  to **every** connected client, including clients still in the lobby room.

## The changes

- `server/src/spawn.rs` — both `apply_spawn_*` resolvers now insert
  `Rooms::single(game_room.0)` on spawned entities (standalone, no `ChildOf` under
  `InGameRoot`, so the hierarchy cascade never reaches them and the explicit tag is
  load-bearing). `GameRoom` threaded through both live systems and `replay_spawn`.
- `server/src/input.rs` — `accumulate_look` gates its `Query<&mut CharacterLook>` on
  `Without<Dead>`: a corpse's rotate actions no longer accumulate into server-side look.
  (Movement was already stopped: `kill_zero_hp` removes `RigidBody`, starving ahoy's KCC.)
- `client/src/gameplay/combat.rs` — `hide_dead` additionally removes
  `AhoyCharacterController` from dead entities, so the **owner's** local prediction stops
  simulating their own corpse (previously the owner's held inputs would rubber-band the
  corpse against the frozen server position until despawn).
- Deliberately deferred: dead-**attacker** gating in `apply_attack`/`apply_kill` — it needs the
  combat caster-resolution fix (crack #3), since combat currently no-ops for everyone.

## Verification rounds

| Round | Procedure | Result |
|---|---|---|
| 1 (rooms) | Server + A (in-game: connect → select → play) + B (connect only, held in Lobby); A triggers `spawn_cube` + `spawn_npc`; `world.query` for `Cube`/`Npc` from both clients | A: 1 cube + 1 NPC. B (lobby): **0** cubes, **0** NPCs — previously both leaked to every client |
| 2 (look baseline) | Alive player: `game/input rotate` (yaw_delta -1, 2 ticks), read server-side `bevy_ahoy::CharacterLook` via BRP before/after | yaw 0.0 → 1.0 — the measurement path through the gated query works while alive |
| 3 (kill attempt) | Server BRP `world.mutate_components`: `shared::combat::HitPoints` `hit_points` → 0 on the player entity | Mutate succeeded — and the victim's client **panicked** \~1 s later. No dead-state observation possible |
| 4 (ablation) | Client-side `hide_dead` change stashed, client rebuilt, identical kill re-run | Client still panicked — the death path itself is broken, not this session's changes. Stash popped, rebuilt, change restored |

## Findings

**F1 — Room filtering for spawned entities: fixed and verified.** In-game client sees its
spawns; a lobby-held client sees none. Note the semantics: `Rooms::single(game_room.0)` is the
same room the **spawning player's connection** joins on `in_game_request`, so cubes/NPCs now
reach exactly the clients who could see the spawner. Cubes/NPCs belonging to a player who
later disconnects keep their room tag until despawned — fine today (the despawn gap is
crack-listed separately), worth revisiting with room-scoped cleanup.

**F2 — Dead-player agency is mostly closed; the last piece is blocked.** Movement (pre-existing
`RigidBody` removal), server-side look (this session's `Without<Dead>` gate), and the owner's
local corpse sim (the `hide_dead` controller removal) are in. Dead-**attacker** gating is
deferred to the combat caster-resolution fix, correctly — there is nothing to gate while
`resolve_attack`/`resolve_kill` no-op for everyone. Live verification of the dead-look gate is
**compile-verified only**: the kill attempt killed the client before a dead-rotate could be
observed (F3). The gate is a one-line query filter on the same query path proven alive in
round 2.

**F3 — NEW pre-existing blocker: killing a player panics the victim's client.** Sequence:
server-side `HitPoints` mutate succeeds → `kill_zero_hp` fires (`EntityDied` broadcast,
`Dead` insert, `Selectable`/`RigidBody`/`Collider` removal) → \~1 s later the victim's client
panics: `lightyear_replication::client::sync_last_confirmed_checkpoint` requires
`Res<ServerMutateTicks>`, which does not exist client-side (`Parameter ... failed validation:
Resource does not exist`), preceded by `VisualCorrection\<AngularVelocity>` remove-command
errors racing the corpse despawn ("Entity despawned ... generation 2"). Ablation-confirmed
pre-existing (round 4): the panic reproduces with this session's client change stashed. This
death path was never exercisable before — combat resolution is broken, so nothing could deal
damage — which is why it surfaces only now. **Hypotheses for the next session (not chased):**
(1) the client's replicon backend never initializes `ServerMutateTicks` because no
client↔server mutation traffic was expected on this code path, and a server-side change to a
replicated component arrives as a mutation the client isn't set up to receive; (2) the
1 s-despawn races lightyear's correction-history cleanup, and the despawn hook errors cascade
into the checkpoint system. Also observed but unexplained: `game/state` reported
`grounded: true` for the whole window and the client never showed the death (it panicked
before the state machine could react).

**F4 — Tool-API / harness notes:**
- `world.mutate_components` schema (Bevy 0.19): `{entity, component: "\<full type path>",
  path: "\<reflect field path>", value: \<json>}` — the field is a reflect *sub-path* (`path:
  "hit_points"`, `value: 0`), not a component-properties object. Cost two failed attempts.
- `shared::combat::Dead` is not reflected, so BRP cannot see it at all (`Unknown component
  type`) — dead-state observation must go through `game/state`'s `dead` field or
  `world.list_components`. Same reflection-visibility family as playtests 0013 (F2) and 0014
  (F2): if QA tooling can't see a component, check reflection before doubting the simulation.
- Client `timeout` values: 400 s survived a multi-step verification; 200 s did not (playtest
  0014's F4, reconfirmed).

## Next steps

1. **Death-path client panic (F3)** — new top blocker: no death can be shipped or verified
   end-to-end while it stands. Start from where `ServerMutateTicks` is initialized upstream
   and why the client backend never creates it.
2. **Joiner hover** (playtest 0014's F3, unchanged) — second joiner floats at spawn height
   until first input.
3. **Combat caster resolution** — unblocks real combat **and** the deferred dead-attacker gate.
4. **KCC-internal rollback registration**, **netcode posture** — unchanged, see AGENTS.md.

## Conclusion

Room-tagged spawns are fixed and live-verified (lobby clients no longer receive other
players' cubes/NPCs). The dead-player window is closed for movement, look, and local corpse
simulation — with dead-attacker gating correctly deferred to the combat fix — but end-to-end
death verification exposed a new pre-existing client panic on the death path (F3), ablation-
attributed and documented as the new top blocker. AGENTS.md updated throughout (rooms gap
closed, dead-player entry rewritten, spawn/combat Server-section bullets corrected).
