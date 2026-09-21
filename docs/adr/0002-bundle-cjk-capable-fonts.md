# 2. Bundle CJK-capable fonts instead of relying on system font discovery

Date: 2026-09-21

## Status

Accepted

## Context

A `ja-JP` locale was added to exercise CJK text rendering end-to-end (translations for every
existing `.ftl` key). Bevy 0.19 changed its text-shaping stack from cosmic-text to
`parley`/`swash`; unlike what an earlier assumption expected, this stack does **not**
automatically fall back to another loaded font when the active font lacks a glyph unless
Bevy's `system_font_discovery` feature is enabled — and confirmed by reading `parley`'s own
`FontCx` source, even then it only discovers whatever font happens to already be installed on
the machine running the client. That's real enough to *verify* CJK rendering works on a dev
machine (this one happens to have Noto Sans CJK installed system-wide), but it's not a
solution: a player's machine has no such guarantee, so shipping on `system_font_discovery`
alone would make CJK text render as tofu for most users.

Separately, and independently, the project's UI font was being swapped from IBM Plex Serif
(Latin/Cyrillic only) to `IosevkaSlabQP`, a source pack already added to `assets_src/fonts/`.

## Decision

Bundle real font assets instead of depending on the host system: `IosevkaSlabQP-Regular.ttf`
as the primary UI font, and `NotoSansJP-Regular.ttf` loaded purely as a registered-but-never-
directly-referenced fallback source. This works because of a confirmed detail in `bevy_text`'s
own asset-loading code (`load_font_assets_into_font_collection`): *every* loaded `Font` asset
gets registered into the same `parley::fontique` collection that text shaping draws from,
whether or not any `TextFont` ever names it directly by handle — so simply keeping
`CommonAssets.noto_sans_jp_font: Handle<Font>` alive is sufficient for it to serve as a real
glyph-fallback source, no explicit per-text-run font selection needed.

Only the `Regular` weight of each font was bundled, not the full family — a deliberate,
measured choice: `IosevkaSlabQP`'s full non-Extended weight set runs to roughly 250-270MB
(individual weights are ~10MB each), against IBM Plex's entire family at 4.5MB total, and a
grep confirmed only one weight is referenced anywhere in the code (`CommonAssets.serif_font`,
used uniformly). Bundling untouched weights "for completeness" would have meant ~50x more
binary size for zero current use.

A second, unrelated bug was found and fixed in the same window: `bevy_feathers` widgets
(`FeathersButton`) don't read Bevy's default-font override slot at all — confirmed via
`bevy_feathers::controls::button`'s source, they always insert their own explicit
`InheritableFont` pointing at feathers' embedded Fira Sans. The existing `override_default_font`
system never reached feathers widgets as a result. Fixed with a second system,
`override_feathers_button_font`, that re-`insert`s (not mutates in place — a plain `&mut`
write doesn't re-trigger feathers' own font-propagation observer, confirmed by testing)
`InheritableFont.font` on every freshly-inserted `InheritableFont`.

## Consequences

- CJK text now renders correctly on any machine, not just ones with a system CJK font already
  installed. `system_font_discovery` is left enabled as a harmless additional safety net but
  is no longer load-bearing for correctness.
- Established a precedent worth keeping for future font additions: bundle only the weight(s)
  actually referenced in code, not the whole family "just in case" — check usage with a grep
  before adding weights, given the size difference seen here (IBM Plex's whole family costs
  less than one single Iosevka weight).
- `IosevkaSlabQP`'s license file was not present in the source directory this was imported
  from (Noto Sans JP's `OFL.txt` was, and was copied alongside the font) — still needs
  sourcing and adding separately before this could be considered redistribution-clean.
- The `bevy_feathers` font-override fix (`override_feathers_button_font`) is a separate,
  general-purpose fix that happened to be found while doing this work, not specific to CJK —
  it's what makes *any* custom UI font (not just the CJK fallback) actually show up in
  feathers-based widgets like the main menu buttons.
