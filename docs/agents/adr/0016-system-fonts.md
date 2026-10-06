# 16. Use the system's fonts instead of shipping font files

| Field | Content |
|---|---|
| ADR | 0016 |
| Title | Use the system's fonts instead of shipping font files |
| Date | 2026-10-07 00:52 +0400 |
| Author | Claude Opus 5.5 (Anthropic), via omp — on the project owner's request |
| Commit | `9158192` Use crates.io version of bevy_markup + uncommitted system-fonts change |
| Status | Accepted |
| Related | ADR 0002 (bundle CJK-capable fonts), ADR 0015 (UI on bevy_markup), playtest 0038 |

## Context

The client loaded its fonts as assets through the `CommonAssets` manifest:

- IosevkaSlabQP Regular and Bold as the UI font (`serif_font`, `serif_bold_font`), registered as
  the CSS `serif` family for every `HtmlUi` (`ui/markup.rs` `register_ui_fonts`) and copied over
  Bevy's default font (`assets/collections.rs` `override_default_font`);
- Noto Sans JP (`noto_sans_jp_font`), loaded only so `parley` had a CJK font to fall back to for
  the `ja-JP` locale (ADR 0002);
- IosevkaSlabMono for the dev console (`dev/console.rs`, by path).

The workspace already enabled Bevy's `system_font_discovery` feature (root `Cargo.toml`), and
bevy_markup 0.3's own examples use the system's fonts through `FontSource::Serif` / `SansSerif` /
`Monospace`. The project owner asked for prototype_19 to do the same.

## Decision

Text uses the host system's fonts; the client loads no font files.

- `ui/markup.rs` `register_ui_fonts` runs at `Startup` (no longer waiting for `CommonAssets`) and
  maps the CSS generics `serif`, `sans-serif` and `monospace` to Bevy's `FontSource::Serif`,
  `SansSerif` and `Monospace`, one source per family; the system picks bold and italic faces.
  `theme.css` is unchanged: it already asks for `serif` and `monospace`.
- Glyphs a family lacks (Japanese kana and kanji) fall back to any installed font through
  `parley` and `system_font_discovery`.
- `CommonAssets` loses `serif_font`, `serif_bold_font` and `noto_sans_jp_font`; the manifest
  (`assets/client/collections/common_assets.assets.ron`) loses their entries.
  `override_default_font` is removed.
- The FPS overlay asks for `FontSource::Monospace`.
- The dev console (`chill_bevy_console` 0.3.1) takes only a font path or a handle, not a system
  family, so it now uses `font_path: None`: Bevy's built-in default font (FiraMono, compiled into
  Bevy), which is not a shipped asset file either.

The font files under `assets/client/fonts/` are left in place; nothing loads them.

## Alternatives considered

- **Keep shipping the fonts** (ADR 0002's position): guarantees the look and CJK coverage on every
  machine, at the cost of shipped files. Superseded by the owner's request.
- **Ship only a CJK fallback font** and use system fonts for Latin text: keeps `ja-JP` working
  everywhere. Not chosen; the request was to use system fonts, and it remains an option if
  players without CJK fonts matter (see Consequences).

## Consequences

- No font files are loaded; the asset manifest is smaller.
- **The look now depends on the player's system.** On the dev machine `serif` resolved to a Noto
  serif (playtest 0038 screenshots), not IosevkaSlabQP.
- **`ja-JP` renders only if the player has a CJK-capable font installed.** On a system without
  one, Japanese text shows missing-glyph boxes. This reverses ADR 0002's guarantee. Steam Deck
  coverage (the min-spec target) is unverified [INFERENCE: SteamOS ships Noto fonts, but whether
  CJK is included was not checked].
- `--no-common-assets` playtests now render the same fonts as normal runs (before, they fell back
  to Bevy's default font).
- ADR 0002's index row is updated to "Superseded by 0016".
