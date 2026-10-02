# prototype_19

A Bevy 0.19 (Rust) 3D multiplayer game prototype, built "always multiplayer": a headless
authoritative `server` simulates the game world (physics, level loading, player spawning,
combat), and a `client` renders whatever the server replicates back and sends player intent
as network messages. Even singleplayer runs a local client *and* server — the idea is that
hosting real multiplayer later is "open a port," not a rewrite. Movement is real
server-authoritative, client-predicted simulation (via `bevy_ahoy`'s kinematic character
controller over `lightyear`'s replicated-input/rollback pipeline), not a stub — a player's
local input is simulated immediately and reconciled against the server's authoritative result,
the same architecture a shipped multiplayer game would use.

**Status: pre-release prototype, actively evolving, not currently playable end-to-end.**
Movement/prediction, combat, and cube/NPC spawning genuinely work now, but known gaps remain —
most visibly, disconnected players' characters aren't cleaned up (blocking a clean reconnect),
and all players share one spawn point — see [`AGENTS.md`](./AGENTS.md)'s "Known gaps"
section for the current, specific set, and
[`docs/agents/adr/`](./docs/agents/adr/) for the reasoning behind major decisions. Expect things to be broken
or half-built; this is a live development snapshot, not a demo.

An agent-driven QA tool API (BRP + MCP, gated behind the `dev-tools` cargo feature) lets an AI
coding agent actually play the game headlessly — connect, navigate menus, move, inject input,
take screenshots — to drive real regression testing and bug-hunting through the same replicated
pipeline a human player uses, not a separate mock. Its findings accumulate as dated reports in
[`docs/agents/playtests/`](./docs/agents/playtests/index.md) (start at that index) rather than being lost
after each session; several real bugs in this repo were found and root-caused this way.

## Getting started

Requires a Rust toolchain supporting edition 2024, and [Git LFS](https://git-lfs.com/) (`git
lfs install`, once per machine) to pull the actual screenshot images referenced by
`docs/agents/playtests/` reports — the repo still clones and builds fine without it, you'd just see
LFS pointer text instead of images for those specific files.

```sh
cargo check --workspace   # fastest way to confirm everything compiles
cargo run -p p19-server --release   # start the authoritative server (listens on 0.0.0.0:6000)
cargo run -p p19-client --release   # start the game client (connects once you press Connect)
```

To drive the client as an agent instead of a human — headless, no window, scriptable over
HTTP — build with `--features dev-tools` and run with `--mcp`; see
[`docs/agents/skills/playtest.md`](./docs/agents/skills/playtest.md) for the full playbook (launch recipe,
tool API surface, known gotchas) and [`docs/agents/adr/0009`](./docs/agents/adr/0009-agent-tool-api-via-brp.md)
for the design behind it.

**Assets are not tracked in git** (`assets/client/`, `assets/server/`, and `assets/src/` are
all gitignored) — there is currently no automated, documented process to provision them from
a fresh clone. If you're picking this up on a new machine, you'll need to copy those
directories over from an existing checkout by hand; there's no `cargo run` that "just works"
from source alone yet. This is a known gap, not an oversight — worth fixing before this repo
needs to support more than one working copy. (This doesn't apply to `docs/agents/playtests/screenshots/`
specifically — those *are* tracked, via Git LFS, since they're small and meant as durable
history rather than runtime content.)

`cargo build -p <p19-client|p19-server> --release` on the host machine produces a binary linked
against the host's glibc, which will *not* run correctly on a Steam Deck or inside the
project's `steamrt4` toolbox — see `AGENTS.md`'s "Commands" section for why, and use
`scripts/steam_deck_toolbox.sh cargo build -p <p19-client|p19-server> --release` instead when targeting
either.

## Project layout

- **`crates/client/`** (package `p19-client`) — the playable game: rendering, UI, input,
  camera, presentation. Sends intent as network messages; never decides outcomes itself.
- **`crates/server/`** (package `p19-server`) — the headless authoritative simulation:
  physics, level loading, player spawning, movement/combat resolution.
- **`crates/shared/`** (package `p19-shared`) — simulation logic and network-message types
  both sides need to agree on (character controller, combat, player bundle, spawners,
  replication registration).
- **`assets/client/`, `assets/server/`** — each binary's runtime asset root (found
  automatically from a development checkout; see `AGENTS.md` "Commands").
- **`assets/src/`** — raw source assets (`.blend` files, downloaded packs) processed into
  `assets/client/`/`assets/server/` for actual runtime use. Nothing loads from it directly.

Networking is [`lightyear`](https://github.com/cBournhonesque/lightyear) 0.30 over UDP/netcode.
Physics is [`avian3d`](https://github.com/Jondolf/avian). Automated tests are a handful of unit
tests (`cargo test --workspace`); there is no integration test suite yet.

## Documentation

- **[`AGENTS.md`](./AGENTS.md)** — the real architecture reference: current module-by-module
  behavior, known gaps, conventions, and non-obvious "confirmed by testing" details. Written
  for (and kept up to date by) AI coding agents working in this repo, but equally useful for a
  human trying to understand *why* something is built the way it is. Start here for anything
  beyond a surface-level look.
- **[`docs/agents/adr/`](./docs/agents/adr/)** — Architecture Decision Records: short, dated writeups of
  specific significant decisions (and the alternatives/tradeoffs considered), kept separate
  from `AGENTS.md`'s "current state" description so the reasoning trail behind a decision
  doesn't get overwritten every time the doc is refreshed to match new code.
- **[`docs/agents/skills/`](./docs/agents/skills/)** — task-specific playbooks for AI agents working in this
  repo, e.g. [`playtest.md`](./docs/agents/skills/playtest.md) (how to drive the game headlessly via
  the agent tool API) and [`bugreport.md`](./docs/agents/skills/bugreport.md) (how to file bug reports) — read the
  relevant one before attempting its task; it encodes gotchas that otherwise cost the same
  debugging time again.
- **[`docs/agents/playtests/`](./docs/agents/playtests/index.md)** — dated reports from every agent-driven
  playtest session (state tours, bug reproductions, fix verifications), each with screenshots
  of what the agent actually saw. Start at the index; every report cites the exact git commit
  it was run against. Written to be a real, searchable debugging history, not a one-off log —
  several real bugs in this repo were root-caused by an agent reading back through these.

There's no `CHANGELOG.md` yet — this is deeply pre-release, so a user-facing changelog isn't
a priority right now; `git log` and the ADRs are the source of truth for what changed and why
in the meantime.

## License

The **code** in `crates/` is dual-licensed under either the
[MIT License](./LICENSE-MIT) or the [Apache License, Version 2.0](./LICENSE-APACHE), at your
option — the standard convention across the Rust and Bevy ecosystem, matching the license of
most of this project's own dependencies.

This does **not** currently cover assets (`assets/client/`, `assets/server/`, `assets/src/`) —
those include third-party content under their own separate license terms (e.g. Kenney's CC0
packs, OFL-licensed fonts), plus original content whose licensing hasn't been decided yet. Not
resolved yet; treat anything under those directories as unlicensed/all-rights-reserved until
that's sorted out.
