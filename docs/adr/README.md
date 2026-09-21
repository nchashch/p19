# Architecture Decision Records

Short, dated writeups of specific significant decisions made in this project — the
alternatives considered, why one was picked, and what it cost. These are deliberately
separate from [`CLAUDE.md`](../../CLAUDE.md): `CLAUDE.md` describes the *current* state of
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

New records should be added to this index in the same PR/commit that adds the file.
