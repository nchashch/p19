# prototype_19

A Bevy 0.19 (Rust) 3D multiplayer game prototype, built "always multiplayer": a headless
authoritative `server` simulates the game world (physics, level loading, player spawning,
combat), and a `client` renders whatever the server replicates back and sends player intent
as network messages. Even singleplayer runs a local client *and* server — the idea is that
hosting real multiplayer later is "open a port," not a rewrite.

**Status: pre-release prototype, actively evolving, not currently playable end-to-end.**
In particular, the character controller has been deliberately gutted (every movement/physics
system is a `todo!()` stub right now) pending a rewrite using lightyear's own client-side
prediction — see [`AGENTS.md`](./AGENTS.md) and [`docs/adr/`](./docs/adr/) for the full story.
Expect things to be broken or half-built; this is a live development snapshot, not a demo.

## Getting started

Requires a Rust toolchain supporting edition 2024.

```sh
cargo check --workspace   # fastest way to confirm everything compiles
cargo run -p server --release   # start the authoritative server (listens on 0.0.0.0:6000)
cargo run -p client --release   # start the game client (connects once you press Connect)
```

**Assets are not tracked in git** (`client/assets/`, `server/assets/`, and `assets_src/` are
all gitignored) — there is currently no automated, documented process to provision them from
a fresh clone. If you're picking this up on a new machine, you'll need to copy those
directories over from an existing checkout by hand; there's no `cargo run` that "just works"
from source alone yet. This is a known gap, not an oversight — worth fixing before this repo
needs to support more than one working copy.

`cargo build -p <client|server> --release` on the host machine produces a binary linked
against the host's glibc, which will *not* run correctly on a Steam Deck or inside the
project's `steamrt4` toolbox — see `AGENTS.md`'s "Commands" section for why, and use
`./steam_deck_toolbox.sh cargo build -p <client|server> --release` instead when targeting
either.

## Project layout

- **`client/`** — the playable game: rendering, UI, input, camera, presentation. Sends intent
  as network messages; never decides outcomes itself.
- **`server/`** — the headless authoritative simulation: physics, level loading, player
  spawning, movement/combat resolution.
- **`shared/`** — simulation logic and network-message types both sides need to agree on
  (character controller, combat, player bundle, spawners, replication registration).
- **`assets_src/`** — raw source assets (`.blend` files, downloaded packs) processed into
  `client/assets/`/`server/assets/` for actual runtime use. Nothing loads from it directly.

Networking is [`lightyear`](https://github.com/cBournhonesque/lightyear) 0.30 over UDP/netcode.
Physics is [`avian3d`](https://github.com/Jondolf/avian). No test suite exists yet.

## Documentation

- **[`AGENTS.md`](./AGENTS.md)** — the real architecture reference: current module-by-module
  behavior, known gaps, conventions, and non-obvious "confirmed by testing" details. Written
  for (and kept up to date by) AI coding agents working in this repo, but equally useful for a
  human trying to understand *why* something is built the way it is. Start here for anything
  beyond a surface-level look.
- **[`docs/adr/`](./docs/adr/)** — Architecture Decision Records: short, dated writeups of
  specific significant decisions (and the alternatives/tradeoffs considered), kept separate
  from `AGENTS.md`'s "current state" description so the reasoning trail behind a decision
  doesn't get overwritten every time the doc is refreshed to match new code.

There's no `CHANGELOG.md` yet — this is deeply pre-release, so a user-facing changelog isn't
a priority right now; `git log` and the ADRs are the source of truth for what changed and why
in the meantime.

## License

The **code** in `client/`, `server/`, and `shared/` is dual-licensed under either the
[MIT License](./LICENSE-MIT) or the [Apache License, Version 2.0](./LICENSE-APACHE), at your
option — the standard convention across the Rust and Bevy ecosystem, matching the license of
most of this project's own dependencies.

This does **not** currently cover assets (`client/assets/`, `server/assets/`, `assets_src/`) —
those include third-party content under their own separate license terms (e.g. Kenney's CC0
packs, OFL-licensed fonts), plus original content whose licensing hasn't been decided yet. Not
resolved yet; treat anything under those directories as unlicensed/all-rights-reserved until
that's sorted out.
