#set document(
  title: "Playtest Index",
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Playtest Index

One entry per run in `docs/playtests/playtest_NNNN/` (see `docs/skills/playtest.md` §10 for
the format and layout these follow). Newest first. Update this file whenever a new playtest is
filed — that's part of filing it, not a separate later chore.

#outline(title: none, indent: auto)

== `playtest_0003` --- Diagnose and Fix the Headless Camera Rendering Bug

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-22 23:00 -- 2026-09-23 00:15 UTC],
  [*Commit*], [Started at `329c9bc`; fix landed as `e5fe6c6` (committed after this run's
  verification) -- spans both sides of a code change, not a static state],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0003/report.typ`],
)

Root-caused and fixed the bug `playtest_0001`/`playtest_0002` both hit: `--mcp` mode's cameras
never rendered anything, because `bevy_render::camera::camera_system`'s `target_info` recompute
silently never fires for a camera retargeted after `Startup` (a one-line fix,
`projection.set_changed()`, in `retarget_cameras_to_offscreen`). This also explained the
previously-separate "KTX2 skybox kills offscreen rendering" finding -- same root cause, not a
real skybox bug; the skybox/TAA/SSAO strip workaround is removed. *Open follow-on*: fixing this
surfaced a UI-panel-renders-under-the-background ordering bug in menu/lobby specifically; three
fix attempts were tried and reverted (each one regressed in-game rendering worse than the
ordering bug itself) -- still broken, see the report's "A follow-on issue found, attempted, and
reverted" section for what was tried and why it's not safe to re-attempt casually. Also found, unrelated: the
"minimal" level has no light source anywhere in its content (renders black on any client, not a
`--mcp`-specific issue).

== `playtest_0002` --- Bug Reproduction Pass (Caster-Resolution + Headless Camera)

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-22 ~23:25 UTC],
  [*Commit*], [`329c9bc` "Add playtest.md skill"],
  [*Agent*], [Claude (Sonnet 5)],
  [*Report*], [`docs/playtests/playtest_0002/report.typ`],
)

A targeted re-verification pass, not a full state tour: checked two things `AGENTS.md`'s gap
list already claimed, with harder evidence than `playtest_0001` had supplied for the same
claims. Confirmed live: `spawn_cube` is silently dropped server-side (zero `Cube` entities
before/after the trigger, total server-log silence) -- *still open, not fixed by any playtest
since*, see `AGENTS.md`'s caster-resolution gap entry for the fix shape. Also found a sharper
diagnostic lead for the headless camera bug (`Camera.computed.target_info` staying `null`
specifically for cameras claimed after `Startup`) that `playtest_0003` went on to root-cause
and fix.

== `playtest_0001` --- Headless (--mcp) Client State Tour & Input Drive

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 5pt,
  [*Date*], [2026-09-23 ~02:38 local (2026-09-22 22:38 UTC)],
  [*Commit*], [`89f5eca` "Implement headless --mcp mode for client"],
  [*Agent*], [opencode session, GLM-5.3-Flash],
  [*Report*], [`docs/playtests/playtest_0001/report.typ`],
)

The first formal playtest: verified the agent/QA tool API (ADR 0009) end-to-end against a fresh
server and a fresh headless client -- drove every reachable client state, injected input through
the real replicated pipeline, and recorded what the agent sees vs. what a human on a windowed
client sees. Confirmed the movement/input/replication data path is fully solid headless (server-
authoritative sim, client prediction, all verified through state reads across a full menu→game
flow). First recorded the in-game blank-rendering bug and the KTX2-skybox-kill finding -- both
later found to be the same root cause, fixed in `playtest_0003`.
