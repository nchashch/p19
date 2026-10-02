# Agent playtests

This directory is the project's running log of **agent-driven playtests**. In each run an AI
coding agent starts the real game headlessly: a `server` plus one or more `client --mcp` or
`--no-render` instances. It plays the game through the agent tool API (BRP + MCP, see
[ADR 0009](../adr/0009-agent-tool-api-via-brp.md)) and writes down what it saw, against the
exact git commit it ran on.

Every report is a permanent record and is never deleted. State tours, bug reproductions and fix
verifications all go here. Several real bugs in this repo were found, or root-caused, by reading
back through these reports.

**Start at [`index.md`](index.md).**

## Layout

| Path | What it is |
|---|---|
| `playtest_NNNN.md` | One report per run, numbered in order (`0001`, `0002`, …). GitHub-flavored Markdown. |
| `index.md` | Newest-first index of reports, with date, commit, agent and a short summary. |
| `screenshots/playtest_NNNN/*.png` | Curated screenshots that the reports link to. Tracked with **Git LFS**; run `git lfs install` before cloning, or you get pointer files instead of images. |
| `dist/` | Gitignored local staging for raw screenshot captures from the tool API. Never committed. |

Some runs also ship their own isolated, plaintext game assets under
[`playtest_assets/`](../../../playtest_assets/) at the repo root (since playtest 0009).

## Writing a new report

Follow [`docs/agents/skills/playtest.md`](../skills/playtest.md) §10. It covers when a report
is required (every run, however small), the house style (metadata table with a `Commit` field,
`## Findings`, image-plus-caption figures) and adding the entry to `index.md`. Take the next free
number, and use the latest report as the template.

## History, and why older reports mention `.typ` files

The reports have changed format and location three times. Each report's text was left as
written, so older reports still describe the layout they were filed under:

1. **2026-09-23 → 2026-10-02: typst, one directory per report.** Playtests began as
   `docs/playtests/playtest_NNNN/report.typ`, compiled with typst to a gitignored
   `dist/playtest_NNNN.pdf`. Screenshots were linked as `../screenshots/...`. Playtest 0001,
   for example, still describes this layout in its introduction.
2. **2026-10-02: moved and flattened.** Agent-facing docs moved under `docs/agents/`
   (`f486cd1`). The per-report directories were then flattened to `playtest_NNNN.typ`
   (`c45e1b0`), to match `docs/agents/bug_reports/`.
3. **2026-10-02: converted to Markdown.** The reports never used typst's typesetting features.
   Markdown renders directly on GitHub and is easier for agents to read and write, and it
   removes typst from the playtest workflow. The `.typ` sources were converted mechanically:
   - **Tables and figures:** tables became Markdown tables, and figures became an image link
     with an italic caption.
   - **Headings and cross-references:** headings became `#` headings, and typst
     cross-references became heading links.
   - **Unchanged:** the text, commit hashes and dates are the same as in the typst originals.
     `git log --follow <file>` shows each report's full history across all three formats.

So a report that mentions `report.typ`, compiling a PDF, or a path like `bug_0004.typ` is
describing the project at the time it was written; it is not a broken link to fix. The bug
reports in [`../bug_reports/`](../bug_reports/) are still typst for now.
