#set document(
  title: "Agent Playtest 0017 — Headless Combat Harness: Full Kill-to-Despawn Loop Verified Without a Window",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0017

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-10-02 02:55 -- 03:40 local],
  [Commit (local state actually running)], [`9377b41` "Fix dead player bug" + uncommitted working tree, per file: `client/src/events.rs` (new `AttackSelected`/`KillSelected` triggers) · `client/src/controls/controls.rs` (hotkey observers now trigger those; new `send_attack`/`send_kill` observers) · `client/src/dev/tool_api.rs` (`game/select` method, `attack`/`kill` trigger events, `selected` field in `game/state`) · `client/src/gameplay/combat.rs` (idempotent `hide_dead`) · `server/src/combat.rs` (combat caster resolution, from the previous turn) · `server/src/replay.rs` (threaded queries) · `AGENTS.md` + `docs/skills/playtest.md` (docs)],
  [Agent], [opencode session, GLM-5.3-Flash],
  [Clients], [2x `target/debug/client --mcp --no-render` (dev-tools feature); client B on fleet ports :15712/:15713; 300-400 s `timeout` lifetimes. *No windowed client, no keypress/mouse mocking*],
  [Server], [`target/release/server` — fresh per round],
  [Level], [`levels/minimal.level.ron`],
  [Transports], [game: UDP/netcode :6000 · client QA: BRP :15702/:15712 + MCP :15710/:15713 · server QA: BRP :15701],
)

= Purpose

Close crack #5 from the review: the full kill-to-despawn loop — attack → `Kill`/`EntityDied`
→ corpse → despawn → Lobby — had every link individually verified but never as one continuous
flow, because attacks couldn't be driven headlessly: crosshair targeting
(`targeting.rs`'s `raycast_from_center` → `Selected`) needs a real window
(`window_query.single()` + `Camera::viewport_to_world`), so on `--mcp`/`--no-render` clients
`Selected` is never set and the hotkey observers return before sending anything. Goal: make
combat drivable through the MCP/BRP harness on `--no-render` clients only, with no keypress
or mouse mocking.

= The harness change

- `events.rs` — new client-local triggers `AttackSelected`/`KillSelected`.
- `controls.rs` — the `AttackAction`/`KillAction` hotkey observers now just trigger those
  events; the actual sends moved to `send_attack`/`send_kill` observers, so the hotkey path
  and the tool path share *one* send implementation. Both send observers registered behind the
  same console/modal gates as the hotkeys.
- `tool_api.rs` — `game/select` (`{"entity": <u64 id>}`): validates the entity is `Selectable`
  *on this client* and injects `Selected`; `game/trigger` gains `attack`/`kill` (the send
  observers' triggers); `game/state` gains a `selected` field (what an attack would hit right
  now). Note the entity id must be the *same client's* local id — take it from that client's
  own `world.query`/`game/state`, not another client's.

= Verification

Two `--no-render` clients, fresh server: A connects → selects level → plays (grounded at
0.915); B connects → plays (hovers at 2.73 — the known joiner-hover crack; in attack range at
~1.8 units regardless). Then, entirely through the harness:

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Step*], [*Observed*],
  [A: `game/select {"entity": <B's player, A's local id>}`], [`{"selected":8589933608}`; `game/state` confirms `selected` matches],
  [A: `game/trigger attack` x2 (0.7 s apart — inside the 0.5 s GCD)], [Server-authoritative HP: B 100 → 51 → 2 (two 49-damage hits through the new caster-resolution path)],
  [A: `game/trigger kill`], [HP = 0 → `kill_zero_hp` (`EntityDied` broadcast, `Dead` insert, `Selectable`/`RigidBody`/`Collider` removal)],
  [Corpse despawn (1 s `Dead` timer)], [Server `PlayerCharacter` count 2 → 1; B's corpse gone everywhere],
  [B's client], [Clean transition to `Lobby`, `player_despawned: true`],
  [Panics], [*0* across both clients and the server],
)

Also exercised along the way (previous turn's changes, regression-checked here): `game/select`
rejects non-`Selectable`/missing entities; the `selected` field clears/updates correctly.

= Findings <findings>

*F1 — The full combat chain works headlessly, end-to-end, zero panics.* attack → server
caster resolution (`owned_players` → player `Gcd`/`Transform`) → range check → damage →
`Attack` broadcast → GCD re-blocks the second rapid attack (observed: the two attacks needed
the 0.7 s spacing — a sub-0.5 s pair would land only once) → kill variant → death → despawn →
Lobby. Both message types (`AttackAttempt` and `KillAttempt`) exercised through the same
`Selected`-driven send path the real hotkeys use.

*F2 — NEW pre-existing race found and fixed mid-verification: `hide_dead` vs the replicated
corpse despawn.* First full-loop run: client A (a *remote observer* of the victim) panicked —
`insert<bevy_camera::visibility::Visibility>` on an already-despawned entity, inside
`apply_deferred`. Cause: `hide_dead` re-queued `Visibility::Hidden` (and the controller
removal) *every frame* while `Dead` was present; a queued command landed after the replicated
despawn applied. Fixed by making `hide_dead` idempotent — it now matches
`(Entity, &Visibility)` and skips corpses already `Hidden`, so nothing is queued for the last
~1 s of the corpse's life once the first pass lands. Re-run: zero panics on both clients and
the server. Note the asymmetry with playtest 0016 (where the *victim's own* client survived
the same race): the remote-observer timing differs, and per-frame command spam is what makes
the window a certainty rather than a race.

*F3 — Targeting injection semantics worth remembering.* `game/select` validates against
`Selectable` but deliberately does *not* range-check: `ATTACK_RANGE` is enforced
server-side in `apply_attack`/`apply_kill`, and out-of-range attempts are silently dropped
there (by design — the server is authoritative). QA can therefore select anything `Selectable`
and observe the server's verdict, which is the more useful signal. Entity ids are
client-local: select from the *same* client's `world.query` output.

*F4 — The windowed-client attempt from the previous turn, explained.* The reason a windowed
client had been involved at all: `raycast_from_center` needs `Window` +
`Camera::viewport_to_world`; headless modes have `primary_window: None`, so `Selected` can
never be set by aiming. A windowed client launched on the live desktop also picked up the
user's real mouse (ROTATE-FIRE lines with physical deltas in its log) and was closed mid-run
("No windows are open, exiting") — desktop-intrusive *and* unreliable for automation, exactly
as suspected. The `game/select` injection removes the need entirely.

= Next steps

1. *Joiner hover* (playtest 0014's F3, unchanged) — the victim hovered at 2.73 through this
   entire loop; ground-level targets want it fixed.
2. *KCC-internal rollback registration* — unchanged.
3. *Netcode posture* — unchanged.
4. Optional QA polish: `game/select` variant that takes a *player name* or auto-selects the
   nearest other player, removing the manual `world.query` step.

= Conclusion

Combat is now fully drivable — and fully verified — on windowless `--no-render` clients:
`game/select` injects targeting, `game/trigger attack|kill` fires the real send path, and a
complete kill-to-despawn loop ran clean (HP 100 → 2 via attacks, kill variant to 0, corpse
despawn, victim back to Lobby, zero panics on all three processes). One new pre-existing race
(`hide_dead` vs replicated despawn, remote-observer side) was found during verification and
fixed in the same pass. AGENTS.md and `docs/skills/playtest.md` §3 updated with the new
methods and the headless-combat recipe.
