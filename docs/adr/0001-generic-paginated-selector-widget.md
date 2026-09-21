# 1. Generic paginated selector widget, replacing continuous scrolling

Date: 2026-09-20

## Status

Accepted

## Context

The main menu needed a language-selection popup: a scrollable list of language options under
a "Language" button. The first implementation used `bevy_ui_widgets::ScrollArea` plus
`bevy_feathers::FeathersScrollbar` — the "standard" continuous-scrolling approach.

This produced a sustained sequence of visual bugs, each fixed only to expose the next: the
focus ring clipped at the top/bottom of the scroll viewport; padding fixed it only until you
actually scrolled, since padding only protects the first/last row of the *whole* list, not
whichever row is at the *viewport edge* during a scroll; `overflow_clip_margin` fixed that;
then the scrollbar itself started overflowing the popup's own border once one was added; then
came a "no clipped item should ever be visible" requirement, which continuous pixel-based
scrolling structurally can't guarantee without scroll-snapping — a scroll-snap attempt was
built and then explicitly rejected ("this is wrong, revert it") for producing the wrong feel.

Each fix addressed a real symptom but the underlying cause was structural: a row is either
fully on-screen or partially clipped depending on the exact scroll offset, and there's no way
to prevent "partially clipped" while scroll position is continuous.

## Decision

Abandon continuous scrolling entirely. Replace it with a **fixed-size paginated window**:
a constant number of row "slots" (`SELECTOR_VISIBLE_ROWS = 5`) at static screen positions,
plus a `SelectorWindowStart` index into the underlying option list. Scrolling/paging rebinds
each slot's displayed content and payload instead of moving anything on screen — a row is
always either one of the fixed slots (fully visible, always row-aligned) or not rendered at
all, never partially clipped, by construction rather than by careful tuning.

Once this worked for languages, it was generalized into `client/src/ui/selector.rs`: a
domain-agnostic widget taking `Vec<SelectorOption { label, payload: Box<dyn Fn(&mut
EntityCommands) + Send + Sync> }>`, firing a `UiSelected { entity }` event when a row is
chosen. A consuming system reads whatever payload component that option's own closure
inserted onto the chosen slot entity — the same "resolve identity from the entity an event
fires on" pattern already used elsewhere in this codebase (e.g. `AttackAttempt`). The language
picker was rebuilt on top of this generic widget, and it was then reused for a stub "Options"
picker and later the lobby's level-selection popup.

A custom discrete scrollbar (thumb size/position from whole-item counts, not pixel ranges)
replaces `FeathersScrollbar`, since that widget is built around continuous `ScrollPosition`
this design doesn't have.

### Alternative considered and rejected: generic type parameter

An earlier attempt tried `selector_popup<T: Component + Clone>(tag: T)`, tagging the popup
with a caller-supplied marker type directly. This ran into a genuine `bsn!` macro parsing
ambiguity: a bare identifier immediately followed by `{expr}` on the next line parses as a
struct-literal field access (`Identifier { expr }`), not two separate component inserts — this
specifically broke embedding a generic type parameter's value this way. Abandoned in favor of
a wrapper-marker-component + post-spawn `set_selector_options()` populate step instead, which
also sidesteps a separate, harder blocker: `SelectorOption::payload`'s `Box<dyn Fn>` can't
implement `Clone`, which `bsn!`'s templated-value embedding requires for every component.

## Consequences

- The entire class of clipping/overflow bugs is gone structurally, not patched around — no
  further scroll-position-dependent visual bugs have recurred since this rewrite.
- One widget now serves three consumers (language picker, options picker, level picker)
  instead of one bespoke implementation per popup.
- Row count is a single shared module constant (`SELECTOR_VISIBLE_ROWS`), not configurable
  per instance — a real scope cut, not an oversight: `bsn!`'s `Children [...]` needs a literal
  list, so a genuinely per-instance count would need either a dynamically-built `SceneList`
  (unverified whether a plain `Vec<impl Scene>` works the way `bsn_list!`'s macro-expanded
  lists do) or a real templated-props mechanism. Every current consumer wants 5 rows, so this
  was deferred rather than solved speculatively.
- The scrollbar is display-only — no drag-to-jump. Not needed yet, easy to add later.
- A same-frame double-`Activate` bug was found and fixed twice during this work (once from a
  duplicate observer registration, once from `bevy_ui_widgets`' own native Enter-key handling
  colliding with this project's synthetic gamepad-confirm bridge) — see the commit history
  around `client/src/ui/selector.rs` and `client/src/ui/ui.rs`'s `on_ui_confirm`/
  `on_ui_confirm_enter` split for the specifics; not otherwise documented as a separate ADR
  since it's a bug fix, not an architectural choice.
