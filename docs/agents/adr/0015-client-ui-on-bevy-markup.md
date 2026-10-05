# 15. Client UI on bevy_markup (HTML templates + CSS + Fluent)

| Field | Content |
|---|---|
| `ADR` | `0015` |
| `Title` | Client UI on bevy_markup (HTML templates + CSS + Fluent) |
| `Date` | 2026-10-05 10:07 +0400 |
| `Author` | Claude Opus 5.5 (Anthropic), via omp (integration; three sub-agents of the same model wrote the surface modules) |
| `Commit` | `ecf2349` "Replicate Transforms" + uncommitted: the whole migration (every file under `crates/client/src/ui/`, `crates/client/src/ui/html/`, `dev/tool_api.rs`'s `game/ui`, `main.rs`, `controls/actions.rs`, `assets/collections.rs`, `events.rs`, `lifecycle/lobby.rs`, workspace/client `Cargo.toml`, `Cargo.lock`, deleted `crates/client/build.rs`, `ui/widgets.rs`, `ui/framework.rs`) |
| `Status` | Accepted |
| `Related` | ADR 0001 (selector behavior, kept), ADR 0002 (CJK fonts, kept), ADR 0009/0011 (`game/ui`); bug_0008 (fixed by this change); playtest 0022 |

## Context

At `ecf2349` the client UI was built three different ways:

- the main menu, lobby and selector popups on `bevy::feathers` buttons inside BSN `bsn!`
  scenes, with gamepad navigation added by hand (`AutoDirectionalNavigation`, `AutoFocus`,
  `selector::LockedWhileSelectorOpen`);
- the HUD and pause menu on a hand-rolled `ui/widgets.rs` (`panel()`, `button()`, its own
  `Activate` event and tooltip system), bridged to the gamepad path through two different
  confirm actions (`UiConfirm` → both `Activate`s, `UiConfirmEnter` → only the legacy one,
  because feathers handled Enter natively and a second activation double-fired);
- a ratatui demo panel (`tui_panel.rs`) on `bevy_tui_texture`, a forked crate whose font loader
  had to be removed and whose font had to be `include_bytes!`'d behind a `build.rs` cfg.

Text was localized by a `LocalizedText(&'static str)` component written into `Text` by a
polling system, plus `localized()` calls for interpolated strings. Known costs of that state:
literal Enter could not activate a feathers button headlessly (AGENTS.md known gap; feathers'
key handling needs a `PrimaryWindow`), `--no-common-assets` crashed on opening the pause menu
(bug_0008), and `game/ui` needed both feathers and `ui_widgets` marker types to know what was
clickable.

The owner asked for every UI surface to be reimplemented on `bevy_markup`, their own crate
(Tera templates → `tl` DOM → Fluent `data-l10n-id` → CSS cascade → plain Bevy UI nodes), added
as a git dependency. Constraints found while reading its source (`5113f7c`):

- The CSS subset has no `position`/`top`/`left`, `z-index`, `overflow`, `border-color`,
  `border-radius`, `text-align` or combinators, and no `<img>`; images come only from
  `border-image`.
- Interaction is `data-on-click`/`-enter`/`-leave` hooks → buffered `ElementSignal` messages
  from picking, plus `:hover`/`:active` via `PseudoState`. No focus, keyboard or gamepad
  support.
- An `HtmlUi` root's children are owned by the pipeline: any template-context, locale or
  template change despawns and respawns them (`HtmlUiBuilt`); style-only changes restyle in
  place (`HtmlUiRestyled`).
- It requires bevy `^0.19.1` (the lock had 0.19.0) and asks apps to patch `fluent-syntax`.

## Decision

### 1. One shared layer, `ui/markup.rs`

`MarkupPlugin` adds `BevyMarkupPlugin` and everything the surfaces share:

- **Embedded templates.** Templates and the single stylesheet (`theme.css`, the
  `DefaultStylesheet`) live in `crates/client/src/ui/html/` and are compiled in with
  `embedded_asset!` (`embedded://p19_client/ui/html/<file>`). `assets/client/` is gitignored, so
  asset-dir templates would not be versioned with the code, and they would be missing under
  `BEVY_ASSET_ROOT` playtest sets.
- **Fonts and locale.** `FontFamilies` gets `CommonAssets.serif_font` and a new optional
  `serif_bold_font` as CSS `serif`. `ActiveLocale` follows bevy_fluent's existing `Locale`
  resource, loading the existing `locales/<id>/main.ftl.yml` bundle manifests. The language
  picker still writes `Locale`. `LocalizationPlugin` remains only for the dev console's
  `localized()` output.
- **One input path.** Pointer clicks arrive as bevy_markup `ElementSignal`s. `UiConfirm`, now
  bound to both gamepad South and Enter (`UiConfirmEnter` deleted), writes the *same* message
  for the focused element. Each surface handles its buttons in one
  `MessageReader<ElementSignal>` system keyed by namespaced signal names.
- **Focus and navigation** are app-side, on Bevy's own `InputFocus` and
  `AutoDirectionalNavigation`. `UiNav` on a root opts its clickables into navigation, and
  `sync_navigation` manages the component. A visible `UiNavModal` root confines navigation;
  this replaces `LockedWhileSelectorOpen`. `repair_focus` restores focus after a rebuild by
  element `id`, else by the `autofocus` class. A dead-end move triggers `UiNavigateEdge`, which
  the selector uses to page. Press-and-hold repeat moved here unchanged. The focus ring is an
  `Outline`.
- **Tooltips** use `data-on-enter/leave="tooltip"`, with the Fluent key and args in
  `data-with`. Each tooltip is its own root, positioned from the element's rect and placed on
  the element's UI camera.

### 2. Placement by roots, not CSS

Every piece of UI that sits at a screen position is its own `HtmlUi` root whose `Node`
(absolute position or full-screen flex centering) and `GlobalZIndex` the spawning code sets:
pause menu 100/101, selector popup 900, tooltips 1000. Borders are faked with nested
backgrounds, and buttons are `<div class="button">`.

### 3. Surfaces

- Main menu: `main_menu.html`, also used for the VR wrist panel with `wrist = true`.
- Selector: `selector.html`. Same behavior as ADR 0001: 5 slots, wheel and edge paging, resume
  at the last pick, a discrete scrollbar. It now re-renders the window and keeps focus by slot
  id, and picks arrive as `SelectorPicked { selector, value }`.
- Lobby: `lobby.html`.
- Pause menu and controls tips: `pause_menu.html`, `controls_tips.html`.
- HUD: crosshair, data frame, and a hotbar kept unspawned behind `HOTBAR_ENABLED`.
- Nameplates and the NPC sign texture.
- VR in-game wrist panel: `quad_panel(.., content: impl Bundle)`.
- TUI panel: ported to `tui_panel.html`. `bevy_tui_texture`, `ratatui` and `build.rs` were
  removed, per the owner's choice.

Per-frame values are never re-rendered; the code mutates the built entities instead. This
covers the crosshair GCD (`MaterialNode` + `Visibility`), nameplate position, fade and fill,
and the TUI gauge.

### 4. Removed

The following are gone: `FeathersPlugins`, `UiTheme`, the `bevy_feathers` feature,
`override_feathers_button_font`, `ui/widgets.rs`, `ui/framework.rs`, `LocalizedText`,
`UiConfirmEnter`, the unused `Play` event, and every `bsn!` scene in `ui/`. In `game/ui`,
clickable now means "has a `data-on-click` hook", pressed and hovered come from `PseudoState`,
and a text block's `TextSpan` runs are included in its text.

## Alternatives considered

- **Keep feathers buttons inside HTML layouts** (attach `FeathersButton` to built elements).
  Rejected: it keeps two activation paths and the windowless Enter gap, and feathers inserts
  its own children, which bevy_markup's restyle shape check would treat as a structure change
  (see Consequences).
- **Templates under `assets/client/ui/`** (hot-reloadable through the file watcher).
  Rejected: that directory is gitignored, and isolated playtest asset roots would lack the
  UI. `embedded_watcher` can restore hot reload for embedded files if wanted.
- **Re-render a template for per-frame values** (crosshair ring, nameplate position, gauge).
  Rejected: every context change is a full despawn/respawn of the subtree.
- **Keep the ratatui panel as is.** The owner chose to port it.
- **`border-image` 9-slice frames for panels.** Rejected for now: the stylesheet would depend
  on an image, and under `--no-render` (no image loader) bevy_markup never builds a UI whose
  stylesheet image can't load.

## Consequences

- The shell UI uses one stylesheet, one input path and one localization path for desktop, VR
  panels and headless agents. Literal Enter now confirms headlessly, which closes the old
  AGENTS.md known gap, and bug_0008 is fixed. Both were verified in playtest 0022, on both
  `--mcp` and `--no-render`.
- **bevy_markup hazard found here.** The restyle-in-place shape check
  (`build.rs::same_shape`) compares `ImageNode` presence on built nodes. An app `ImageNode`
  inserted on a built element (the first version of the controls-tips icons) made every restyle
  a rebuild. The `PseudoState` insert after each build is itself a restyle trigger, so the tips
  rebuilt every frame: the playtest saw 11 label entities change within 0.5 s, and `game/ui`
  never listed them. The workaround puts icons on a nested empty `HtmlUi` child (`icon.html`),
  which the shape check skips. Upstream candidate fix: ignore app-attached components that the
  node spec didn't produce. `MaterialNode`/`Outline` are unaffected.
- The CSS subset forces placement into Rust: every overlay is a root with hand-set offsets, and
  the selector popup and tooltips compute rects from `ComputedNode`/`UiGlobalTransform`.
- Visual style changed from feathers' dark theme and the slate widgets to `theme.css`'s flat dark
  palette with a gold focus ring. That was a deliberate restyle, not a port of the old pixels.
- Open:
  - VR wrist panels were not run on a headset.
  - The NPC sign quad was not seen in-game: `decorate_npcs` never ran during the playtest
    (B0004 warning, no `Decorated`). That code was not touched by this change, and the cause
    was not established (playtest 0022 F4).
  - The TUI panel re-renders once a second by design.
