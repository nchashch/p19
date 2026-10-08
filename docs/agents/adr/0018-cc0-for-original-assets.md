# 18. License the original assets CC0 1.0

| Field | Content |
|---|---|
| `ADR` | `0018` |
| `Title` | License the original assets CC0 1.0 |
| `Date` | `2026-10-08 09:57 +0400` |
| `Author` | GLM 5.3 Flash (Z.ai), via omp |
| `Commit` | `90b3e2e` Bump bevy_mcp_harness version + uncommitted license files (`LICENSE-CC0`, README "License", `assets/CREDITS.md`, in-game credits outro and its Fluent keys) |
| `Status` | `Accepted` |
| `Related` | [0016](./0016-system-fonts.md) (font licensing shaped this repo's asset posture) |

## Context

The repository has always had a split license posture, and one side of it was deliberately
left open:

- The **code** in `crates/` is dual-licensed MIT OR Apache-2.0 (`LICENSE-MIT`/`LICENSE-APACHE`,
  `workspace.package.license = "MIT OR Apache-2.0"`) — the standard Rust/Bevy ecosystem
  convention, matching most dependencies.
- The **third-party assets** shipped under `assets/` are all CC0 1.0, credited in
  `assets/CREDITS.md` and on the in-game Credits screen: Kenney's Input Prompts glyph sheets,
  ambientCG's Night Sky HDRI 012 skybox, and Poly Haven's Rubber Tiles floor texture (baked
  into the start-level `.glb`s).
- The **original assets** — everything else under `assets/client/` and `assets/server/`
  (rigs, levels, shaders, translations, manifests, config) — had "no license decided yet":
  the README told reusers to treat them as unlicensed/all-rights-reserved.

That state is workable only while nobody wants to reuse anything. The repo is public
(`github.com/nchashch/p19`), the README already advertises the agent-playtest workflow as a
model for others, and playtest 0039 established that the game runs end to end on exactly the
git-tracked asset set — i.e. the assets are a self-contained, redistributable whole *except*
that the license said "no". An earlier licensing headache already shaped this project: the
bundled-fonts attempt (ADR 0002) died partly over license-file cleanliness (a missing
`OFL.txt`), and ADR 0016 moved fonts to the system partly for that reason. Asset-licensing
hygiene is a real concern here, not paperwork.

## Decision

Dedicate all original assets to the public domain under CC0 1.0, same as the third-party
assets already shipped:

1. Add `LICENSE-CC0` at the repository root — the full CC0 1.0 legal code plus a short
   scoping note (original assets in `assets/client/`+`assets/server/` minus the
   `assets/CREDITS.md` attributions; code stays MIT OR Apache-2.0). Naming follows the
   existing `LICENSE-MIT`/`LICENSE-APACHE` files.
2. Scope: everything under the two asset roots that is not attributed to a third party in
   `assets/CREDITS.md` (verified against the tracked asset tree: `locales/`, `rigs/`,
   `collections/`, `shaders/`, `levels/`, `stats/`, `controllers/`, `characters/`,
   `config.toml`, `manifest.toml`, plus the non-input-prompt textures and skyboxes). Does
   **not** cover `crates/` code and does not re-license the three third-party assets (they
   remain CC0 under their authors' own dedications).
3. Documentation updated in the same change: the README's "License" section, the closing
   note of `assets/CREDITS.md`, and the in-game Credits screen outro — the
   `credits-original` Fluent key in `en-US`/`ru-RU`/`ja-JP` plus the embedded English
   fallback in `crates/client/src/ui/html/credits.html` now say the original work is
   likewise public domain (CC0 1.0). The third-party list and its Fluent keys are unchanged.

CC0 requires no attribution, so the Credits screen keeps crediting the third-party authors
purely as a courtesy ("credited here with thanks"), which is what it already does.

## Alternatives considered

- **Keep the assets unlicensed (all-rights-reserved)** — the status quo. Lost: the only
  option that makes a public repo *worse* than private, since default copyright forbids
  exactly the copying a public repository invites. Every third-party input is already CC0;
  wrapping original-only output in all-rights-reserved is incoherent.
- **CC-BY 4.0** — reuse with mandatory attribution. Keeps a paper trail of authorship, but
  imposes on every reuser an attribution obligation for placeholder prototype art the author
  doesn't want credit for, while CC0 third-party packs (Kenney, ambientCG, Poly Haven) sit
  right next to it with no such requirement. Heavier for zero benefit here.
- **CC-BY-SA 4.0** — share-alike. Wrong tool: the surrounding asset ecosystem is CC0 (not
  share-alike), and a prototype's levels/shaders gain nothing from virality. It would also
  complicate mixing: a CC0 Kenney sheet inside a SA-licensed rig is a per-file license
  determination forever.
- **A bespoke permissive notice** — custom asset licenses are exactly the ambiguity CC0
  exists to eliminate; drafting one was rejected without serious consideration.

## Consequences

- **Gained**: the repository is now legally redistributable and reusable end to end — code
  MIT OR Apache-2.0, every asset CC0. Downstream can fork, extract rigs/levels/shaders, or
  ship derivative builds without contacting the author. The README's License section makes
  one clear claim instead of an "unlicensed" warning.
- **Cost**: the author waives all copyright in the original art, including attribution
  (CC0's trademark/patent carve-outs and the patent position don't matter for these asset
  types). Anything genuinely worth reserving must be licensed differently *before* it is
  committed — e.g. a per-directory LICENSE — because CC0's waiver is irrevocable.
- **Still open**: nothing for the current asset set. The standing rule from
  `assets/CREDITS.md` still applies: any new third-party asset goes into `assets/CREDITS.md`
  *and* `crates/client/src/ui/credits.rs`'s `CREDITS` with its `credits-use-<id>` Fluent key
  in every locale.
