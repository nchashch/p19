# Skill: Filing bug reports (`docs/agents/bug_reports/`)

Read this before filing a bug report. Bug reports are the project's permanent defect ledger:
every reproducible flaw, regression, misbehavior, or design-level hazard gets one `.typ` file,
numbered, dated, pinned to the exact code state it was found in — and **retained forever, even
after the bug is fixed** (fixed reports are marked `Fixed` in place, never deleted). Fixed
reports are regression armor: they encode the repro, the root cause, and the fix so the same
class of bug is cheap to recognize the next time. Supplements (does not replace) `AGENTS.md`
and `docs/agents/skills/playtest.md` (whose harness drives the reproductions).

## 1. When to file

- Any *reproducible* defect: crash, wrong behavior, desync, data loss, security hole,
  performance cliff, broken tooling, doc/code mismatch that cost real debugging time.
- Design-level hazards (e.g. shared spawn points stacking joiners — see playtest 0018) are
  filed too, tagged `by-design-question` until the owner rules on them.
- **One bug per file.** Two symptoms with one root cause = one file (say so); two unrelated
  causes = two files, cross-referenced.
- File even when the bug is *already fixed by the time you write* — the report then documents
  the failure window and the fix (that is how `Fixed` reports accumulate history).

## 2. Layout and numbering

- One file per bug: `docs/agents/bug_reports/bug_XXXX.typ` (`bug_0001`, `bug_0002`, …).
- **Find the next free number; never reuse or renumber.** Numbers are permanent identities —
  reports are retained forever, fixed or not, so other documents can cite `bug_0007` stably.
- The `.typ` source is the record of truth and is **tracked in git**. Compile it in place for
  review:
  ```sh
  typst compile --root docs docs/agents/bug_reports/bug_XXXX.typ
  ```
  The produced `bug_XXXX.pdf` is gitignored (`/docs/agents/bug_reports/*.pdf`) — regenerable, never
  committed. (Do not put PDFs in `dist/`; there is no per-bug dist — the source is the
  artifact.)

## 3. Metadata table (top of every report)

Follow the playtest-report house style: `#set document(title: "Bug 0007 — <one-line summary>", …)`,
numbered headings, a metadata `#table`, a `<findings>` label on the findings block, and
single-star Typst strong (`*bold*` — `**` is Markdown and silently renders unbolded; this bit
three playtest reports before being caught).

Required fields:

| Field | Content |
|---|---|
| `Bug` | `bug_XXXX` |
| `Date discovered` | When first observed |
| `Commit (local state actually running)` | Exact local `git log -1 --format="%h %s"` **plus a per-file list of uncommitted changes that were in the running binary** — pushed or not. The report must reflect the code that actually ran (convention established by the owner; playtests 0017–0019 follow it) |
| `Discovered by` | Session/agent/human |
| `Component` | Module(s) involved (`server::combat`, `client::lifecycle/networking`, …) |
| `Severity` | S1–S4 (see §6) |
| `Status` | `Open` → `Investigating` → `Fix in progress` → **`Fixed in <commit>`** / `Won't fix` / `By design` — updated **in place** as the bug moves |
| `Related` | Playtest reports (by number + finding, e.g. "playtest 0015 F3"), other bug numbers, AGENTS.md gap entries |

## 4. Required sections

1. **Summary** — one paragraph: what is broken, where, and the user-visible consequence. No
   root-cause speculation here.
2. **Steps to reproduce** — exact, copy-pasteable, from a cold start: launch lines (with env
   vars — `BEVY_ASSET_ROOT`/`CARGO_MANIFEST_DIR` matter, see `docs/agents/skills/playtest.md` §2),
   every `game/*` harness call with its *actual* JSON payload, waits/sleeps, and the
   observation command that shows the failure. A repro someone cannot paste into a shell is
   not a repro. Prefer the QA harness (`game/state`, `game/select`, `game/trigger`,
   `world.query`) over pixel/mouse mocking (see playtest.md §7 — data over pixels), and note
   which client modes it applies to (`--no-render` vs rendered vs windowed — several bugs in
   this project are mode-specific).
3. **Expected vs actual** — one line each, concrete (positions, HP values, log lines).
4. **Evidence** — verbatim log lines with timestamps, BRP dumps, entity/component listings.
   Attach what you saw, not a paraphrase.
5. **Root cause** — only what is *confirmed*. Hypotheses go here explicitly labeled
   `Hypothesis (not confirmed)` with the discriminating experiment that would confirm them.
   If you ran an ablation (stash the candidate fix, re-test), say so — ablations are what
   turned playtest 0015's "misleading `ServerMutateTicks` error" into playtest 0016's actual
   root cause (`--no-render` missing `SyncWorldPlugin`).
6. **Fix** (once fixed) — commit hash, files changed, one paragraph on the mechanism, and the
   verification evidence (the repro run going green).
7. **Follow-ups** — secondary bugs found while diagnosing, doc corrections applied, scope
   deliberately deferred (say why and where it is tracked).

## 5. Best practices (the canons)

- **Reproduce before filing.** Read `docs/agents/skills/playtest.md` and drive the game through the
  harness; verify the build actually rebuilt (a stale binary silently tests old code — this
  has cost multiple sessions). An unreproducible bug is still filed, marked
  `Not reproduced`, with everything attempted listed — that inventory is what eventually
  cracks it.
- **Distinguish observation from conclusion.** "B's HP stayed 100 after the attack" is an
  observation; "attacks don't work" is a conclusion that was wrong twice in this project's
  history (attacks were silently dropped by caster resolution; then `Selected` was never set
  headlessly). Write observations; keep conclusions in Root cause with their evidence.
- **Corrections, not rewrites.** If a filed root cause turns out wrong (see playtest 0014's
  joiner-hover misdiagnosis, re-verified and corrected in playtest 0018), *append* a
  correction note with the new evidence and strike/annotate the old text — never silently
  rewrite. The wrong-turn is diagnostic information (it records which experiment
  discriminated).
- **Attribute races and multi-cause failures by ablation.** When two changes are in flight,
  stash one and re-run; say in the report which commit was stashed and what the control run
  showed (playtests 0014/0015 both did this; playtest 0015's F3 would have been misfiled
  otherwise).
- **A fix updates three places**: the bug's `Status`/`Fix` section, `AGENTS.md` (gap entries
  and module bullets that described the bug), and — if the repro revealed a harness gap —
  `docs/agents/skills/playtest.md` or `dev::tool_api`. A bug fix that leaves stale documentation
  behind is an unfinished fix (this exact debt accumulated before the AGENTS.md-update
  convention was adopted).
- **Severity honestly, not aspirationally.** Severity reflects player/user impact today, not
  how interesting the bug is.
- **Fixed means verified.** `Fixed in <commit>` requires the repro run observed green after
  the fix — "it compiles" is not verification. If verification is blocked, the report stays
  `Fix in progress` with the blocker named (see bug: the death-path panic sat behind
  `ServerMutateTicks` until playtest 0016 root-caused it).

## 6. Severity guide

| Level | Meaning | Examples from this project |
|---|---|---|
| S1 | Crash, data loss, or the app unusable for its primary flow | Death-path client panic (playtest 0015/0016) |
| S2 | Core gameplay flow broken, no crash | Caster resolution silently dropping every attack/kill |
| S3 | Degraded experience, correct behavior reachable another way | Joiner hover (spawn stacking); room-filtering leaks |
| S4 | Cosmetic, tooling friction, doc/code mismatch | `Npc` invisible to BRP queries; stale doc comments |

## 7. Status lifecycle

`Open` → (`Investigating`) → (`Fix in progress`) → `Fixed in <hash>`; or `Won't fix` / `By
design` (with the owner's ruling quoted). Reopening is allowed: flip back to `Open` with a
note on the new evidence and the new commit — the number never changes.

## 8. Cross-references

- Playtest reports live in `docs/agents/playtests/playtest_NNNN/report.typ`; cite as
  "playtest NNNN F<finding>".
- Bugs that produced harness/tooling fixes should name the tool method that now covers them
  (e.g. `game/select` exists because headless clients cannot aim the crosshair — see the
  `docs/agents/skills/playtest.md` §3 table).
- Backfilling known-open issues into this directory is encouraged: the current open set at
  the time this skill was written is the KCC-internal rollback-registration gap
  (`CharacterControllerState`/`AccumulatedInput` not rollback-registered — needs ahoy-side
  exposure), plus the by-design questions (spawn-point separation, production netcode
  upgrade) that playtests 0018/0019 closed out.
