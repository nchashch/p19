#set document(
  title: "Agent Playtest 0007 — Add game/keyboard + game/mouse, Verify Real UI Clicks",
  author: ("Claude (Sonnet 5), in Claude Code",),
)
#set page(margin: 2cm, numbering: "1 / 1")
#set text(size: 10pt)
#set heading(numbering: "1.")

= Agent Playtest 0007

#table(
  columns: (auto, auto),
  stroke: 0.5pt,
  inset: 6pt,
  [*Field*], [*Value*],
  [Date], [2026-09-23 08:33 -- 08:41 UTC],
  [Commit], [Started at `b340541` "Add gamepad input injection capability to MCP"; the
  `game/keyboard`/`game/mouse` addition described here is not yet committed as of this report],
  [Agent], [Claude (Sonnet 5), driving the tool API over loopback HTTP, editing code between runs],
  [Client], [`target/debug/client --mcp` -- dev profile + `dev-tools` feature, rebuilt between runs],
  [Server], [`target/release/server`],
  [Level], [`levels/minimal.level.ron` ("Minimal level")],
  [Transports], [game: UDP/netcode :6000 · QA tool API: BRP HTTP :15702 (+ MCP :15710)],
)

= Purpose

Requested directly by the project owner, extending `playtest_0006`'s `game/gamepad` work:
"add the same kind of thing but for keyboard + mouse, so you can virtually press `KeyCode`s and
inject mouse motion, mouse button, mouse wheel events, etc -- so you can trigger the exact
codepaths clicking a button or moving the mouse would trigger if a human did it with an actual
peripheral." Unlike `game/gamepad`, this is the first method in the tool API that can reach UI
*by screen position* -- clicking an actual button where a screenshot shows it, not navigating
focus with directional input and confirming.

= Design

`game/keyboard` mocks `ButtonInput<KeyCode>` directly (`bevy_enhanced_input`'s
`Binding::Keyboard` reads this resource, confirmed via
`bevy_enhanced_input-0.26.0/src/context/input_reader.rs`, same reading pattern as the
button/`ButtonInput<MouseButton>` case -- no analog/digital split the way `GamepadButton` has).
`KeyCode` deserializes directly from its own `serde` impl (already enabled project-wide via the
`serialize` feature), so every one of Bevy's 160+ variants works via `{"key":"KeyW","pressed":
true}` without a hand-maintained name list.

`game/mouse` is two mechanisms at once, matching how a real click actually works: raw
`ButtonInput<MouseButton>` (for mouse-bound gameplay actions) AND `bevy_picking`'s real
`PointerInput`/`PointerAction` event pipeline (for UI hit-testing/`Interaction`/`Activate`).
Investigated whether headless mode has a real pointer to drive at all -- confirmed
`bevy_picking::input::spawn_mouse_pointer` unconditionally spawns `PointerId::Mouse` at
`Startup` regardless of window existence, so this reuses that real id rather than needing
`PointerId::Custom(Uuid)` or a new dependency. `{"input":"move_to","x":..,"y":..}` targets the
same 1280x720 pixel space `game/screenshot` captures, so a button's on-screen rect from a
screenshot maps directly to a click position.

= A real bug caught by testing, not by review

First implementation of `"motion"`/`"wheel"` set `AccumulatedMouseMotion`/`AccumulatedMouseScroll`
directly via `world.insert_resource(...)`. Compiled clean, ran with no error -- but
`game/state`'s `look_yaw` stayed exactly `0.0` after a `{"input":"motion","dx":200,"dy":0}` call
that should have turned the camera. Read `bevy_input-0.19.0/src/mouse.rs` directly:
`accumulate_mouse_motion_system`/`accumulate_mouse_scroll_system` unconditionally overwrite these
resources from `MouseMotion`/`MouseWheel` *events* every single frame ("reset to zero every
frame", per their own doc comments) -- a direct resource write gets silently wiped before
`bevy_enhanced_input`'s reader ever observes it. Fixed by writing real `MouseMotion`/`MouseWheel`
events instead (`world.write_message(...)`), letting those systems compute the accumulated value
on their own schedule -- the same mechanism a real winit event uses. Same *shape* of bug as
`playtest_0006`'s gamepad analog/digital finding (a static-analysis-invisible "wrong resource,
not wrong logic" mistake), different root cause.

Also hit, and fixed without needing new debugging: running the freshly-rebuilt binaries directly
(`target/debug/client`/`target/release/server`, not via `cargo run`) panicked on missing assets,
because bevy's default asset root resolves from *runtime* `CARGO_MANIFEST_DIR`/`BEVY_ASSET_ROOT`,
not a compile-time path -- exactly what this skill's own "1. The three processes" section already
documents launching with `BEVY_ASSET_ROOT=...`. All verification below was actually run against
a manual `assets` symlink next to each binary as a faster workaround for this specific pass
(removed afterward, since it's not the documented approach and target/ is gitignored regardless)
-- the effect is identical to the documented env-var approach; this is a bookkeeping note, not a
finding that changes any recommendation.

= Verification: real UI clicks and real key input

#figure(
  image("../screenshots/playtest_0007/1790138046757-kbm-test-mainmenu.png", width: 65%),
  caption: [Main menu, before any agent input this run. "Connect" is already highlighted --
  `AutoFocus`, not agent-driven hover.],
)

`{"input":"move_to","x":161,"y":327}` (the "Options" button's center, read off the screenshot
above) then `{"input":"button","button":"Left","pressed":true}`/`pressed:false`:

#figure(
  image("../screenshots/playtest_0007/1790138119065-kbm-click-options.png", width: 65%),
  caption: [The Options `selector` popup opens for real -- through the actual
  `bevy_ui`/`bevy_picking` hit-testing and the button's `Activate` observer, confirmed
  independently via the `client::ui::selector` "after toggle popup open = true" / "Selector
  visible" log lines. Clicking "Options" again (same move_to + button press/release) closed it
  just as cleanly (log: "after toggle popup open = false" / "Selector hidden").],
)

Then, in sequence: `move_to` "Connect" + click -- transitioned `MainMenu` -> `Lobby` (confirmed
via `game/state`); selected `levels/minimal.level.ron` and triggered `play` -- reached `InGame`
with a real player entity; `{"key":"KeyW","pressed":true}` for ~1s then `pressed:false` -- the
player's Z position advanced from `0.0` to `-13.07` and velocity showed `-12.0` on Z while held,
returning to `0.0` on release, through the real replicated-BEI movement pipeline (not
`game/input`'s action-level mock); `{"input":"motion","dx":200,"dy":0}` -- `look_yaw` changed
from `0.0` to `-1.0` (confirming the `MouseMotion`-event fix works, after the direct-resource-
write attempt above measurably didn't); `{"input":"reset"}` -- no crash, camera yaw returned to
its pre-turn heading.

= Findings <findings>

- *`game/keyboard` and `game/mouse` implemented and working*, including the first
  screen-position-based real UI click this tool API has ever driven (previous methods --
  `game/gamepad`, `game/input` -- only reach UI by navigating focus, never by clicking a
  specific screen location).
- *A real bug, caught only by running the code, not by review*: `AccumulatedMouseMotion`/
  `AccumulatedMouseScroll` cannot be set with a direct resource write -- Bevy's own per-frame
  reset-from-events systems silently undo it. Documented in three places (the tool's own doc
  comment, `docs/skills/playtest.md` §5b, this report) specifically so a future refactor doesn't
  revert to the more obvious-looking direct write.
- *Confirmed, not merely designed-for*: `bevy_picking`'s pre-existing `spawn_mouse_pointer`
  (unconditional, headless-safe) means no new entity/dependency was needed to get a real pointer
  id to drive -- the same `PointerId::Mouse` a real mouse would use.
- *A minor, deliberately unfixed cosmetic gap, same category as `playtest_0006`'s*: no visible
  hover-highlight difference was observed on the "Options" button after a `move_to` alone
  (screenshot not included -- visually identical to the pre-hover main-menu shot). Not
  investigated further: the click itself demonstrably worked (hit-testing succeeded), so this
  is very likely just this UI theme's hover state being visually subtle or absent, not a
  picking-pipeline problem; out of scope for this task either way.
- *Process note*: like `playtest_0006`, this covers one continuous investigation arc (the failed
  direct-resource-write attempt, then the fixed event-based one), filed as one report per this
  session's established convention.

= Artifacts & bookkeeping

- Screenshots: `docs/playtests/screenshots/playtest_0007/` (LFS-tracked, two of the four capture
  labels from this run -- the main-menu baseline and the Options-popup click; the hover-probe and
  final black-minimal-level shots were omitted as redundant/already-documented-elsewhere), plus
  the tool's raw capture staging directory `docs/playtests/dist/screenshots/` (gitignored, all
  four).
- Tool API surface used: `game/trigger` (`connect`, `play`), `game/select_level`, `game/state`,
  `game/keyboard` (new), `game/mouse` (new), `game/screenshot[+ /get]`, plus direct reading of
  `bevy_input-0.19.0`/`bevy_picking-0.19.0`/`bevy_ui-0.19.0` registry source for the design and
  the bug fix -- see `client/src/dev/tool_api.rs`, `docs/skills/playtest.md` §5b, ADR 0009.
- Code changed: `client/src/dev/tool_api.rs` (`game/keyboard`/`game/mouse` BRP methods,
  `keyboard_input`/`mouse_input` MCP tools, `AGENT_POINTER`/`current_pointer_location` helpers,
  `parse_mouse_button`/`mouse_button_to_pointer_button`).
- Living documentation updated this session: `AGENTS.md`'s dev-tools and `--mcp` bullets;
  `docs/skills/playtest.md` (new §5b).
