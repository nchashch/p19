# Agent Playtest 0039 — End to End on the Git-Tracked Assets Alone

| Field | Value |
|---|---|
| Date | 2026-10-07 01:38 – 01:40 +0400 |
| Commit | `ff3fc3b` "Use system fonts instead of bundled asset fonts" + uncommitted `assets/client`, `assets/server` (to be committed: binary files via Git LFS, `.gitattributes` per extension; `network/` ignored) |
| Agent | omp session, Claude Opus 5.5 (Anthropic) |
| Clients | 1× `target/debug/p19-client --mcp` (dev-tools, rendered), `BEVY_ASSET_ROOT=/tmp/p19-e2e/client` |
| Server | `target/release/p19-server`, fresh, `BEVY_ASSET_ROOT=/tmp/p19-e2e/server` |
| Level | `levels/minimal.level.ron` |
| Transports | game: UDP/netcode :6000 · token endpoint: HTTPS :6001 · client QA: BRP :15702 + MCP :15710 |

## Purpose

The assets now go into git (binary files through Git LFS). A fresh clone must run end to end
from them alone: no files copied from another checkout, no pre-existing network identity.

## Setup

The 47 files `git add --dry-run assets/` selects (19 LFS: 14 `.glb`, 1 `.ktx2`, 2 `.png`,
2 `.wav`; 28 text) were copied into an isolated asset root per binary
(`/tmp/p19-e2e/<client|server>/assets/`). The ignored `network/` directories were left out,
as a clone wouldn't have them.

## Verification

| Step | Observed |
|---|---|
| Server start | `generated new netcode private key`, `generated new self-signed token TLS identity`; listening on :6000 / :6001 |
| Client start | asset loading → `MainMenu`; buttons `Connect, Options, Credits, Quit, Language` |
| Connect | `Lobby`; the client created `network/token-tls-fingerprint.txt` (TOFU pin) |
| Level list | `levels/minimal.level.ron` (`level-minimal-name`) |
| Select level, Play | `InGame`; player at (0, 0.915, 0) |
| Move forward 40 ticks | player at (0, 0.915, −8.67) |
| Spawn NPC, nameplates on | NPC model, its nameplate and the skybox render |

![in_game_tracked_assets.png](screenshots/playtest_0039/in_game_tracked_assets.png)

## Findings

**F1 — The git-tracked assets are sufficient.** Nothing outside them is needed to run the game;
the server's and client's network state is generated on first run.

**F2 — Not tested here:** an actual `git clone` (the assets weren't committed yet; this run
used exactly the set git would commit), a clone without Git LFS (pointer files instead of
assets), and the windowed client.
