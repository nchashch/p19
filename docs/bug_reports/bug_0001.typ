#set document(
  title: "Bug 0001 — Combat caster resolution: every attack/kill silently dropped",
  author: ("opencode agent (GLM-5.3-Flash)",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Bug 0001 — Combat caster resolution silently drops every attack and kill

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Bug*], [bug_0001],
  [*Date discovered*], [2026-10-01 (regression introduced by the M2 caster-split architecture migration; root cause documented in AGENTS.md before the fix, live-confirmed during this arc)],
  [*Commit (state actually running)*], [Live-confirmed at `012b7c8` "Only build steamrt4 client" + uncommitted work tree (the M2-era regression itself predates this log window). *Fixed in* `6d2fb0a` "Fix combat, extend MCP harness to cover combat"],
  [*Discovered by*], [opencode agent (GLM-5.3-Flash), via AGENTS.md gap-list review + code inspection],
  [*Component*], [`server::combat` (`apply_attack`/`apply_kill`)],
  [*Severity*], [S2 — core gameplay flow broken, no crash],
  [*Status*], [*Fixed in* `6d2fb0a` — verified live (attack dropped target HP 100 → 51 through the full new path)],
  [*Related*], [bug_0003 (same-turn sibling fix), playtest 0017 (end-to-end headless combat verification), AGENTS.md "Fixed: the caster-resolution regression" entry],
)

= Summary

Every client→server `AttackAttempt`/`KillAttempt` was silently dropped server-side. The
resolution code looked the caster's `Gcd` (and, for the range check, the caster's `Transform`)
up *on the connection entity* — but since the M2 architecture change the connection entity
is separate from the player character, which is a distinct `ControlledBy { owner: connection }`
entity that carries both. Result: combat was completely non-functional for weeks without any
error, warning, or client-side indication.

= Steps to reproduce

1. Launch the server (`BEVY_ASSET_ROOT=$PWD/server ./target/release/server`) and a client
   (`./target/debug/client --mcp --no-render`).
2. Connect → select `levels/minimal.level.ron` → play. Confirm in-game.
3. Trigger an attack through any path (hotkey, or `game/trigger attack` after
   `game/select`).
4. Observe the target's server-side `HitPoints` (`world.query` with
   `shared::combat::HitPoints`): unchanged. No `Attack`/`Kill` broadcast is ever sent.

*Expected:* `HitPoints` drops by `DAMAGE` (49) per landed attack, `Attack` is broadcast, and
at 0 HP the target dies.
*Actual:* nothing happens — the request dies at the `casters.get_mut(connection)` guard
(`Err` → early return), with no log line at the default log level.

= Evidence

Server-side BRP query after an attack: target `HitPoints` still 100. The `debug!` gate line
added by the fix ("attack from `<connection>` dropped: no living player character") confirms
the drop point when run with `RUST_LOG=server=debug`.

= Root cause

Pre-M2, the client's connection entity *was* the player, so `casters.get_mut(receiver_entity)`
worked. M2 split the player onto a separate entity; the connection kept only the
`MessageReceiver<T>`. The fix resolves the caster's player character via
`ControlledBy { owner: connection }` (`networking::owned_players` — select-then-borrow, the
stable-rustc-safe pattern also used in `server::spawn`), then reads `Gcd`/`Transform` from it.

= Fix

- `6d2fb0a` — both `apply_attack`/`apply_kill` resolve the caster's living player character
  before the cooldown/range checks; the broadcast `Attack.attacker`/`Kill.killer` field now
  carries the *player character* entity (what client presentation keys off), not the
  connection.
- Bonus: the resolution excludes dead players — closing the deferred dead-attacker gate
  (a corpse keeps its `Gcd`, so the exclusion is what stops corpses attacking).
- Verified live: attack dropped the target's server-side HP 100 → 51; `game/select
  {"name": …}`-targeted attack confirmed again after the unique-name feature landed.

= Follow-ups

- The same regression class hit `server::spawn` first (fixed same arc, see AGENTS.md's
  caster-resolution entry) — when adding new client→server "act" messages, caster resolution
  through `owned_players` is mandatory, not optional.
- Headless-combat QA note: crosshair targeting needs a window; headless clients use
  `game/select` + `game/trigger attack|kill` (see `docs/skills/playtest.md` §3).
