# Agent Playtest 0049 — bevy_fluent Ripped Out: a Minimal `LocaleSelection`, All Localization Through bevy_markup

| Field | Value |
|---|---|
| Date | 2026-10-08 12:49 – 13:00 +0400 |
| Commit | `3870b8f` "Upgrade bevy_markup to 0.4.0; locale bundles to .ftl.ron" + uncommitted bevy_fluent removal |
| Agent | omp session, GLM 5.3 Flash (Z.ai) |
| Client | 1× `target/debug/p19-client --mcp` (rendered) for the language run; 1× windowed (same binary, real Wayland window) for the console; both rebuilt with `--features dev-tools` (0 errors) |
| Server | none needed — both surfaces are client-local; a pre-existing server on :6000 was left untouched |
| Transports | BRP :15702 |

## Purpose

bevy_fluent (and with it `fluent`, `fluent_content`, the direct `unic-langid` dependency, and the
console's `.ftl` files) is removed from the repo. Replacing it:

- a minimal reflected resource, `p19_client::ui::markup::LocaleSelection(pub String)` (default
  `en-US`), written by the language picker (`ui::ui`) and mirrored into bevy_markup's
  `ActiveLocale` by `sync_active_locale`, which loads `locales/<id>/main.ftl.ron` — all UI text
  keeps resolving through bevy_markup's `data-l10n-id`;
- the dev console output is plain English literals; `ui/localization.rs` (the bevy_fluent
  `Localization` plugin) is deleted.

Verify the language switch still works end to end through the new path, that switching back is
instant, that no bevy_fluent state survives at runtime, and that the console prints English.

## Verification

Driven by gamepad (`game/gamepad`) — the default input method; every step read back from
`game/ui` text, not pixels.

| Step | Observed |
|---|---|
| Boot to main menu | `Connect / Options / Credits / Quit`, English |
| Options → Language → Русский (DPad + South through the selector popup) | every label switches: `Подключиться`, `Настройки`, `Авторы`, `Выход`, `Язык`, `Чувствительность мыши: 1.00×`, `Предсказание на клиенте: Вкл.`, `Назад`; the TUI panel's live rows too (`Отрисовано из HTML + CSS в реальном времени`, `t = 44 с`) |
| Language → 日本語 (popup resumed at the last pick, `slot-1`) | `接続`, `オプション`, `クレジット`, `終了`, `言語`, `マウス感度：1.00×`, `クライアント側予測：オン`, `戻る` |
| Language → English (popup resumed at `slot-2`, DPadUp ×2) | all labels revert immediately (the `LocaleBundles` cache holds each visited bundle, so no reload) |
| `world.list_resources` | `p19_client::ui::markup::LocaleSelection` present (reflected); **zero** `bevy_fluent` resources |
| Console output | plain English — see F4 |

![ru_options.png](screenshots/playtest_0049/ru_options.png)

*The options screen and main menu in Russian after picking through the new `LocaleSelection` path.*

![ja_options.png](screenshots/playtest_0049/ja_options.png)

*The same surface in Japanese, switched from Russian (the popup resumed at the last pick).*

## Findings

**F1 — the switch works end to end through the minimal resource.** Picker → `LocaleSelection` →
`sync_active_locale` → bevy_markup `ActiveLocale` → in-place `HtmlUi` re-render (the element
entities survive; the mechanism is the one verified in playtests 0030/0040 — only the source of
the selected language changed).

**F2 — switching back is instant.** `LocaleBundles` keeps every loaded bundle's handle keyed by
language id, so en → ru → ja → en never re-loads a bundle.

**F3 — runtime proof of the dependency cut.** `world.list_resources` shows the one new
`LocaleSelection` resource and nothing from bevy_fluent (`Locale`, `Localization` are gone with
the crate); the `Localization` plugin no longer exists to insert them.

**F4 — console is English-only.** Verified manually by the user on the windowed client (the
console's output strings are now plain literals in `dev/console.rs`). The agent's own attempt to
drive the console headlessly through a real Wayland window failed for harness reasons — see F5 —
so the manual check stands in for it.

**F5 — tooling note (unresolved, not chased further).** `wtype` keystrokes did reach the game
window (a typed Enter activated the focused element and closed the options screen), but the
backtick toggle — a `just_pressed` consumer on real `KeyboardInput` events, exactly the §5c
use case — never fired across several attempts (`wtype '`'`, `wtype -k grave`). Compositor
keyboard-focus dependency is the prime suspect (§5c's caveats); once the user had verified the
console manually, this was dropped rather than debugged.

**F6 — incident, corrected.** A South press meant for the Language toggle landed on
`prediction` (its focus order in `options.html` is Language, slider, prediction, Back — I
navigated up from `options-back` assuming the reverse). `Client-side prediction` flipped On;
it was restored to Off before teardown and the report's screenshots show it On in ru/ja — a
record of the state at capture time, not a language-related difference. Incidentally confirms
the toggle still works from the gamepad path.
