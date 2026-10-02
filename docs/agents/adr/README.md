# Architecture Decision Records

Short, dated writeups of specific significant decisions made in this project — the
alternatives considered, why one was picked, and what it cost. These are deliberately
separate from [`AGENTS.md`](../../AGENTS.md): `AGENTS.md` describes the *current* state of
the code and gets rewritten as things change, which means the reasoning trail behind a past
decision (what was tried first, why it didn't work, what the tradeoff actually was) tends to
get compressed away or dropped entirely once it's no longer "current." ADRs are meant to stay
put — even once a decision is superseded, the old record stays as history rather than being
edited to match.

Each record uses the same shape: **Context** (the problem, forces, constraints), **Decision**
(what was actually done), **Consequences** (what it bought, what it cost, what's still open).

## Index

| # | Title | Status |
|---|-------|--------|
| [0001](./0001-generic-paginated-selector-widget.md) | Generic paginated selector widget, replacing continuous scrolling | Accepted |
| [0002](./0002-bundle-cjk-capable-fonts.md) | Bundle CJK-capable fonts instead of relying on system font discovery | Accepted |
| [0003](./0003-room-based-interest-management.md) | Room-based interest management for lobby/in-game separation | Accepted |
| [0004](./0004-decouple-player-character-from-connection-entity.md) | Decouple the player character from the client connection entity | Accepted |
| [0005](./0005-gut-character-controller-for-prediction-rewrite.md) | Gut the character controller pending a client-side-prediction rewrite | Accepted |
| [0006](./0006-rename-claude-md-to-llm-md.md) | Rename CLAUDE.md to LLM.md, making agent guidance tool-agnostic | Superseded by [0007](./0007-adopt-agents-md-standard.md) |
| [0007](./0007-adopt-agents-md-standard.md) | Adopt the AGENTS.md standard for the agent-guidance file | Accepted |
| [0008](./0008-adopt-bevy-ahoy-character-controller.md) | Adopt bevy_ahoy as the character controller, over lightyear-replicated BEI input | Accepted |
| [0009](./0009-agent-tool-api-via-brp.md) | Expose an agent tool API on the client via the Bevy Remote Protocol, MCP-wrappable | Accepted |
| [0010](./0010-device-level-input-mocking-for-agent-tool-api.md) | Device-level input mocking (gamepad, keyboard, mouse) for the agent tool API | Accepted |
| [0011](./0011-agent-vision-and-fleet-improvements.md) | Vision and fleet improvements: data-first observation, UI-tree dumps, state-fused/cropped captures, per-client ports | Accepted |
| [0012](./0012-no-render-agent-client-mode.md) | A `--no-render` agent-client mode: the headless host without wgpu/Vulkan | Accepted |
| [0013](./0013-server-determinism-and-session-replay.md) | Server-side determinism hardening and session recording/replay for post-release debugging | Accepted |
| [0014](./0014-multiplayer-hardening-netcode-tokens-and-portable-builds.md) | Multiplayer hardening, per-client netcode tokens over TLS, and portable CI builds | Accepted |

New records should be added to this index in the same PR/commit that adds the file. From 0014
onward, records follow [`docs/agents/skills/adr.md`](../skills/adr.md) (header table, author
model, commit, evidence rules); 0001–0013 predate it and are intentionally left as written.
