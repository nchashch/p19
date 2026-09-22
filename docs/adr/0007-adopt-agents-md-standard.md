# 7. Adopt the AGENTS.md standard for the agent-guidance file

Date: 2026-09-22

## Status

Accepted (supersedes [0006](./0006-rename-claude-md-to-llm-md.md))

## Context

ADR 0006 renamed `CLAUDE.md` to `LLM.md` to strip one vendor's name off the agent-guidance
document, on the rationale that the file serves LLM agents in general, not Claude
specifically. That rationale stands. What 0006 didn't yet account for is that `LLM.md` is
not a convention at all: no tool discovers it, it has no spec or steward, and nothing
outside this repo's own README links uses the name. The rename fixed the vendor-branding
problem while leaving the file just as invisible to tooling as before — discovery still
worked only through the project's own README.

`AGENTS.md` is the actual cross-vendor standard: originated mid-2025 (OpenAI Codex, Amp,
Cursor, Factory, Google Jules), donated in December 2025 to the Linux Foundation's Agentic
AI Foundation, and natively read by Codex, Gemini CLI, Cursor, Windsurf, Devin, GitHub
Copilot, VS Code, JetBrains (Junie), Zed, Warp, Aider, goose, opencode, RooCode, and more —
60k+ GitHub repositories. The format is plain Markdown with no required fields, so this
repo's existing document is conformant as-is. Notably, opencode — the agent harness this
repo is currently developed in — is on the adopters list and began reading the file
automatically the moment it appeared under the right name.

0006 recorded `AGENTS.md` as "the strongest alternative" and passed it over on the
reasoning that the project reads this file deliberately rather than via tool magic —
treating deliberate reading and auto-discovery as mutually exclusive. They are not: nothing
about the standard prevents a human or agent from reading the file on purpose, while
filename-based discovery is pure additional upside. With the standard's adoption scale
confirmed, that tradeoff resolves the other way.

## Decision

Rename `LLM.md` to `AGENTS.md`, reword its header to "This file provides guidance to AI
coding agents when working with code in this repository", and sweep every `LLM.md` pointer
in the same change: 23 references across 14 files — `README.md`, the root and member
`Cargo.toml`s, `steam_deck_toolbox.sh`, six Rust source comments, `docs/adr/README.md`'s
relative link, and prose pointers in ADRs 0003–0005 — plus the file's own title. Per this
directory's house rule, ADR 0006's body is untouched; only its Status section changes, so
the CLAUDE.md → LLM.md step remains part of the recorded history rather than being edited
away.

Claude Code — the one notable holdout, still reading `CLAUDE.md` natively (native
`AGENTS.md` support is an open feature request there as of 2026-03) — is deliberately *not*
accommodated with a pointer file: the one-line `CLAUDE.md`-pointing-at-`AGENTS.md` shim is
exactly the second file whose only job is to not drift, which 0006 correctly rejected when
proposed for the old name, and this decision rejects equally. If Claude Code auto-loading
ever matters here, add the shim then, as a compatible follow-up.

## Consequences

- The file is now auto-discovered by every AGENTS.md-reading agent — including opencode,
  the harness this repo is currently developed with — with no reliance on README links or
  explicit instructions. Deliberate, top-to-bottom reading (the documented usage pattern)
  is unchanged.
- Claude Code does not auto-load the file. Accepted; the known follow-up (a one-line
  pointer shim) is documented above rather than pre-emptively added.
- The sweep is complete as of this change: zero `LLM.md` references remain anywhere except
  ADR 0006's own historical body and the index entry titling it.
- From git's perspective the `LLM.md` hop never existed — that file was never committed —
  so history will record a single `CLAUDE.md` → `AGENTS.md` rename. ADRs 0006 and 0007
  together are the record of the actual two-step path: rename for tool-agnosticism first,
  standardize once the standard's adoption was confirmed.
