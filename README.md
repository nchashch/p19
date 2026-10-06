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

**Status: pre-release prototype, actively evolving.** It runs end to end from a fresh clone:
start the server and the client, connect, pick a level in the lobby, and play — movement with
prediction, combat, and cube/NPC spawning all work. Known gaps remain — most visibly,
disconnected players' characters aren't cleaned up (rejoining needs a server restart), and all
players share one spawn point — see [`AGENTS.md`](./AGENTS.md)'s "Known gaps" section for the
current, specific set, and [`docs/agents/adr/`](./docs/agents/adr/) for the reasoning behind
major decisions. Expect rough edges; this is a live development snapshot, not a finished game.

An agent-driven QA tool API (BRP + MCP, gated behind the `dev-tools` cargo feature) lets an AI
coding agent actually play the game headlessly — connect, navigate menus, move, inject input,
take screenshots — to drive real regression testing and bug-hunting through the same replicated
pipeline a human player uses, not a separate mock. Its findings accumulate as dated reports in
[`docs/agents/playtests/`](./docs/agents/playtests/index.md) (start at that index) rather than being lost
after each session; several real bugs in this repo were found and root-caused this way.

## Getting started

Requirements:

- a Rust toolchain supporting edition 2024;
- [Git LFS](https://git-lfs.com/) — **required**: the binary game assets (models, textures,
  the skybox, sounds) and the playtest screenshots are stored as LFS objects;
- on Linux, Bevy's system libraries (audio, input, windowing; see Bevy's
  [Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)).

```sh
git lfs install                                  # once per machine, before cloning
git clone https://github.com/nchashch/p19.git
cd p19
cargo run -p p19-server --release                # terminal 1: the authoritative server
cargo run -p p19-client --release                # terminal 2: the game client
```

In the client: **Connect** → in the lobby, pick a level → **Play**. The client connects to
`127.0.0.1` by default; to play against a server on another machine, set `server_ip` in
`assets/client/config.toml`.

If you cloned before installing Git LFS, the asset files are small text pointers (they start
with `version https://git-lfs.github.com/spec/v1`) and the game can't load them; run
`git lfs install && git lfs pull` to fetch the real files.

On first run, the server creates its netcode key and token-TLS identity in
`assets/server/network/`, and the client pins the server's certificate fingerprint in
`assets/client/network/` the first time it connects. Both directories are machine-local and
gitignored. If the server's identity is regenerated, delete
`assets/client/network/token-tls-fingerprint.txt` so the client pins the new one.

To drive the client as an agent instead of a human — headless, no window, scriptable over
HTTP — build with `--features dev-tools` and run with `--mcp`; see
[`docs/agents/skills/playtest.md`](./docs/agents/skills/playtest.md) for the full playbook (launch recipe,
tool API surface, known gotchas) and [`docs/agents/adr/0009`](./docs/agents/adr/0009-agent-tool-api-via-brp.md)
for the design behind it.

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
  automatically from a development checkout; see `AGENTS.md` "Commands"). Binary assets are
  stored with Git LFS, text assets (levels, translations, shaders, config) in plain git.
- **`assets/src/`** — raw source assets (`.blend` files, downloaded packs) processed into
  `assets/client/`/`assets/server/` for actual runtime use. Nothing loads from it directly, and
  it isn't in git.

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

This does **not** cover the assets (`assets/client/`, `assets/server/`), which are in this
repository for running the game but are not licensed for reuse: the original content (models,
levels, translations, shaders) has no license decided yet, so treat it as
unlicensed/all-rights-reserved. The third-party content in it is public domain (CC0); see
[`assets/CREDITS.md`](./assets/CREDITS.md) for full attribution:

- input-prompt glyph sheets (`textures/input_prompts/`): Kenney,
  [Input Prompts](https://kenney.nl/assets/input-prompts);
- the night-sky skybox (`skyboxes/night_sky.ktx2`): ambientCG,
  [Night Sky HDRI 012](https://ambientcg.com/view?id=NightSkyHDRI012);
- the floor texture in `rigs/environment/start.glb`: Poly Haven,
  [Rubber Tiles](https://polyhaven.com/a/rubber_tiles) by Amal Kumar.
