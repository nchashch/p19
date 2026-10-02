# Skill: writing Architecture Decision Records

ADRs live in `docs/agents/adr/` and record *why* a significant decision was made: what was
tried, what was rejected, and what it cost. `AGENTS.md` describes the code's **current** state
and is rewritten as the code changes. An ADR is a **dated snapshot** and stays as written.
Read this skill before writing one.

This skill applies to ADRs written **from 0014 onward**. Records 0001–0013 came before it and
are **not** retrofitted to this format — never edit them to match.

**`docs/humans/` is human-only: never create, edit, move or delete anything there** (see
`AGENTS.md` "Rules"). ADRs go in `docs/agents/adr/` only.

## 1. When to write one

Write an ADR when a change:

- picks one approach over real alternatives (a dependency, an architecture split, a protocol,
  a security posture);
- changes a contract other code relies on (what is replicated, who owns an entity, how
  identity is resolved);
- reverses or supersedes an earlier ADR;
- closes a span of work since the last ADR. In that case write a **catch-up ADR** that covers
  the whole span (see §6).

Don't write one for a bug fix with a single obvious cause and no alternatives. File a bug
report for that (`docs/agents/skills/bugreport.md`); the ADR's "Also since" section can
mention it.

## 2. File name and numbering

- `docs/agents/adr/NNNN-kebab-case-summary.md`, using the next free 4-digit number. Numbers
  are never reused, even if a record is withdrawn.
- Add the record to the index table in `docs/agents/adr/README.md` **in the same change**.
  Use the same title and status.

## 3. Header block (required, at the very top)

After the `# N. Title` heading, add a two-column table, modelled on the playtest report
header:

| Field | Content |
|---|---|
| `ADR` | `NNNN` |
| `Title` | Same as the heading |
| `Date` | ISO datetime with UTC offset (`2026-10-02 07:14 +0400`), from `date '+%Y-%m-%d %H:%M %z'`, never guessed |
| `Author` | The **exact model** that wrote it plus the harness (e.g. `Claude Opus 5.5 (Anthropic), via omp`). A human author gives their name. Never "AI" or "agent" alone |
| `Commit` | Short hash + title of `HEAD` when written (`git log -1 --format='%h %s'`), plus `+ uncommitted <what>` if the working tree differs |
| `Span` | Catch-up ADRs only: the commit range covered (`d2de40d..d333a09`) |
| `Status` | `Proposed` / `Accepted` / `Superseded by NNNN` / `Rejected` |
| `Supersedes` / `Related` | ADR numbers, bug reports (`bug_0005`), playtests (`playtest 0019`) |

## 4. Body sections (in this order)

1. **Context**: the problem, the forces and constraints, and what was already true. State
   facts, not intentions.
2. **Decision**: what was actually done. Name the concrete files, types and config fields
   (`server::networking::start_endpoint`, `NetcodeConfig.server_addr_check`). Use numbered
   subsections if there are several decisions.
3. **Alternatives considered**: every real option that was weighed, with the reason it lost.
   An option that was tried and reverted belongs here, with the evidence that killed it.
4. **Consequences**: what it gained (measured where possible), what it cost, and what is
   still open. Open items must be concrete enough that someone else can pick them up.
5. **Also since NNNN** (optional): smaller changes in the same span that don't deserve their
   own record, one line each.

## 5. Evidence rules

- Every factual claim must be traceable: a commit hash, a file or symbol, a playtest or bug
  number, or a measurement. If you didn't observe it, mark it `[INFERENCE]`.
- Get commit hashes from `git log`. Never copy them from prose.
- Describe what the code *does*, checked against the code. If `AGENTS.md` and the code
  disagree, trust the code and say that the doc is stale.
- Measurements need their method (`/proc/<pid>/stat`, three 5 s windows) as well as the number.

## 6. Catch-up ADRs

When several commits landed with no ADR:

1. `git log --reverse <last-ADR-commit>..HEAD` lists the span. Find the last ADR's commit
   with `git log --follow -- docs/agents/adr/<last>.md`.
2. Group the commits into **decisions**, not into commits. Several commits often serve one
   decision (fixes, CI attempts, follow-ups).
3. Each decision gets its own Decision subsection, alternatives and consequences. Pure
   housekeeping goes in "Also since".
4. Cite the playtests and bug reports produced during the span. Don't repeat their contents;
   link to them.

## 7. Immutability

- Once an ADR is `Accepted`, its body is **never edited** to match later changes; a new ADR
  supersedes it. Only two kinds of edit are allowed afterwards: updating its `Status` row
  (`Superseded by NNNN`) together with the matching index row, and fixing typos or broken
  links. Changes of meaning are not allowed.
- Never rewrite older ADRs to this skill's format (see the top of this file).

## 8. Checklist before finishing

- [ ] Next free number; file name in kebab-case
- [ ] Header table filled in with real `date`, `git log -1` and exact model name
- [ ] Context / Decision / Alternatives / Consequences all present
- [ ] Every claim traceable; unobserved claims marked `[INFERENCE]`
- [ ] `docs/agents/adr/README.md` index row added
- [ ] Any superseded ADR has its Status line and index row updated, and nothing else
