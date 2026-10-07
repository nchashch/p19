# Bug reports

This directory is the project's **permanent defect ledger**. Every reproducible flaw,
regression, misbehavior or design-level hazard gets its own numbered report, `bug_NNNN.md`.
Each report is pinned to the exact code state it was found in and includes repro steps,
evidence, root cause and severity.

Reports are **never deleted**. When a bug is fixed, its `Status` field changes in place to
`Fixed in <commit>`. A fixed report stays as regression armor: the repro and root cause make
the same class of bug quick to recognize next time.

## Ledger

| Bug | Summary | Status |
|---|---|---|
| [bug_0001](bug_0001.md) | Combat caster resolution: every attack/kill silently dropped | Fixed in `6d2fb0a` |
| [bug_0002](bug_0002.md) | Killing a player panics the victim's client (`--no-render` death path) | Fixed in `9377b41` |
| [bug_0003](bug_0003.md) | Spawned cubes/NPCs bypass room filtering and replicate to every client | Fixed in `6d59fa0` |
| [bug_0004](bug_0004.md) | KCC-internal state is not rollback-registered (ghost jumps after corrections) | Fixed in `c2c4b76` |
| [bug_0005](bug_0005.md) | Connect token embedded the client's own IP as the server address | Fixed in `89e44cc` |
| [bug_0006](bug_0006.md) | Netcode server rejects all connect tokens (wildcard bind vs whitelist) | Fixed in `89e44cc` |
| [bug_0007](bug_0007.md) | Dev console / pause modal doesn't lock movement: WASD/Space reach the server while typing | Fixed (uncommitted at filing) |
| [bug_0008](bug_0008.md) | `--no-common-assets` client panics when the pause modal opens (missing icon atlas) | Fixed (uncommitted at filing) |
| [bug_0009](bug_0009.md) | Walking at an angle: look input in the tick a menu opens turns the server's look but not the camera | Fixed (uncommitted at filing) |
| [bug_0010](bug_0010.md) | Standing on a spinning cube turns the server's look but not the camera, so W walks at an angle | Fixed (uncommitted at filing) |
| [bug_0011](bug_0011.md) | The server's look pitch is the mirror of the client's | Fixed (uncommitted at filing) |

Each report's own `Status` row is authoritative. Update this table when you file or close a bug.

## Filing a new report

Follow [`docs/agents/skills/bugreport.md`](../skills/bugreport.md). It covers:

- when to file, with one bug per file;
- the metadata table, including a `Commit` field that lists uncommitted changes in the
  running binary;
- the required sections, the severity scale and the status lifecycle.

Take the next free number and never renumber. Playtest reports that reproduce or verify a bug
live in [`../playtests/`](../playtests/), cited as "playtest NNNN F\<finding>".

## History, and why some text mentions `.typ` files

- **2026-10-02, `1f6a99f`: typst.** The ledger was created as `docs/bug_reports/bug_NNNN.typ`.
  Each report was compiled to a gitignored PDF next to its source.
- **2026-10-02, `f486cd1`: moved.** The ledger moved under `docs/agents/` with the other
  agent-facing docs.
- **2026-10-02: converted to Markdown**, right after the playtest reports were (see
  [`../playtests/README.md`](../playtests/README.md)). The reports never used typst's
  typesetting features. Markdown renders on GitHub, is easier for agents to read and write, and
  removes typst from the agent workflow entirely. The conversion was mechanical:
  - **Tables:** the metadata tables became Markdown tables.
  - **Headings:** typst headings became `##` headings.
  - **Cross-references:** paths to other bug reports were updated from `.typ` to `.md` so they
    resolve.
  - **Unchanged:** all other text, hashes and dates. `git log --follow <file>` shows each
    report's history across both formats.

Other documents written before the conversion, such as playtest reports 0019–0020 and ADR
0014, may still mention `bug_NNNN.typ` or compiling a bug report. They describe the project
as it was at the time; they are not broken links to fix.
