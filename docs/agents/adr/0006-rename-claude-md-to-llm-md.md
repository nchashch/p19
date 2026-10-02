# 6. Rename CLAUDE.md to LLM.md, making agent guidance tool-agnostic

Date: 2026-09-22

## Status

Superseded by [ADR 0007](./0007-adopt-agents-md-standard.md)

## Context

The workspace root has always carried a single, large "current state of the repo" document
for AI coding agents — module-by-module behavior, conventions, commands, confirmed-by-testing
gotchas, and the known-gaps list — linked from `README.md` as the real architecture reference
and kept up to date by agents working in the repo. It was originally named `CLAUDE.md`,
following the convention popularized by Anthropic's Claude Code (which auto-loads a file of
that name as agent guidance), and its header said as much: "This file provides guidance to
Claude Code (claude.ai/code) when working with code in this repository."

Two things made that name wrong for what the file actually is:

- **The content is vendor-agnostic.** Nothing in the file depends on Claude or Claude Code
  specifically. It is repo knowledge — architecture, conventions, commands, gaps — useful to
  any LLM agent asked to work in this codebase, and to humans. Naming it after one vendor's
  tool undersold its audience and tied a general-purpose document to a product name.
- **The name had leaked deep into the codebase as a pointer.** Sixteen references across
  thirteen files — in-code comments, `Cargo.toml` comments, `steam_deck_toolbox.sh`, Rust doc
  comments, ADR prose, and `README.md` — directed readers to "CLAUDE.md's" sections. Any
  rename is only complete if it sweeps those in the same change, or the pointers rot into
  dead ends. This is the same second-source-of-truth drift risk the project has documented
  elsewhere (e.g. the dropped `manifest.toml` id→path indirection), just with filenames.

## Decision

Rename the file to `LLM.md` — the name states plainly what the file is: guidance for LLM
agents in general, not for one tool — and reword its header to match ("This file provides
guidance to LLMs when working with code in this repository."). Sweep every stale pointer to
the old name in the same change: all 16 occurrences across 13 files, including the relative
link in `docs/agents/adr/README.md`, both member `Cargo.toml`s, the root `Cargo.toml`,
`steam_deck_toolbox.sh`, six Rust source comments, and prose pointers in ADRs 0003–0005.
Git's rename detection preserves the file's history linkage when the change is staged.

Alternatives considered and passed over:

- **Keep `CLAUDE.md`, add a pointer file for other tools** (e.g. an `AGENTS.md` that just
  says "read CLAUDE.md"): creates a second file whose only job is to not drift from the
  first — precisely the failure mode this project rejects elsewhere. A symlink avoids the
  duplication but adds a class of filesystem object the repo doesn't otherwise use, with
  checkout-portability and web-UI caveats, to save a one-line rename.
- **Rename to `AGENTS.md`** (the emerging cross-tool convention several agent tools
  auto-discover): the strongest alternative, since it buys general auto-discovery. Passed
  over because this project's usage pattern is deliberate reading, not tool magic — the file
  is linked from `README.md`, pointed at explicitly, and treated as an architecture reference
  rather than something a tool happens to skim by filename. `LLM.md` says what the file is
  without deferring to whichever convention is currently ascendant.

## Consequences

- The filename now matches the file's actual audience. No content moved; only the name, the
  header sentence, and the 16 stale pointers changed.
- Tools that auto-loaded `CLAUDE.md` by filename no longer do so, and tools that auto-load
  only `AGENTS.md` never did. The file is reached deliberately — via its `README.md` link or
  an explicit instruction — which matches how it was already being used. If friction shows
  up, adding an `AGENTS.md` pointer later is a compatible follow-up, not a redesign.
- The sweep is complete as of this change: zero case-insensitive matches for `claude.md`
  remain in the working tree. New prose pointing at the document must say `LLM.md`.
- ADRs 0003–0005 were touched, but only to update forward pointers ("see the gap list"),
  never their substantive records — consistent with this directory's rule that records stay
  put. The decisions themselves are untouched; only the name of the living doc they point
  readers at changed.
